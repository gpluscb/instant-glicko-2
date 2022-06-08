//! Utilities that are needed or useful for the rest of the crate,
//! but that don't really have anything to do with the core of the crate.

use std::slice::{IterMut, SliceIndex};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A wrapper around [`Vec`] that only let's you append.
/// This allows for indic into the [`Vec`] to always stay valid.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
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
    /// Creates a new, empty `PushOnlyVec<T>`.
    ///
    /// See also: [`Vec::new()`]
    #[must_use]
    pub fn new() -> Self {
        PushOnlyVec(Vec::new())
    }

    /// Returns an immutable reference to the underlying [`Vec`].
    /// Use this for all immutable operations on the [`Vec`].
    #[must_use]
    pub fn vec(&self) -> &Vec<T> {
        &self.0
    }

    /// Appends an element to the underlying [`Vec`].
    ///
    /// See also: [`Vec::push()`]
    ///
    /// # Panics
    ///
    /// This function panics if the capacity of the underlying [`Vec`] exceeds [`isize::MAX`] bytes.
    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }

    /// Returns a mutable reference to an element or subslice depending on the
    /// type of index or `None` if the index is out of bounds.
    ///
    /// See also: [`[T]::get_mut`]
    #[must_use]
    pub fn get_mut<I>(&mut self, index: I) -> Option<&mut I::Output>
    where
        I: SliceIndex<[T]>,
    {
        self.0.get_mut(index)
    }

    /// Returns an iterator that allows mutating each value.
    ///
    /// See also: [`[T]::iter_mut()`]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.0.iter_mut()
    }
}
