use std::{fmt::Display, str::FromStr};

use chrono::{Date, Utc};

use crate::error::PricerError;

#[derive(Debug, PartialEq, Eq, PartialOrd, Hash, Clone)]
pub struct Ticker(pub String);

impl Display for Ticker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Stock {
    pub ticker: Ticker,
}

impl Stock {
    pub fn new(ticker: Ticker) -> Self {
        Self { ticker }
    }
}

#[derive(
    Debug, PartialEq, Eq, strum_macros::EnumString, strum_macros::Display, Clone, Copy, Hash,
)]
#[strum(serialize_all = "snake_case")]
pub enum OptionType {
    Call,
    Put,
}

impl OptionType {
    pub fn parse_option_type(s: &str) -> Result<OptionType, PricerError> {
        OptionType::from_str(s).map_err(|_| PricerError::InvalidOptionTypeError(s.to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct StockOption {
    pub underlying: Stock,
    pub strike: f64,
    pub expiry: Date<Utc>,
    pub option_type: OptionType,
}

impl StockOption {
    pub fn new(underlying: Stock, strike: f64, expiry: Date<Utc>, option_type: OptionType) -> Self {
        Self {
            underlying,
            strike,
            expiry,
            option_type,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Instrument {
    Option(StockOption),
    Stock(Stock),
}
