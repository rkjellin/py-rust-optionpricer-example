use std::fmt::Display;

use pricingcore::error::PricerError;
use pyo3::{create_exception, PyErr};
use thiserror::Error;

#[derive(Debug, Error)]
pub struct CorePricerError(PricerError);

impl Display for CorePricerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<PricerError> for CorePricerError {
    fn from(e: PricerError) -> Self {
        Self(e)
    }
}

impl From<CorePricerError> for PyErr {
    fn from(e: CorePricerError) -> Self {
        PyCorePricerError::new_err(e.to_string())
    }
}

create_exception!(
    optpricerpy,
    PyCorePricerError,
    pyo3::exceptions::PyException
);
