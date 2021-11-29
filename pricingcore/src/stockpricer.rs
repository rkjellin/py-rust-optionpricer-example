use std::sync::Arc;

use crate::{
    error::{MarketDataKind, PricerError},
    instrument::Stock,
    pricingctx::{InstrumentRef, Measure, PricingCtx},
};

fn price_stock_price(ctx: &Arc<PricingCtx>, stock: &Stock) -> Result<f64, PricerError> {
    if let Some(mprv) = ctx.market_price_provider(&InstrumentRef::Stock(stock)) {
        mprv.eval()
    } else {
        Err(PricerError::MissingMarketDataError {
            market_data_kind: MarketDataKind::Price,
            ticker: stock.ticker.clone(),
        })
    }
}

pub fn price_stock(
    ctx: &Arc<PricingCtx>,
    measure: Measure,
    stock: &Stock,
) -> Result<Option<f64>, PricerError> {
    match measure {
        Measure::Price => Ok(Some(price_stock_price(ctx, stock)?)),
        Measure::Exposure => Ok(Some(
            ctx.price(Measure::Price, &InstrumentRef::Stock(stock))?,
        )),
        _ => Ok(None),
    }
}
