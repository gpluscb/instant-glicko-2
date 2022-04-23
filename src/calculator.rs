#![allow(missing_docs)] // TODO

use std::time::{Duration, SystemTime};

use crate::algorithm::{self, ScaledPlayerResult};
use crate::{FromWithParameters, IntoWithParameters, Parameters, ScaledRating};

pub struct ScaledPlayer {
    rating: ScaledRating,
    last_rating_period_start: SystemTime,
    current_rating_period_results: Vec<ScaledPlayerResult>,
}

// In this case, just calculator::Rating does not tell enough about the purpose of the struct in my opinion.
#[allow(clippy::module_name_repetitions)]
pub struct RatingCalculator {
    rating_period_duration: Duration,
    parameters: Parameters,
}

impl RatingCalculator {
    #[must_use]
    pub fn new(rating_period_duration: Duration, parameters: Parameters) -> Self {
        RatingCalculator {
            rating_period_duration,
            parameters,
        }
    }

    #[must_use]
    pub fn player_rating_at<R>(&mut self, player: &mut ScaledPlayer, time: SystemTime) -> (R, u32)
    where
        R: FromWithParameters<ScaledRating>,
    {
        let (elapsed_periods, closed_periods) =
            self.maybe_close_player_rating_periods_at(player, time);

        let rating = algorithm::rate_player_scaled(
            player.rating,
            &player.current_rating_period_results,
            elapsed_periods,
            self.parameters,
        )
        .into_with_parameters(self.parameters);

        (rating, closed_periods)
    }

    pub fn maybe_close_rating_periods<P>(&mut self, player: &mut ScaledPlayer) -> (f64, u32) {
        self.maybe_close_player_rating_periods_at(player, SystemTime::now())
    }

    pub fn maybe_close_player_rating_periods_at(
        &mut self,
        player: &mut ScaledPlayer,
        time: SystemTime,
    ) -> (f64, u32) {
        let elapsed_periods = self.elapsed_periods_at(player, time);

        // We won't have negative elapsed_periods. Truncation this is the wanted result.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let periods_to_close = elapsed_periods as u32;

        // Every result is in the first rating period that needs to be closed.
        // This is guaranteed because we call this method before every time a new result gets added.
        for _ in 0..periods_to_close {
            algorithm::close_player_rating_period_scaled(
                &mut player.rating,
                &player.current_rating_period_results,
                self.parameters,
            );

            // We have now submitted the results to the players rating
            player.current_rating_period_results.clear();
        }

        player.last_rating_period_start += periods_to_close * self.rating_period_duration;

        (elapsed_periods.fract(), periods_to_close)
    }

    #[must_use]
    pub fn elapsed_periods(&self, player: &ScaledPlayer) -> f64 {
        self.elapsed_periods_at(player, SystemTime::now())
    }

    #[must_use]
    pub fn elapsed_periods_at(&self, player: &ScaledPlayer, time: SystemTime) -> f64 {
        if let Ok(elapsed_duration) = time.duration_since(player.last_rating_period_start) {
            elapsed_duration.as_secs_f64() / self.rating_period_duration.as_secs_f64()
        } else {
            0.0
        }
    }
}
