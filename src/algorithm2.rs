//! This module hosts the methods and types necessary to perform Glicko-2 calculations with fractional rating periods.

use std::f64::consts::PI;
use std::time::{Duration, SystemTime};

use crate::{
    constants, FromWithParameters, InternalRating, IntoWithParameters, Parameters, PublicRating,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A rating at a specific point in time.
/// This is a *public* rating, meaning it is meant to be displayed to users,
/// but it needs to be converted to an internal rating before use in rating calculations.
///
/// The timing of the rating is important because the deviation increases over the time no games are recorded.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimedPublicRating {
    last_updated: SystemTime,
    rating: PublicRating,
}

impl TimedPublicRating {
    /// Creates a new [`TimedPublicRating`] at the given `last_updated` time with the given `rating`.
    #[must_use]
    pub fn new(last_updated: SystemTime, rating: PublicRating) -> Self {
        Self {
            last_updated,
            rating,
        }
    }

    /// The time this rating was last updated.
    #[must_use]
    pub fn last_updated(&self) -> SystemTime {
        self.last_updated
    }

    /// The rating at the time it was last updated.
    #[must_use]
    pub fn raw_public_rating(&self) -> PublicRating {
        self.rating
    }

    /// The rating with the deviation updated to the current time after no games were played since the last update.
    /// Convenience for `self.public_rating_at(SystemTime::now(), parameters, rating_period_duration)`.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is in the future, or if the `rating_period_duration` is zero.
    #[must_use]
    pub fn public_rating_now(
        &self,
        parameters: Parameters,
        rating_period_duration: Duration,
    ) -> PublicRating {
        self.public_rating_at(SystemTime::now(), parameters, rating_period_duration)
    }

    /// The rating with the deviation updated to the given time after no games were played since the last update.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is after `time`, or if the `rating_period_duration` is zero.
    #[must_use]
    pub fn public_rating_at(
        &self,
        time: SystemTime,
        parameters: Parameters,
        rating_period_duration: Duration,
    ) -> PublicRating {
        let internal_rating: InternalRating = self.rating.into_with_parameters(parameters);

        let new_deviation = calculate_pre_rating_period_value(
            internal_rating.volatility(),
            internal_rating,
            self.elapsed_rating_periods(time, rating_period_duration),
        );

        InternalRating {
            deviation: new_deviation,
            ..internal_rating
        }
        .into_with_parameters(parameters)
    }

    /// # Panics
    ///
    /// This function panics if `time` is **before** the last rating update, or if the `rating_period_duration` is zero.
    #[must_use]
    fn elapsed_rating_periods(&self, time: SystemTime, rating_period_duration: Duration) -> f64 {
        time.duration_since(self.last_updated)
            .expect("Player rating was updated after the game to rate")
            .as_secs_f64()
            / rating_period_duration.as_secs_f64()
    }
}

impl FromWithParameters<TimedInternalRating> for TimedPublicRating {
    fn from_with_parameters(internal: TimedInternalRating, parameters: Parameters) -> Self {
        TimedPublicRating {
            last_updated: internal.last_updated,
            rating: internal.rating.into_with_parameters(parameters),
        }
    }
}

/// A rating at a specific point in time.
/// This is an *internal* rating, meaning it can be used immediately in rating calculations,
/// but should be converted to a public rating before displaying to users.
///
/// The timing of the rating is important because the deviation increases over the time no games are recorded.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimedInternalRating {
    last_updated: SystemTime,
    rating: InternalRating,
}

impl TimedInternalRating {
    /// Creates a new [`TimedInternalRating`] at the given `last_updated` time with the given `rating`.
    #[must_use]
    pub fn new(last_updated: SystemTime, rating: InternalRating) -> Self {
        Self {
            last_updated,
            rating,
        }
    }

    /// The time this rating was last updated.
    #[must_use]
    pub fn last_updated(&self) -> SystemTime {
        self.last_updated
    }

    /// The rating at the time it was last updated.
    #[must_use]
    pub fn raw_internal_rating(&self) -> InternalRating {
        self.rating
    }

    /// The rating with the deviation updated to the current time after no games were played since the last update.
    /// Convenience for `self.public_rating_at(SystemTime::now(), parameters, rating_period_duration)`.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is in the future, or if the `rating_period_duration` is zero.
    #[must_use]
    pub fn internal_rating_now(&self, rating_period_duration: Duration) -> InternalRating {
        self.internal_rating_at(SystemTime::now(), rating_period_duration)
    }

    /// The rating with the deviation updated to the given time after no games were played since the last update.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is after `time`, or if the `rating_period_duration` is zero.
    #[must_use]
    pub fn internal_rating_at(
        &self,
        time: SystemTime,
        rating_period_duration: Duration,
    ) -> InternalRating {
        let new_deviation = calculate_pre_rating_period_value(
            self.rating.volatility(),
            self.rating,
            self.elapsed_rating_periods(time, rating_period_duration),
        );

        InternalRating {
            deviation: new_deviation,
            ..self.rating
        }
    }

    /// # Panics
    ///
    /// This function panics if `time` is **before** the last rating update, or if the `rating_period_duration` is zero.
    #[must_use]
    fn elapsed_rating_periods(&self, time: SystemTime, rating_period_duration: Duration) -> f64 {
        time.duration_since(self.last_updated)
            .expect("Player rating was updated after the game to rate")
            .as_secs_f64()
            / rating_period_duration.as_secs_f64()
    }
}

impl FromWithParameters<TimedPublicRating> for TimedInternalRating {
    fn from_with_parameters(public: TimedPublicRating, parameters: Parameters) -> Self {
        TimedInternalRating {
            last_updated: public.last_updated,
            rating: public.rating.into_with_parameters(parameters),
        }
    }
}

/// Game information encompassing the opponent's rating at the time of the game
/// as well as the game score as a number between `0.0` (decicive opponent win) and `1.0` (decicive player win).
///
/// Keep in mind that this struct does not hold information about the player's rating, only the opponent's.
/// This is because it is used in relation to registering games on and therefore update the player's rating struct.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PublicGame {
    opponent: PublicRating,
    score: f64,
}

impl PublicGame {
    /// Creates a new [`PublicGame`] with the given `opponent` and `score`.
    /// `score` is a number between 0.0 (decicive opponent win) and `1.0` (decicive player win).
    ///
    /// # Panics
    ///
    /// This function panics if `score` is less than `0.0` or greater than `1.0`.
    #[must_use]
    pub fn new(opponent: PublicRating, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        Self { opponent, score }
    }

    /// The opponent's rating.
    #[must_use]
    pub fn opponent(&self) -> PublicRating {
        self.opponent
    }

    /// The game score as a number between `0.0` (decicive opponent win) and `1.0` (decicive player win).
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Converts a [`TimedPublicGame`] to a [`PublicGame`],
    /// erasing the timing information and resolving the opponents rating to their rating at the time of the game.
    ///
    /// # Panics
    ///
    /// This function panics if `timed_game`'s opponent rating was updated after the game was recorded, or if the `rating_period_duration` is zero.
    #[must_use]
    pub fn from_timed_public_game(
        timed_game: TimedPublicGame,
        parameters: Parameters,
        rating_period_duration: Duration,
    ) -> Self {
        timed_game.to_public_game(parameters, rating_period_duration)
    }
}

impl FromWithParameters<InternalGame> for PublicGame {
    fn from_with_parameters(internal: InternalGame, parameters: Parameters) -> Self {
        PublicGame {
            opponent: internal.opponent.into_with_parameters(parameters),
            score: internal.score,
        }
    }
}

/// Game information encompassing the opponent's internal rating at the time of the game
/// as well as the game score as a number between `0.0` (decicive opponent win) and `1.0` (decicive player win).
///
/// Keep in mind that this struct does not hold information about the player's rating, only the opponent's.
/// This is because it is used in relation to registering games on and therefore update the player's rating struct.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InternalGame {
    opponent: InternalRating,
    score: f64,
}

impl InternalGame {
    /// Creates a new [`InternalGame`] with the given `opponent` and `score`.
    /// `score` is a number between 0.0 (decicive opponent win) and `1.0` (decicive player win).
    ///
    /// # Panics
    ///
    /// This function panics if `score` is less than `0.0` or greater than `1.0`.
    #[must_use]
    pub fn new(opponent: InternalRating, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        InternalGame { opponent, score }
    }

    /// The opponent's rating.
    #[must_use]
    pub fn opponent(&self) -> InternalRating {
        self.opponent
    }

    /// The game score as a number between `0.0` (decicive opponent win) and `1.0` (decicive player win).
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Converts a [`TimedInternalGame`] to an [`InternalGame`],
    /// erasing the timing information and resolving the opponents rating to their rating at the time of the game.
    ///
    /// # Panics
    ///
    /// This function panics if `timed_game`'s opponent rating was updated after the game was recorded, or if the `rating_period_duration` is zero.
    #[must_use]
    pub fn from_timed_internal_game(
        timed_game: TimedInternalGame,
        rating_period_duration: Duration,
    ) -> Self {
        timed_game.to_internal_game(rating_period_duration)
    }
}

impl FromWithParameters<PublicGame> for InternalGame {
    fn from_with_parameters(public: PublicGame, parameters: Parameters) -> Self {
        InternalGame {
            opponent: public.opponent.into_with_parameters(parameters),
            score: public.score,
        }
    }
}

/// Game information encompassing
/// - The time the game was recorded
/// - The [`TimedPublicRating`] of the opponent
/// - The score as a number between `0.0` (decicive opponent win) and `1.0` (decicive player win)
///
/// Keep in mind that this struct does not hold information about the player's rating, only the opponent's.
/// This is because it is used to register games on and therefore update the player's rating struct.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimedPublicGame {
    time: SystemTime,
    opponent: TimedPublicRating,
    score: f64,
}

impl TimedPublicGame {
    /// Creates a new [`TimedPublicGame`] at the given `time` with the given `opponent` and `score`.
    /// `score` is a number between 0.0 (decicive opponent win) and `1.0` (decicive player win).
    ///
    /// # Panics
    ///
    /// This function panics if `score` is less than `0.0` or greater than `1.0`.
    #[must_use]
    pub fn new(time: SystemTime, opponent: TimedPublicRating, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        Self {
            time,
            opponent,
            score,
        }
    }

    /// The time this game was recorded.
    #[must_use]
    pub fn time(&self) -> SystemTime {
        self.time
    }

    /// The opponent's rating.
    #[must_use]
    pub fn opponent(&self) -> TimedPublicRating {
        self.opponent
    }

    /// The game score as a number between `0.0` (decicive opponent win) and `1.0` (decicive player win).
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Converts this [`TimedPublicGame`] to a [`PublicGame`],
    /// erasing the timing information and resolving the opponents rating to their rating at the time of the game.
    ///
    /// # Panics
    ///
    /// This function panics if the opponent rating was updated after the game was recorded, or if the `rating_period_duration` is zero.
    #[must_use]
    pub fn to_public_game(
        &self,
        parameters: Parameters,
        rating_period_duration: Duration,
    ) -> PublicGame {
        let opponent =
            self.opponent()
                .public_rating_at(self.time, parameters, rating_period_duration);

        PublicGame {
            opponent,
            score: self.score,
        }
    }
}

impl FromWithParameters<TimedInternalGame> for TimedPublicGame {
    fn from_with_parameters(internal: TimedInternalGame, parameters: Parameters) -> Self {
        TimedPublicGame {
            time: internal.time,
            opponent: internal.opponent.into_with_parameters(parameters),
            score: internal.score,
        }
    }
}

/// Game information encompassing
/// - The time the game was recorded
/// - The [`TimedInternalRating`] of the opponent
/// - The score as a number between `0.0` (decicive opponent win) and `1.0` (decicive player win)
///
/// Keep in mind that this struct does not hold information about the player's rating, only the opponent's.
/// This is because it is used to register games on and therefore update the player's rating struct.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimedInternalGame {
    time: SystemTime,
    opponent: TimedInternalRating,
    score: f64,
}

impl TimedInternalGame {
    /// Creates a new [`TimedInternalGame`] at the given `time` with the given `opponent` and `score`.
    /// `score` is a number between 0.0 (decicive opponent win) and `1.0` (decicive player win).
    ///
    /// # Panics
    ///
    /// This function panics if `score` is less than `0.0` or greater than `1.0`.
    #[must_use]
    pub fn new(time: SystemTime, opponent: TimedInternalRating, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        Self {
            time,
            opponent,
            score,
        }
    }

    /// The time this game was recorded.
    #[must_use]
    pub fn time(&self) -> SystemTime {
        self.time
    }

    /// The opponent's rating.
    #[must_use]
    pub fn opponent(&self) -> TimedInternalRating {
        self.opponent
    }

    /// The game score as a number between `0.0` (decicive opponent win) and `1.0` (decicive player win).
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Converts this [`TimedInternalGame`] to an [`InternalGame`],
    /// erasing the timing information and resolving the opponents rating to their rating at the time of the game.
    ///
    /// # Panics
    ///
    /// This function panics if the opponent rating was updated after the game was recorded, or if the `rating_period_duration` is zero.
    #[must_use]
    pub fn to_internal_game(&self, rating_period_duration: Duration) -> InternalGame {
        let opponent = self
            .opponent()
            .internal_rating_at(self.time, rating_period_duration);

        InternalGame {
            opponent,
            score: self.score,
        }
    }
}

impl FromWithParameters<TimedPublicGame> for TimedInternalGame {
    fn from_with_parameters(public: TimedPublicGame, parameters: Parameters) -> Self {
        TimedInternalGame {
            time: public.time,
            opponent: public.opponent.into_with_parameters(parameters),
            score: public.score,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimedOpponentPublicGame {
    opponent: TimedPublicRating,
    score: f64,
}

impl TimedOpponentPublicGame {
    #[must_use]
    pub fn new(opponent: TimedPublicRating, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        TimedOpponentPublicGame { opponent, score }
    }

    #[must_use]
    pub fn opponent(&self) -> TimedPublicRating {
        self.opponent
    }

    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

    #[must_use]
    pub fn timed_public_game_at(&self, time: SystemTime) -> TimedPublicGame {
        TimedPublicGame {
            time,
            opponent: self.opponent,
            score: self.score,
        }
    }

    #[must_use]
    pub fn public_game_at(
        &self,
        time: SystemTime,
        parameters: Parameters,
        rating_period_duration: Duration,
    ) -> PublicGame {
        let opponent = self
            .opponent
            .public_rating_at(time, parameters, rating_period_duration);

        PublicGame {
            opponent,
            score: self.score,
        }
    }
}

impl FromWithParameters<TimedOpponentInternalGame> for TimedOpponentPublicGame {
    fn from_with_parameters(internal: TimedOpponentInternalGame, parameters: Parameters) -> Self {
        TimedOpponentPublicGame {
            opponent: internal.opponent.into_with_parameters(parameters),
            score: internal.score,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimedOpponentInternalGame {
    opponent: TimedInternalRating,
    score: f64,
}

impl TimedOpponentInternalGame {
    #[must_use]
    pub fn new(opponent: TimedInternalRating, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        TimedOpponentInternalGame { opponent, score }
    }

    #[must_use]
    pub fn opponent(&self) -> TimedInternalRating {
        self.opponent
    }

    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

    #[must_use]
    pub fn timed_internal_game_at(&self, time: SystemTime) -> TimedInternalGame {
        TimedInternalGame {
            time,
            opponent: self.opponent,
            score: self.score,
        }
    }

    #[must_use]
    pub fn internal_game_at(
        &self,
        time: SystemTime,
        rating_period_duration: Duration,
    ) -> InternalGame {
        let opponent = self
            .opponent
            .internal_rating_at(time, rating_period_duration);

        InternalGame {
            opponent,
            score: self.score,
        }
    }
}

impl FromWithParameters<TimedOpponentPublicGame> for TimedOpponentInternalGame {
    fn from_with_parameters(public: TimedOpponentPublicGame, parameters: Parameters) -> Self {
        TimedOpponentInternalGame {
            opponent: public.opponent.into_with_parameters(parameters),
            score: public.score,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimedPublicGames {
    time: SystemTime,
    games: Vec<TimedOpponentPublicGame>,
}

impl TimedPublicGames {
    #[must_use]
    pub fn new(time: SystemTime, games: Vec<TimedOpponentPublicGame>) -> Self {
        TimedPublicGames { time, games }
    }

    #[must_use]
    pub fn single(game: TimedPublicGame) -> Self {
        TimedPublicGames {
            time: game.time(),
            games: vec![TimedOpponentPublicGame {
                opponent: game.opponent(),
                score: game.score(),
            }],
        }
    }

    #[must_use]
    pub fn time(&self) -> SystemTime {
        self.time
    }

    #[must_use]
    pub fn games(&self) -> &[TimedOpponentPublicGame] {
        &self.games
    }

    pub fn timed_games(&self) -> impl Iterator<Item = TimedPublicGame> + '_ {
        self.games
            .iter()
            .map(|game| game.timed_public_game_at(self.time()))
    }
}

impl From<TimedPublicGame> for TimedPublicGames {
    fn from(game: TimedPublicGame) -> Self {
        TimedPublicGames::single(game)
    }
}

impl FromWithParameters<TimedInternalGames> for TimedPublicGames {
    fn from_with_parameters(internal: TimedInternalGames, parameters: Parameters) -> Self {
        let public_games = internal
            .games()
            .iter()
            .map(|&game| game.into_with_parameters(parameters))
            .collect();

        TimedPublicGames {
            time: internal.time(),
            games: public_games,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimedInternalGames {
    time: SystemTime,
    games: Vec<TimedOpponentInternalGame>,
}

impl TimedInternalGames {
    #[must_use]
    pub fn new(time: SystemTime, games: Vec<TimedOpponentInternalGame>) -> Self {
        TimedInternalGames { time, games }
    }

    #[must_use]
    pub fn single(game: TimedInternalGame) -> Self {
        TimedInternalGames {
            time: game.time(),
            games: vec![TimedOpponentInternalGame {
                opponent: game.opponent(),
                score: game.score(),
            }],
        }
    }

    #[must_use]
    pub fn time(&self) -> SystemTime {
        self.time
    }

    #[must_use]
    pub fn games(&self) -> &[TimedOpponentInternalGame] {
        &self.games
    }

    pub fn timed_games(&self) -> impl Iterator<Item = TimedInternalGame> + '_ {
        self.games
            .iter()
            .map(|game| game.timed_internal_game_at(self.time()))
    }
}

impl From<TimedInternalGame> for TimedInternalGames {
    fn from(game: TimedInternalGame) -> Self {
        TimedInternalGames::single(game)
    }
}

impl FromWithParameters<TimedPublicGames> for TimedInternalGames {
    fn from_with_parameters(public: TimedPublicGames, parameters: Parameters) -> Self {
        let internal_games = public
            .games()
            .iter()
            .map(|&game| game.into_with_parameters(parameters))
            .collect();

        TimedInternalGames {
            time: public.time(),
            games: internal_games,
        }
    }
}

/// Calculates the new internal player rating after a `TimedInternalGame` using the Glicko-2 algorithm.
///
/// # Panics
///
/// This function panics if the `player_rating` was updated after the game was played, or if the `rating_period_duration` is zero.
///
/// It can also panic if `parameters.convergance_tolerance()` is unreasonably low.
#[must_use]
pub fn rate_game(
    player_rating: TimedInternalRating,
    game: TimedInternalGame,
    rating_period_duration: Duration,
    parameters: Parameters,
) -> TimedInternalRating {
    rate_games(
        player_rating,
        &TimedInternalGames::single(game),
        rating_period_duration,
        parameters,
    )
}

#[must_use]
pub fn rate_games(
    player_rating: TimedInternalRating,
    games: &TimedInternalGames,
    rating_period_duration: Duration,
    parameters: Parameters,
) -> TimedInternalRating {
    // Step 1. (initialising) doesn't apply, we have already set the starting ratings.
    // Step 2. (converting to internal scale) doesn't apply either, we get typed checked internal rating here

    // If `games` is empty, only Step 6. applies, which TimedInternalRating does automatically
    if games.games().is_empty() {
        return player_rating;
    }

    let game_time = games.time();

    // How many rating periods have elapsed
    let elapsed_periods = game_time
        .duration_since(player_rating.last_updated())
        .expect("Game was played before last player update")
        .as_secs_f64()
        / rating_period_duration.as_secs_f64();

    // Find rating at the time the game was played
    let player_rating = player_rating.internal_rating_at(game_time, rating_period_duration);

    let internal_games: Vec<_> = games
        .timed_games()
        .map(|game| game.to_internal_game(rating_period_duration))
        .collect();

    // Step 3.
    let estimated_variance =
        calculate_estimated_variance(player_rating, internal_games.iter().copied());

    // Step 4.
    let estimated_improvement = calculate_estimated_improvement(
        estimated_variance,
        player_rating,
        internal_games.iter().copied(),
    );

    // Step 5.
    let new_volatility = calculate_new_volatility(
        estimated_improvement,
        estimated_variance,
        player_rating,
        parameters.volatility_change(),
        parameters.convergence_tolerance(),
    );

    // Step 6.
    let pre_rating_period_value =
        calculate_pre_rating_period_value(new_volatility, player_rating, elapsed_periods);

    // Step 7.
    let new_deviation = calculate_new_rating_deviation(pre_rating_period_value, estimated_variance);

    let new_rating = calculate_new_rating(new_deviation, player_rating, internal_games);

    // Step 8. (converting to display scale) doesn't apply
    TimedInternalRating {
        last_updated: game_time,
        rating: InternalRating {
            rating: new_rating,
            deviation: new_deviation,
            volatility: new_volatility,
        },
    }
}

/// Step 3.
///
/// This function's return value and panic behaviuor is unspecified if the results iterator is empty.
/// It will terminate.
///
/// # Panics
///
/// This function might panic if the results iterator is empty.
#[must_use]
fn calculate_estimated_variance(
    player_rating: InternalRating,
    games: impl IntoIterator<Item = InternalGame>,
) -> f64 {
    1.0 / games
        .into_iter()
        .map(|game| {
            let opponent_rating = game.opponent();

            let g = calculate_g(opponent_rating.deviation());
            let e = calculate_e(g, player_rating.rating(), opponent_rating.rating());

            g * g * e * (1.0 - e)
        })
        .sum::<f64>()
}

/// Step 4.
#[must_use]
fn calculate_estimated_improvement(
    estimated_variance: f64,
    player_rating: InternalRating,
    games: impl IntoIterator<Item = InternalGame>,
) -> f64 {
    estimated_variance
        * games
            .into_iter()
            .map(|game| {
                let opponent_rating = game.opponent();

                let g = calculate_g(opponent_rating.deviation());
                let e = calculate_e(g, player_rating.rating(), opponent_rating.rating());

                g * (game.score - e)
            })
            .sum::<f64>()
}

// TODO: cached?
// Optimizer is prolly smart enough to notice we call it with the same value thrice
// Even if not, like, ... this is likely not a bottleneck
#[must_use]
fn calculate_g(deviation: f64) -> f64 {
    1.0 / f64::sqrt(1.0 + 3.0 * deviation * deviation / (PI * PI))
}

#[must_use]
fn calculate_e(g: f64, player_rating: f64, opponent_rating: f64) -> f64 {
    1.0 / (1.0 + f64::exp(-g * (player_rating - opponent_rating)))
}

/// Step 5.
///
/// # Panics
///
/// This function might panic if `parameters.convergence_tolerance()` is unreasonably low.
#[must_use]
fn calculate_new_volatility(
    estimated_improvement: f64,
    estimated_variance: f64,
    player_rating: InternalRating,
    volatility_change: f64,
    convergence_tolerance: f64,
) -> f64 {
    let deviation = player_rating.deviation();
    let deviation_sq = deviation * deviation;
    let current_volatility = player_rating.volatility();

    let estimated_improvement_sq = estimated_improvement * estimated_improvement;

    // 1.
    let a = f64::ln(current_volatility * current_volatility);

    let f = |x| {
        let x_exp = f64::exp(x);

        let tmp_1 = x_exp * (estimated_improvement_sq - deviation_sq - estimated_variance - x_exp);

        let tmp_2 = 2.0 * {
            let tmp = deviation_sq + estimated_variance + x_exp;
            tmp * tmp
        };

        let tmp_3 = x - a;

        let tmp_4 = volatility_change * volatility_change;

        tmp_1 / tmp_2 - tmp_3 / tmp_4
    };

    // 2.
    // Copy so the mutated value doesn't get captured by f
    let mut a = a;

    let mut b = if estimated_improvement_sq > deviation_sq + estimated_variance {
        f64::ln(estimated_improvement_sq - deviation_sq - estimated_variance)
    } else {
        // (i)
        let mut k = 1.0;

        loop {
            // (ii)
            let estimated_b = a - k * volatility_change;

            if f(estimated_b) < 0.0 {
                k += 1.0;
            } else {
                break estimated_b;
            }
        }
    };

    // 3.
    let mut f_a = f(a);
    let mut f_b = f(b);

    // 4.
    let mut iteration = 0;
    while f64::abs(b - a) > convergence_tolerance {
        assert!(
            iteration <= constants::MAX_ITERATIONS,
            "Maximum number of iterations ({}) in converging loop algorithm exceeded. Is the convergence tolerance ({}) unreasonably low?",
            constants::MAX_ITERATIONS, convergence_tolerance
        );

        // (a)
        let c = a + (a - b) * f_a / (f_b - f_a);
        let f_c = f(c);

        // (b)
        if f_c * f_b <= 0.0 {
            a = b;
            f_a = f_b;
        } else {
            f_a /= 2.0;
        }

        // (c)
        b = c;
        f_b = f_c;

        iteration += 1;
        // (d) checked by loop
    }

    // 5.
    f64::exp(a / 2.0)
}

/// Step 6.
#[must_use]
fn calculate_pre_rating_period_value(
    new_volatility: f64,
    player_rating: InternalRating,
    elapsed_periods: f64,
) -> f64 {
    let current_deviation = player_rating.deviation();

    // See Lichess' implementation: https://github.com/lichess-org/lila/blob/d6a175d25228b0f3d9053a30301fce90850ceb2d/modules/rating/src/main/java/glicko2/RatingCalculator.java#L316
    f64::sqrt(
        current_deviation * current_deviation + elapsed_periods * new_volatility * new_volatility,
    )
}

/// Step 7.1.
#[must_use]
fn calculate_new_rating_deviation(pre_rating_period_value: f64, estimated_variance: f64) -> f64 {
    1.0 / f64::sqrt(
        1.0 / (pre_rating_period_value * pre_rating_period_value) + 1.0 / estimated_variance,
    )
}

/// Step 7.2
#[must_use]
fn calculate_new_rating(
    new_deviation: f64,
    player_rating: InternalRating,
    games: impl IntoIterator<Item = InternalGame>,
) -> f64 {
    player_rating.rating()
        + new_deviation
            * new_deviation
            * games
                .into_iter()
                .map(|game| {
                    let opponent_rating = game.opponent();

                    let g = calculate_g(opponent_rating.deviation());
                    let e = calculate_e(g, player_rating.rating(), opponent_rating.rating());

                    g * (game.score() - e)
                })
                .sum::<f64>()
}

#[cfg(test)]
mod test {
    use std::time::{Duration, SystemTime};

    use crate::algorithm2::{
        rate_games, TimedOpponentPublicGame, TimedPublicGames, TimedPublicRating,
    };
    use crate::{FromWithParameters, IntoWithParameters, Parameters, PublicRating};

    macro_rules! assert_approx_eq {
        ($a:expr, $b:expr, $tolerance:expr) => {{
            let a_val = $a;
            let b_val = $b;

            assert!(
                (a_val - b_val).abs() <= $tolerance,
                "{} = {a_val} is not approximately equal to {} = {b_val}",
                stringify!($a),
                stringify!($b)
            )
        }};
    }

    #[test]
    fn test_start_time() {
        let parameters = Parameters::default().with_volatility_change(0.5);

        let start_time = SystemTime::UNIX_EPOCH;
        let rating_period_duration = Duration::from_secs(1);

        let player = TimedPublicRating::new(start_time, PublicRating::new(1500.0, 200.0, 0.06));
        let rating_at_start =
            player.public_rating_at(SystemTime::UNIX_EPOCH, parameters, rating_period_duration);

        assert_approx_eq!(rating_at_start.rating(), 1500.0, f64::EPSILON);
        assert_approx_eq!(rating_at_start.deviation(), 200.0, f64::EPSILON);
        assert_approx_eq!(rating_at_start.volatility(), 0.06, f64::EPSILON);
    }

    /// This tests the example calculation in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    #[test]
    fn test_paper_example() {
        let parameters = Parameters::default().with_volatility_change(0.5);

        let start_time = SystemTime::UNIX_EPOCH;
        let rating_period_duration = Duration::from_secs(1);
        let end_time = start_time + rating_period_duration;

        let player = TimedPublicRating::new(start_time, PublicRating::new(1500.0, 200.0, 0.06));

        // Volatility on opponents is not specified in the paper and doesn't matter in the calculation.
        // Constructor asserts it to be > 0.0
        let opponent_a = TimedPublicRating::new(
            start_time,
            PublicRating::new(1400.0, 30.0, parameters.start_rating().volatility()),
        );
        let opponent_b = TimedPublicRating::new(
            start_time,
            PublicRating::new(1550.0, 100.0, parameters.start_rating().volatility()),
        );
        let opponent_c = TimedPublicRating::new(
            start_time,
            PublicRating::new(1700.0, 300.0, parameters.start_rating().volatility()),
        );

        let games = vec![
            TimedOpponentPublicGame::new(opponent_a, 1.0),
            TimedOpponentPublicGame::new(opponent_b, 0.0),
            TimedOpponentPublicGame::new(opponent_c, 0.0),
        ];

        let games = TimedPublicGames::new(start_time, games);

        let new_rating = rate_games(
            player.into_with_parameters(parameters),
            &games.into_with_parameters(parameters),
            rating_period_duration,
            parameters,
        );

        // All games are considered to occur at the same time in the example
        let new_public_rating = TimedPublicRating::from_with_parameters(new_rating, parameters)
            .public_rating_at(end_time, parameters, rating_period_duration);

        assert_approx_eq!(new_public_rating.rating(), 1464.06, 0.01);
        assert_approx_eq!(new_public_rating.deviation(), 151.52, 0.01);
        assert_approx_eq!(new_public_rating.volatility(), 0.05999, 0.0001);
    }
}
