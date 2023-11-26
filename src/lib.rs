//! This crate provides an implementation of the [Glicko-2](https://www.glicko.net/glicko/glicko2.pdf) rating system.
//! Due to the concept of rating periods, Glicko-2 has the problem that rankings cannot easily be updated instantly after a match concludes.
//!
//! This implementation aims to solve that problem by allowing fractional rating periods, so that ratings can be updated directly after every game, and not just once a rating period closes.
//! This draws inspiration from the [rating system implementation](https://github.com/lichess-org/lila/tree/master/modules/rating/src/main/glicko2) for open-source chess website [Lichess](https://lichess.org),
//! as well as two blogpost ([1](https://blog.hypersect.com/the-online-skill-ranking-of-inversus-deluxe/), [2](https://blog.hypersect.com/additional-thoughts-on-skill-ratings/)) by Ryan Juckett on skill ratings for [INVERSUS Deluxe](https://www.inversusgame.com/).
//!
//! # Examples
//!
//! Example calculation from [Glickman's paper](https://www.glicko.net/glicko/glicko2.pdf) using [`algorithm`]:
//!
//! ```
//! use instant_glicko_2::{GlickoSettings, PublicRating, IntoWithSettings};
//! use instant_glicko_2::algorithm::{self, PublicGame};
//!
//! let settings = GlickoSettings::default().with_volatility_change(0.5);
//!
//! // Create our player's rating
//! let mut player = PublicRating::new(1500.0, 200.0, 0.06);
//!
//! // Create our opponents
//! // Their volatility is not specified in the paper and it doesn't matter in the calculation,
//! // so we're just using the default starting volatility.
//! let opponent_a = PublicRating::new(1400.0, 30.0, settings.start_rating().volatility());
//! let opponent_b = PublicRating::new(1550.0, 100.0, settings.start_rating().volatility());
//! let opponent_c = PublicRating::new(1700.0, 300.0, settings.start_rating().volatility());
//!
//! // Create match results for our player
//! let results = [
//!     // Wins first game (score 1.0)
//!     PublicGame::new(opponent_a, 1.0).into_with_settings(settings),
//!     // Loses second game (score 0.0)
//!     PublicGame::new(opponent_b, 0.0).into_with_settings(settings),
//!     // Loses third game (score 0.0)
//!     PublicGame::new(opponent_c, 0.0).into_with_settings(settings),
//! ];
//!
//! // Update rating after rating period
//! let new_rating: PublicRating = algorithm::rate_games_untimed(player.into_with_settings(settings), &results, 1.0, settings).into_with_settings(settings);
//!
//! // The rating after the rating period are very close to the results from the paper
//! assert!((new_rating.rating() - 1464.06).abs() < 0.01);
//! assert!((new_rating.deviation() - 151.52).abs() < 0.01);
//! assert!((new_rating.volatility() - 0.05999).abs() < 0.0001);
//! ```
//!
//! Different example using [`RatingEngine`][engine::RatingEngine]:
//!
//! ```
//! use std::time::Duration;
//!
//! use instant_glicko_2::{GlickoSettings, PublicRating};
//! use instant_glicko_2::engine::{MatchResult, RatingEngine};
//!
//! let settings = GlickoSettings::default();
//!
//! // Create a RatingEngine with a one day rating period duration
//! // The first rating period starts instantly
//! let mut engine = RatingEngine::start_new(GlickoSettings::default());
//!
//! // Register two players
//! // The first player is relatively strong
//! let player_1_rating_old = PublicRating::new(1700.0, 300.0, 0.06);
//! let player_1 = engine.register_player(player_1_rating_old).0;
//! // The second player hasn't played any games
//! let player_2_rating_old = settings.start_rating();
//! let player_2 = engine.register_player(player_2_rating_old).0;
//!
//! // They play and player_2 wins
//! engine.register_result(
//!     player_1,
//!     player_2,
//!     &MatchResult::Loss,
//! );
//!
//! // Print the new ratings
//! // Type signatures are needed because we could also work with the internal InternalRating
//! // That skips one step of calculation,
//! // but the rating values are not as pretty and not comparable to the original Glicko ratings
//! let player_1_rating_new: PublicRating = engine.player_rating(player_1).0;
//! println!("Player 1 old rating: {player_1_rating_old:?}, new rating: {player_1_rating_new:?}");
//! let player_2_rating_new: PublicRating = engine.player_rating(player_2).0;
//! println!("Player 2 old rating: {player_2_rating_old:?}, new rating: {player_2_rating_new:?}");
//!
//! // Loser's rating goes down, winner's rating goes up
//! assert!(player_1_rating_old.rating() > player_1_rating_new.rating());
//! assert!(player_2_rating_old.rating() < player_2_rating_new.rating());
//! ```
//!
//! The [`algorithm`] module provides an implementation of the Glicko-2 algorithm that allows for fractional rating periods.
//!
//! The [`engine`] module provides the [`RatingEngine`][engine::RatingEngine] struct which allows for adding games
//! and getting the current rating of managed players at any point in time.

#![warn(clippy::pedantic)]
#![warn(clippy::cargo)]
#![warn(
    missing_docs,
    rustdoc::missing_crate_level_docs,
    rustdoc::private_doc_tests
)]
#![deny(
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    rustdoc::invalid_codeblock_attributes,
    rustdoc::invalid_rust_codeblocks
)]
#![forbid(unsafe_code)]

// TODO: Lots of const fn

use constants::RATING_SCALING_RATIO;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub mod algorithm;
pub mod constants;
pub mod engine;
pub mod util;

/// Marker type for the public scale of Glicko-2 ratings. See [`RatingScale`].
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Debug)]
pub enum Public {}

/// Marker type for the internal scale of Glicko-2 ratings. See [`RatingScale`].
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Debug)]
pub enum Internal {}

/// Marker trait for any rating scale.
/// Rating scales are used to convert between different representations of ratings.
/// All Glicko-2 calculations use the [`Internal`] rating scale,
/// but it is recommended that these internal ratings are not displayed.
/// Display [`Public`] ratings instead.
///
/// You can convert between the scales [`FromWithSettings`] and [`IntoWithSettings`].
/// The requirement for that is that [`ConvertToScale`] is implemented for the given scales,
/// but this is the case for [`Public`] and [`Internal`].
pub trait RatingScale: Eq + PartialEq + Ord + PartialOrd + Copy + Clone + Debug {}

impl RatingScale for Public {}
impl RatingScale for Internal {}

/// A trait to define how [`Rating`]s are converted between different [`RatingScale`]s.
pub trait ConvertToScale<S: RatingScale>: RatingScale {
    /// Converts a rating of the [`Self`] scale to the new `S` scale.
    #[must_use]
    fn convert(rating: Rating<Self>, settings: GlickoSettings) -> Rating<S>;
}

impl<S: RatingScale> ConvertToScale<S> for S {
    fn convert(rating: Rating<Self>, _settings: GlickoSettings) -> Rating<S> {
        rating
    }
}

impl ConvertToScale<Public> for Internal {
    fn convert(internal: Rating<Self>, settings: GlickoSettings) -> Rating<Public> {
        let public_rating =
            internal.rating() * RATING_SCALING_RATIO + settings.start_rating().rating();
        let public_deviation = internal.deviation() * RATING_SCALING_RATIO;

        Rating::new(public_rating, public_deviation, internal.volatility())
    }
}

impl ConvertToScale<Internal> for Public {
    fn convert(public: Rating<Self>, settings: GlickoSettings) -> Rating<Internal> {
        let internal_rating =
            (public.rating() - settings.start_rating().rating()) / RATING_SCALING_RATIO;
        let internal_deviation = public.deviation() / RATING_SCALING_RATIO;

        InternalRating::new(internal_rating, internal_deviation, public.volatility())
    }
}

/// Trait to convert between two types with [`GlickoSettings`].
/// Usually used to convert between the [`Internal`] and [`Public`] Glicko-2 rating scales.
pub trait FromWithSettings<T: ?Sized> {
    /// Performs the conversion
    #[must_use]
    fn from_with_settings(_: T, settings: GlickoSettings) -> Self;
}

/// Trait to convert between two types with [`GlickoSettings`].
/// Usually used to convert between the [`Internal`] the [`Public`] Glicko-2 rating scales.
///
/// This trait is automatically provided for any type `T` where [`FromWithSettings<T>`] is implemented.
pub trait IntoWithSettings<T> {
    /// Performs the conversion
    fn into_with_settings(self, settings: GlickoSettings) -> T;
}

impl<T, U> IntoWithSettings<U> for T
where
    U: FromWithSettings<T>,
{
    fn into_with_settings(self, settings: GlickoSettings) -> U {
        U::from_with_settings(self, settings)
    }
}

/// A Glicko-2 skill rating.
///
/// If `Scale` is [`Internal`], this is scaled to the internal rating scale.
/// If `Scale` is [`Public`], this is scaled to the public rating scale (so that it can be displayed).
#[derive(Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "", serialize = "")))]
pub struct Rating<Scale: RatingScale> {
    rating: f64,
    deviation: f64,
    volatility: f64,
    #[cfg_attr(feature = "serde", serde(skip))]
    _scale: PhantomData<Scale>,
}

/// A Glicko-2 rating of [`Public`] scale. See [`Rating`].
pub type PublicRating = Rating<Public>;

/// A Glicko-2 rating of [`Internal`] scale. See [`Rating`].
pub type InternalRating = Rating<Internal>;

impl<Scale: RatingScale> PartialOrd for Rating<Scale> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.rating.partial_cmp(&other.rating)
    }
}

impl<Scale1: RatingScale, Scale2: RatingScale> FromWithSettings<Rating<Scale1>> for Rating<Scale2>
where
    Scale1: ConvertToScale<Scale2>,
{
    fn from_with_settings(rating: Rating<Scale1>, settings: GlickoSettings) -> Self {
        ConvertToScale::convert(rating, settings)
    }
}

impl<Scale: RatingScale> Rating<Scale> {
    /// Creates a new [`Rating`] with the specified values.
    ///  
    /// # Panics
    ///
    /// This function panics if `deviation <= 0.0` or `volatility <= 0.0`.
    #[must_use]
    pub fn new(rating: f64, deviation: f64, volatility: f64) -> Self {
        assert!(deviation > 0.0, "deviation <= 0: {deviation}");
        assert!(volatility > 0.0, "volatility <= 0: {volatility}");

        Rating {
            rating,
            deviation,
            volatility,
            _scale: PhantomData,
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

/// The settings used by the Glicko-2 algorithm.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GlickoSettings {
    start_rating: PublicRating,
    volatility_change: f64,
    convergence_tolerance: f64,
    rating_period_duration: Duration,
}

impl GlickoSettings {
    /// Creates [`GlickoSettings`] with the given settings.
    ///
    /// # Arguments
    ///
    /// * `start_rating` - The rating value a new player starts out with. See also [`constants::DEFAULT_START_RATING`].
    /// * `volatility_change` - Also called "system constant" or "τ".
    /// This constant constraints change in volatility over time.
    /// Reasonable choices are between `0.3` and `1.2`.
    /// Small values prevent volatility and therefore rating from changing too much after improbable results.
    /// See also "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf) and [`constants::DEFAULT_VOLATILITY_CHANGE`].
    /// * `convergence_tolerance` - The cutoff value for the converging loop algorithm in "Step 5.1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    /// See also [`constants::DEFAULT_CONVERGENCE_TOLERANCE`].
    /// * `rating_period_duration` - The duration of one (virtual) rating period.
    /// According to [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf), the rating period duration should be such that
    /// an average of at least 10-15 games are played within one period.
    ///
    /// # Panics
    ///
    /// This function panics if `convergence_tolerance <= 0.0` or if `rating_period_duration` is zero.
    #[must_use]
    pub fn new(
        start_rating: PublicRating,
        volatility_change: f64,
        convergence_tolerance: f64,
        rating_period_duration: Duration,
    ) -> Self {
        assert!(
            convergence_tolerance > 0.0,
            "convergence_tolerance <= 0: {convergence_tolerance}"
        );
        assert!(
            !rating_period_duration.is_zero(),
            "rating_period_duration may not be zero"
        );

        GlickoSettings {
            start_rating,
            volatility_change,
            convergence_tolerance,
            rating_period_duration,
        }
    }

    /// Creates [`GlickoSettings`] with the same settings as `self`, only changing the start rating to `start_rating`.
    #[must_use]
    pub fn with_start_rating(self, start_rating: PublicRating) -> Self {
        GlickoSettings {
            start_rating,
            ..self
        }
    }

    /// Creates [`GlickoSettings`] with the same settings as `self`, only changing the volatility change to `volatility_change`.
    #[must_use]
    pub fn with_volatility_change(self, volatility_change: f64) -> Self {
        GlickoSettings {
            volatility_change,
            ..self
        }
    }

    /// Creates [`GlickoSettings`] with the same settings as `self`, only changing the convergence tolerance to `convergence_tolerance`.
    #[must_use]
    pub fn with_convergence_tolerance(self, convergence_tolerance: f64) -> Self {
        GlickoSettings {
            convergence_tolerance,
            ..self
        }
    }

    /// Creates [`GlickoSettings`] with the same settings as `self`, only changing the rating period duration to `rating_period_duration`.
    #[must_use]
    pub fn with_rating_period_duration(self, rating_period_duration: Duration) -> Self {
        GlickoSettings {
            rating_period_duration,
            ..self
        }
    }

    /// The rating value a new player starts out with.
    ///
    /// See also [`constants::DEFAULT_START_RATING`].
    #[must_use]
    pub fn start_rating(&self) -> PublicRating {
        self.start_rating
    }

    /// `volatility_change` - Also called "system constant" or "τ".
    /// This constant constraints change in volatility over time.
    /// Reasonable choices are between `0.3` and `1.2`.
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

    /// The
    #[must_use]
    pub fn rating_period_duration(&self) -> Duration {
        self.rating_period_duration
    }
}

impl Default for GlickoSettings {
    /// Creates a default version of this struct with the settings defined in [`constants`].
    fn default() -> Self {
        GlickoSettings::new(
            constants::DEFAULT_START_RATING,
            constants::DEFAULT_VOLATILITY_CHANGE,
            constants::DEFAULT_CONVERGENCE_TOLERANCE,
            constants::DEFAULT_RATING_PERIOD_DURATION,
        )
    }
}
