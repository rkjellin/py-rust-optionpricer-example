use std::{collections::HashMap, fmt::Display};

use atomic_counter::{AtomicCounter, RelaxedCounter};
use chrono::{Date, Utc};
use soa_derive::StructOfArray;

use crate::{
    error::PricerError,
    instrument::{Instrument, OptionType, StockOption, Ticker},
    pricingctx::InstrumentRef,
};

type PositionSize = f64;

#[derive(Debug)]
pub struct Position {
    pub size: PositionSize,
    pub instrument: Instrument,
}

impl Position {
    fn new(size: PositionSize, instrument: Instrument) -> Self {
        Self { size, instrument }
    }

    fn new_flat(instrument: Instrument) -> Self {
        Position::new(0.0, instrument)
    }
}

#[derive(Debug, StructOfArray)]
pub struct PositionData {
    pub position_id: PositionId,
    pub size: PositionSize,
    pub ticker: Ticker,
    pub strike: Option<f64>,
    pub expiry: Option<Date<Utc>>,
    pub option_type: Option<OptionType>,
}

impl PositionData {
    pub fn new(
        position_id: PositionId,
        size: PositionSize,
        ticker: Ticker,
        strike: Option<f64>,
        expiry: Option<Date<Utc>>,
        option_type: Option<OptionType>,
    ) -> Self {
        Self {
            position_id,
            size,
            ticker,
            strike,
            expiry,
            option_type,
        }
    }
}

impl From<(&PositionId, &Position)> for PositionData {
    fn from(pidpos: (&PositionId, &Position)) -> Self {
        let (pid, pos) = pidpos;
        let size = pos.size;
        match &pos.instrument {
            Instrument::Option(opt) => PositionData::new(
                pid.clone(),
                size,
                opt.underlying.ticker.clone(),
                Some(opt.strike),
                Some(opt.expiry),
                Some(opt.option_type),
            ),
            Instrument::Stock(s) => {
                PositionData::new(pid.clone(), size, s.ticker.clone(), None, None, None)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PositionId(String);

impl Display for PositionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct Portfolio {
    trade_id_counter: RelaxedCounter,
    positions: HashMap<PositionId, Position>,
    pub position_ids: Vec<PositionId>,
}

impl Portfolio {
    pub fn new() -> Self {
        Self {
            trade_id_counter: RelaxedCounter::new(0),
            positions: HashMap::new(),
            position_ids: vec![],
        }
    }

    fn gen_trade_id(&mut self, opt: &StockOption) -> String {
        let tid = self.trade_id_counter.inc();
        let opttype_char = match opt.option_type {
            OptionType::Call => "C",
            OptionType::Put => "P",
        };
        format!(
            "{}{}{}{}_{}",
            &opt.underlying.ticker.0,
            opt.expiry.format("%Y%m%d"),
            opttype_char,
            opt.strike,
            tid
        )
    }

    pub fn add_trade(
        &mut self,
        instrument: &Instrument,
        size: PositionSize,
    ) -> Result<(), PricerError> {
        let pid = {
            let pidstr = match instrument {
                Instrument::Option(opt) => self.gen_trade_id(opt),
                Instrument::Stock(s) => s.ticker.0.clone(),
            };
            PositionId(pidstr)
        };
        if let Some(pos) = self.positions.get_mut(&pid) {
            pos.size += size;
        } else {
            let pos = Position::new(size, instrument.clone());
            self.positions.insert(pid.clone(), pos);
            self.position_ids.push(pid);
        }
        Ok(())
    }

    pub fn instruments(&self) -> Vec<(String, InstrumentRef)> {
        self.positions
            .iter()
            .map(|(tid, p)| (tid.0.clone(), InstrumentRef::from(&p.instrument)))
            .collect()
    }

    pub fn positions_in_order(&self) -> impl Iterator<Item = (&PositionId, &Position)> {
        self.position_ids
            .iter()
            .filter_map(|pid| self.positions.get(pid).map(|pos| (pid, pos)))
    }

    pub fn get_position(&self, pid: &PositionId) -> Option<&Position> {
        self.positions.get(pid)
    }

    pub fn get_position_data(&self) -> PositionDataVec {
        PositionDataVec::from_iter(self.positions_in_order().map(PositionData::from))
    }

    pub fn equity_pos_len(&self) -> usize {
        self.positions
            .values()
            .filter(|p| matches!(p.instrument, Instrument::Stock(_)))
            .count()
    }

    pub fn option_trades_len(&self) -> usize {
        self.positions
            .values()
            .filter(|p| matches!(p.instrument, Instrument::Option(_)))
            .count()
    }
}

impl Default for Portfolio {
    fn default() -> Self {
        Self::new()
    }
}
