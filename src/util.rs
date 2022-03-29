use std::slice::{Iter, IterMut};

#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PushOnlyVec<T>(Vec<T>);

impl<T> PushOnlyVec<T> {
    #[must_use]
    pub fn into_inner(self) -> Vec<T> {
        self.0
    }

    #[must_use]
    pub fn vec(&self) -> &Vec<T> {
        &self.0
    }

    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }

    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.0.get_mut(index)
    }

    #[must_use]
    pub fn iter(&self) -> Iter<'_, T> {
        self.0.iter()
    }

    #[must_use]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.0.iter_mut()
    }
}
