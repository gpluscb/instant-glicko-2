//! This module hosts the methods and types necessary to perform Glicko-2 calculations with fractional rating periods.

use std::borrow::Borrow;
use std::f64::consts::PI;

use crate::{constants, FromWithParameters, IntoWithParameters, Parameters, Rating, ScaledRating};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A match result as it pertains to a specific player.
///
/// This struct only holds the opponent's rating, and the player's score.
/// The player's rating is stored outside of this struct.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

impl FromWithParameters<&'_ [ScaledPlayerResult]> for Box<[PlayerResult]> {
    fn from_with_parameters(scaled: &'_ [ScaledPlayerResult], parameters: Parameters) -> Self {
        scaled
            .iter()
            .map(|&s| s.into_with_parameters(parameters))
            .collect()
    }
}

impl<const N: usize> FromWithParameters<[ScaledPlayerResult; N]> for [PlayerResult; N] {
    fn from_with_parameters(scaled: [ScaledPlayerResult; N], parameters: Parameters) -> Self {
        scaled.map(|s| s.into_with_parameters(parameters))
    }
}

impl FromWithParameters<Vec<ScaledPlayerResult>> for Vec<PlayerResult> {
    fn from_with_parameters(scaled: Vec<ScaledPlayerResult>, parameters: Parameters) -> Self {
        scaled
            .into_iter()
            .map(|s| s.into_with_parameters(parameters))
            .collect()
    }
}

impl PlayerResult {
    /// Creates a new [`PlayerResult`] with the given `opponent` and and the player's `score`.
    #[must_use]
    pub fn new(opponent: Rating, score: f64) -> Self {
        PlayerResult { opponent, score }
    }

    /// The opponent's rating.
    #[must_use]
    pub fn opponent(&self) -> Rating {
        self.opponent
    }

    /// The player's score.
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }
}

/// A match result as it pertains to a specific player with all fields scaled to the internal Glicko-2 scale.
/// See "Step 2." and "Step 8." in [Glickmans' paper](http://www.glicko.net/glicko/glicko2.pdf).
///
/// This struct only holds the opponent's rating, and the player's score.
/// The player's rating is stored outside of this struct.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

impl FromWithParameters<&'_ [PlayerResult]> for Box<[ScaledPlayerResult]> {
    fn from_with_parameters(results: &'_ [PlayerResult], parameters: Parameters) -> Self {
        results
            .iter()
            .map(|&r| r.into_with_parameters(parameters))
            .collect()
    }
}

impl<const N: usize> FromWithParameters<[PlayerResult; N]> for [ScaledPlayerResult; N] {
    fn from_with_parameters(results: [PlayerResult; N], parameters: Parameters) -> Self {
        results.map(|r| r.into_with_parameters(parameters))
    }
}

impl FromWithParameters<Vec<PlayerResult>> for Vec<ScaledPlayerResult> {
    fn from_with_parameters(results: Vec<PlayerResult>, parameters: Parameters) -> Self {
        results
            .into_iter()
            .map(|r| r.into_with_parameters(parameters))
            .collect()
    }
}

impl ScaledPlayerResult {
    /// Creates a new [`ScaledPlayerResult`] with the given `opponent` and and the player's `score`.
    #[must_use]
    pub fn new(opponent: ScaledRating, score: f64) -> Self {
        ScaledPlayerResult { opponent, score }
    }

    /// The opponent's rating.
    #[must_use]
    pub fn opponent(&self) -> ScaledRating {
        self.opponent
    }

    /// The player's score.
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score
    }
}

/// This is a wrapper for [`generic_close_player_rating_period`].
/// If you work with ratings that are not scaled to the internal Glicko-2 scale (see "Step 2." and "Step 8." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf)),
/// this function avoids you having to manually specify generic type parameters.
///
/// See [`generic_close_player_rating_period`] for more documentation.
///
/// # Panics
///
/// This function might panic if `parameters.convergence_tolerance()` is unreasonably low.
pub fn close_player_rating_period(
    player_rating: &mut Rating,
    results: &[PlayerResult],
    parameters: Parameters,
) {
    generic_close_player_rating_period::<_, _, Box<_>>(player_rating, results, parameters);
}

/// This is a wrapper for [`generic_close_player_rating_period`].
/// If you work with ratings that are scaled to the internal Glicko-2 scale (see "Step 2." and "Step 8." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf)),
/// this function avoids you having to manually specify generic type parameters.
///
/// See [`generic_close_player_rating_period`] for more documentation.
///
/// # Panics
///
/// This function might panic if `parameters.convergence_tolerance()` is unreasonably low.
pub fn close_player_rating_period_scaled(
    player_rating: &mut ScaledRating,
    results: &[ScaledPlayerResult],
    parameters: Parameters,
) {
    generic_close_player_rating_period::<_, _, &_>(player_rating, results, parameters);
}

/// Finalises a rating period for a player, taking into account all `results`.
///
/// See also: [`close_player_rating_period`] and [`close_player_rating_period_scaled`]
///
/// # Arguments
///
/// * `player_rating` - The rating of the player **at the onset of the rating period**
/// * `results` - The results of the player that occurred in the current rating period
/// * `parameters`
///
/// # Panics
///
/// This function might panic if `parameters.convergence_tolerance()` is unreasonably low.
pub fn generic_close_player_rating_period<Rating, Results, ResultsSlice>(
    player_rating: &mut Rating,
    results: Results,
    parameters: Parameters,
) where
    Rating: IntoWithParameters<ScaledRating> + FromWithParameters<ScaledRating> + Copy,
    Results: IntoWithParameters<ResultsSlice>,
    ResultsSlice: Borrow<[ScaledPlayerResult]>,
{
    *player_rating = generic_rate_player(*player_rating, results, 1.0, parameters);
}

/// This is a wrapper for [`generic_rate_player`].
/// If you work with ratings that are not scaled to the internal Glicko-2 scale (see "Step 2." and "Step 8." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf)),
/// this function avoids you having to manually specify generic type parameters.
///
/// See [`generic_rate_player`] for more documentation.
///
/// # Panics
///
/// This function might panic if `parameters.convergence_tolerance()` is unreasonably low.
#[must_use]
pub fn rate_player(
    player_rating: Rating,
    results: &[PlayerResult],
    elapsed_periods: f64,
    parameters: Parameters,
) -> Rating {
    generic_rate_player::<_, _, _, Box<_>>(player_rating, results, elapsed_periods, parameters)
}

/// This is a wrapper for [`generic_rate_player`].
/// If you work with ratings that are scaled to the internal Glicko-2 scale (see "Step 2." and "Step 8." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf)),
/// this function avoids you having to manually specify generic type parameters.
///
/// See [`generic_rate_player`] for more documentation.
///
/// # Panics
///
/// This function might panic if `parameters.convergence_tolerance()` is unreasonably low.
#[must_use]
pub fn rate_player_scaled(
    player_rating: ScaledRating,
    results: &[ScaledPlayerResult],
    elapsed_periods: f64,
    parameters: Parameters,
) -> ScaledRating {
    generic_rate_player::<_, _, _, &_>(player_rating, results, elapsed_periods, parameters)
}

/// If `results` is empty, only the rating deviation changes according to `elapsed_periods`.
///
/// See also: [`rate_player`] and [`rate_player_scaled`]
///
/// # Arguments
///
/// * `player_rating` - The rating of the player **at the onset of the rating period**
/// * `results` - All results of the player collected in the rating period at the current time
/// * `elapsed_periods` - What fraction of a rating period has elapsed while the `results` were collected
/// * `parameters`
///
/// # Panics
///
/// This function might panic if `parameters.convergence_tolerance()` is unreasonably low.
#[must_use]
pub fn generic_rate_player<Rating, Return, Results, ScaledResults>(
    player_rating: Rating,
    results: Results,
    elapsed_periods: f64,
    parameters: Parameters,
) -> Return
where
    Rating: IntoWithParameters<ScaledRating>,
    Return: FromWithParameters<ScaledRating>,
    Results: IntoWithParameters<ScaledResults>,
    ScaledResults: Borrow<[ScaledPlayerResult]>,
{
    // Step 1. (initialising) doesn't apply, we have already set the starting ratings.
    // Maybe Step 2.
    let player_rating = player_rating.into_with_parameters(parameters);
    let results = results.into_with_parameters(parameters);
    let results = results.borrow();

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
        )
        .into_with_parameters(parameters);
    }

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

    // Maybe Step 8..
    ScaledRating::new(new_rating, new_deviation, new_volatility).into_with_parameters(parameters)
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
    use crate::{IntoWithParameters, Parameters, Rating};

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
        let opponent_a = Rating::new(1400.0, 30.0, parameters.start_rating().volatility());
        let opponent_b = Rating::new(1550.0, 100.0, parameters.start_rating().volatility());
        let opponent_c = Rating::new(1700.0, 300.0, parameters.start_rating().volatility());

        let results = [
            PlayerResult::new(opponent_a, 1.0),
            PlayerResult::new(opponent_b, 0.0),
            PlayerResult::new(opponent_c, 0.0),
        ];

        let new_rating: Rating =
            super::rate_player(player, &results, 1.0, parameters).into_with_parameters(parameters);

        assert_approx_eq!(new_rating.rating(), 1464.06, 0.01);
        assert_approx_eq!(new_rating.deviation(), 151.52, 0.01);
        assert_approx_eq!(new_rating.volatility(), 0.05999, 0.0001);
    }
}
