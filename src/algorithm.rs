use std::f64::consts::PI;

use crate::model::{Parameters, Rating, ScaledRating};
use crate::util::PushOnlyVec;
use crate::{FromWithParameters, IntoWithParameters};

pub trait Score {
    fn player_score(&self) -> f64;
    fn opponent_score(&self) -> f64;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum MatchResult {
    Win,
    Draw,
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
    #[must_use]
    pub fn invert(self) -> Self {
        match self {
            MatchResult::Win => MatchResult::Loss,
            MatchResult::Draw => MatchResult::Draw,
            MatchResult::Loss => MatchResult::Win,
        }
    }
}

pub struct ScaledRatingPeriodResults<S> {
    // Push only just in case, otherwise modifying would invalidate results.
    participants: PushOnlyVec<ScaledRating>,
    results: Vec<RatingResult<S>>,
}

impl<S> ScaledRatingPeriodResults<S> {
    pub fn indices(&self) -> impl Iterator<Item = usize> {
        0..self.participants.vec().len()
    }

    /// # Panics
    ///
    /// This function panics if `player_idx` is out of bounds.
    #[must_use]
    pub fn player_results(&self, player_idx: usize) -> (&ScaledRating, Vec<ScaledPlayerResult>)
    where
        S: Score,
    {
        let vec = self
            .results
            .iter()
            .filter_map(|result| {
                let opponent_idx = result.opponent_idx(player_idx)?;

                let opponent = *self
                    .participants
                    .vec()
                    .get(opponent_idx)
                    .expect("player idx in results out of bounds");

                Some(ScaledPlayerResult {
                    opponent,
                    score: result
                        .player_score(player_idx)
                        .expect("player idx was not in result after verifying it is in result"),
                })
            })
            .collect();

        let player = self
            .participants
            .vec()
            .get(player_idx)
            .expect("player_idx out of bounds");

        (player, vec)
    }

    #[must_use]
    pub fn into_participants(self) -> Vec<ScaledRating> {
        self.participants.into()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RatingResult<S> {
    player_1_idx: usize,
    player_2_idx: usize,
    score: S,
}

impl<S> RatingResult<S> {
    #[must_use]
    pub fn player_1_idx(&self) -> usize {
        self.player_1_idx
    }

    #[must_use]
    pub fn player_2_idx(&self) -> usize {
        self.player_2_idx
    }

    #[must_use]
    pub fn score(&self) -> &S {
        &self.score
    }

    #[must_use]
    pub fn opponent_idx(&self, player_idx: usize) -> Option<usize> {
        if self.player_1_idx == player_idx {
            Some(self.player_2_idx)
        } else if self.player_2_idx == player_idx {
            Some(self.player_1_idx)
        } else {
            None
        }
    }

    #[must_use]
    pub fn player_score(&self, player_idx: usize) -> Option<f64>
    where
        S: Score,
    {
        if self.player_1_idx == player_idx {
            Some(self.score.player_score())
        } else if self.player_2_idx == player_idx {
            Some(self.score.opponent_score())
        } else {
            None
        }
    }

    #[must_use]
    pub fn includes(&self, player_idx: usize) -> bool {
        self.player_1_idx == player_idx || self.player_2_idx == player_idx
    }
}

pub struct PlayerResult {
    opponent: Rating,
    score: f64,
}

impl FromWithParameters<ScaledPlayerResult> for PlayerResult {
    fn from_with_parameters(scaled: ScaledPlayerResult, parameters: Parameters) -> Self {
        PlayerResult {
            opponent: scaled.opponent.into_with_parameters(parameters),
            score: scaled.score,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScaledPlayerResult {
    opponent: ScaledRating,
    score: f64,
}

impl FromWithParameters<PlayerResult> for ScaledPlayerResult {
    fn from_with_parameters(result: PlayerResult, parameters: Parameters) -> Self {
        ScaledPlayerResult {
            opponent: result.opponent.into_with_parameters(parameters),
            score: result.score,
        }
    }
}

impl ScaledPlayerResult {
    #[must_use]
    pub fn new(opponent: ScaledRating, score: f64) -> Self {
        ScaledPlayerResult { opponent, score }
    }

    #[must_use]
    pub fn opponent(&self) -> ScaledRating {
        self.opponent
    }

    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }
}

pub fn close_rating_period<S: Score>(
    results: &mut ScaledRatingPeriodResults<S>,
    parameters: Parameters,
) {
    #[allow(clippy::needless_collect)]
    // If we follow the lint suggestion, we run into lifetime issues
    let after_ratings = results
        .indices()
        .map(|idx| {
            let (rating, results) = results.player_results(idx);
            // We must not mutate the original ranking quite yet,
            // because on later iterations, we still need to
            let mut rating_copy = *rating;

            close_player_rating_period(&mut rating_copy, &results, parameters);

            rating_copy
        })
        .collect::<Vec<_>>();

    for (before_rating, after_rating) in results
        .participants
        .iter_mut()
        .zip(after_ratings.into_iter())
    {
        *before_rating = after_rating;
    }
}

/// Finalises a rating period for a player, taking into account all `results`.
///
/// # Arguments
///
/// * `player_rating` - The rating of the player **at the onset of the rating period**
/// * `results` - The results of the player that occurred in the current rating period
/// * `parameters`
pub fn close_player_rating_period(
    player_rating: &mut ScaledRating,
    results: &[ScaledPlayerResult],
    parameters: Parameters,
) {
    *player_rating = rate_player(*player_rating, results, 1.0, parameters);
}

/// If `results` is empty, only the rating deviation changes according to `elapsed_periods`.
///
/// # Arguments
///
/// * `player_rating` - The rating of the player **at the onset of the rating period**
/// * `results` - All results of the player collected in the rating period at the current time
/// * `elapsed_periods` - What fraction of a rating period has elapsed while the `results` were collected
/// * `parameters`
#[must_use]
pub fn rate_player(
    player_rating: ScaledRating,
    results: &[ScaledPlayerResult],
    elapsed_periods: f64,
    parameters: Parameters,
) -> ScaledRating {
    if results.is_empty() {
        // If `results` is empty, only Step 6. applies
        let new_deviation = calculate_pre_rating_period_value(
            player_rating.volatility(),
            player_rating,
            elapsed_periods,
        );

        return ScaledRating::new(
            player_rating.rating(),
            new_deviation,
            player_rating.volatility(),
        );
    }

    // Step 1. (initialising) doesn't apply, we have already set the starting ratings.
    // Step 2. (scaling down) doesn't apply, we already have a `ScaledRating` and `ScaledPlayerResult`s.
    // Step 3.
    let estimated_variance = calculate_estimated_variance(player_rating, results.iter().copied());

    // Step 4.
    let estimated_improvement =
        calculate_estimated_improvement(estimated_variance, player_rating, results.iter().copied());

    // Step 5.
    let new_volatility = calculate_new_volatility(
        estimated_improvement,
        estimated_variance,
        player_rating,
        parameters,
    );

    // Step 6.
    let pre_rating_period_value =
        calculate_pre_rating_period_value(new_volatility, player_rating, elapsed_periods);

    // Step 7.
    let new_deviation = calculate_new_rating_deviation(pre_rating_period_value, estimated_variance);

    let new_rating = calculate_new_rating(new_deviation, player_rating, results.iter().copied());

    // Step 8. doesn't really apply, can be done manually later.
    ScaledRating::new(new_rating, new_deviation, new_volatility)
}

/// Step 3.
///
/// This function returns [`f64::NAN`] if the results iterator is empty.
#[must_use]
fn calculate_estimated_variance(
    player_rating: ScaledRating,
    results: impl IntoIterator<Item = ScaledPlayerResult>,
) -> f64 {
    1.0 / results
        .into_iter()
        .map(|result| {
            let opponent_rating = result.opponent;

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
    results: impl IntoIterator<Item = ScaledPlayerResult>,
) -> f64 {
    estimated_variance
        * results
            .into_iter()
            .map(|result| {
                let opponent_rating = result.opponent;

                let g = calculate_g(opponent_rating.deviation());
                let e = calculate_e(g, player_rating.rating(), opponent_rating.rating());

                g * (result.score - e)
            })
            .sum::<f64>()
}

// TODO: cached?
// Optimizer is prolly smart enough to notice we call it with the same value thrice
// Even if not, like, come on... this is likely not a bottleneck
#[must_use]
fn calculate_g(deviation: f64) -> f64 {
    1.0 / f64::sqrt(1.0 + 3.0 * deviation * deviation / (PI * PI))
}

#[must_use]
fn calculate_e(g: f64, player_rating: f64, opponent_rating: f64) -> f64 {
    1.0 / (1.0 + f64::exp(-g * (player_rating - opponent_rating)))
}

/// Step 5.
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

    let volatility_change = parameters.volatility_change;

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
    // TODO: iterations cap -> panic or something?
    while f64::abs(b - a) > parameters.convergence_tolerance {
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
    results: impl IntoIterator<Item = ScaledPlayerResult>,
) -> f64 {
    player_rating.rating()
        + new_deviation
            * new_deviation
            * results
                .into_iter()
                .map(|result| {
                    let opponent_rating = result.opponent;

                    let g = calculate_g(opponent_rating.deviation());
                    let e = calculate_e(g, player_rating.rating(), opponent_rating.rating());

                    g * (result.score - e)
                })
                .sum::<f64>()
}

#[cfg(test)]
mod test {
    use crate::model::{Parameters, Rating};
    use crate::IntoWithParameters;

    use super::PlayerResult;

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

    /// This tests the example calculation in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    #[test]
    fn test_paper_example() {
        let parameters = Parameters::default().with_volatility_change(0.5);

        let player = Rating::new(1500.0, 200.0, 0.06);

        // Volatility on opponents is not specified in the paper and doesn't matter in the calculation.
        // Constructor asserts it to be > 0.0
        let opponent_a = Rating::new(1400.0, 30.0, parameters.start_volatility);
        let opponent_b = Rating::new(1550.0, 100.0, parameters.start_volatility);
        let opponent_c = Rating::new(1700.0, 300.0, parameters.start_volatility);

        let results = [
            PlayerResult {
                opponent: opponent_a,
                score: 1.0,
            }
            .into_with_parameters(parameters),
            PlayerResult {
                opponent: opponent_b,
                score: 0.0,
            }
            .into_with_parameters(parameters),
            PlayerResult {
                opponent: opponent_c,
                score: 0.0,
            }
            .into_with_parameters(parameters),
        ];

        let new_rating: Rating = super::rate_player(
            player.into_with_parameters(parameters),
            &results,
            1.0,
            parameters,
        )
        .into_with_parameters(parameters);

        assert_approx_eq!(new_rating.rating(), 1464.06, 0.01);
        assert_approx_eq!(new_rating.deviation(), 151.52, 0.01);
        assert_approx_eq!(new_rating.volatility(), 0.05999, 0.0001);
    }
}
