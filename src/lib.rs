#![warn(clippy::pedantic)]
#![warn(clippy::cargo)]
// #![warn(missing_docs)] // TODO
#![forbid(unsafe_code)]

// TODO: can probably easily get nostd to work at least partially

use model::Parameters;

pub mod algorithm;
pub mod constants;
pub mod engine;
pub mod model;
mod util;

pub trait FromWithParameters<T> {
    fn from_with_parameters(_: T, parameters: Parameters) -> Self;
}

pub trait IntoWithParameters<T> {
    fn into_with_parameters(self, parameters: Parameters) -> T;
}

impl<T, U> IntoWithParameters<U> for T
where
    U: FromWithParameters<T>,
{
    fn into_with_parameters(self, parameters: Parameters) -> U {
        U::from_with_parameters(self, parameters)
    }
}
