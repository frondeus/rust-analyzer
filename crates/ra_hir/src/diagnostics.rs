use std::{fmt, any::Any};

use ra_syntax::SyntaxNodePtr;

use crate::HirFileId;

pub trait Diagnostic: Any + Send + Sync + fmt::Debug + 'static {
    fn file(&self) -> HirFileId;
    fn syntax_node(&self) -> SyntaxNodePtr;
    fn dyn_eq(&self, other: &dyn Diagnostic) -> bool;
    fn _dyn_eq(&self, other: &dyn Diagnostic) -> bool
    where
        Self: Sized + Eq,
    {
        match other.downcast_ref::<Self>() {
            None => false,
            Some(it) => it == self,
        }
    }
}

impl dyn Diagnostic {
    pub fn downcast_ref<D: Diagnostic>(&self) -> Option<&D> {
        self.downcast_ref()
    }
}

impl PartialEq for dyn Diagnostic {
    fn eq(&self, other: &dyn Diagnostic) -> bool {
        self.dyn_eq(other)
    }
}

impl Eq for dyn Diagnostic {}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    data: Vec<Box<dyn Diagnostic>>,
}

impl Diagnostics {
    pub fn push(&mut self, d: impl Diagnostic) {
        self.data.push(Box::new(d))
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a dyn Diagnostic> + 'a {
        self.data.iter().map(|it| it.as_ref())
    }
}
