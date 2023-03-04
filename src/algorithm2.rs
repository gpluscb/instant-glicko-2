use std::f64::consts::PI;
use std::iter;
use std::time::{Duration, SystemTime};

use crate::{constants, FromWithParameters, IntoWithParameters, Parameters, Rating, ScaledRating};

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
    rating: Rating,
}

impl TimedPublicRating {
    /// Creates a new [`TimedPublicRating`] at the given `last_updated` time with the given `rating`.
    #[must_use]
    pub fn new(last_updated: SystemTime, rating: Rating) -> Self {
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
    pub fn raw_public_rating(&self) -> Rating {
        self.rating
    }

    /// The rating with the deviation updated to the current time after no games were played since the last update.
    /// Convenience for `self.public_rating_at(SystemTime::now(), parameters, rating_period_duration)`.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is in the future.
    #[must_use]
    pub fn public_rating_now(
        &self,
        parameters: Parameters,
        rating_period_duration: Duration,
    ) -> Rating {
        self.public_rating_at(SystemTime::now(), parameters, rating_period_duration)
    }

    /// The rating with the deviation updated to the given time after no games were played since the last update.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is after `time`.
    #[must_use]
    pub fn public_rating_at(
        &self,
        time: SystemTime,
        parameters: Parameters,
        rating_period_duration: Duration,
    ) -> Rating {
        let new_deviation = calculate_pre_rating_period_value(
            self.rating.volatility(),
            self.rating.into_with_parameters(parameters),
            self.elapsed_rating_periods(time, rating_period_duration),
        );

        Rating {
            deviation: new_deviation,
            ..self.rating
        }
    }

    /// # Panics
    ///
    /// This function panics if `time` is **before** the last rating update.
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
    rating: ScaledRating,
}

impl TimedInternalRating {
    /// Creates a new [`TimedInternalRating`] at the given `last_updated` time with the given `rating`.
    #[must_use]
    pub fn new(last_updated: SystemTime, rating: ScaledRating) -> Self {
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
    pub fn raw_internal_rating(&self) -> ScaledRating {
        self.rating
    }

    /// The rating with the deviation updated to the current time after no games were played since the last update.
    /// Convenience for `self.public_rating_at(SystemTime::now(), parameters, rating_period_duration)`.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is in the future.
    #[must_use]
    pub fn internal_rating_now(&self, rating_period_duration: Duration) -> ScaledRating {
        self.internal_rating_at(SystemTime::now(), rating_period_duration)
    }

    /// The rating with the deviation updated to the given time after no games were played since the last update.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is after `time`.
    #[must_use]
    pub fn internal_rating_at(
        &self,
        time: SystemTime,
        rating_period_duration: Duration,
    ) -> ScaledRating {
        let new_deviation = calculate_pre_rating_period_value(
            self.rating.volatility(),
            self.rating,
            self.elapsed_rating_periods(time, rating_period_duration),
        );

        ScaledRating {
            deviation: new_deviation,
            ..self.rating
        }
    }

    /// # Panics
    ///
    /// This function panics if `time` is **before** the last rating update.
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
    /// This function panics if the opponent rating was updated after the game was recorded.
    #[must_use]
    pub fn to_public_game(
        &self,
        parameters: Parameters,
        rating_period_duration: Duration,
    ) -> PublicGame {
        let opponent_rating =
            self.opponent()
                .public_rating_at(self.time, parameters, rating_period_duration);

        PublicGame {
            opponent_rating,
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
    /// This function panics if the opponent rating was updated after the game was recorded.
    #[must_use]
    pub fn to_internal_game(&self, rating_period_duration: Duration) -> InternalGame {
        let opponent_rating = self
            .opponent()
            .internal_rating_at(self.time, rating_period_duration);

        InternalGame {
            opponent_rating,
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
pub struct PublicGame {
    opponent_rating: Rating,
    score: f64,
}

impl PublicGame {
    #[must_use]
    pub fn new(opponent_rating: Rating, score: f64) -> Self {
        Self {
            opponent_rating,
            score,
        }
    }

    #[must_use]
    pub fn opponent_rating(&self) -> Rating {
        self.opponent_rating
    }

    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

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
            opponent_rating: internal.opponent_rating.into_with_parameters(parameters),
            score: internal.score,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InternalGame {
    opponent_rating: ScaledRating,
    score: f64,
}

impl InternalGame {
    #[must_use]
    pub fn new(opponent_rating: ScaledRating, score: f64) -> Self {
        Self {
            opponent_rating,
            score,
        }
    }

    #[must_use]
    pub fn opponent_rating(&self) -> ScaledRating {
        self.opponent_rating
    }

    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

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
            opponent_rating: public.opponent_rating.into_with_parameters(parameters),
            score: public.score,
        }
    }
}

#[must_use]
pub fn rate_game(
    player_rating: TimedInternalRating,
    game: TimedInternalGame,
    rating_period_duration: Duration,
    parameters: Parameters,
) -> TimedInternalRating {
    assert!(
        player_rating.last_updated() < game.time(),
        "Game was played before last player update"
    );

    // Step 1. (initialising) doesn't apply, we have already set the starting ratings.
    // Step 2. (converting to internal scale) doesn't apply either, we get typed checked internal rating here

    let game_time = game.time();

    let internal_game = game.to_internal_game(rating_period_duration);

    // Find rating at the time the game was played
    let rating = player_rating.internal_rating_at(game_time, rating_period_duration);

    // How many rating periods have elapsed
    let elapsed_periods = game_time
        .duration_since(player_rating.last_updated())
        .expect("Player rating was updated after the game to rate")
        .as_secs_f64()
        / rating_period_duration.as_secs_f64();

    // Step 3.
    let estimated_variance = calculate_estimated_variance(rating, iter::once(internal_game));

    // Step 4.
    let estimated_improvement =
        calculate_estimated_improvement(estimated_variance, rating, iter::once(internal_game));

    // Step 5.
    let new_volatility = calculate_new_volatility(
        estimated_improvement,
        estimated_variance,
        rating,
        parameters,
    );

    // Step 6.
    let pre_rating_period_value =
        calculate_pre_rating_period_value(new_volatility, rating, elapsed_periods);

    // Step 7.
    let new_deviation = calculate_new_rating_deviation(pre_rating_period_value, estimated_variance);

    let new_rating = calculate_new_rating(new_deviation, rating, iter::once(internal_game));

    // Step 8. (converting to display scale) doesn't apply
    TimedInternalRating {
        last_updated: game_time,
        rating: ScaledRating {
            rating: new_rating,
            deviation: new_deviation,
            volatility: new_volatility,
        },
    }
}

/// Step 3.
///
/// This function returns [`f64::NAN`] if the results iterator is empty.
#[must_use]
fn calculate_estimated_variance(
    player_rating: ScaledRating,
    games: impl IntoIterator<Item = InternalGame>,
) -> f64 {
    1.0 / games
        .into_iter()
        .map(|game| {
            let opponent_rating = game.opponent_rating();

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
    player_rating: ScaledRating,
    games: impl IntoIterator<Item = InternalGame>,
) -> f64 {
    estimated_variance
        * games
            .into_iter()
            .map(|game| {
                let opponent_rating = game.opponent_rating();

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
    player_rating: ScaledRating,
    parameters: Parameters,
) -> f64 {
    let deviation = player_rating.deviation();
    let deviation_sq = deviation * deviation;
    let current_volatility = player_rating.volatility();

    let estimated_improvement_sq = estimated_improvement * estimated_improvement;

    let volatility_change = parameters.volatility_change();

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
    while f64::abs(b - a) > parameters.convergence_tolerance() {
        assert!(
            iteration <= constants::MAX_ITERATIONS,
            "Maximum number of iterations ({}) in converging loop algorithm exceeded. Is the convergence tolerance ({}) unreasonably low?",
            constants::MAX_ITERATIONS, parameters.convergence_tolerance()
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
    player_rating: ScaledRating,
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
    player_rating: ScaledRating,
    games: impl IntoIterator<Item = InternalGame>,
) -> f64 {
    player_rating.rating()
        + new_deviation
            * new_deviation
            * games
                .into_iter()
                .map(|game| {
                    let opponent_rating = game.opponent_rating();

                    let g = calculate_g(opponent_rating.deviation());
                    let e = calculate_e(g, player_rating.rating(), opponent_rating.rating());

                    g * (game.score() - e)
                })
                .sum::<f64>()
}
