use std::slice::IterMut;

#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PushOnlyVec<T>(Vec<T>);

impl<T> From<Vec<T>> for PushOnlyVec<T> {
    fn from(vec: Vec<T>) -> Self {
        PushOnlyVec(vec)
    }
}

impl<T> From<PushOnlyVec<T>> for Vec<T> {
    fn from(push_only: PushOnlyVec<T>) -> Self {
        push_only.0
    }
}

impl<T> PushOnlyVec<T> {
    #[must_use]
    pub fn new() -> Self {
        PushOnlyVec(Vec::new())
    }

    #[must_use]
    pub fn vec(&self) -> &Vec<T> {
        &self.0
    }

    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }

    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.0.get_mut(index)
    }

    #[must_use]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.0.iter_mut()
    }
}
