//! Various constants defined or recommended in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf),
//! including defaults for [`model::Parameters`][crate::Parameters].

use crate::Rating;

/// Constant for converting between the original Glicko scale, and the internal Glicko-2 scale.
///
/// See also "Step 2." and "Step 8." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
pub const RATING_SCALING_RATIO: f64 = 173.7178;

/// Default start rating as defined by "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
// TODO: Make Rating::new const (blocked by https://github.com/rust-lang/rust/issues/57563)
pub const DEFAULT_START_RATING: Rating = Rating {
    rating: 1500.0,
    deviation: 350.0,
    volatility: 0.06,
};
/// Default system constant.
/// This value is right in the middle of the reasonable range described by "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf) (`0.3` to `1.2`),
/// but it might need to be fine-tuned for your application.
pub const DEFAULT_VOLATILITY_CHANGE: f64 = 0.75;

/// Default cutoff value for the converging loop algorithm as recommended by "Step 5.1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
/// Higher values may result in slightly better performance at the cost of less accuracy.
pub const DEFAULT_CONVERGENCE_TOLERANCE: f64 = 0.000_001;
/// The maximum number of iterations for the converging loop algorithm for "Step 5.4." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
/// This is a fail-safe so we don't enter an infinite loop (even tho that shouldn't happen if the convergence tolerance is reasonable).
/// If the maximum number of iterations is exceeded, the function panics.
pub const MAX_ITERATIONS: u32 = 10_000;
