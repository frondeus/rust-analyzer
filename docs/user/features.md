This documents is an index of features that rust-analyzer language server
provides. Shortcuts are for the default VS Code layout. If there's no shortcut,
you can use <kbd>Ctrl+Shift+P</kbd> to search for the corresponding action.

### Workspace Symbol <kbd>ctrl+t</kbd>

Uses fuzzy-search to find types, modules and function by name across your
project and dependencies. This **the** most useful feature, which improves code
navigation tremendously. It mostly works on top of the built-in LSP
functionality, however `#` and `*` symbols can be used to narrow down the
search. Specifically,

- `Foo` searches for `Foo` type in the current workspace
- `foo#` searches for `foo` function in the current workspace
- `Foo*` searches for `Foo` type among dependencies, excluding `stdlib`
- `foo#*` searches for `foo` function among dependencies.

That is, `#` switches from "types" to all symbols, `*` switches from the current
workspace to dependencies.

### Document Symbol <kbd>ctrl+shift+o</kbd>

Provides a tree of the symbols defined in the file. Can be used to

* fuzzy search symbol in a file (super useful)
* draw breadcrumbs to describe the context around the cursor
* draw outline of the file

### On Typing Assists

Some features trigger on typing certain characters:

- typing `let =` tries to smartly add `;` if `=` is followed by an existing expression.
- Enter inside comments automatically inserts `///`
- typing `.` in a chain method call auto-indents

### Commands <kbd>ctrl+shift+p</kbd>

#### Extend Selection

Extends the current selection to the encompassing syntactic construct
(expression, statement, item, module, etc). It works with multiple cursors. Do
bind this command to a key, it's super-useful! Expected to be upstreamed to LSP
soonish: https://github.com/Microsoft/language-server-protocol/issues/613

#### Run

Shows popup suggesting to run a test/benchmark/binary **at the current cursor
location**. Super useful for repeatedly running just a single test. Do bind this
to a shortcut!

#### Parent Module

Navigates to the parent module of the current module.

#### Matching Brace

If the cursor is on any brace (`<>(){}[]`) which is a part of a brace-pair,
moves cursor to the matching brace. It uses the actual parser to determine
braces, so it won't confuse generics with comparisons.

#### Join Lines

Join selected lines into one, smartly fixing up whitespace and trailing commas.

#### Show Syntax Tree

Shows the parse tree of the current file. It exists mostly for debugging
rust-analyzer itself.

#### Status

Shows internal statistic about memory usage of rust-analyzer

#### Run garbage collection

Manually triggers GC

### Code Actions (Assists)

These are triggered in a particular context via light bulb. We use custom code on
the VS Code side to be able to position cursor. `<|>` signifies cursor

- Add `#[derive]`

```rust
// before:
struct Foo {
    <|>x: i32
}
// after:
#[derive(<|>)]
struct Foo {
    x: i32
}
```

- Add `impl`

```rust
// before:
struct Foo<'a, T: Debug> {
    <|>t: T
}
// after:
struct Foo<'a, T: Debug> {
    t: T
}

impl<'a, T: Debug> Foo<'a, T> {
    <|>
}
```

- Add missing `impl` members

```rust
// before:
trait Foo {
    fn foo(&self);
    fn bar(&self);
    fn baz(&self);
}

struct S;

impl Foo for S {
    fn bar(&self) {}
    <|>
}

// after:
trait Foo {
    fn foo(&self);
    fn bar(&self);
    fn baz(&self);
}

struct S;

impl Foo for S {
    fn bar(&self) {}
    fn foo(&self) { unimplemented!() }
    fn baz(&self) { unimplemented!() }<|>
}
```

- Import path

```rust
// before:
impl std::fmt::Debug<|> for Foo {
}

// after:
use std::fmt::Debug;

impl Debug<|> for Foo {
}
```

- Change Visibility

```rust
// before:
<|>fn foo() {}

// after:
<|>pub(crate) fn foo() {}

// after:
<|>pub fn foo() {}
```

- Fill match arms

```rust
// before:
enum A {
    As,
    Bs,
    Cs(String),
    Ds(String, String),
    Es{x: usize, y: usize}
}

fn main() {
    let a = A::As;
    match a<|> {}
}

// after:
enum A {
    As,
    Bs,
    Cs(String),
    Ds(String, String),
    Es{x: usize, y: usize}
}

fn main() {
    let a = A::As;
    match <|>a {
        A::As => (),
        A::Bs => (),
        A::Cs(_) => (),
        A::Ds(_, _) => (),
        A::Es{x, y} => (),
    }
}
```

-- Fill struct fields

```rust
// before:
struct S<'a, D> {
    a: u32,
    b: String,
    c: (i32, i32),
    d: D,
    r: &'a str,
}

fn main() {
    let s = S<|> {}
}

// after:
struct S<'a, D> {
    a: u32,
    b: String,
    c: (i32, i32),
    d: D,
    r: &'a str,
}

fn main() {
    let s = <|>S {
        a: (),
        b: (),
        c: (),
        d: (),
        r: (),
    }
}
```

- Flip `,`

```rust
// before:
fn foo(x: usize,<|> dim: (usize, usize)) {}
// after:
fn foo(dim: (usize, usize), x: usize) {}
```

- Introduce variable:

```rust
// before:
fn foo() {
    foo(<|>1 + 1<|>);
}

// after:
fn foo() {
    let var_name = 1 + 1;
    foo(var_name);
}
```

-- Remove `dbg!`

```rust
// before:
fn foo(n: usize) {
    if let Some(_) = dbg!(n.<|>checked_sub(4)) {
        // ...
    }
}

// after:
fn foo(n: usize) {
    if let Some(_) = n.<|>checked_sub(4) {
        // ...
    }
}
```

- Replace if-let with match:

```rust
// before:
impl VariantData {
    pub fn is_struct(&self) -> bool {
        if <|>let VariantData::Struct(..) = *self {
            true
        } else {
            false
        }
    }
}

// after:
impl VariantData {
    pub fn is_struct(&self) -> bool {
        <|>match *self {
            VariantData::Struct(..) => true,
            _ => false,
        }
    }
}
```

- Split import

```rust
// before:
use algo:<|>:visitor::{Visitor, visit};
//after:
use algo::{<|>visitor::{Visitor, visit}};
```

### Magic Completions

In addition to usual reference completion, rust-analyzer provides some ✨magic✨
completions as well:

Keywords like `if`, `else` `while`, `loop` are completed with braces, and cursor
is placed at the appropriate position. Even though `if` is easy to type, you
still want to complete it, to get ` { }` for free! `return` is inserted with a
space or `;` depending on the return type of the function.

When completing a function call, `()` are automatically inserted. If function
takes arguments, cursor is positioned inside the parenthesis.

There are postifx completions, which can be triggerd by typing something like
`foo().if`. The word after `.` determines postifx completion, possible variants are:

- `expr.if` -> `if expr {}`
- `expr.match` -> `match expr {}`
- `expr.while` -> `while expr {}`
- `expr.ref` -> `&expr`
- `expr.refm` -> `&mut expr`
- `expr.not` -> `!expr`
- `expr.dbg` -> `dbg!(expr)`

There also snippet completions:

#### Inside Expressions

- `pd` -> `println!("{:?}")`
- `ppd` -> `println!("{:#?}")`

#### Inside Modules

- `tfn` -> `#[test] fn f(){}`

