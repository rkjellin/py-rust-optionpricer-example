use chrono::{Date, Utc};

use crate::error::PricerError;

pub trait DatePeriodProvider: Send + Sync {
    fn period_in_year(&self, dt: Date<Utc>) -> Result<f64, PricerError>;
}

pub struct FixedValuationDatePeriodProvider {
    valuation_date: Date<Utc>,
}

impl FixedValuationDatePeriodProvider {
    pub fn new(valuation_date: Date<Utc>) -> Self {
        Self { valuation_date }
    }
}

impl DatePeriodProvider for FixedValuationDatePeriodProvider {
    fn period_in_year(&self, dt: Date<Utc>) -> Result<f64, PricerError> {
        let days = dt - self.valuation_date;
        Ok(days.num_days() as f64 / 365.0)
    }
}

pub trait MarketPriceProvider: Send + Sync {
    fn eval(&self) -> Result<f64, PricerError>;
}

pub struct SinglePriceProvider {
    price: f64,
}

impl SinglePriceProvider {
    pub fn new(price: f64) -> Self {
        Self { price }
    }
}

impl MarketPriceProvider for SinglePriceProvider {
    fn eval(&self) -> Result<f64, PricerError> {
        Ok(self.price)
    }
}

pub trait RateProvider: Send + Sync {
    fn eval(&self) -> Result<f64, PricerError>; // should take from/to dates for discounting
}

pub struct FixedZeroRateProvider;

impl RateProvider for FixedZeroRateProvider {
    fn eval(&self) -> Result<f64, PricerError> {
        Ok(0.0)
    }
}

pub trait VolProvider: Send + Sync {
    fn eval(&self, spot: f64, strike: f64, tte: f64) -> Result<f64, PricerError>;
}
pub struct FixedVolSurface {
    vol: f64,
}

impl FixedVolSurface {
    pub fn new(vol: f64) -> Self {
        Self { vol }
    }
}

impl VolProvider for FixedVolSurface {
    fn eval(&self, _spot: f64, _strike: f64, _tte: f64) -> Result<f64, PricerError> {
        Ok(self.vol)
    }
}
