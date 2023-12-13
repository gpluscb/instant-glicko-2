//! This mod defines the [`RatingEngine`] struct which abstracts away the rating period from rating calculations.

use std::time::SystemTime;

use crate::algorithm::{self, Game, InternalGame};
use crate::util::PushOnlyVec;
use crate::{
    ConvertToScale, FromWithSettings, GlickoSettings, Internal, IntoWithSettings, Public, Rating,
    RatingScale,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An opaque index pointing to a player.
/// This is handed out by [`RatingEngine`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PlayerHandle(usize);

/// A player as managed by [`RatingEngine`].
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(serialize = "", deserialize = "")))]
pub struct EnginePlayer<Scale: RatingScale> {
    rating: Rating<Scale>,
    current_rating_period_results: Vec<Game<Scale>>,
}

pub type PublicEnginePlayerTODO = EnginePlayer<Public>;
pub type InternalEnginePlayerTODO = EnginePlayer<Internal>;

impl<Scale: RatingScale> EnginePlayer<Scale> {
    /// The rating of this player at the start of the current rating period.
    #[must_use]
    pub fn rating(&self) -> Rating<Scale> {
        self.rating
    }

    /// The match results the player had in the current rating period.
    #[must_use]
    pub fn current_rating_period_results(&self) -> &[Game<Scale>] {
        &self.current_rating_period_results
    }
}

impl<Scale1: RatingScale, Scale2: RatingScale> FromWithSettings<EnginePlayer<Scale1>>
    for EnginePlayer<Scale2>
where
    Scale1: ConvertToScale<Scale2>,
{
    fn from_with_settings(player: EnginePlayer<Scale1>, settings: GlickoSettings) -> Self {
        EnginePlayer {
            rating: player.rating.into_with_settings(settings),
            current_rating_period_results: player
                .current_rating_period_results
                .into_iter()
                .map(|game| game.into_with_settings(settings))
                .collect(),
        }
    }
}

/// A score of a match between a player and an opponent.
pub trait Score {
    /// The player score.
    /// Should be between `0.0` (loss) and `1.0` (win).
    #[must_use]
    fn player_score(&self) -> f64;

    /// The opponent score.
    /// Should be between `0.0` (loss) and `1.0` (win).
    #[must_use]
    fn opponent_score(&self) -> f64;
}

/// A simple match result.
/// Can be `Win`, `Draw`, or `Loss`.
///
/// Implements [`Score`] with a `Win` being `1.0` points, a `Loss` being `0.0` points, and a `Draw` being `0.5` points.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MatchResult {
    /// The player won.
    Win,
    /// The players drew.
    Draw,
    /// The opponent won.
    Loss,
}

impl Score for MatchResult {
    fn player_score(&self) -> f64 {
        match self {
            MatchResult::Win => 1.0,
            MatchResult::Draw => 0.5,
            MatchResult::Loss => 0.0,
        }
    }

    fn opponent_score(&self) -> f64 {
        self.invert().player_score()
    }
}

impl MatchResult {
    /// Returns a [`MatchResult`] for the opponent.
    #[must_use]
    pub fn invert(self) -> Self {
        match self {
            MatchResult::Win => MatchResult::Loss,
            MatchResult::Draw => MatchResult::Draw,
            MatchResult::Loss => MatchResult::Win,
        }
    }
}

/// Struct for managing player ratings and calculating them based on match results.
///
/// It uses the Glicko-2 algorithm with the given settings.
/// Matches can be added at any time, and participant ratings will update instantly.
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// use instant_glicko_2::{GlickoSettings, PublicRating};
/// use instant_glicko_2::engine::{MatchResult, RatingEngine};
///
/// let settings = GlickoSettings::default();
///
/// // Create a RatingEngine with a one day rating period duration
/// // The first rating period starts instantly
/// let mut engine = RatingEngine::start_new(GlickoSettings::default());
///
/// // Register two players
/// // The first player is relatively strong
/// let player_1_rating_old = PublicRating::new(1700.0, 300.0, 0.06);
/// let player_1 = engine.register_player(player_1_rating_old).0;
/// // The second player hasn't played any games
/// let player_2_rating_old = settings.start_rating();
/// let player_2 = engine.register_player(player_2_rating_old).0;
///
/// // They play and player_2 wins
/// engine.register_result(
///     player_1,
///     player_2,
///     &MatchResult::Loss,
/// );
///
/// // Print the new ratings
/// // Type signatures are needed because we could also work with the internal ScaledRating
/// // That skips one step of calculation,
/// // but the rating values are not as pretty and not comparable to the original Glicko ratings
/// let player_1_rating_new: PublicRating = engine.player_rating(player_1).0;
/// println!("Player 1 old rating: {player_1_rating_old:?}, new rating: {player_1_rating_new:?}");
/// let player_2_rating_new: PublicRating = engine.player_rating(player_2).0;
/// println!("Player 2 old rating: {player_2_rating_old:?}, new rating: {player_2_rating_new:?}");
///
/// // Loser's rating goes down, winner's rating goes up
/// assert!(player_1_rating_old.rating() > player_1_rating_new.rating());
/// assert!(player_2_rating_old.rating() < player_2_rating_new.rating());
/// ```
// In this case, just engine::Rating does not tell enough about the purpose of the struct in my opinion.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RatingEngine {
    last_rating_period_start: SystemTime,
    // This should be a PushOnlyVec because we hand out index references.
    managed_players: PushOnlyVec<InternalEnginePlayerTODO>,
    settings: GlickoSettings,
}

impl RatingEngine {
    /// Creates a new [`RatingEngine`], starting the first rating period immediately.
    #[must_use]
    pub fn start_new(settings: GlickoSettings) -> Self {
        Self::start_new_at(SystemTime::now(), settings)
    }

    /// Creates a new [`RatingEngine`], starting the first rating period at the specified point in time.
    ///
    /// This function is meant mostly for testability.
    #[must_use]
    pub fn start_new_at(start_time: SystemTime, settings: GlickoSettings) -> Self {
        RatingEngine {
            last_rating_period_start: start_time,
            managed_players: PushOnlyVec::new(),
            settings,
        }
    }

    /// The start of the last opened rating period.
    #[must_use]
    pub fn last_rating_period_start(&self) -> SystemTime {
        self.last_rating_period_start
    }

    /// The settings.
    #[must_use]
    pub fn settings(&self) -> GlickoSettings {
        self.settings
    }

    /// The rating of a player at the **start of** the last opened rating period.
    ///
    /// # Panics
    ///
    /// This function might panic or behave undesirably if `player` doesn't belong to this [`RatingEngine`].
    #[must_use]
    pub fn last_rating_period_rating<Scale: RatingScale>(
        &self,
        player: PlayerHandle,
    ) -> Rating<Scale>
    where
        Internal: ConvertToScale<Scale>,
    {
        self.managed_players
            .vec()
            .get(player.0)
            .expect("Player didn't belong to this RatingEngine")
            .rating()
            .into_with_settings(self.settings)
    }

    /// Returns an [`Iterator`] over all registered players.
    pub fn player_handles(&self) -> impl Iterator<Item = PlayerHandle> {
        (0..self.managed_players.vec().len()).map(PlayerHandle)
    }

    /// Registers a new player with a given rating to this engine at the start of the current rating period.
    ///
    /// This function can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    ///
    /// # Returns
    ///
    /// A tuple containing a value that can be later used to identify this player with this engine
    /// and the number of rating periods that were closed for this operation.
    ///
    /// This function might panic if the set settings' convergence tolerance is unreasonably low.
    // TODO: a way to register Right Now (so that the deviation is exactly the same at the now timestamp)
    pub fn register_player<Scale: RatingScale>(
        &mut self,
        rating: Rating<Scale>,
    ) -> (PlayerHandle, u32)
    where
        Scale: ConvertToScale<Internal>,
    {
        self.register_player_at(rating, SystemTime::now())
    }

    /// Registers a new player with a given rating to this engine at the start of what is the current rating period at the given time.
    /// If `time` is earlier than the start of the last rating period, the player will be registered at the start of the last rating period.
    ///
    /// This function can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    /// If `time` is earlier than the start of the last rating period, no rating periods will be closed.
    ///
    /// This function is meant mostly for testability.
    ///
    /// # Returns
    ///
    /// A tuple containing a value that can be later used to identify this player with this engine
    /// and the number of rating periods that were closed for this operation.
    ///
    /// # Panics
    ///
    /// This function panics if `time` is earlier than the start of the last rating period.
    ///
    /// This function might panic if the set settings' convergence tolerance is unreasonably low.
    pub fn register_player_at<Scale: RatingScale>(
        &mut self,
        rating: Rating<Scale>,
        time: SystemTime,
    ) -> (PlayerHandle, u32)
    where
        Scale: ConvertToScale<Internal>,
    {
        let (_, closed_periods) = self.maybe_close_rating_periods_at(time);

        let rating = rating.into_with_settings(self.settings);

        let index = self.managed_players.vec().len();

        self.managed_players.push(InternalEnginePlayerTODO {
            rating,
            current_rating_period_results: Vec::new(),
        });

        (PlayerHandle(index), closed_periods)
    }

    /// Registers a result in the current rating period.
    /// Calculating the resulting ratings happens only when the Rating is inspected.
    ///
    /// This function can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    ///
    /// # Returns
    ///
    /// The number of rating periods that were closed for this operation.
    ///
    /// # Panics
    ///
    /// This function might panic or behave undesirable if the `result`'s players do not come from this `RatingEngine`.
    ///
    /// This function might panic if the set settings' convergence tolerance is unreasonably low.
    pub fn register_result<S: Score>(
        &mut self,
        player_1: PlayerHandle,
        player_2: PlayerHandle,
        score: &S,
    ) -> u32 {
        self.register_result_at(player_1, player_2, score, SystemTime::now())
    }

    /// Registers a result at the given time in the current rating period.
    /// Calculating the resulting ratings happens only when the Rating is inspected.
    ///
    /// This function can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    /// If `time` is earlier than the start of the last rating period, no rating periods will be closed.
    ///
    /// This function is meant mostly for testability.
    ///
    /// # Returns
    ///
    /// The number of rating periods that were closed for this operation.
    ///
    /// # Panics
    ///
    /// This function might panic or behave undesirable if the `result`'s players do not come from this `RatingEngine`.
    ///
    /// This function might panic if the set settings' convergence tolerance is unreasonably low.
    pub fn register_result_at<S: Score>(
        &mut self,
        player_1: PlayerHandle,
        player_2: PlayerHandle,
        score: &S,
        time: SystemTime,
    ) -> u32 {
        // We have to maybe close so the results will be added in the right rating period.
        let (_, closed_periods) = self.maybe_close_rating_periods_at(time);

        // Split the result into two ScaledPlayerResults and save that on the players
        let player_1_rating = self
            .managed_players
            .vec()
            .get(player_1.0)
            .expect("Result didn't belong to this RatingEngine")
            .rating;

        let player_2_rating = self
            .managed_players
            .vec()
            .get(player_2.0)
            .expect("Result didn't belong to this RatingEngine")
            .rating;

        self.managed_players
            .get_mut(player_1.0)
            .unwrap()
            .current_rating_period_results
            .push(InternalGame::new(player_2_rating, score.player_score()));

        self.managed_players
            .get_mut(player_2.0)
            .unwrap()
            .current_rating_period_results
            .push(InternalGame::new(player_1_rating, score.opponent_score()));

        closed_periods
    }

    /// Calculates a player's rating at this point in time.
    /// The calculation is based on the registered results for this player (see [`register_result`][Self::register_result]).
    /// Note that this function does the actual rating computation.
    /// The rating deviation of this result also depends on the current time, because rating deviation increases with time.
    ///
    /// This function takes `self` mutably because it can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    ///
    /// # Returns
    ///
    /// A tuple containing the player's current rating and the number of rating periods that were closed for this operation.
    ///
    /// # Panics
    ///
    /// This function might panic or return a meaningless result if `player` wasn't sourced from this [`RatingEngine`].
    ///
    /// This function might panic if the set settings' convergence tolerance is unreasonably low.
    #[must_use]
    pub fn player_rating<Scale: RatingScale>(
        &mut self,
        player: PlayerHandle,
    ) -> (Rating<Scale>, u32)
    where
        Internal: ConvertToScale<Scale>,
    {
        self.player_rating_at(player, SystemTime::now())
    }

    /// Calculates a player's rating at a given point in time.
    /// The calculation is based on the registered results for this player (see [`register_result`][Self::register_result]).
    /// Note that this function does the actual rating computation.
    /// The rating deviation of this result also depends on the current time, because rating deviation increases with time.
    ///
    /// This function is meant mostly for testability.
    ///
    /// This function takes `self` mutably because it can close old rating periods (see [`maybe_close_rating_periods`][Self::maybe_close_rating_periods]).
    ///
    /// # Returns
    ///
    /// A tuple containing the player's current rating and the number of rating periods that were closed for this operation.
    ///
    /// If `time` is earlier than the start of the last rating period,
    /// no rating periods will be closed and the function will return the rating at the start of the last rating period
    /// after applying the rating change from the registered results.
    ///
    /// # Panics
    ///
    /// This function might panic or return a meaningless result if `player` wasn't sourced from this [`RatingEngine`].
    ///
    /// This function might panic if the set settings' convergence tolerance is unreasonably low.
    #[must_use]
    pub fn player_rating_at<Scale: RatingScale>(
        &mut self,
        player: PlayerHandle,
        time: SystemTime,
    ) -> (Rating<Scale>, u32)
    where
        Internal: ConvertToScale<Scale>,
    {
        let (elapsed_periods, closed_periods) = self.maybe_close_rating_periods_at(time);

        let player = self
            .managed_players
            .vec()
            .get(player.0)
            .expect("Player didn't belong to this RatingEngine");

        let rating = algorithm::rate_games_untimed(
            player.rating,
            &player.current_rating_period_results,
            elapsed_periods,
            self.settings,
        )
        .into_with_settings(self.settings);

        (rating, closed_periods)
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
    ///
    /// # Panics
    ///
    /// This function might panic if the set settings' convergence tolerance is unreasonably low.
    pub fn maybe_close_rating_periods(&mut self) -> (f64, u32) {
        self.maybe_close_rating_periods_at(SystemTime::now())
    }

    /// Closes all open rating periods that have elapsed by a given point in time.
    /// This doesn't need to be called manually.
    ///
    /// When a rating period is closed, the stored results are cleared and the players' ratings
    /// at the end of the period are stored as their ratings at the beginning of the next one.
    ///
    /// This function is meant mostly for testability.
    ///
    /// # Returns
    ///
    /// A tuple containing the elapsed periods in the current rating period *after* all previous periods have been closed as a fraction
    /// as well as the amount of rating periods that have been closed.
    /// The elapsed periods will always be smaller than `1.0`.
    ///
    /// If `time` is earlier than the start of the last rating period,
    /// no rating periods will be closed and the returned value will be `(0.0, 0)`.
    ///
    /// # Panics
    ///
    /// This function might panic if the set settings' convergence tolerance is unreasonably low.
    pub fn maybe_close_rating_periods_at(&mut self, time: SystemTime) -> (f64, u32) {
        let elapsed_periods = self.elapsed_periods_at(time);

        // We won't have negative elapsed_periods. Truncation this is the wanted result.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let periods_to_close = elapsed_periods as u32;

        // Every result is in the first rating period that needs to be closed.
        // This is guaranteed because we call this method before every time a new result gets added.
        for _ in 0..periods_to_close {
            for player in self.managed_players.iter_mut() {
                player.rating = algorithm::rate_games_untimed(
                    player.rating,
                    &player.current_rating_period_results,
                    1.0,
                    self.settings,
                );

                // We have now submitted the results to the players rating
                player.current_rating_period_results.clear();
            }
        }

        self.last_rating_period_start += periods_to_close * self.settings.rating_period_duration;

        (elapsed_periods.fract(), periods_to_close)
    }

    /// The amount of rating periods that have elapsed since the last one was closed as a fraction.
    #[must_use]
    pub fn elapsed_periods(&self) -> f64 {
        self.elapsed_periods_at(SystemTime::now())
    }

    /// The amount of rating periods that have elapsed at the given point in time since the last one was closed as a fraction.
    ///
    /// If `time` is earlier than the start of the last rating period, this function returns `0.0`.
    ///
    /// This function is meant mostly for testability.
    #[must_use]
    pub fn elapsed_periods_at(&self, time: SystemTime) -> f64 {
        if let Ok(elapsed_duration) = time.duration_since(self.last_rating_period_start) {
            elapsed_duration.as_secs_f64() / self.settings.rating_period_duration.as_secs_f64()
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, SystemTime};

    use super::{MatchResult, RatingEngine};
    use crate::{GlickoSettings, Public, PublicRating};

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
        let settings = GlickoSettings::default()
            .with_volatility_change(0.5)
            .with_rating_period_duration(Duration::from_secs(1));

        let start_time = SystemTime::UNIX_EPOCH;

        let mut engine = RatingEngine::start_new_at(start_time, settings);

        let player = engine
            .register_player_at(PublicRating::new(1500.0, 200.0, 0.06), start_time)
            .0;

        let opponent_a = engine
            .register_player_at(
                PublicRating::new(1400.0, 30.0, settings.start_rating().volatility()),
                start_time,
            )
            .0;
        let opponent_b = engine
            .register_player_at(
                PublicRating::new(1550.0, 100.0, settings.start_rating().volatility()),
                start_time,
            )
            .0;
        let opponent_c = engine
            .register_player_at(
                PublicRating::new(1700.0, 300.0, settings.start_rating().volatility()),
                start_time,
            )
            .0;

        engine.register_result_at(player, opponent_a, &MatchResult::Win, start_time);
        engine.register_result_at(player, opponent_b, &MatchResult::Loss, start_time);
        engine.register_result_at(player, opponent_c, &MatchResult::Loss, start_time);

        let rating_period_end_time = start_time + Duration::from_secs(1);

        let new_rating: PublicRating = engine.player_rating_at(player, rating_period_end_time).0;

        assert_approx_eq!(new_rating.rating(), 1464.06, 0.01);
        assert_approx_eq!(new_rating.deviation(), 151.52, 0.01);
        assert_approx_eq!(new_rating.volatility(), 0.05999, 0.0001);
    }

    #[test]
    fn test_rating_period_close() {
        // Setup similar to paper setup
        let settings =
            GlickoSettings::default().with_rating_period_duration(Duration::from_secs(1));

        let start_time = SystemTime::UNIX_EPOCH;

        let mut engine = RatingEngine::start_new_at(start_time, settings);

        let player = engine
            .register_player_at(PublicRating::new(1500.0, 200.0, 0.06), start_time)
            .0;

        let opponent = engine
            .register_player_at(
                PublicRating::new(1400.0, 30.0, settings.start_rating().volatility()),
                start_time,
            )
            .0;

        engine.register_result_at(player, opponent, &MatchResult::Win, start_time);

        assert_approx_eq!(engine.elapsed_periods_at(start_time), 0.0, f64::EPSILON);
        let (elapsed_periods, closed_periods) = engine.maybe_close_rating_periods_at(start_time);
        assert_approx_eq!(elapsed_periods, 0.0, f64::EPSILON);
        assert_eq!(closed_periods, 0);

        // Test that rating doesn't radically change across rating periods
        let right_before = start_time + (Duration::from_secs(1) - Duration::from_nanos(1));
        let (rating_right_before, closed_periods) =
            engine.player_rating_at::<Public>(player, right_before);
        assert_eq!(closed_periods, 0);

        let right_after = start_time + (Duration::from_secs(1) + Duration::from_nanos(1));
        let (rating_right_after, closed_periods) =
            engine.player_rating_at::<Public>(player, right_after);
        assert_eq!(closed_periods, 1);

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
        let settings =
            GlickoSettings::default().with_rating_period_duration(Duration::from_secs(60 * 60));

        let start_time = SystemTime::UNIX_EPOCH;

        let mut engine = RatingEngine::start_new_at(start_time, settings);

        let player = engine
            .register_player_at(settings.start_rating(), start_time)
            .0;

        let rating_at_start: PublicRating = engine.player_rating_at(player, start_time).0;
        let rating_after_year: PublicRating = engine
            .player_rating_at(player, start_time + Duration::from_secs(60 * 60 * 24 * 365))
            .0;

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
