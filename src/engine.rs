//! This mod defines the [`RatingEngine`] struct which abstracts away the rating period from rating calculations.

use std::time::{Duration, Instant};

use crate::algorithm::{self, RatingResult, ScaledPlayerResult, Score};
use crate::util::PushOnlyVec;
use crate::{FromWithParameters, IntoWithParameters, Parameters, ScaledRating};

pub struct ScaledPlayer {
    rating: ScaledRating,
    current_rating_period_results: Vec<ScaledPlayerResult>,
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
/// use instant_glicko_2::algorithm::{MatchResult, RatingResult};
/// use instant_glicko_2::engine::RatingEngine;
///
/// let parameters = Parameters::default();
///
/// // Create RatingEngine with one day rating period duration
/// // The first rating period starts instantly
/// let mut engine = RatingEngine::start_new(Duration::from_secs(60 * 60 * 24), Parameters::default());
///
/// // Register two players
/// // The first player is relatively strong
/// let player_1_rating_old = Rating::new(1700.0, 300.0, 0.06);
/// let player_1 = engine.register_player(player_1_rating_old);
/// // The second player hasn't played any games
/// let player_2_rating_old = Rating::default_from_parameters(parameters);
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
/// // Type signatures are needed because we could also work with the internal ScaledRating.
/// // That skips one step of calculation, but the rating values are not as pretty and not comparable to the original Glicko ratings.
/// let player_1_rating_new: Rating = engine.player_rating(player_1);
/// println!("Player 1 old rating: {player_1_rating_old:?}, new rating: {player_1_rating_new:?}");
/// let player_2_rating_new: Rating = engine.player_rating(player_2);
/// println!("Player 2 old rating: {player_2_rating_old:?}, new rating: {player_2_rating_new:?}");
///
/// assert_ne!(player_1_rating_old, player_1_rating_new);
/// assert_ne!(player_2_rating_old, player_2_rating_new);
/// ```

// In this case, just engine::Rating does not tell enough about the purpose of the struct in my opinion.
#[allow(clippy::module_name_repetitions)]
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
        RatingEngine {
            rating_period_duration,
            last_rating_period_start: Instant::now(),
            managed_players: PushOnlyVec::new(),
            parameters,
        }
    }

    // TODO: Newtype for index, maybe some better support in crate::utils
    pub fn register_player<R>(&mut self, rating: R) -> usize
    where
        R: IntoWithParameters<ScaledRating>,
    {
        let rating = rating.into_with_parameters(self.parameters);

        let index = self.managed_players.vec().len();

        self.managed_players.push(ScaledPlayer {
            rating,
            current_rating_period_results: Vec::new(),
        });

        index
    }

    /// Registers a result in the current rating period.
    /// Calculating the resulting ratings happens only when the Rating is inspected.
    ///
    /// # Panics
    ///
    /// This function might panic if the `result`'s players do not come from this `RatingEngine`.
    pub fn register_result<S: Score>(&mut self, result: &RatingResult<S>) {
        // We have to maybe close so the results will be added in the right rating period.
        self.maybe_close_rating_periods();

        // Split the result into two ScaledPlayerResults and save that on the players
        let player_1_rating = self
            .managed_players
            .vec()
            .get(result.player_1_idx())
            .expect("Result didn't belong to this RatingEngine")
            .rating;

        let player_2_rating = self
            .managed_players
            .vec()
            .get(result.player_2_idx())
            .expect("Result didn't belong to this RatingEngine")
            .rating;

        self.managed_players
            .get_mut(result.player_1_idx())
            .unwrap()
            .current_rating_period_results
            .push(ScaledPlayerResult::new(
                player_2_rating,
                result.score().player_score(),
            ));

        self.managed_players
            .get_mut(result.player_2_idx())
            .unwrap()
            .current_rating_period_results
            .push(ScaledPlayerResult::new(
                player_1_rating,
                result.score().opponent_score(),
            ));
    }

    #[must_use]
    pub fn player_rating<R>(&mut self, player_idx: usize) -> R
    where
        R: FromWithParameters<ScaledRating>,
    {
        let (elapsed_periods, _) = self.maybe_close_rating_periods();

        let player = self
            .managed_players
            .vec()
            .get(player_idx)
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
    /// # Returns
    ///
    /// A tuple containing the elapsed periods in the current rating period *after* all previous periods have been closed as a fraction
    /// as well as the amount of rating periods that have been closed.
    /// The elapsed periods will always be smaller than 1.
    pub fn maybe_close_rating_periods(&mut self) -> (f64, u32) {
        let elapsed_periods = self.elapsed_periods();

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
        let elapsed_duration = self.last_rating_period_start.elapsed();

        elapsed_duration.as_secs_f64() / self.rating_period_duration.as_secs_f64()
    }
}
