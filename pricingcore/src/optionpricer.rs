use std::sync::Arc;

use crate::{
    error::PricerError,
    instrument::StockOption,
    model::bsmodel::{black_scholes, BlackScholesParams},
    pricingctx::{InstrumentRef, Measure, PricingCtx},
};

fn price_option_price(ctx: &Arc<PricingCtx>, option: &StockOption) -> Result<f64, PricerError> {
    let insref = InstrumentRef::Option(option);
    let undprice = ctx.price(Measure::UnderlyingPrice, &insref)?;
    let rate = ctx.price(Measure::Rate, &insref)?;
    let vol = ctx.price(Measure::Vol, &insref)?;
    let tte = ctx.price(Measure::TimeToExpiry, &insref)?;
    let price = black_scholes(
        BlackScholesParams::new(undprice, option.strike, tte, rate, vol),
        option.option_type,
    )?;
    Ok(price)
}

fn price_option_underlying_price(
    ctx: &Arc<PricingCtx>,
    option: &StockOption,
) -> Result<f64, PricerError> {
    let underlying_ref = InstrumentRef::Stock(&option.underlying);
    ctx.price(Measure::Price, &underlying_ref)
}

fn price_option_exposure(ctx: &Arc<PricingCtx>, option: &StockOption) -> Result<f64, PricerError> {
    let instrument = InstrumentRef::Option(option);
    let uprice = ctx.price(Measure::UnderlyingPrice, &instrument)?;
    let delta = ctx.price(Measure::Delta, &instrument)?;
    Ok(uprice * delta)
}

pub fn price_option(
    ctx: &Arc<PricingCtx>,
    measure: Measure,
    option: &StockOption,
) -> Result<Option<f64>, PricerError> {
    let insref = InstrumentRef::Option(option);
    let pr = match measure {
        Measure::Price => price_option_price(ctx, option),
        Measure::Exposure => price_option_exposure(ctx, option),
        Measure::UnderlyingPrice => price_option_underlying_price(ctx, option),
        Measure::Vol => {
            if let Some(vp) = ctx.vol_provider(&insref) {
                let spot = ctx.price(Measure::UnderlyingPrice, &insref)?;
                let tte = ctx.price(Measure::TimeToExpiry, &insref)?;
                vp.eval(spot, option.strike, tte)
            } else {
                Err(PricerError::InfrastructureError(
                    "No vol provider found".to_string(),
                ))
            }
        }
        Measure::Rate => {
            if let Some(irp) = ctx.ir_provider(&InstrumentRef::Option(option)) {
                irp.eval()
            } else {
                Err(PricerError::InfrastructureError(
                    "No rate provider found".to_string(),
                ))
            }
        }
        Measure::TimeToExpiry => {
            if let Some(dtp) = ctx.period_provider(&InstrumentRef::Option(option)) {
                dtp.period_in_year(option.expiry)
            } else {
                Err(PricerError::InfrastructureError(
                    "No perdio provider found".to_string(),
                ))
            }
        }
        _ => return Ok(None),
    }?;
    Ok(Some(pr))
}
