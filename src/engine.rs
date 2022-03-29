use std::time::{Duration, Instant};

use crate::algorithm::{RatingResult, ScaledPlayerResult, Score};
use crate::model::{Parameters, ScaledRating};
use crate::util::PushOnlyVec;

pub struct ScaledPlayer {
    last_active: Option<Instant>, // TODO: Maybe last_active_rating_period?
    rating: ScaledRating,
    // PushOnlyVec because we only push.
    current_rating_period_results: PushOnlyVec<ScaledPlayerResult>,
}

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
    pub fn register_player(&mut self, rating: ScaledRating) -> usize {
        let index = self.managed_players.vec().len();

        self.managed_players.push(ScaledPlayer {
            last_active: None,
            rating,
            current_rating_period_results: PushOnlyVec::new(),
        });

        index
    }

    /// # Panics
    ///
    /// This function might panic if the `result`'s players do not come from this `RatingEngine`.
    pub fn register_result<S: Score>(&mut self, result: &RatingResult<S>) {
        // FIXME: Check if rating period should close

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

    pub fn player_rating(&mut self, player_idx: usize) -> ScaledRating {
        // FIXME: Check if rating period should close

        todo!() // TODO
    }
}
