/// Constant for converting between the original Glicko scale, and the internal Glicko-2 scale.
///
/// See also "Step 2." and "Step 8." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
pub const RATING_SCALING_RATIO: f64 = 173.7178;

/// Default start rating as defined by "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
pub const DEFAULT_START_RATING: f64 = 1500.0;
/// Default start rating deviation as defined by "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
pub const DEFAULT_START_DEVIATION: f64 = 350.0;
/// Default start rating volatility as defined by "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
pub const DEFAULT_START_VOLATILITY: f64 = 0.06;
/// Default start system constant.
/// This value is right in the middle of the reasonable range (0.3 to 1.2) described by "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf),
/// but it might need to be fine-tuned for your application.
pub const DEFAULT_VOLATILITY_CHANGE: f64 = 0.75;

/// Default cutoff value for the converging loop algorithm as recommended by "Step 5.1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
/// Higher values may result in slightly better performance at the cost of less accuracy.
pub const CONVERGENCE_TOLERANCE: f64 = 0.000_001;
