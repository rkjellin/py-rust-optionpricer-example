use thiserror::Error;

use crate::{
    instrument::{Instrument, Ticker},
    model::modelerror::ModelError,
    pricingctx::Measure,
};

#[derive(
    Debug, PartialEq, Eq, strum_macros::EnumString, strum_macros::Display, Clone, Copy, Hash,
)]
#[strum(serialize_all = "snake_case")]
pub enum MarketDataKind {
    Price,
    Vol,
}

#[derive(Debug, Error)]
pub enum PricerError {
    #[error("scenario definition is invalid: {0}")]
    InvalidScenario(String),

    #[error("Invalid risk factor: {0}")]
    InvalidRiskFactorError(String),

    #[error("Invalid option type: {0}")]
    InvalidOptionTypeError(String),

    #[error("Invalid measure: {0}")]
    InvalidMeasureError(String),

    #[error("Invalid shape: {0}")]
    ShapeError(String),

    #[error("Missing calculator for measure: {0}")]
    MissingCalculatorError(Measure),

    #[error("Missing {market_data_kind} for ticker {ticker}")]
    MissingMarketDataError {
        market_data_kind: MarketDataKind,
        ticker: Ticker,
    },

    #[error(transparent)]
    ModelError(#[from] ModelError),

    #[error("Failed to execute shift for {ins:?}: {message}")]
    ShiftExecutionError { ins: Instrument, message: String },

    #[error("Invalid infrastructure: {0}")]
    InfrastructureError(String),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error>),
}
