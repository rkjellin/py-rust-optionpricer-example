use std::{collections::HashSet, str::FromStr};

use crate::{
    error::PricerError,
    instrument::Ticker,
    pricingctx::{InstrumentRef, Measure},
};

#[derive(
    Debug, PartialEq, Eq, strum_macros::EnumString, strum_macros::Display, Clone, Copy, Hash,
)]
#[strum(serialize_all = "snake_case")]
pub enum RiskFactor {
    Equity,
}

impl RiskFactor {
    pub fn parse_risk_factor(s: &str) -> Result<RiskFactor, PricerError> {
        RiskFactor::from_str(s).map_err(|_| PricerError::InvalidRiskFactorError(s.to_string()))
    }
}

#[derive(Clone)]
pub enum InstrumentFilter {
    Passthrough,
    TickerFilter(Ticker),
    RiskFactorFilter(RiskFactor),
}

impl InstrumentFilter {
    pub fn accept(&self, instrument: &InstrumentRef) -> bool {
        match self {
            InstrumentFilter::TickerFilter(ticker) => match instrument {
                InstrumentRef::Stock(s) => s.ticker == *ticker,
                _ => false,
            },
            InstrumentFilter::Passthrough => true,
            InstrumentFilter::RiskFactorFilter(rf) => match rf {
                RiskFactor::Equity => matches!(instrument, InstrumentRef::Stock(_)),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ShiftTarget {
    SingleMeasureTarget(Measure),
    MultiTarget(HashSet<Measure>),
}

impl ShiftTarget {
    pub fn is_target(&self, m: Measure) -> bool {
        match self {
            ShiftTarget::SingleMeasureTarget(tm) => tm == &m,
            ShiftTarget::MultiTarget(tms) => tms.contains(&m),
        }
    }
}

pub struct ShiftItem {
    pub instrument_filter: InstrumentFilter,
    pub shift_target: ShiftTarget,
    pub rel_shift: f64,
    pub abs_shift: f64,
}

impl ShiftItem {
    pub fn new(
        instrument_filter: &InstrumentFilter,
        shift_target: &ShiftTarget,
        rel_shift: f64,
        abs_shift: f64,
    ) -> Self {
        Self {
            instrument_filter: instrument_filter.clone(),
            shift_target: shift_target.clone(),
            rel_shift,
            abs_shift,
        }
    }
}

pub struct ShiftDefinition {
    instrument_filter: InstrumentFilter,
    rel_shifts: Vec<f64>,
    abs_shifts: Vec<f64>,
}

impl ShiftDefinition {
    pub fn new(
        instrument_filter: InstrumentFilter,
        rel_shifts: Vec<f64>,
        abs_shifts: Vec<f64>,
    ) -> Result<Self, PricerError> {
        if rel_shifts.len() != abs_shifts.len() {
            return Err(PricerError::InvalidScenario(
                "abs and rel shift lengths differ!".to_string(),
            ));
        }
        Ok(Self {
            instrument_filter,
            rel_shifts,
            abs_shifts,
        })
    }

    fn len(&self) -> usize {
        self.rel_shifts.len()
    }
}

pub struct TransformDefinition {
    shift_target: ShiftTarget,
    shift_definition: ShiftDefinition,
}

impl TransformDefinition {
    pub fn len(&self) -> usize {
        self.shift_definition.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn new_scalar_measure_transform(
        target: Measure,
        filter: InstrumentFilter,
        abs_shift: f64,
        rel_shift: f64,
    ) -> Result<TransformDefinition, PricerError> {
        let shift_definition = ShiftDefinition::new(filter, vec![rel_shift], vec![abs_shift])?;
        let shift_target = ShiftTarget::SingleMeasureTarget(target);
        let td = TransformDefinition {
            shift_target,
            shift_definition,
        };
        Ok(td)
    }

    pub fn new_vector_measure_transform(
        target: Measure,
        filter: InstrumentFilter,
        abs_shifts: Vec<f64>,
        rel_shifts: Vec<f64>,
    ) -> Result<TransformDefinition, PricerError> {
        let shift_definition = ShiftDefinition::new(filter, rel_shifts, abs_shifts)?;
        let shift_target = ShiftTarget::SingleMeasureTarget(target);
        let td = TransformDefinition {
            shift_target,
            shift_definition,
        };
        Ok(td)
    }

    pub fn shift_item_at(&self, i: usize) -> Result<ShiftItem, PricerError> {
        if i >= self.shift_definition.len() {
            return Err(PricerError::InvalidScenario(
                "accessing shift definition outside bounds".to_string(),
            ));
        }
        let abs_shift = self.shift_definition.abs_shifts[i];
        let rel_shift = self.shift_definition.rel_shifts[i];
        let si = ShiftItem::new(
            &self.shift_definition.instrument_filter,
            &self.shift_target,
            rel_shift,
            abs_shift,
        );
        Ok(si)
    }
}

pub enum TransformAlignment {
    Stacked,
    Orthogonal,
}
