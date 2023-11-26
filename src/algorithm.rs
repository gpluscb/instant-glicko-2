//! This module hosts the methods and types necessary to perform Glicko-2 calculations with fractional rating periods.

use std::cmp::Ordering;
use std::f64::consts::PI;
use std::time::{Duration, SystemTime};

use crate::{
    constants, ConvertToScale, FromWithSettings, GlickoSettings, Internal, InternalRating,
    IntoWithSettings, Public, Rating, RatingScale,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A rating at a specific point in time.
///
/// The timing of the rating is important because the deviation increases over the time no games are recorded.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "", serialize = "")))]
pub struct TimedRating<Scale: RatingScale> {
    last_updated: SystemTime,
    rating: Rating<Scale>,
}

/// A [`TimedRating`] of [`Public`] scale.
pub type PublicTimedRating = TimedRating<Public>;

/// A [`TimedRating`] of [`Internal`] scale.
pub type InternalTimedRating = TimedRating<Internal>;

impl<Scale: RatingScale> PartialOrd for TimedRating<Scale> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.rating.partial_cmp(&other.rating)
    }
}

impl<Scale: RatingScale> TimedRating<Scale> {
    /// Creates a new [`TimedRating`] at the given `last_updated` time with the given `rating`.
    #[must_use]
    pub fn new(last_updated: SystemTime, rating: Rating<Scale>) -> Self {
        TimedRating {
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
    pub fn raw_rating(&self) -> Rating<Scale> {
        self.rating
    }

    /// The rating with the deviation updated to the current time after no games were played since the last update.
    /// Convenience for `self.rating_at(SystemTime::now(), settings)`.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is in the future.
    #[must_use]
    pub fn rating_now(&self, settings: GlickoSettings) -> Rating<Scale>
    where
        Scale: ConvertToScale<Internal>,
        Internal: ConvertToScale<Scale>,
    {
        self.rating_at(SystemTime::now(), settings)
    }

    /// The rating with the deviation updated to the given time after no games were played since the last update.
    ///
    /// # Panics
    ///
    /// This function panics if `last_updated` is after `time`.
    #[must_use]
    pub fn rating_at(&self, time: SystemTime, settings: GlickoSettings) -> Rating<Scale>
    where
        Scale: ConvertToScale<Internal>,
        Internal: ConvertToScale<Scale>,
    {
        let internal_rating: InternalRating = self.rating.into_with_settings(settings);

        let new_deviation = calculate_pre_rating_period_value(
            internal_rating.volatility(),
            internal_rating,
            self.elapsed_rating_periods(time, settings.rating_period_duration),
        );

        InternalRating {
            deviation: new_deviation,
            ..internal_rating
        }
        .into_with_settings(settings)
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

impl<Scale1: RatingScale, Scale2: RatingScale> FromWithSettings<TimedRating<Scale1>>
    for TimedRating<Scale2>
where
    Scale1: ConvertToScale<Scale2>,
{
    fn from_with_settings(rating: TimedRating<Scale1>, settings: GlickoSettings) -> Self {
        TimedRating::new(
            rating.last_updated,
            rating.rating.into_with_settings(settings),
        )
    }
}

/// Game information encompassing the opponent's rating at the time of the game
/// as well as the game score as a number between `0.0` (decisive opponent win) and `1.0` (decisive player win).
///
/// Keep in mind that this struct does not hold information about the player's rating, only the opponent's.
/// This is because it is used in relation to registering games on and therefore update the player's rating struct.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "", serialize = "")))]
pub struct Game<Scale: RatingScale> {
    opponent: Rating<Scale>,
    score: f64,
}

/// A [`Game`] of [`Public`] scale.
pub type PublicGame = Game<Public>;
/// A [`Game`] of [`Internal`] scale.
pub type InternalGame = Game<Internal>;

impl<Scale: RatingScale> Game<Scale> {
    /// Creates a new [`PublicGame`] with the given `opponent` and `score`.
    /// `score` is a number between 0.0 (decisive opponent win) and `1.0` (decisive player win).
    ///
    /// # Panics
    ///
    /// This function panics if `score` is less than `0.0` or greater than `1.0`.
    #[must_use]
    pub fn new(opponent: Rating<Scale>, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        Game { opponent, score }
    }

    /// The opponent's rating.
    #[must_use]
    pub fn opponent(&self) -> Rating<Scale> {
        self.opponent
    }

    /// The game score as a number between `0.0` (decisive opponent win) and `1.0` (decisive player win).
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }
}

impl<Scale1: RatingScale, Scale2: RatingScale> FromWithSettings<Game<Scale1>> for Game<Scale2>
where
    Scale1: ConvertToScale<Scale2>,
{
    fn from_with_settings(game: Game<Scale1>, settings: GlickoSettings) -> Self {
        Game::new(game.opponent.into_with_settings(settings), game.score)
    }
}

/// Game information encompassing
/// - The time the game was recorded
/// - The [`TimedRating`] of the opponent
/// - The score as a number between `0.0` (decisive opponent win) and `1.0` (decisive player win)
///
/// Keep in mind that this struct does not hold information about the player's rating, only the opponent's.
/// This is because it is used to register games on and therefore update the player's rating struct.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "", serialize = "")))]
pub struct TimedGame<Scale: RatingScale> {
    time: SystemTime,
    opponent: TimedRating<Scale>,
    score: f64,
}

/// A [`TimedGame`] of [`Public`] scale.
pub type PublicTimedGame = TimedGame<Public>;

/// A [`TimedGame`] of [`Internal`] scale.
pub type InternalTimedGame = TimedGame<Internal>;

impl<Scale: RatingScale> TimedGame<Scale> {
    /// Creates a new [`TimedGame`] at the given `time` with the given `opponent` and `score`.
    /// `score` is a number between 0.0 (decisive opponent win) and `1.0` (decisive player win).
    ///
    /// # Panics
    ///
    /// This function panics if `score` is less than `0.0` or greater than `1.0`.
    #[must_use]
    pub fn new(time: SystemTime, opponent: TimedRating<Scale>, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        TimedGame {
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
    pub fn opponent(&self) -> TimedRating<Scale> {
        self.opponent
    }

    /// The game score as a number between `0.0` (decisive opponent win) and `1.0` (decisive player win).
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

    /// The game with timing information erased
    /// and the opponent's rating resolved to their rating at the time of the last update.
    #[must_use]
    pub fn raw_game(&self) -> Game<Scale> {
        Game::new(self.opponent().raw_rating(), self.score())
    }

    /// Converts this [`TimedGame`] to a [`Game`],
    /// erasing the timing information and resolving the opponent's rating to their rating at the time of the game.
    ///
    /// # Panics
    ///
    /// This function panics if the opponent rating was updated after the game was recorded.
    #[must_use]
    pub fn to_game(&self, settings: GlickoSettings) -> Game<Scale>
    where
        Scale: ConvertToScale<Internal>,
        Internal: ConvertToScale<Scale>,
    {
        self.game_at(self.time(), settings)
    }

    /// Converts this [`TimedGame`] to a [`Game`],
    /// erasing the timing information and resolving the opponent's rating to their rating at the given `time`.
    ///
    /// # Panics
    ///
    /// This function panics if the given `time` is before the opponent rating's last update.
    #[must_use]
    pub fn game_at(&self, time: SystemTime, settings: GlickoSettings) -> Game<Scale>
    where
        Scale: ConvertToScale<Internal>,
        Internal: ConvertToScale<Scale>,
    {
        let opponent = self.opponent().rating_at(time, settings);

        Game::new(opponent, self.score())
    }
}

impl<Scale1: RatingScale, Scale2: RatingScale> FromWithSettings<TimedGame<Scale1>>
    for TimedGame<Scale2>
where
    Scale1: ConvertToScale<Scale2>,
{
    fn from_with_settings(game: TimedGame<Scale1>, settings: GlickoSettings) -> Self {
        TimedGame::new(
            game.time,
            game.opponent.into_with_settings(settings),
            game.score,
        )
    }
}

/// A match result where the opponent's rating is a [`TimedRating`].
/// The game itself is not timed.
/// The struct holds only the opponent's rating and the game score from the player's perspective.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "", serialize = "")))]
pub struct TimedOpponentGame<Scale: RatingScale> {
    opponent: TimedRating<Scale>,
    score: f64,
}

/// A [`TimedOpponentGame`] of [`Public`] scale.
pub type PublicTimedOpponentGame = TimedOpponentGame<Public>;

/// A [`TimedOpponentGame`] of [`Internal`] scale.
pub type InternalTimedOpponentGame = TimedOpponentGame<Internal>;

impl<Scale: RatingScale> TimedOpponentGame<Scale> {
    /// Creates a new [`TimedOpponentGame`] with the given `opponent` and the player's `score`.
    /// `score` is a number between 0.0 (decisive opponent win) and `1.0` (decisive player win).
    ///
    /// # Panics
    ///
    /// This function panics if `score` is less than `0.0` or greater than `1.0`.
    #[must_use]
    pub fn new(opponent: TimedRating<Scale>, score: f64) -> Self {
        assert!((0.0..=1.0).contains(&score));

        TimedOpponentGame { opponent, score }
    }

    /// The opponent's rating.
    #[must_use]
    pub fn opponent(&self) -> TimedRating<Scale> {
        self.opponent
    }

    /// The game score as a number between `0.0` (decisive opponent win) and `1.0` (decisive player win).
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Returns a [`TimedGame`] which represents the given game happening at the given `time`.
    #[must_use]
    pub fn timed_game_at(&self, time: SystemTime) -> TimedGame<Scale> {
        TimedGame::new(time, self.opponent, self.score)
    }

    /// Returns a [`Game`], resolving the opponent's rating to their rating at the given time.
    ///
    /// # Panics
    ///
    /// This function panics if the given `time` is before the opponent rating's last update.
    #[must_use]
    pub fn game_at(&self, time: SystemTime, settings: GlickoSettings) -> Game<Scale>
    where
        Scale: ConvertToScale<Internal>,
        Internal: ConvertToScale<Scale>,
    {
        let opponent = self.opponent.rating_at(time, settings);

        Game::new(opponent, self.score)
    }
}

impl<Scale1: RatingScale, Scale2: RatingScale> FromWithSettings<TimedOpponentGame<Scale1>>
    for TimedOpponentGame<Scale2>
where
    Scale1: ConvertToScale<Scale2>,
{
    fn from_with_settings(game: TimedOpponentGame<Scale1>, settings: GlickoSettings) -> Self {
        TimedOpponentGame::new(game.opponent.into_with_settings(settings), game.score)
    }
}

/// A collection of [`TimedOpponentGame`]s where the games are
/// all considered to be played at the same, known time.
/// This struct does not hold information about the player, only about the opponents.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "", serialize = "")))]
pub struct TimedGames<Scale: RatingScale> {
    time: SystemTime,
    games: Vec<TimedOpponentGame<Scale>>,
}

/// A [`TimedGames`] of [`Public`] scale.
pub type PublicTimedGames = TimedGames<Public>;
/// A [`TimedGames`] of [`Internal`] scale.
pub type InternalTimedGames = TimedGames<Internal>;

impl<Scale: RatingScale> TimedGames<Scale> {
    /// Creates a new [`TimedGames`] instance
    /// where all `games` are considered to be played at the given `time`.
    #[must_use]
    pub fn new(time: SystemTime, games: Vec<TimedOpponentGame<Scale>>) -> Self {
        TimedGames { time, games }
    }

    /// Creates a new [`TimedGames`] instance from a single `game`,
    /// which is considered to be played at the given `time`.
    #[must_use]
    pub fn single(game: TimedGame<Scale>) -> Self {
        TimedGames::new(
            game.time(),
            vec![TimedOpponentGame::new(game.opponent(), game.score())],
        )
    }

    /// The time the games are considered to be played at.
    #[must_use]
    pub fn time(&self) -> SystemTime {
        self.time
    }

    /// The games without information about when the games were played.
    #[must_use]
    pub fn games(&self) -> &[TimedOpponentGame<Scale>] {
        &self.games
    }

    /// Iterator over the games with information about when they were played.
    pub fn timed_games(&self) -> impl Iterator<Item = TimedGame<Scale>> + '_ {
        self.games
            .iter()
            .map(|game| game.timed_game_at(self.time()))
    }
}

impl<Scale: RatingScale> From<TimedGame<Scale>> for TimedGames<Scale> {
    fn from(game: TimedGame<Scale>) -> Self {
        TimedGames::single(game)
    }
}

impl<Scale1: RatingScale, Scale2: RatingScale> FromWithSettings<TimedGames<Scale1>>
    for TimedGames<Scale2>
where
    Scale1: ConvertToScale<Scale2>,
{
    fn from_with_settings(games: TimedGames<Scale1>, settings: GlickoSettings) -> Self {
        let public_games = games
            .games()
            .iter()
            .map(|&game| game.into_with_settings(settings))
            .collect();

        TimedGames::new(games.time(), public_games)
    }
}

/// Calculates the new internal player rating after a `TimedInternalGame` using the Glicko-2 algorithm.
///
/// However, this function only provides an *approximation* for the actual new rating
/// because it considers opponent ratings at the time of the game instead of at the time the player's rating was last updated.
/// The errors this introduces are relatively small, and can be considered worth it for avoiding the bookkeeping
/// that would come with tracking an older opponent rating for every possible player rating.
// TODO: Can we work backwards to that rating to get a better approximation? That would be pretty cool. Glicko-2 compliant time travel. Imagine that.
/// For a version more accurate to Glicko-2, see [`rate_games_untimed`].
///
/// # Panics
///
/// This function panics if the `player_rating` or any opponent ratings were updated after the game was played.
///
/// It can also panic if `settings.convergence_tolerance()` is unreasonably low.
#[must_use]
pub fn rate_game(
    player_rating: InternalTimedRating,
    game: InternalTimedGame,
    settings: GlickoSettings,
) -> InternalTimedRating {
    rate_games(player_rating, &InternalTimedGames::single(game), settings)
}

/// Calculates the new internal player rating after the given [`InternalTimedGames`] using the Glicko-2 algorithm.
/// This is useful since Glicko-2 assumes all games within a rating period were played at the same point in time.
///
/// However, this function only provides an *approximation* for the actual new rating
/// because it considers opponent ratings at the time of the game instead of at the time the player's rating was last updated.
/// The errors this introduces are relatively small, and can be considered worth it for avoiding the bookkeeping
/// that would come with tracking an older opponent rating for every possible player rating.
// TODO: Can we work backwards to that rating to get a better approximation? That would be pretty cool. Glicko-2 compliant time travel. Imagine that.
/// For a version more accurate to Glicko-2, see [`rate_games_untimed`].
///
/// # Panics
///
/// This function panics if the `player_rating` or any opponent ratings were updated after the games were played.
///
/// It can also panic if `settings.convergence_tolerance()` is unreasonably low.
#[must_use]
pub fn rate_games(
    player_rating: InternalTimedRating,
    games: &InternalTimedGames,
    settings: GlickoSettings,
) -> InternalTimedRating {
    // Step 1. (initialising) doesn't apply, we have already set the starting ratings.
    // Step 2. (converting to internal scale) doesn't apply either, we get typed checked internal rating here

    let game_time = games.time();

    // If `games` is empty, only Step 6. applies, which TimedInternalRating does automatically
    if games.games().is_empty() {
        return player_rating;
    }

    // Raw rating because pre_rating_period_value will handle that
    let player_rating = player_rating.rating_at(game_time, settings);

    let internal_games: Vec<_> = games
        .timed_games()
        // Technically we should get internal game at time player_last_updated,
        // but that would make all opponents being last updated before that a requirement,
        // which is unreasonable. Errors because of this tend to be small.
        .map(|game| game.to_game(settings))
        .collect();

    // elapsed_periods is 0.0 since we set last_updated to game_time (no time elapsed since game)
    let new_rating = rate_games_untimed(player_rating, &internal_games, 0.0, settings);

    TimedRating::new(
        game_time,
        InternalRating::new(
            new_rating.rating(),
            new_rating.deviation(),
            new_rating.volatility(),
        ),
    )
}

/// Calculates the new internal player rating after the given [`InternalGame`]s were played
/// and the given amount of rating periods `elapsed_periods` were elapsed using the Glicko-2 algorithm.
///
/// # Panics
///
/// This function panics if `elapsed_periods` is less than `0`.
///
/// It can also panic if `settings.convergence_tolerance()` is unreasonably low.
#[must_use]
pub fn rate_games_untimed(
    player_rating: InternalRating,
    results: &[InternalGame],
    elapsed_periods: f64,
    settings: GlickoSettings,
) -> InternalRating {
    assert!(elapsed_periods >= 0.0);

    // Step 1. (initialising) doesn't apply, we have already set the starting ratings.
    // Step 2. (converting to internal scale) doesn't apply either, we get typed checked internal rating here

    if results.iter().next().is_none() {
        // If `results` is empty, only Step 6. applies
        let new_deviation = calculate_pre_rating_period_value(
            player_rating.volatility(),
            player_rating,
            elapsed_periods,
        );

        return InternalRating::new(
            player_rating.rating(),
            new_deviation,
            player_rating.volatility(),
        )
        .into_with_settings(settings);
    }

    // Step 3.
    let estimated_variance = calculate_estimated_variance(player_rating, results.iter().copied());

    // Step 4.
    let performance_sum = calculate_performance_sum(player_rating, results.iter().copied());
    let estimated_improvement =
        calculate_estimated_improvement(estimated_variance, performance_sum);

    // Step 5.
    let new_volatility = calculate_new_volatility(
        estimated_improvement,
        estimated_variance,
        player_rating,
        settings.volatility_change(),
        settings.convergence_tolerance(),
    );

    // Step 6.
    let pre_rating_period_value =
        calculate_pre_rating_period_value(new_volatility, player_rating, elapsed_periods);

    // Step 7.
    let new_deviation = calculate_new_rating_deviation(pre_rating_period_value, estimated_variance);
    let new_rating = calculate_new_rating(new_deviation, player_rating, performance_sum);

    // Step 8. (converting back to public) doesn't apply
    InternalRating::new(new_rating, new_deviation, new_volatility)
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

/// Calculates sum value for Steps 4. and 7.2.
fn calculate_performance_sum(
    player_rating: InternalRating,
    games: impl IntoIterator<Item = InternalGame>,
) -> f64 {
    games
        .into_iter()
        .map(|game| {
            let opponent_rating = game.opponent();

            let g = calculate_g(opponent_rating.deviation());
            let e = calculate_e(g, player_rating.rating(), opponent_rating.rating());

            g * (game.score() - e)
        })
        .sum::<f64>()
}

/// Step 4.
#[must_use]
fn calculate_estimated_improvement(estimated_variance: f64, performance_sum: f64) -> f64 {
    estimated_variance * performance_sum
}

// TODO: cached?
// Optimizer is prolly smart enough to notice we call it with the same value twice
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
/// This function might panic if `convergence_tolerance` is unreasonably low.
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

/// Step 7.2.
#[must_use]
fn calculate_new_rating(
    new_deviation: f64,
    player_rating: InternalRating,
    performance_sum: f64,
) -> f64 {
    player_rating.rating() + new_deviation * new_deviation * performance_sum
}

#[cfg(test)]
mod test {
    use std::time::{Duration, SystemTime};

    use crate::algorithm::{
        rate_games, rate_games_untimed, PublicGame, PublicTimedGames, PublicTimedOpponentGame,
        PublicTimedRating,
    };
    use crate::{FromWithSettings, GlickoSettings, IntoWithSettings, PublicRating};

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
        let settings = GlickoSettings::default()
            .with_volatility_change(0.5)
            .with_rating_period_duration(Duration::from_secs(1));

        let start_time = SystemTime::UNIX_EPOCH;

        let player = PublicTimedRating::new(start_time, PublicRating::new(1500.0, 200.0, 0.06));
        let rating_at_start = player.rating_at(SystemTime::UNIX_EPOCH, settings);

        assert_approx_eq!(rating_at_start.rating(), 1500.0, f64::EPSILON);
        assert_approx_eq!(rating_at_start.deviation(), 200.0, f64::EPSILON);
        assert_approx_eq!(rating_at_start.volatility(), 0.06, f64::EPSILON);
    }

    /// This tests the example calculation in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    #[test]
    fn test_paper_example() {
        let rating_period_duration = Duration::from_secs(1);
        let settings = GlickoSettings::default()
            .with_volatility_change(0.5)
            .with_rating_period_duration(rating_period_duration);

        let start_time = SystemTime::UNIX_EPOCH;
        let end_time = start_time + rating_period_duration;

        let player = PublicTimedRating::new(start_time, PublicRating::new(1500.0, 200.0, 0.06));

        // Volatility on opponents is not specified in the paper and doesn't matter in the calculation.
        // Constructor asserts it to be > 0.0
        let opponent_a = PublicTimedRating::new(
            start_time,
            PublicRating::new(1400.0, 30.0, settings.start_rating().volatility()),
        );
        let opponent_b = PublicTimedRating::new(
            start_time,
            PublicRating::new(1550.0, 100.0, settings.start_rating().volatility()),
        );
        let opponent_c = PublicTimedRating::new(
            start_time,
            PublicRating::new(1700.0, 300.0, settings.start_rating().volatility()),
        );

        let games = vec![
            PublicTimedOpponentGame::new(opponent_a, 1.0),
            PublicTimedOpponentGame::new(opponent_b, 0.0),
            PublicTimedOpponentGame::new(opponent_c, 0.0),
        ];

        let games = PublicTimedGames::new(end_time, games);

        let new_rating = rate_games(
            player.into_with_settings(settings),
            &games.into_with_settings(settings),
            settings,
        );

        // All games are considered to occur at the same time in the example
        let new_public_rating = PublicTimedRating::from_with_settings(new_rating, settings)
            .rating_at(end_time, settings);

        // However, we make an compromise for the opponent ratings to be able to be updated before the player ratings
        // which makes the algorithm a bit less accurate, thus the slightly higher tolerances
        assert_approx_eq!(new_public_rating.rating(), 1464.06, 0.05);
        assert_approx_eq!(new_public_rating.deviation(), 151.52, 0.05);
        assert_approx_eq!(new_public_rating.volatility(), 0.05999, 0.0001);
    }

    /// This tests the example calculation in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    #[test]
    fn test_paper_example_untimed() {
        let settings = GlickoSettings::default().with_volatility_change(0.5);

        let player = PublicRating::new(1500.0, 200.0, 0.06);

        // Volatility on opponents is not specified in the paper and doesn't matter in the calculation.
        // Constructor asserts it to be > 0.0
        let opponent_a = PublicRating::new(1400.0, 30.0, settings.start_rating().volatility());
        let opponent_b = PublicRating::new(1550.0, 100.0, settings.start_rating().volatility());
        let opponent_c = PublicRating::new(1700.0, 300.0, settings.start_rating().volatility());

        let games = vec![
            PublicGame::new(opponent_a, 1.0).into_with_settings(settings),
            PublicGame::new(opponent_b, 0.0).into_with_settings(settings),
            PublicGame::new(opponent_c, 0.0).into_with_settings(settings),
        ];

        let new_rating =
            rate_games_untimed(player.into_with_settings(settings), &games, 1.0, settings);

        let new_public_rating = PublicRating::from_with_settings(new_rating, settings);

        assert_approx_eq!(new_public_rating.rating(), 1464.06, 0.01);
        assert_approx_eq!(new_public_rating.deviation(), 151.52, 0.01);
        assert_approx_eq!(new_public_rating.volatility(), 0.05999, 0.0001);
    }
}
