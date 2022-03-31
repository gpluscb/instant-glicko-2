//! This mod defines the [`RatingEngine`] struct which abstracts away the rating period from rating calculations.

use std::time::{Duration, Instant};

use crate::algorithm::{self, PlayerResult, ScaledPlayerResult, Score};
use crate::util::PushOnlyVec;
use crate::{FromWithParameters, IntoWithParameters, Parameters, Rating, ScaledRating};

/// An opaque index pointing to a player.
/// This is handed out by [`RatingEngine`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlayerHandle(usize);

/// A player as managed by [`RatingEngine`].

// TODO: Should this be public or even exist?
#[derive(Clone, PartialEq, Debug)]
pub struct Player {
    rating: Rating,
    current_rating_period_results: Vec<PlayerResult>,
}

impl FromWithParameters<ScaledPlayer> for Player {
    fn from_with_parameters(scaled: ScaledPlayer, parameters: Parameters) -> Self {
        Player {
            rating: scaled.rating.into_with_parameters(parameters),
            current_rating_period_results: scaled
                .current_rating_period_results
                .into_with_parameters(parameters),
        }
    }
}

/// A player as managed by [`RatingEngine`] with all values scaled to the internal rating scale.
/// See "Step 2." and "Step 8." in [Glickmans' paper](http://www.glicko.net/glicko/glicko2.pdf).
#[derive(Clone, PartialEq, Debug)]
pub struct ScaledPlayer {
    rating: ScaledRating,
    current_rating_period_results: Vec<ScaledPlayerResult>,
}

impl FromWithParameters<Player> for ScaledPlayer {
    fn from_with_parameters(player: Player, parameters: Parameters) -> Self {
        ScaledPlayer {
            rating: player.rating.into_with_parameters(parameters),
            current_rating_period_results: player
                .current_rating_period_results
                .into_with_parameters(parameters),
        }
    }
}

impl ScaledPlayer {
    #[must_use]
    pub fn rating(&self) -> ScaledRating {
        self.rating
    }

    #[must_use]
    pub fn current_rating_period_results(&self) -> &[ScaledPlayerResult] {
        self.current_rating_period_results.as_ref()
    }
}

/// A result of a match between two players managed by the same [`RatingEngine`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RatingResult<S> {
    player_1: PlayerHandle,
    player_2: PlayerHandle,
    score: S,
}

impl<S> RatingResult<S> {
    #[must_use]
    pub fn new(player_1: PlayerHandle, player_2: PlayerHandle, score: S) -> Self {
        RatingResult {
            player_1,
            player_2,
            score,
        }
    }

    #[must_use]
    pub fn player_1(&self) -> PlayerHandle {
        self.player_1
    }

    #[must_use]
    pub fn player_2(&self) -> PlayerHandle {
        self.player_2
    }

    #[must_use]
    pub fn score(&self) -> &S {
        &self.score
    }

    #[must_use]
    pub fn opponent(&self, player: PlayerHandle) -> Option<PlayerHandle> {
        if self.player_1 == player {
            Some(self.player_2)
        } else if self.player_2 == player {
            Some(self.player_1)
        } else {
            None
        }
    }

    #[must_use]
    pub fn player_score(&self, player: PlayerHandle) -> Option<f64>
    where
        S: Score,
    {
        if self.player_1 == player {
            Some(self.score.player_score())
        } else if self.player_2 == player {
            Some(self.score.opponent_score())
        } else {
            None
        }
    }

    #[must_use]
    pub fn includes(&self, player: PlayerHandle) -> bool {
        self.player_1 == player || self.player_2 == player
    }
}

/// Struct for managing player ratings and calculating them based on match results.
///
/// It uses the Glicko-2 algorithm with a given rating period duration and given parameters.
/// Matches can be added at any time, and participant ratings will update instantly.
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// use instant_glicko_2::{Parameters, Rating};
/// use instant_glicko_2::algorithm::MatchResult;
/// use instant_glicko_2::engine::{RatingEngine, RatingResult};
///
/// let parameters = Parameters::default();
///
/// // Create RatingEngine with one day rating period duration
/// // The first rating period starts instantly
/// let mut engine = RatingEngine::start_new(
///     Duration::from_secs(60 * 60 * 24),
///     Parameters::default(),
/// );
///
/// // Register two players
/// // The first player is relatively strong
/// let player_1_rating_old = Rating::new(1700.0, 300.0, 0.06);
/// let player_1 = engine.register_player(player_1_rating_old);
/// // The second player hasn't played any games
/// let player_2_rating_old = parameters.start_rating();
/// let player_2 = engine.register_player(player_2_rating_old);
///
/// // They play and player_2 wins
/// engine.register_result(&RatingResult::new(
///     player_1,
///     player_2,
///     MatchResult::Loss,
/// ));
///
/// // Print the new ratings
/// // Type signatures are needed because we could also work with the internal ScaledRating
/// // That skips one step of calculation,
/// // but the rating values are not as pretty and not comparable to the original Glicko ratings
/// let player_1_rating_new: Rating = engine.player_rating(player_1);
/// println!("Player 1 old rating: {player_1_rating_old:?}, new rating: {player_1_rating_new:?}");
/// let player_2_rating_new: Rating = engine.player_rating(player_2);
/// println!("Player 2 old rating: {player_2_rating_old:?}, new rating: {player_2_rating_new:?}");
///
/// // Loser's rating goes down, winner's rating goes up
/// assert!(player_1_rating_old.rating() > player_1_rating_new.rating());
/// assert!(player_2_rating_old.rating() < player_2_rating_new.rating());
/// ```

// In this case, just engine::Rating does not tell enough about the purpose of the struct in my opinion.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, PartialEq, Debug)]
pub struct RatingEngine {
    rating_period_duration: Duration,
    last_rating_period_start: Instant,
    // This should be a PushOnlyVec because we hand out index references.
    managed_players: PushOnlyVec<ScaledPlayer>,
    parameters: Parameters,
}

impl RatingEngine {
    /// Creates a new [`RatingEngine`], starting the first rating period immediately.
    #[must_use]
    pub fn start_new(rating_period_duration: Duration, parameters: Parameters) -> Self {
        Self::start_new_at(rating_period_duration, Instant::now(), parameters)
    }

    /// Creates a new [`RatingEngine`], starting the first rating period at the specified point in time.
    /// `start_time` may be at any point in the past, but not in the future.
    /// This is a requirement to prevent potential panics in other functions.
    ///
    /// This function is meant mostly for testability.
    ///
    /// # Panics
    ///
    /// This function panics if `start_time` is in the future.
    #[must_use]
    pub fn start_new_at(
        rating_period_duration: Duration,
        start_time: Instant,
        parameters: Parameters,
    ) -> Self {
        assert!(start_time < Instant::now(), "Start time was in the past");

        RatingEngine {
            rating_period_duration,
            last_rating_period_start: start_time,
            managed_players: PushOnlyVec::new(),
            parameters,
        }
    }

    /// Registers a new player with a given rating to this engine.
    ///
    /// # Returns
    ///
    /// Returns a value that can be later used to identify this player with this engine
    /// to get their ratings.

    // TODO: Newtype for index, maybe some better support in crate::utils
    pub fn register_player<R>(&mut self, rating: R) -> PlayerHandle
    where
        R: IntoWithParameters<ScaledRating>,
    {
        let rating = rating.into_with_parameters(self.parameters);

        let index = self.managed_players.vec().len();

        self.managed_players.push(ScaledPlayer {
            rating,
            current_rating_period_results: Vec::new(),
        });

        PlayerHandle(index)
    }

    /// Registers a result in the current rating period.
    /// Calculating the resulting ratings happens only when the Rating is inspected.
    ///
    /// This function can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    ///
    /// # Panics
    ///
    /// This function might panic if the `result`'s players do not come from this `RatingEngine`.
    pub fn register_result<S: Score>(&mut self, result: &RatingResult<S>) {
        let player_1_idx = result.player_1().0;
        let player_2_idx = result.player_2().0;

        // We have to maybe close so the results will be added in the right rating period.
        self.maybe_close_rating_periods();

        // Split the result into two ScaledPlayerResults and save that on the players
        let player_1_rating = self
            .managed_players
            .vec()
            .get(player_1_idx)
            .expect("Result didn't belong to this RatingEngine")
            .rating;

        let player_2_rating = self
            .managed_players
            .vec()
            .get(player_2_idx)
            .expect("Result didn't belong to this RatingEngine")
            .rating;

        self.managed_players
            .get_mut(player_1_idx)
            .unwrap()
            .current_rating_period_results
            .push(ScaledPlayerResult::new(
                player_2_rating,
                result.score().player_score(),
            ));

        self.managed_players
            .get_mut(player_2_idx)
            .unwrap()
            .current_rating_period_results
            .push(ScaledPlayerResult::new(
                player_1_rating,
                result.score().opponent_score(),
            ));
    }

    /// Calculates a player's rating at this point in time.
    /// The calculation is based on the registered results for this player (see [`register_result`][Self::register_result]).
    /// Note that this function is not cheap.
    /// The rating deviation of this result also depends on the current time, because rating deviation increases with time.
    ///
    /// This function takes `self` mutably because it can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    ///
    /// # Panics
    ///
    /// This function might panic or return a meaningless result if `player` wasn't sourced from this [`RatingEngine`].
    #[must_use]
    pub fn player_rating<R>(&mut self, player: PlayerHandle) -> R
    where
        R: FromWithParameters<ScaledRating>,
    {
        self.player_rating_at(player, Instant::now())
    }

    /// Returns a player
    ///
    /// This function is meant mostly for testability.
    ///
    /// This function takes `self` mutably because it can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    ///
    /// # Panics
    ///
    /// This function panics if `time` is earlier than the start of the last rating period.
    #[must_use]
    pub fn player_rating_at<R>(&mut self, player: PlayerHandle, time: Instant) -> R
    where
        R: FromWithParameters<ScaledRating>,
    {
        let (elapsed_periods, _) = self.maybe_close_rating_periods_at(time);

        let player = self
            .managed_players
            .vec()
            .get(player.0)
            .expect("Player index didn't belong to this RatingEngine");

        algorithm::rate_player_scaled(
            player.rating,
            &player.current_rating_period_results,
            elapsed_periods,
            self.parameters,
        )
        .into_with_parameters(self.parameters)
    }

    /// Closes all open rating periods that have elapsed by now.
    /// This doesn't need to be called manually.
    ///
    /// When a rating period is closed, the stored results are cleared and the players' ratings
    /// at the end of the period are stored as their ratings at the beginning of the next one.
    ///
    /// # Returns
    ///
    /// A tuple containing the elapsed periods in the current rating period *after* all previous periods have been closed as a fraction
    /// as well as the amount of rating periods that have been closed.
    /// The elapsed periods will always be smaller than 1.
    pub fn maybe_close_rating_periods(&mut self) -> (f64, u32) {
        self.maybe_close_rating_periods_at(Instant::now())
    }

    /// Closes all open rating periods that have elapsed by a given point in time.
    /// This doesn't need to be called manually.
    ///
    /// When a rating period is closed, the stored results are cleared and the players' ratings
    /// at the end of the period are stored as their ratings at the beginning of the next one.
    ///
    /// This function is meant mostly for testability.
    ///
    /// # Panics
    ///
    /// This function panics if `time` is earlier than the start of the last rating period.
    pub fn maybe_close_rating_periods_at(&mut self, time: Instant) -> (f64, u32) {
        let elapsed_periods = self.elapsed_periods_at(time);

        // We won't have negative elapsed_periods. Truncation this is the wanted result.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let periods_to_close = elapsed_periods as u32;

        // Every result is in the first rating period that needs to be closed.
        // This is guaranteed because we call this method before every time a new result gets added.
        for player in self.managed_players.iter_mut() {
            for _ in 0..periods_to_close {
                algorithm::close_player_rating_period_scaled(
                    &mut player.rating,
                    &player.current_rating_period_results,
                    self.parameters,
                );

                // We have now submitted the results to the players rating
                player.current_rating_period_results.clear();
            }
        }

        self.last_rating_period_start += periods_to_close * self.rating_period_duration;

        (elapsed_periods.fract(), periods_to_close)
    }

    /// The amount of rating periods that have elapsed since the last one was closed as a fraction.
    #[must_use]
    pub fn elapsed_periods(&self) -> f64 {
        self.elapsed_periods_at(Instant::now())
    }

    /// The amount of rating periods that have elapsed at the given point in time since the last one was closed as a fraction.
    ///
    /// This function is meant mostly for testability.
    ///
    /// # Panics
    ///
    /// This function panics if `time` is earlier than the start of the last rating period.
    #[must_use]
    pub fn elapsed_periods_at(&self, time: Instant) -> f64 {
        let elapsed_duration = time.duration_since(self.last_rating_period_start);

        elapsed_duration.as_secs_f64() / self.rating_period_duration.as_secs_f64()
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use super::{RatingEngine, RatingResult};
    use crate::algorithm::MatchResult;
    use crate::{Parameters, Rating};

    macro_rules! assert_approx_eq {
        ($a:expr, $b:expr, $tolerance:expr $(,)?) => {{
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

    /// This tests the example calculation in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    #[test]
    fn test_paper_example() {
        let parameters = Parameters::default().with_volatility_change(0.5);

        let start_instant = Instant::now();

        let mut engine =
            RatingEngine::start_new_at(Duration::from_secs(1), start_instant, parameters);

        let player = engine.register_player(Rating::new(1500.0, 200.0, 0.06));

        let opponent_a = engine.register_player(Rating::new(
            1400.0,
            30.0,
            parameters.start_rating().volatility(),
        ));
        let opponent_b = engine.register_player(Rating::new(
            1550.0,
            100.0,
            parameters.start_rating().volatility(),
        ));
        let opponent_c = engine.register_player(Rating::new(
            1700.0,
            300.0,
            parameters.start_rating().volatility(),
        ));

        engine.register_result(&RatingResult::new(player, opponent_a, MatchResult::Win));
        engine.register_result(&RatingResult::new(player, opponent_b, MatchResult::Loss));
        engine.register_result(&RatingResult::new(player, opponent_c, MatchResult::Loss));

        let rating_period_end_time = start_instant + Duration::from_secs(1);

        let new_rating: Rating = engine.player_rating_at(player, rating_period_end_time);

        assert_approx_eq!(new_rating.rating(), 1464.06, 0.01);
        assert_approx_eq!(new_rating.deviation(), 151.52, 0.01);
        assert_approx_eq!(new_rating.volatility(), 0.05999, 0.0001);
    }

    #[test]
    fn test_rating_period_close() {
        // Setup similar to paper setup
        let parameters = Parameters::default();

        let start_instant = Instant::now();

        let mut engine =
            RatingEngine::start_new_at(Duration::from_secs(1), start_instant, parameters);

        let player = engine.register_player(Rating::new(1500.0, 200.0, 0.06));

        let opponent = engine.register_player(Rating::new(
            1400.0,
            30.0,
            parameters.start_rating().volatility(),
        ));

        engine.register_result(&RatingResult::new(player, opponent, MatchResult::Win));

        assert_approx_eq!(engine.elapsed_periods_at(start_instant), 0.0, f64::EPSILON);
        let (elapsed_period, closed_periods) = engine.maybe_close_rating_periods_at(start_instant);
        assert_approx_eq!(elapsed_period, 0.0, f64::EPSILON);
        assert_eq!(closed_periods, 0);

        // Test that rating doesn't radically change across rating periods
        let right_before = start_instant + (Duration::from_secs(1) - Duration::from_nanos(1));
        let rating_right_before: Rating = engine.player_rating_at(player, right_before);

        let right_after = start_instant + (Duration::from_secs(1) + Duration::from_nanos(1));
        let rating_right_after: Rating = engine.player_rating_at(player, right_after);

        assert_approx_eq!(
            rating_right_before.rating(),
            rating_right_after.rating(),
            0.000_000_001,
        );
        // Rating deviation is supposed to change over time.
        // 2 nanoseconds won't change deviation much,
        // but they theoretically can change it a little and it's fine
        assert_approx_eq!(
            rating_right_before.deviation(),
            rating_right_after.deviation(),
            0.000_000_001,
        );
        assert_approx_eq!(
            rating_right_before.volatility(),
            rating_right_after.volatility(),
            0.000_000_001,
        );
    }

    #[test]
    fn test_time_change() {
        // Setup similar to paper setup
        let parameters = Parameters::default();

        let start_instant = Instant::now();

        let mut engine =
            RatingEngine::start_new_at(Duration::from_secs(60 * 60), start_instant, parameters);

        let player = engine.register_player(parameters.start_rating());

        let rating_at_start: Rating = engine.player_rating_at(player, start_instant);
        let rating_after_year: Rating = engine.player_rating_at(
            player,
            start_instant + Duration::from_secs(60 * 60 * 24 * 365),
        );

        // Deviation grows over time, rest should stay the same
        assert_approx_eq!(
            rating_at_start.rating(),
            rating_after_year.rating(),
            0.000_000_001,
        );
        // Adding one to make sure it grows somewhat significantly
        // TODO: Make actual calculation on what deviation is expected
        assert!(rating_at_start.deviation() + 1.0 < rating_after_year.deviation());
        assert_approx_eq!(
            rating_at_start.volatility(),
            rating_after_year.volatility(),
            0.000_000_001,
        );
    }
}
