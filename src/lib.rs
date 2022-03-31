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

// TODO: Lots of const fn

use constants::RATING_SCALING_RATIO;

pub mod algorithm;
pub mod constants;
pub mod engine;
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

/// A Glicko-2 skill rating.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rating {
    rating: f64,
    deviation: f64,
    volatility: f64,
}

impl FromWithParameters<ScaledRating> for Rating {
    fn from_with_parameters(scaled: ScaledRating, parameters: Parameters) -> Self {
        let public_rating = scaled.rating * RATING_SCALING_RATIO + parameters.start_rating;
        let public_deviation = scaled.deviation * RATING_SCALING_RATIO;

        Rating::new(public_rating, public_deviation, scaled.volatility)
    }
}

impl Rating {
    /// Creates a new [`Rating`] with the specified parameters.
    ///  
    /// # Panics
    ///
    /// This function panics if `deviation` or `volatility` was <= 0.
    #[must_use]
    pub fn new(rating: f64, deviation: f64, volatility: f64) -> Self {
        assert!(deviation > 0.0, "deviation <= 0: {deviation}");
        assert!(volatility > 0.0, "volatility <= 0: {volatility}");

        Rating {
            rating,
            deviation,
            volatility,
        }
    }

    /// Creates a new rating with the starting values specified in `parameters`.
    #[must_use]
    pub fn default_from_parameters(parameters: Parameters) -> Self {
        Rating::new(
            parameters.start_rating,
            parameters.start_deviation,
            parameters.start_volatility,
        )
    }

    /// The rating value.
    #[must_use]
    pub fn rating(&self) -> f64 {
        self.rating
    }

    /// The rating deviation.
    #[must_use]
    pub fn deviation(&self) -> f64 {
        self.deviation
    }

    /// The rating volatility.
    #[must_use]
    pub fn volatility(&self) -> f64 {
        self.volatility
    }
}

/// A Glicko-2 rating scaled to the internal rating scale.
/// See "Step 2." and "Step 8." in [Glickmans' paper](http://www.glicko.net/glicko/glicko2.pdf).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScaledRating {
    rating: f64,
    deviation: f64,
    volatility: f64,
}

impl FromWithParameters<Rating> for ScaledRating {
    fn from_with_parameters(rating: Rating, parameters: Parameters) -> Self {
        let scaled_rating = (rating.rating - parameters.start_rating) / RATING_SCALING_RATIO;
        let scaled_deviation = rating.deviation / RATING_SCALING_RATIO;

        ScaledRating::new(scaled_rating, scaled_deviation, rating.volatility)
    }
}

impl ScaledRating {
    /// Creates a new [`ScaledRating`] with the specified parameters.
    ///
    /// # Panics
    ///
    /// This function panics if `deviation` or `volatility` was <= 0.
    #[must_use]
    pub fn new(rating: f64, deviation: f64, volatility: f64) -> Self {
        assert!(deviation > 0.0, "deviation <= 0: {deviation}");
        assert!(volatility > 0.0, "volatility <= 0: {volatility}");

        ScaledRating {
            rating,
            deviation,
            volatility,
        }
    }

    /// The rating value.
    #[must_use]
    pub fn rating(&self) -> f64 {
        self.rating
    }

    /// The rating deviation.
    #[must_use]
    pub fn deviation(&self) -> f64 {
        self.deviation
    }

    /// The rating volatility.
    #[must_use]
    pub fn volatility(&self) -> f64 {
        self.volatility
    }
}

/// The parameters used by the Glicko-2 algorithm.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Parameters {
    // TODO: Private fields
    pub start_rating: f64,
    pub start_deviation: f64,
    pub start_volatility: f64,
    pub volatility_change: f64,
    pub convergence_tolerance: f64,
}

impl Parameters {
    /// Creates [`Parameters`] with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `start_rating` - The rating value a new player starts out with. See also [`Rating::default_from_parameters`], [`constants::DEFAULT_START_RATING`].
    /// * `start_deviation` - The rating deviation a new player starts out with. See also [`Rating::default_from_parameters`], [`constants::DEFAULT_START_DEVIATION`].
    /// * `start_volatility` - The rating volatility a new player starts out with. See also [`Rating::default_from_parameters`], [`constants::DEFAULT_START_VOLATILITY`].
    /// * `volatility_change` - Also called "system constant" or "τ".
    /// This constant constraints change in volatility over time.
    /// Reasonable choices are between 0.3 and 1.2.
    /// Small values prevent volatility and therefore rating from changing too much after improbable results.
    /// See also "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf) and [`constants::DEFAULT_VOLATILITY_CHANGE`].
    /// * `convergence_tolerance` - The cutoff value for the converging loop algorithm in "Step 5.1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    /// See also [`constants::DEFAULT_CONVERGENCE_TOLERANCE`].
    ///
    /// # Panics
    ///
    /// This function panics if `start_deviation`, `start_volatility`, or `convergence_tolerance` was <= 0.
    #[must_use]
    #[allow(clippy::too_many_arguments)] // TODO: Maybe builder pattern idk?
    pub fn new(
        // TODO: Collapse rating, deviation, volatility into Rating
        start_rating: f64,
        start_deviation: f64,
        start_volatility: f64,
        volatility_change: f64,
        convergence_tolerance: f64,
    ) -> Self {
        assert!(
            start_deviation > 0.0,
            "start_deviation <= 0: {start_deviation}"
        );
        assert!(
            start_volatility > 0.0,
            "start_volatility <= 0: {start_volatility}"
        );
        assert!(
            convergence_tolerance > 0.0,
            "convergence_tolerance <= 0: {convergence_tolerance}"
        );

        Parameters {
            start_rating,
            start_deviation,
            start_volatility,
            volatility_change,
            convergence_tolerance,
        }
    }

    /// Creates [`Parameters`] with the same parameters as `self`, only changing the volatility change to `volatility_change`.
    #[must_use]
    pub fn with_volatility_change(self, volatility_change: f64) -> Self {
        Parameters {
            volatility_change,
            ..self
        }
    }

    /// The rating value a new player starts out with.
    ///
    /// See also [`Rating::default_from_parameters`], [`constants::DEFAULT_START_RATING`].
    #[must_use]
    pub fn start_rating(&self) -> f64 {
        self.start_rating
    }

    /// The rating deviation a new player starts out with.
    ///
    /// See also [`Rating::default_from_parameters`], [`constants::DEFAULT_START_DEVIATION`].
    #[must_use]
    pub fn start_deviation(&self) -> f64 {
        self.start_deviation
    }

    /// The rating volatility a new player starts out with.
    ///
    /// See also [`Rating::default_from_parameters`], [`constants::DEFAULT_START_VOLATILITY`].
    #[must_use]
    pub fn start_volatility(&self) -> f64 {
        self.start_volatility
    }

    /// `volatility_change` - Also called "system constant" or "τ".
    /// This constant constraints change in volatility over time.
    /// Reasonable choices are between 0.3 and 1.2.
    /// Small values prevent volatility and therefore rating from changing too much after improbable results.
    ///
    /// See also "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf) and [`constants::DEFAULT_VOLATILITY_CHANGE`].
    #[must_use]
    pub fn volatility_change(&self) -> f64 {
        self.volatility_change
    }

    /// The cutoff value for the converging loop algorithm in "Step 5.1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    ///
    /// See also [`constants::DEFAULT_CONVERGENCE_TOLERANCE`].
    #[must_use]
    pub fn convergence_tolerance(&self) -> f64 {
        self.convergence_tolerance
    }
}

impl Default for Parameters {
    /// Creates a default version of this struct with the parameters defined in [`constants`].
    fn default() -> Self {
        Parameters {
            start_rating: constants::DEFAULT_START_RATING,
            start_deviation: constants::DEFAULT_START_DEVIATION,
            start_volatility: constants::DEFAULT_START_VOLATILITY,
            volatility_change: constants::DEFAULT_VOLATILITY_CHANGE,
            convergence_tolerance: constants::DEFAULT_CONVERGENCE_TOLERANCE,
        }
    }
}
