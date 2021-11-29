use ndarray::prelude::*;
use std::{str::FromStr, sync::Arc};

use crate::{
    dispatcher::dispatch_measure,
    error::PricerError,
    instrument::{Instrument, Stock, StockOption},
    lookupctx::LookupCtx,
    marketdataproviders::{DatePeriodProvider, MarketPriceProvider, RateProvider, VolProvider},
    portfolio::Position,
    positioncalculator::PositionScaler,
    stackedvectorctx::ShiftedExecutor,
};

#[derive(Debug)]
pub enum InstrumentRef<'a> {
    Option(&'a StockOption),
    Stock(&'a Stock),
}

impl<'a> From<&'a Instrument> for InstrumentRef<'a> {
    fn from(instrument: &'a Instrument) -> Self {
        match instrument {
            Instrument::Option(opt) => InstrumentRef::Option(opt),
            Instrument::Stock(s) => InstrumentRef::Stock(s),
        }
    }
}

impl<'a> From<&'a InstrumentRef<'a>> for Instrument {
    fn from(ins: &'a InstrumentRef<'a>) -> Self {
        match ins {
            InstrumentRef::Option(opt) => Instrument::Option((*opt).clone()),
            InstrumentRef::Stock(s) => Instrument::Stock((*s).clone()),
        }
    }
}

#[derive(Debug)]
pub struct VectorResult {
    pub arr: ArrayD<f64>,
}

impl VectorResult {
    pub fn new(arr: ArrayD<f64>) -> Self {
        Self { arr }
    }

    pub fn dimensionality(&self) -> usize {
        self.arr.ndim()
    }

    pub fn resultview_1d(&self) -> Result<ArrayView1<f64>, PricerError> {
        self.arr.view().into_dimensionality::<Ix1>().map_err(|_| {
            PricerError::ShapeError(format!(
                "vector result is not 1d. shape {}",
                self.arr.ndim()
            ))
        })
    }

    pub fn resultview_2d(&self) -> Result<ArrayView2<f64>, PricerError> {
        self.arr.view().into_dimensionality::<Ix2>().map_err(|_| {
            PricerError::ShapeError(format!(
                "vector result is not 2d. shape {}",
                self.arr.ndim()
            ))
        })
    }
}

#[derive(
    Debug, PartialEq, Eq, strum_macros::EnumString, strum_macros::Display, Clone, Copy, Hash,
)]
#[strum(serialize_all = "snake_case")]
pub enum Measure {
    Price,
    Exposure,
    UnderlyingPrice,
    Vol,
    Rate,
    TimeToExpiry,
    Delta,
    Gamma,
}

pub enum PositionScaling {
    NoScaling,
    Linear,
}

impl Measure {
    pub fn parse_measure(s: &str) -> Result<Measure, PricerError> {
        Measure::from_str(s).map_err(|_| PricerError::InvalidMeasureError(s.to_string()))
    }

    pub fn position_scaling(&self) -> PositionScaling {
        match self {
            Measure::Price => PositionScaling::NoScaling,
            Measure::UnderlyingPrice => PositionScaling::NoScaling,
            Measure::Vol => PositionScaling::NoScaling,
            Measure::Rate => PositionScaling::NoScaling,
            Measure::TimeToExpiry => PositionScaling::NoScaling,
            Measure::Delta => PositionScaling::NoScaling,
            Measure::Gamma => PositionScaling::NoScaling,
            Measure::Exposure => PositionScaling::Linear,
        }
    }
}

pub enum PricingCtx {
    LookupExecutor(LookupCtx),
    ShiftedExecutor(ShiftedExecutor),
}

impl PricingCtx {
    pub fn price_position(
        self: &Arc<Self>,
        measure: Measure,
        position: &Position,
    ) -> Result<f64, PricerError> {
        let insref = InstrumentRef::from(&position.instrument);
        let v = self.price(measure, &insref)?;
        let position_scaler = PositionScaler::new(measure, position);
        Ok(position_scaler.scale(v))
    }

    pub fn price(
        self: &Arc<Self>,
        measure: Measure,
        instrument: &InstrumentRef,
    ) -> Result<f64, PricerError> {
        let v = dispatch_measure(self, measure, instrument)?;
        self.process_shifts(measure, instrument, v)
    }

    pub fn process_shifts(
        self: &Arc<Self>,
        measure: Measure,
        instrument: &InstrumentRef,
        v: f64,
    ) -> Result<f64, PricerError> {
        match &**self {
            PricingCtx::LookupExecutor(_) => Ok(v),
            PricingCtx::ShiftedExecutor(se) => se.process(measure, instrument, v),
        }
    }

    pub fn vol_provider(
        self: &Arc<Self>,
        instrument: &InstrumentRef,
    ) -> Option<Arc<dyn VolProvider>> {
        match &**self {
            PricingCtx::LookupExecutor(le) => le.vol_provider(instrument),
            PricingCtx::ShiftedExecutor(se) => se.base_ctx.vol_provider(instrument),
        }
    }

    pub fn market_price_provider(
        self: &Arc<Self>,
        instrument: &InstrumentRef,
    ) -> Option<Arc<dyn MarketPriceProvider>> {
        match &**self {
            PricingCtx::LookupExecutor(le) => le.market_price_provider(instrument),
            PricingCtx::ShiftedExecutor(se) => se.base_ctx.market_price_provider(instrument),
        }
    }

    pub fn ir_provider(
        self: &Arc<Self>,
        instrument: &InstrumentRef,
    ) -> Option<Arc<dyn RateProvider>> {
        match &**self {
            PricingCtx::LookupExecutor(le) => le.ir_provider(instrument),
            PricingCtx::ShiftedExecutor(se) => se.base_ctx.ir_provider(instrument),
        }
    }

    pub fn period_provider(
        self: &Arc<Self>,
        instrument: &InstrumentRef,
    ) -> Option<Arc<dyn DatePeriodProvider>> {
        match &**self {
            PricingCtx::LookupExecutor(le) => le.period_provider(instrument),
            PricingCtx::ShiftedExecutor(se) => se.base_ctx.period_provider(instrument),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub axis_id: usize,
    pub dim: usize,
}

impl Axis {
    pub fn new_base_axis(dim: usize) -> Self {
        Axis::new(0, dim)
    }

    pub fn new(axis_id: usize, dim: usize) -> Self {
        Self { axis_id, dim }
    }

    pub fn new_from_parent(parent_axis: Axis, dim: usize) -> Self {
        let axis_id = parent_axis.axis_id + 1;
        Axis::new(axis_id, dim)
    }
}

pub trait VectorizedPricingCtx {
    fn price(
        self: &Arc<Self>,
        measure: Measure,
        instrument: &InstrumentRef,
    ) -> Result<VectorResult, PricerError>;

    fn price_position(
        self: &Arc<Self>,
        measure: Measure,
        position: &Position,
    ) -> Result<VectorResult, PricerError>;

    fn axes(&self) -> &[Axis];
    fn axis(&self) -> Axis;
    fn shape(&self) -> &[usize];
}
