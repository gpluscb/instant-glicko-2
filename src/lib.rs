//! This crate provides an implementation of the [Glicko-2](https://www.glicko.net/glicko/glicko2.pdf) rating system.
//! Due to the concept of rating periods, Glicko-2 has the problem that rankings cannot easily be updated instantly after a match concludes.
//! This implementation aims to solve that problem by allowing fractional rating periods, so that ratings can be updated directly after every game, and not just once a rating period closes.
//! This draws inspiration from the [rating system implementation](https://github.com/lichess-org/lila/tree/master/modules/rating/src/main/java/glicko2) for open-source chess website [Lichess](https://lichess.org),
//! as well as two blogpost ([1](https://blog.hypersect.com/the-online-skill-ranking-of-inversus-deluxe/), [2](https://blog.hypersect.com/additional-thoughts-on-skill-ratings/)) by Ryan Juckett on skill ratings for [INVERSUS Deluxe](https://www.inversusgame.com/).
//!
//! The [`algorithm`] module provides an implementation of the Glicko-2 algorithm that allows for fractional rating periods.
//!
//! The [`engine`] module provides the [`RatingEngine`][engine::RatingEngine] struct which allows for adding games
//! and getting the current rating of managed players at any point in time.

#![warn(clippy::pedantic)]
#![warn(clippy::cargo)]
// #![warn(
//     missing_docs,
//     rustdoc::missing_crate_level_docs,
//     rustdoc::private_doc_tests
// )] // TODO
#![deny(
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    rustdoc::invalid_codeblock_attributes,
    rustdoc::invalid_rust_codeblocks
)]
#![forbid(unsafe_code)]

// TODO: can probably easily get nostd to work at least partially

use model::Parameters;

pub mod algorithm;
pub mod constants;
pub mod engine;
pub mod model;
pub mod util;

/// Trait to convert between two types with [`Parameters`].
/// Usually used to convert between the internal rating scaling and the public Glicko rating scaling.
///
/// A blanket implementation [`FromWithParameters<T>`] for any `T` is provided.
pub trait FromWithParameters<T: ?Sized> {
    /// Performs the conversion
    fn from_with_parameters(_: T, parameters: Parameters) -> Self;
}

impl<T> FromWithParameters<T> for T {
    fn from_with_parameters(t: T, _: Parameters) -> Self {
        t
    }
}

/// Trait to convert between two types with [`Parameters`].
/// Usually used to convert between the internal rating scaling and the public Glicko rating scaling.
///
/// This trait is automatically provided for any type `T` where [`FromWithParameters<T>`] is implemented.
pub trait IntoWithParameters<T> {
    /// Performs the conversion
    fn into_with_parameters(self, parameters: Parameters) -> T;
}

impl<T, U> IntoWithParameters<U> for T
where
    U: FromWithParameters<T>,
{
    fn into_with_parameters(self, parameters: Parameters) -> U {
        U::from_with_parameters(self, parameters)
    }
}
