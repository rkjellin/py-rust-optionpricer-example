use std::sync::Arc;

use crate::{
    error::PricerError,
    pricingctx::{InstrumentRef, Measure, PricingCtx, VectorizedPricingCtx},
    stackedvectorctx::StackedVectorizedPricingCtx,
    transform::{InstrumentFilter, RiskFactor, TransformDefinition},
};

fn price_equity_delta(
    ctx: &Arc<PricingCtx>,
    instrument: &InstrumentRef,
) -> Result<f64, PricerError> {
    let transform_def = TransformDefinition::new_vector_measure_transform(
        Measure::Price,
        InstrumentFilter::RiskFactorFilter(RiskFactor::Equity),
        vec![0.0, 0.0, 0.0],
        vec![-0.01, 0.0, 0.01],
    )?;
    let vcxt = StackedVectorizedPricingCtx::shift_base_ctx(ctx, transform_def);
    let vres = vcxt.price(Measure::Price, instrument)?;
    if let Some([vdown, v, vup]) = vres.resultview_1d()?.as_slice() {
        if *v == 0.0 {
            if f64::abs(*vup - *vdown) < f64::EPSILON {
                return Ok(0.0);
            }
            return Err(PricerError::ShiftExecutionError {
                ins: instrument.into(),
                message: "zero price in relshift".to_string(),
            });
        }
        let h = v * 0.01;
        Ok((vup - vdown) / (2.0 * h))
    } else {
        Err(PricerError::ShiftExecutionError {
            ins: instrument.into(),
            message: "unknown scenario dimension".to_string(),
        })
    }
}

fn price_equity_gamma(
    ctx: &Arc<PricingCtx>,
    instrument: &InstrumentRef,
) -> Result<f64, PricerError> {
    let transform_def = TransformDefinition::new_vector_measure_transform(
        Measure::Price,
        InstrumentFilter::RiskFactorFilter(RiskFactor::Equity),
        vec![0.0, 0.0, 0.0],
        vec![-0.01, 0.0, 0.01],
    )?;
    let vcxt = StackedVectorizedPricingCtx::shift_base_ctx(ctx, transform_def);
    let vres = vcxt.price(Measure::Price, instrument)?;
    if let Some([vdown, v, vup]) = vres.resultview_1d()?.as_slice() {
        if *v == 0.0 {
            if f64::abs(*vup - *vdown) < f64::EPSILON {
                return Ok(0.0);
            }
            return Err(PricerError::ShiftExecutionError {
                ins: instrument.into(),
                message: "zero price in relshift".to_string(),
            });
        }
        let h = v * 0.01;
        Ok((vup - 2.0 * v + vdown) / f64::powf(h, 2.0))
    } else {
        Err(PricerError::ShiftExecutionError {
            ins: instrument.into(),
            message: "unknown scenario dimension".to_string(),
        })
    }
}

pub fn price_generic(
    ctx: &Arc<PricingCtx>,
    measure: Measure,
    instrument: &InstrumentRef,
) -> Result<f64, PricerError> {
    match measure {
        Measure::Delta => price_equity_delta(ctx, instrument),
        Measure::Gamma => price_equity_gamma(ctx, instrument),
        _ => Err(PricerError::MissingCalculatorError(measure)),
    }
}
