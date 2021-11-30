use arrow::array::TimestampNanosecondArray;
use arrow::temporal_conversions::timestamp_ns_to_datetime;
use rayon::prelude::*;
use std::collections::HashMap;

use std::str::FromStr;
use std::vec::Vec;

use arrow::datatypes::{DataType, Field, Schema};
use arrow::{
    array::{Float64Array, StringArray},
    error::ArrowError,
    record_batch::RecordBatch,
};
use chrono::{Date, Datelike, TimeZone, Utc};
use ndarray::{Array, ArrayD};
use numpy::IntoPyArray;
use numpy::PyArrayDyn;
use pricingcore::lookupctx::LookupCtx;
use pricingcore::pricingctx::VectorizedPricingCtx;
use pricingcore::stackedvectorctx::StackedVectorizedPricingCtx;
use pricingcore::transform::{
    InstrumentFilter, RiskFactor, TransformAlignment, TransformDefinition,
};
use pricingcore::{instrument::Ticker, pricingctx::Measure};
use pyo3::types::{PyDate, PyDateAccess};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use std::sync::Arc;

use crate::error::CorePricerError;

use crate::portfolio::Portfolio;

#[derive(Debug)]
struct MarketDataObservation {
    pub market_prices: HashMap<Ticker, f64>,
    pub vols: HashMap<Ticker, f64>,
}

impl MarketDataObservation {
    fn new() -> Self {
        Self {
            market_prices: HashMap::new(),
            vols: HashMap::new(),
        }
    }
}

#[pyclass]
#[derive(Debug)]
pub struct MarketData {
    observations: HashMap<Date<Utc>, MarketDataObservation>,
}

#[pymethods]
impl MarketData {
    #[new]
    fn new() -> Self {
        Self {
            observations: HashMap::new(),
        }
    }

    pub fn load_market_data(&mut self, marketdata: RecordBatch) -> PyResult<()> {
        let schema = marketdata.schema();
        let ticker_col = marketdata
            .column(schema.index_of("ticker")?)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| ArrowError::ParseError("Expects an str array".to_string()))?;

        let date_col = marketdata
            .column(schema.index_of("date")?)
            .as_any()
            .downcast_ref::<TimestampNanosecondArray>()
            .ok_or_else(|| ArrowError::ParseError("Expects a timestamp[ns] array".to_string()))?;

        let spot_col = marketdata
            .column(schema.index_of("spot")?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| ArrowError::ParseError("Expects an float array".to_string()))?;

        let vol_col = marketdata
            .column(schema.index_of("vol")?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| ArrowError::ParseError("Expects an float array".to_string()))?;

        for i in 0..marketdata.num_rows() {
            let ticker = ticker_col.value(i);
            let date = {
                let naive_dt = timestamp_ns_to_datetime(date_col.value(i)).date();
                Utc.ymd(naive_dt.year(), naive_dt.month(), naive_dt.day())
            };
            let spot = spot_col.value(i);
            let vol = vol_col.value(i);
            let mobs = self
                .observations
                .entry(date)
                .or_insert_with(MarketDataObservation::new);
            mobs.market_prices.insert(Ticker(ticker.to_string()), spot);
            mobs.vols.insert(Ticker(ticker.to_string()), vol);
        }
        Ok(())
    }
}

#[pyclass]
pub struct FilterDef {
    filter: InstrumentFilter,
}

#[pymethods]
impl FilterDef {
    #[new]
    pub fn new(filter_kind: &str, criteria: &str) -> PyResult<Self> {
        let filter = match filter_kind {
            "risk_factor_filter" => {
                let rf = RiskFactor::parse_risk_factor(criteria).map_err(CorePricerError::from)?;
                InstrumentFilter::RiskFactorFilter(rf)
            }
            "ticker_filter" => InstrumentFilter::TickerFilter(Ticker(criteria.to_string())),
            s => {
                return Err(PyRuntimeError::new_err(format!(
                    "Unknown filter_kind {:?}",
                    s
                )))
            }
        };
        Ok(Self { filter })
    }

    #[staticmethod]
    pub fn new_passthrough() -> Self {
        Self {
            filter: InstrumentFilter::Passthrough,
        }
    }
}

#[pyclass]
pub struct ScenarioShift {
    target_measure: Measure,
    filter_def: Py<FilterDef>,
    abs_shifts: Vec<f64>,
    rel_shifts: Vec<f64>,
}

#[pymethods]
impl ScenarioShift {
    #[new]
    pub fn new(
        target_measure: &str,
        filter_def: Py<FilterDef>,
        abs_shifts: Vec<f64>,
        rel_shifts: Vec<f64>,
    ) -> PyResult<Self> {
        let tm = Measure::parse_measure(target_measure).map_err(CorePricerError::from)?;
        Ok(Self {
            target_measure: tm,
            filter_def,
            abs_shifts,
            rel_shifts,
        })
    }
}

#[pyclass]
pub struct ScenarioDefinition {
    shifts: Vec<(Py<ScenarioShift>, bool)>,
}

#[pymethods]
impl ScenarioDefinition {
    #[new]
    pub fn new(shifts: Vec<(Py<ScenarioShift>, bool)>) -> Self {
        Self { shifts }
    }
}

pub fn make_vector_ctx<'a>(
    py: Python<'a>,
    valuation_date: &PyDate,
    marketdata: &PyCell<MarketData>,
    scenario_def: &PyCell<ScenarioDefinition>,
) -> PyResult<Arc<StackedVectorizedPricingCtx>> {
    let vdt = Utc.ymd(
        valuation_date.get_year(),
        valuation_date.get_month() as u32,
        valuation_date.get_day() as u32,
    );
    let md = marketdata.borrow();
    let mobs = md
        .observations
        .get(&vdt)
        .ok_or_else(|| PyRuntimeError::new_err(format!("No market data for {}", vdt)))?;
    let pctx = LookupCtx::new_from_prices_and_vols(vdt, &mobs.market_prices, &mobs.vols);
    let mut vctx = None;
    for (shift, is_ortho) in scenario_def.borrow().shifts.iter() {
        if let Some(ref vc) = vctx {
            let alignment = if *is_ortho {
                TransformAlignment::Orthogonal
            } else {
                TransformAlignment::Stacked
            };
            vctx = Some(StackedVectorizedPricingCtx::shift_vector_ctx(
                vc,
                TransformDefinition::new_vector_measure_transform(
                    shift.borrow(py).target_measure,
                    shift.borrow(py).filter_def.borrow(py).filter.clone(),
                    shift.borrow(py).abs_shifts.clone(),
                    shift.borrow(py).rel_shifts.clone(),
                )
                .map_err(CorePricerError::from)?,
                alignment,
            ));
        } else {
            vctx = Some(StackedVectorizedPricingCtx::shift_base_ctx(
                &pctx,
                TransformDefinition::new_vector_measure_transform(
                    shift.borrow(py).target_measure,
                    shift.borrow(py).filter_def.borrow(py).filter.clone(),
                    shift.borrow(py).abs_shifts.clone(),
                    shift.borrow(py).rel_shifts.clone(),
                )
                .map_err(CorePricerError::from)?,
            ));
        }
    }
    vctx.ok_or_else(|| PyRuntimeError::new_err("No shifts in scenario"))
}

#[pyclass]
pub struct PricingEngine {}

#[pymethods]
impl PricingEngine {
    #[new]
    pub fn new() -> Self {
        Self {}
    }

    pub fn price_portfolio_scenario<'a>(
        &self,
        py: Python<'a>,
        measure: &str,
        valuation_date: &PyDate,
        portfolio: &PyCell<Portfolio>,
        marketdata: &PyCell<MarketData>,
        scenario_def: &PyCell<ScenarioDefinition>,
    ) -> PyResult<&'a PyArrayDyn<f64>> {
        let m = Measure::from_str(measure).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let vctx = make_vector_ctx(py, valuation_date, marketdata, scenario_def)?;

        let vecres: Vec<Result<ArrayD<_>, PyErr>> = portfolio
            .borrow()
            .portfolio
            .positions_in_order()
            .collect::<Vec<_>>()
            .par_iter()
            .try_fold(Vec::new, |mut v, (_, pos)| {
                let res: Result<_, PyErr> = Ok(vctx
                    .price_position(m, pos)
                    .map(|vr| vr.arr)
                    .map_err(CorePricerError::from)?);
                v.push(res);
                let vok: Result<_, PyErr> = Ok(v);
                vok
            })
            .try_reduce(Vec::new, |mut x, y| {
                x.extend(y);
                Ok(x)
            })?;
        let resvec: Result<Vec<_>, PyErr> = vecres.into_iter().collect();
        let res = resvec?;
        let viewres: Vec<_> = res.iter().map(|arr| arr.view()).collect();
        let concat_res = ndarray::stack(ndarray::Axis(0), &viewres)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(concat_res.into_pyarray(py))
    }

    pub fn price_ladder<'a>(
        &self,
        py: Python<'a>,
        measures: Vec<String>,
        valuation_date: &PyDate,
        portfolio: &PyCell<Portfolio>,
        marketdata: &PyCell<MarketData>,
        ladder_definition: &PyCell<ScenarioShift>,
    ) -> PyResult<RecordBatch> {
        let vdt = Utc.ymd(
            valuation_date.get_year(),
            valuation_date.get_month() as u32,
            valuation_date.get_day() as u32,
        );
        let measure_vec_res: Result<Vec<Measure>, _> = measures
            .iter()
            .map(|m| Measure::parse_measure(m).map_err(CorePricerError::from))
            .collect();
        let measure_vec = measure_vec_res?;
        let md = marketdata.borrow();

        let mobs = md
            .observations
            .get(&vdt)
            .ok_or_else(|| PyRuntimeError::new_err(format!("No market data for {}", vdt)))?;
        let pctx = LookupCtx::new_from_prices_and_vols(vdt, &mobs.market_prices, &mobs.vols);
        let ladder_ref = ladder_definition.borrow();
        let vctx = StackedVectorizedPricingCtx::shift_base_ctx(
            &pctx,
            TransformDefinition::new_vector_measure_transform(
                ladder_ref.target_measure,
                ladder_ref.filter_def.borrow(py).filter.clone(),
                ladder_ref.abs_shifts.clone(),
                ladder_ref.rel_shifts.clone(),
            )
            .map_err(CorePricerError::from)?,
        );
        let mut trade_id_vec = vec![];
        let mut measure_out_vec = vec![];
        let mut measure_value_vec = vec![];
        let mut rel_shift_vec = vec![];
        let mut abs_shift_vec = vec![];
        for (tid, pos) in portfolio.borrow().portfolio.positions_in_order() {
            for m in measure_vec.iter() {
                let x = vctx
                    .price_position(*m, pos)
                    .map_err(CorePricerError::from)?;
                for (val, (rel_shift, abs_shift)) in x.arr.iter().zip(
                    ladder_ref
                        .rel_shifts
                        .iter()
                        .zip(ladder_ref.abs_shifts.iter()),
                ) {
                    trade_id_vec.push(tid.to_string());
                    measure_out_vec.push(m.to_string());
                    measure_value_vec.push(*val);
                    rel_shift_vec.push(*rel_shift);
                    abs_shift_vec.push(*abs_shift);
                }
            }
        }

        let trade_id_array = StringArray::from(trade_id_vec);
        let measure_array = StringArray::from(measure_out_vec);
        let measure_value_array = Float64Array::from(measure_value_vec);
        let rel_shift_array = Float64Array::from(rel_shift_vec);
        let abs_shift_array = Float64Array::from(abs_shift_vec);
        let schema = Schema::new(vec![
            Field::new("trade_id", DataType::Utf8, false),
            Field::new("measure", DataType::Utf8, false),
            Field::new("measure_value", DataType::Float64, false),
            Field::new("rel_shift", DataType::Float64, false),
            Field::new("abs_shift", DataType::Float64, false),
        ]);

        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(trade_id_array),
                Arc::new(measure_array),
                Arc::new(measure_value_array),
                Arc::new(rel_shift_array),
                Arc::new(abs_shift_array),
            ],
        )?;
        Ok(batch)
    }

    pub fn price(
        &self,
        measures: Vec<String>,
        valuation_date: &PyDate,
        portfolio: &PyCell<Portfolio>,
        marketdata: &PyCell<MarketData>,
    ) -> PyResult<RecordBatch> {
        let vdt = Utc.ymd(
            valuation_date.get_year(),
            valuation_date.get_month() as u32,
            valuation_date.get_day() as u32,
        );
        let measure_vec_res: Result<Vec<Measure>, _> = measures
            .iter()
            .map(|m| Measure::parse_measure(m).map_err(CorePricerError::from))
            .collect();
        let measure_vec = measure_vec_res?;

        let md = marketdata.borrow();
        let mobs = md
            .observations
            .get(&vdt)
            .ok_or_else(|| PyRuntimeError::new_err(format!("No market data for {}", vdt)))?;
        let pctx = LookupCtx::new_from_prices_and_vols(vdt, &mobs.market_prices, &mobs.vols);

        let mut trade_id_vec = vec![];
        let mut measure_out_vec = vec![];
        let mut measure_value_vec = vec![];
        for (tid, pos) in portfolio.borrow().portfolio.positions_in_order() {
            for m in measure_vec.iter() {
                let x = pctx
                    .price_position(*m, pos)
                    .map_err(CorePricerError::from)?;
                trade_id_vec.push(tid.to_string());
                measure_out_vec.push(m.to_string());
                measure_value_vec.push(x);
            }
        }

        let trade_id_array = StringArray::from(trade_id_vec);
        let measure_array = StringArray::from(measure_out_vec);
        let measure_value_array = Float64Array::from(measure_value_vec);
        let schema = Schema::new(vec![
            Field::new("trade_id", DataType::Utf8, false),
            Field::new("measure", DataType::Utf8, false),
            Field::new("measure_value", DataType::Float64, false),
        ]);

        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(trade_id_array),
                Arc::new(measure_array),
                Arc::new(measure_value_array),
            ],
        )?;
        Ok(batch)
    }
}

impl Default for PricingEngine {
    fn default() -> Self {
        Self::new()
    }
}
