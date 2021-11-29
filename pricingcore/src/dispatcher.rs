use std::sync::Arc;

use crate::{
    error::PricerError,
    genericpricer::price_generic,
    optionpricer::price_option,
    pricingctx::{InstrumentRef, Measure, PricingCtx},
    stockpricer::price_stock,
};

pub fn dispatch_measure(
    ctx: &Arc<PricingCtx>,
    measure: Measure,
    instrument: &InstrumentRef,
) -> Result<f64, PricerError> {
    let m = match instrument {
        InstrumentRef::Option(option) => price_option(ctx, measure, option),
        InstrumentRef::Stock(stock) => price_stock(ctx, measure, stock),
    }?;
    if let Some(pr) = m {
        Ok(pr)
    } else {
        price_generic(ctx, measure, instrument)
    }
}
