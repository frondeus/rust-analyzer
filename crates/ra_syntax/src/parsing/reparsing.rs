//! Implementation of incremental re-parsing.
//!
//! We use two simple strategies for this:
//!   - if the edit modifies only a single token (like changing an identifier's
//!     letter), we replace only this token.
//!   - otherwise, we search for the nearest `{}` block which contains the edit
//!     and try to parse only this block.

use ra_text_edit::AtomTextEdit;
use ra_parser::Reparser;

use crate::{
    SyntaxKind::*, TextRange, TextUnit, SyntaxError,
    algo,
    syntax_node::{GreenNode, SyntaxNode},
    parsing::{
        text_token_source::TextTokenSource,
        text_tree_sink::TextTreeSink,
        lexer::{tokenize, Token},
    }
};

pub(crate) fn incremental_reparse(
    node: &SyntaxNode,
    edit: &AtomTextEdit,
    errors: Vec<SyntaxError>,
) -> Option<(GreenNode, Vec<SyntaxError>)> {
    let (node, green, new_errors) =
        reparse_leaf(node, &edit).or_else(|| reparse_block(node, &edit))?;
    let green_root = node.replace_with(green);
    let errors = merge_errors(errors, new_errors, node, edit);
    Some((green_root, errors))
}

fn reparse_leaf<'node>(
    root: &'node SyntaxNode,
    edit: &AtomTextEdit,
) -> Option<(&'node SyntaxNode, GreenNode, Vec<SyntaxError>)> {
    let node = algo::find_covering_node(root, edit.delete);
    match node.kind() {
        WHITESPACE | COMMENT | IDENT | STRING | RAW_STRING => {
            if node.kind() == WHITESPACE || node.kind() == COMMENT {
                // removing a new line may extends previous token
                if node.text().to_string()[edit.delete - node.range().start()].contains('\n') {
                    return None;
                }
            }

            let text = get_text_after_edit(node, &edit);
            let tokens = tokenize(&text);
            let token = match tokens[..] {
                [token] if token.kind == node.kind() => token,
                _ => return None,
            };

            if token.kind == IDENT && is_contextual_kw(&text) {
                return None;
            }

            if let Some(next_char) = root.text().char_at(node.range().end()) {
                let tokens_with_next_char = tokenize(&format!("{}{}", text, next_char));
                if tokens_with_next_char.len() == 1 {
                    return None;
                }
            }

            let green = GreenNode::new_leaf(node.kind(), text.into());
            let new_errors = vec![];
            Some((node, green, new_errors))
        }
        _ => None,
    }
}

fn reparse_block<'node>(
    node: &'node SyntaxNode,
    edit: &AtomTextEdit,
) -> Option<(&'node SyntaxNode, GreenNode, Vec<SyntaxError>)> {
    let (node, reparser) = find_reparsable_node(node, edit.delete)?;
    let text = get_text_after_edit(node, &edit);
    let tokens = tokenize(&text);
    if !is_balanced(&tokens) {
        return None;
    }
    let token_source = TextTokenSource::new(&text, &tokens);
    let mut tree_sink = TextTreeSink::new(&text, &tokens);
    reparser.parse(&token_source, &mut tree_sink);
    let (green, new_errors) = tree_sink.finish();
    Some((node, green, new_errors))
}

fn get_text_after_edit(node: &SyntaxNode, edit: &AtomTextEdit) -> String {
    let edit = AtomTextEdit::replace(edit.delete - node.range().start(), edit.insert.clone());
    edit.apply(node.text().to_string())
}

fn is_contextual_kw(text: &str) -> bool {
    match text {
        "auto" | "default" | "union" => true,
        _ => false,
    }
}

fn find_reparsable_node(node: &SyntaxNode, range: TextRange) -> Option<(&SyntaxNode, Reparser)> {
    let node = algo::find_covering_node(node, range);
    node.ancestors().find_map(|node| {
        let first_child = node.first_child().map(|it| it.kind());
        let parent = node.parent().map(|it| it.kind());
        Reparser::for_node(node.kind(), first_child, parent).map(|r| (node, r))
    })
}

fn is_balanced(tokens: &[Token]) -> bool {
    if tokens.is_empty()
        || tokens.first().unwrap().kind != L_CURLY
        || tokens.last().unwrap().kind != R_CURLY
    {
        return false;
    }
    let mut balance = 0usize;
    for t in &tokens[1..tokens.len() - 1] {
        match t.kind {
            L_CURLY => balance += 1,
            R_CURLY => {
                balance = match balance.checked_sub(1) {
                    Some(b) => b,
                    None => return false,
                }
            }
            _ => (),
        }
    }
    balance == 0
}

fn merge_errors(
    old_errors: Vec<SyntaxError>,
    new_errors: Vec<SyntaxError>,
    old_node: &SyntaxNode,
    edit: &AtomTextEdit,
) -> Vec<SyntaxError> {
    let mut res = Vec::new();
    for e in old_errors {
        if e.offset() <= old_node.range().start() {
            res.push(e)
        } else if e.offset() >= old_node.range().end() {
            res.push(e.add_offset(TextUnit::of_str(&edit.insert), edit.delete.len()));
        }
    }
    for e in new_errors {
        res.push(e.add_offset(old_node.range().start(), 0.into()));
    }
    res
}

#[cfg(test)]
mod tests {
    use test_utils::{extract_range, assert_eq_text};

    use crate::{SourceFile, AstNode};
    use super::*;

    fn do_check<F>(before: &str, replace_with: &str, reparser: F)
    where
        for<'a> F: Fn(
            &'a SyntaxNode,
            &AtomTextEdit,
        ) -> Option<(&'a SyntaxNode, GreenNode, Vec<SyntaxError>)>,
    {
        let (range, before) = extract_range(before);
        let edit = AtomTextEdit::replace(range, replace_with.to_owned());
        let after = edit.apply(before.clone());

        let fully_reparsed = SourceFile::parse(&after);
        let incrementally_reparsed = {
            let f = SourceFile::parse(&before);
            let edit = AtomTextEdit { delete: range, insert: replace_with.to_string() };
            let (node, green, new_errors) =
                reparser(f.syntax(), &edit).expect("cannot incrementally reparse");
            let green_root = node.replace_with(green);
            let errors = super::merge_errors(f.errors(), new_errors, node, &edit);
            SourceFile::new(green_root, errors)
        };

        assert_eq_text!(
            &fully_reparsed.syntax().debug_dump(),
            &incrementally_reparsed.syntax().debug_dump(),
        )
    }

    #[test]
    fn reparse_block_tests() {
        let do_check = |before, replace_to| do_check(before, replace_to, reparse_block);

        do_check(
            r"
fn foo() {
    let x = foo + <|>bar<|>
}
",
            "baz",
        );
        do_check(
            r"
fn foo() {
    let x = foo<|> + bar<|>
}
",
            "baz",
        );
        do_check(
            r"
struct Foo {
    f: foo<|><|>
}
",
            ",\n    g: (),",
        );
        do_check(
            r"
fn foo {
    let;
    1 + 1;
    <|>92<|>;
}
",
            "62",
        );
        do_check(
            r"
mod foo {
    fn <|><|>
}
",
            "bar",
        );
        do_check(
            r"
trait Foo {
    type <|>Foo<|>;
}
",
            "Output",
        );
        do_check(
            r"
impl IntoIterator<Item=i32> for Foo {
    f<|><|>
}
",
            "n next(",
        );
        do_check(
            r"
use a::b::{foo,<|>,bar<|>};
    ",
            "baz",
        );
        do_check(
            r"
pub enum A {
    Foo<|><|>
}
",
            "\nBar;\n",
        );
        do_check(
            r"
foo!{a, b<|><|> d}
",
            ", c[3]",
        );
        do_check(
            r"
fn foo() {
    vec![<|><|>]
}
",
            "123",
        );
        do_check(
            r"
extern {
    fn<|>;<|>
}
",
            " exit(code: c_int)",
        );
    }

    #[test]
    fn reparse_leaf_tests() {
        let do_check = |before, replace_to| do_check(before, replace_to, reparse_leaf);

        do_check(
            r"<|><|>
fn foo() -> i32 { 1 }
",
            "\n\n\n   \n",
        );
        do_check(
            r"
fn foo() -> <|><|> {}
",
            "  \n",
        );
        do_check(
            r"
fn <|>foo<|>() -> i32 { 1 }
",
            "bar",
        );
        do_check(
            r"
fn foo<|><|>foo() {  }
",
            "bar",
        );
        do_check(
            r"
fn foo /* <|><|> */ () {}
",
            "some comment",
        );
        do_check(
            r"
fn baz <|><|> () {}
",
            "    \t\t\n\n",
        );
        do_check(
            r"
fn baz <|><|> () {}
",
            "    \t\t\n\n",
        );
        do_check(
            r"
/// foo <|><|>omment
mod { }
",
            "c",
        );
        do_check(
            r#"
fn -> &str { "Hello<|><|>" }
"#,
            ", world",
        );
        do_check(
            r#"
fn -> &str { // "Hello<|><|>"
"#,
            ", world",
        );
        do_check(
            r##"
fn -> &str { r#"Hello<|><|>"#
"##,
            ", world",
        );
        do_check(
            r"
#[derive(<|>Copy<|>)]
enum Foo {

}
",
            "Clone",
        );
    }
}
