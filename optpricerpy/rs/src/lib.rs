use crate::error::PyCorePricerError;
use pyo3::prelude::*;

use pyo3::wrap_pyfunction;

use crate::portfolio::Portfolio;
use crate::pricingengine::{
    FilterDef, MarketData, PricingEngine, ScenarioDefinition, ScenarioShift,
};
use arrow::array::{ArrayRef, StringArray};
use arrow::error::ArrowError;
use arrow::pyarrow::PyArrowConvert;

mod error;
mod portfolio;
mod pricingengine;

#[pyfunction]
fn load_equities(tickers: &PyAny, _py: Python) -> PyResult<Vec<String>> {
    // import
    let array = ArrayRef::from_pyarrow(tickers)?;
    // perform some operation
    let tickers = array
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| ArrowError::ParseError("Expects an str array".to_string()))?;
    let mut res = vec![];
    for ticker in tickers.iter().flatten() {
        res.push(ticker.to_string());
    }
    Ok(res)
}

#[pyclass]
struct ExampleClass {
    #[pyo3(get, set)]
    value: i32,
}

#[pymethods]
impl ExampleClass {
    #[new]
    pub fn new(value: i32) -> Self {
        ExampleClass { value }
    }
}

/// An example module implemented in Rust using PyO3.
#[pymodule]
fn _optpricerpy(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(load_equities))?;
    m.add_class::<ExampleClass>()?;
    m.add_class::<Portfolio>()?;
    m.add_class::<PricingEngine>()?;
    m.add_class::<MarketData>()?;
    m.add_class::<FilterDef>()?;
    m.add_class::<ScenarioShift>()?;
    m.add_class::<ScenarioDefinition>()?;
    m.add("PyCorePricerError", py.get_type::<PyCorePricerError>())?;
    Ok(())
}
