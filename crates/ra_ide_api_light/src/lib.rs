//! This crate provides those IDE features which use only a single file.
//!
//! This usually means functions which take syntax tree as an input and produce
//! an edit or some auxiliary info.

mod structure;
mod typing;

use rustc_hash::FxHashSet;
use ra_text_edit::TextEditBuilder;
use ra_syntax::{
    SourceFile, SyntaxNode, TextRange, TextUnit, Direction,
    algo::find_leaf_at_offset,
    SyntaxKind::{self, *},
    ast::{self, AstNode},
};

pub use crate::{
    structure::{file_structure, StructureNode},
    typing::{on_enter, on_dot_typed, on_eq_typed},
};

#[derive(Debug)]
pub struct LocalEdit {
    pub label: String,
    pub edit: ra_text_edit::TextEdit,
    pub cursor_position: Option<TextUnit>,
}

#[derive(Debug)]
pub struct HighlightedRange {
    pub range: TextRange,
    pub tag: &'static str,
}

#[derive(Debug, Copy, Clone)]
pub enum Severity {
    Error,
    WeakWarning,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub range: TextRange,
    pub msg: String,
    pub severity: Severity,
    pub fix: Option<LocalEdit>,
}

pub fn matching_brace(file: &SourceFile, offset: TextUnit) -> Option<TextUnit> {
    const BRACES: &[SyntaxKind] =
        &[L_CURLY, R_CURLY, L_BRACK, R_BRACK, L_PAREN, R_PAREN, L_ANGLE, R_ANGLE];
    let (brace_node, brace_idx) = find_leaf_at_offset(file.syntax(), offset)
        .filter_map(|node| {
            let idx = BRACES.iter().position(|&brace| brace == node.kind())?;
            Some((node, idx))
        })
        .next()?;
    let parent = brace_node.parent()?;
    let matching_kind = BRACES[brace_idx ^ 1];
    let matching_node = parent.children().find(|node| node.kind() == matching_kind)?;
    Some(matching_node.range().start())
}

pub fn highlight(root: &SyntaxNode) -> Vec<HighlightedRange> {
    // Visited nodes to handle highlighting priorities
    let mut highlighted = FxHashSet::default();
    let mut res = Vec::new();
    for node in root.descendants() {
        if highlighted.contains(&node) {
            continue;
        }
        let tag = match node.kind() {
            COMMENT => "comment",
            STRING | RAW_STRING | RAW_BYTE_STRING | BYTE_STRING => "string",
            ATTR => "attribute",
            NAME_REF => "text",
            NAME => "function",
            INT_NUMBER | FLOAT_NUMBER | CHAR | BYTE => "literal",
            LIFETIME => "parameter",
            k if k.is_keyword() => "keyword",
            _ => {
                if let Some(macro_call) = ast::MacroCall::cast(node) {
                    if let Some(path) = macro_call.path() {
                        if let Some(segment) = path.segment() {
                            if let Some(name_ref) = segment.name_ref() {
                                highlighted.insert(name_ref.syntax());
                                let range_start = name_ref.syntax().range().start();
                                let mut range_end = name_ref.syntax().range().end();
                                for sibling in path.syntax().siblings(Direction::Next) {
                                    match sibling.kind() {
                                        EXCL | IDENT => range_end = sibling.range().end(),
                                        _ => (),
                                    }
                                }
                                res.push(HighlightedRange {
                                    range: TextRange::from_to(range_start, range_end),
                                    tag: "macro",
                                })
                            }
                        }
                    }
                }
                continue;
            }
        };
        res.push(HighlightedRange { range: node.range(), tag })
    }
    res
}

#[cfg(test)]
mod tests {
    use ra_syntax::AstNode;
    use insta::assert_debug_snapshot_matches;

    use test_utils::{add_cursor, assert_eq_text, extract_offset};

    use super::*;

    #[test]
    fn test_highlighting() {
        let file = SourceFile::parse(
            r#"
// comment
fn main() {}
    println!("Hello, {}!", 92);
"#,
        );
        let hls = highlight(file.syntax());
        assert_debug_snapshot_matches!("highlighting", hls);
    }

    #[test]
    fn test_matching_brace() {
        fn do_check(before: &str, after: &str) {
            let (pos, before) = extract_offset(before);
            let file = SourceFile::parse(&before);
            let new_pos = match matching_brace(&file, pos) {
                None => pos,
                Some(pos) => pos,
            };
            let actual = add_cursor(&before, new_pos);
            assert_eq_text!(after, &actual);
        }

        do_check("struct Foo { a: i32, }<|>", "struct Foo <|>{ a: i32, }");
    }

}
