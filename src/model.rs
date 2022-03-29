use crate::constants::{self, RATING_SCALING_RATIO};
use crate::FromWithParameters;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rating {
    rating: f64,
    deviation: f64,
    volatility: f64,
}

impl FromWithParameters<ScaledRating> for Rating {
    fn from_with_parameters(scaled: ScaledRating, parameters: Parameters) -> Self {
        let public_rating = scaled.rating * RATING_SCALING_RATIO + parameters.start_rating;
        let public_deviation = scaled.deviation * RATING_SCALING_RATIO;

        Rating::new(public_rating, public_deviation, scaled.volatility)
    }
}

impl Rating {
    /// # Panics
    ///
    /// This function panics if `deviation` or `volatility` was <= 0.
    #[must_use]
    pub fn new(rating: f64, deviation: f64, volatility: f64) -> Self {
        assert!(deviation > 0.0, "deviation <= 0: {deviation}");
        assert!(volatility > 0.0, "volatility <= 0: {volatility}");

        Rating {
            rating,
            deviation,
            volatility,
        }
    }

    #[must_use]
    pub fn default_from_parameters(parameters: Parameters) -> Self {
        Rating::new(
            parameters.start_rating,
            parameters.start_deviation,
            parameters.start_volatility,
        )
    }

    #[must_use]
    pub fn rating(&self) -> f64 {
        self.rating
    }

    #[must_use]
    pub fn deviation(&self) -> f64 {
        self.deviation
    }

    #[must_use]
    pub fn volatility(&self) -> f64 {
        self.volatility
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScaledRating {
    rating: f64,
    deviation: f64,
    volatility: f64,
}

impl FromWithParameters<Rating> for ScaledRating {
    fn from_with_parameters(rating: Rating, parameters: Parameters) -> Self {
        let scaled_rating = (rating.rating - parameters.start_rating) / RATING_SCALING_RATIO;
        let scaled_deviation = rating.deviation / RATING_SCALING_RATIO;

        ScaledRating::new(scaled_rating, scaled_deviation, rating.volatility)
    }
}

impl ScaledRating {
    /// # Panics
    ///
    /// This function panics if `deviation` or `volatility` was <= 0.
    #[must_use]
    pub fn new(rating: f64, deviation: f64, volatility: f64) -> Self {
        assert!(deviation > 0.0, "deviation <= 0: {deviation}");
        assert!(volatility > 0.0, "volatility <= 0: {volatility}");

        ScaledRating {
            rating,
            deviation,
            volatility,
        }
    }

    #[must_use]
    pub fn rating(&self) -> f64 {
        self.rating
    }

    #[must_use]
    pub fn deviation(&self) -> f64 {
        self.deviation
    }

    #[must_use]
    pub fn volatility(&self) -> f64 {
        self.volatility
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Parameters {
    pub start_rating: f64,
    pub start_deviation: f64,
    pub start_volatility: f64,
    /// Also called "system constant" or "τ".
    /// This constant constraints change in volatility over time.
    /// Reasonable choices are between 0.3 and 1.2.
    /// Small values prevent volatility and therefore rating from changing too much after improbable results.
    ///
    /// See also "Step 1." in [Glickman's paper](http://www.glicko.net/glicko/glicko2.pdf).
    pub volatility_change: f64,
    pub convergence_tolerance: f64,
}

impl Parameters {
    /// # Panics
    ///
    /// This function panics if `start_deviation`, `start_volatility`, or `convergence_tolerance` was <= 0.
    #[must_use]
    #[allow(clippy::too_many_arguments)] // TODO: Maybe builder pattern idk?
    pub fn new(
        start_rating: f64,
        start_deviation: f64,
        start_volatility: f64,
        volatility_change: f64,
        convergence_tolerance: f64,
    ) -> Self {
        assert!(
            start_deviation > 0.0,
            "start_deviation <= 0: {start_deviation}"
        );
        assert!(
            start_volatility > 0.0,
            "start_volatility <= 0: {start_volatility}"
        );
        assert!(
            convergence_tolerance > 0.0,
            "convergence_tolerance <= 0: {convergence_tolerance}"
        );

        Parameters {
            start_rating,
            start_deviation,
            start_volatility,
            volatility_change,
            convergence_tolerance,
        }
    }

    #[must_use]
    pub fn with_volatility_change(self, volatility_change: f64) -> Self {
        Parameters {
            volatility_change,
            ..self
        }
    }
}

impl Default for Parameters {
    /// Creates a default version of this struct with the parameters defined in [`constants`].
    fn default() -> Self {
        Parameters {
            start_rating: constants::DEFAULT_START_RATING,
            start_deviation: constants::DEFAULT_START_DEVIATION,
            start_volatility: constants::DEFAULT_START_VOLATILITY,
            volatility_change: constants::DEFAULT_VOLATILITY_CHANGE,
            convergence_tolerance: constants::CONVERGENCE_TOLERANCE,
        }
    }
}
