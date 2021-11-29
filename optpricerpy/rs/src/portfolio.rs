use std::sync::Arc;

use crate::error::CorePricerError;
use arrow::{
    array::{Date64Array, Float64Array, StringArray, TimestampNanosecondArray},
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
    temporal_conversions::timestamp_ns_to_datetime,
};
use chrono::{Datelike, NaiveTime, TimeZone, Utc};
use pricingcore::{
    instrument::{Instrument, OptionType, Stock, StockOption, Ticker},
    portfolio::Portfolio as CorePortfolio,
};
use pyo3::prelude::*;

#[pyclass]
pub struct Portfolio {
    pub portfolio: CorePortfolio,
}

#[pymethods]
impl Portfolio {
    #[new]
    pub fn new() -> Self {
        let cport = CorePortfolio::new();
        Self { portfolio: cport }
    }

    pub fn equity_pos_len(&self) -> usize {
        self.portfolio.equity_pos_len()
    }

    pub fn option_trades_len(&self) -> usize {
        self.portfolio.option_trades_len()
    }

    pub fn position_ids(&self) -> Vec<String> {
        self.portfolio
            .position_ids
            .iter()
            .map(|pid| pid.to_string())
            .collect()
    }

    pub fn position_image(&self) -> PyResult<RecordBatch> {
        let posdatavec = self.portfolio.get_position_data();
        let trade_id_array = StringArray::from(
            posdatavec
                .position_id
                .iter()
                .map(|pid| pid.to_string())
                .collect::<Vec<String>>(),
        );
        let ticker_array = StringArray::from(
            posdatavec
                .ticker
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>(),
        );
        let strike_array = Float64Array::from(posdatavec.strike.clone());
        let expiry_array = Date64Array::from(
            posdatavec
                .expiry
                .iter()
                .map(|dtopt| {
                    dtopt
                        .and_then(|dt| dt.and_time(NaiveTime::from_num_seconds_from_midnight(0, 0)))
                        .map(|dt| dt.timestamp() * 1_000)
                })
                .collect::<Vec<Option<i64>>>(),
        );

        let opttypevec = posdatavec
            .option_type
            .iter()
            .map(|opttypeopt| opttypeopt.map(|opttype| opttype.to_string()))
            .collect::<Vec<Option<String>>>();
        let option_type_array = StringArray::from(
            opttypevec
                .iter()
                .map(Option::as_deref)
                .collect::<Vec<Option<&str>>>(),
        );
        let schema = Schema::new(vec![
            Field::new("trade_id", DataType::Utf8, false),
            Field::new("ticker", DataType::Utf8, false),
            Field::new("strike", DataType::Float64, true),
            Field::new("expiry", DataType::Date64, true),
            Field::new("option_type", DataType::Utf8, true),
        ]);

        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(trade_id_array),
                Arc::new(ticker_array),
                Arc::new(strike_array),
                Arc::new(expiry_array),
                Arc::new(option_type_array),
            ],
        )?;
        Ok(batch)
    }

    pub fn add_equity(&mut self, ticker: &str, size: f64) -> PyResult<()> {
        self.portfolio
            .add_trade(
                &Instrument::Stock(Stock::new(Ticker(ticker.to_string()))),
                size,
            )
            .map_err(CorePricerError::from)?;
        Ok(())
    }

    pub fn load_equity_trades(&mut self, trades: RecordBatch) -> PyResult<()> {
        let schema = trades.schema();
        let ticker_col = trades
            .column(schema.index_of("ticker")?)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| ArrowError::ParseError("Expects an str array".to_string()))?;

        let size_col = trades
            .column(schema.index_of("size")?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| ArrowError::ParseError("Expects an float array".to_string()))?;

        for i in 0..trades.num_rows() {
            let ticker = ticker_col.value(i);
            let size = size_col.value(i);
            let stock = Instrument::Stock(Stock::new(Ticker(ticker.to_string())));
            self.portfolio
                .add_trade(&stock, size)
                .map_err(CorePricerError::from)?;
        }
        Ok(())
    }

    pub fn load_option_trades(&mut self, trades: RecordBatch) -> PyResult<()> {
        let schema = trades.schema();
        let ticker_col = trades
            .column(schema.index_of("ticker")?)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| ArrowError::ParseError("Expects an str array".to_string()))?;

        let size_col = trades
            .column(schema.index_of("size")?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| ArrowError::ParseError("Expects an float array".to_string()))?;

        let strike_col = trades
            .column(schema.index_of("strike")?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| ArrowError::ParseError("Expects an float array".to_string()))?;

        let option_type_col = trades
            .column(schema.index_of("option_type")?)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| ArrowError::ParseError("Expects an str array".to_string()))?;

        let expiry_col = trades
            .column(schema.index_of("expiry")?)
            .as_any()
            .downcast_ref::<TimestampNanosecondArray>()
            .ok_or_else(|| ArrowError::ParseError("Expects a timestamp[ns] array".to_string()))?;

        for i in 0..trades.num_rows() {
            let ticker = ticker_col.value(i);
            let size = size_col.value(i);
            let strike = strike_col.value(i);
            let option_type = OptionType::parse_option_type(option_type_col.value(i))
                .map_err(CorePricerError::from)?;
            let expiry = timestamp_ns_to_datetime(expiry_col.value(i)).date();

            let option = Instrument::Option(StockOption::new(
                Stock::new(Ticker(ticker.to_string())),
                strike,
                Utc.ymd(expiry.year(), expiry.month(), expiry.day()), // perform a correct tz-aware conversion,
                option_type,
            ));
            self.portfolio
                .add_trade(&option, size)
                .map_err(CorePricerError::from)?;
        }
        Ok(())
    }
}

impl Default for Portfolio {
    fn default() -> Self {
        Self::new()
    }
}
