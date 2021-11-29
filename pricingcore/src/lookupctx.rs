use std::{collections::HashMap, sync::Arc};

use chrono::{Date, Utc};

use crate::{
    instrument::Ticker,
    marketdataproviders::{
        DatePeriodProvider, FixedValuationDatePeriodProvider, FixedVolSurface,
        FixedZeroRateProvider, MarketPriceProvider, RateProvider, SinglePriceProvider, VolProvider,
    },
    pricingctx::{InstrumentRef, PricingCtx},
};

pub struct LookupCtx {
    vol_providers: HashMap<Ticker, Arc<dyn VolProvider>>,
    market_price_providers: HashMap<Ticker, Arc<dyn MarketPriceProvider>>,
    rate_provider: Arc<dyn RateProvider>, // should be mapped per currency in real world
    date_period_provider: Arc<dyn DatePeriodProvider>,
}

impl LookupCtx {
    pub fn new_from_prices_and_vols(
        valuation_date: Date<Utc>,
        market_prices: &HashMap<Ticker, f64>,
        vols: &HashMap<Ticker, f64>,
    ) -> Arc<PricingCtx> {
        let mprvs: HashMap<Ticker, Arc<dyn MarketPriceProvider>> = market_prices
            .iter()
            .map(|(ticker, price)| {
                (
                    ticker.clone(),
                    Arc::new(SinglePriceProvider::new(*price)) as Arc<dyn MarketPriceProvider>,
                )
            })
            .collect();

        let vprvs: HashMap<Ticker, Arc<dyn VolProvider>> = vols
            .iter()
            .map(|(ticker, vol)| {
                (
                    ticker.clone(),
                    Arc::new(FixedVolSurface::new(*vol)) as Arc<dyn VolProvider>,
                )
            })
            .collect();

        Arc::new(PricingCtx::LookupExecutor(Self {
            vol_providers: vprvs,
            market_price_providers: mprvs,
            rate_provider: Arc::new(FixedZeroRateProvider {}),
            date_period_provider: Arc::new(FixedValuationDatePeriodProvider::new(valuation_date)),
        }))
    }

    pub fn new_market_price_only(
        valuation_date: Date<Utc>,
        market_prices: &HashMap<Ticker, f64>,
    ) -> Arc<PricingCtx> {
        LookupCtx::new_from_prices_and_vols(valuation_date, market_prices, &HashMap::new())
    }

    pub fn vol_provider(&self, instrument: &InstrumentRef) -> Option<Arc<dyn VolProvider>> {
        let ticker = match instrument {
            InstrumentRef::Option(opt) => &opt.underlying.ticker,
            InstrumentRef::Stock(stock) => &stock.ticker,
        };
        self.vol_providers.get(ticker).cloned()
    }

    pub fn market_price_provider(
        &self,
        instrument: &InstrumentRef,
    ) -> Option<Arc<dyn MarketPriceProvider>> {
        let ticker = match instrument {
            InstrumentRef::Option(opt) => &opt.underlying.ticker,
            InstrumentRef::Stock(stock) => &stock.ticker,
        };
        self.market_price_providers.get(ticker).cloned()
    }

    pub fn ir_provider(&self, _instrument: &InstrumentRef) -> Option<Arc<dyn RateProvider>> {
        Some(self.rate_provider.clone())
    }

    pub fn period_provider(
        &self,
        _instrument: &InstrumentRef,
    ) -> Option<Arc<dyn DatePeriodProvider>> {
        Some(self.date_period_provider.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::PricerError,
        instrument::{OptionType, Stock, StockOption, Ticker},
        lookupctx::LookupCtx,
        pricingctx::{InstrumentRef, Measure},
    };

    use approx::assert_abs_diff_eq;
    use chrono::{TimeZone, Utc};
    use maplit::hashmap;

    #[test]
    fn measure_roundtrip() -> Result<(), PricerError> {
        let tte = Measure::TimeToExpiry;
        let tte_str = tte.to_string();
        assert_eq!(tte_str, "time_to_expiry");
        let tte_rt = Measure::parse_measure(&tte_str)?;
        assert_eq!(tte, tte_rt);
        Ok(())
    }

    #[test]
    fn price_stock() -> Result<(), PricerError> {
        let stock = Stock::new(Ticker("AAPL".to_string()));
        let prices = hashmap! {
            Ticker("AAPL".to_string()) => 105.0
        };
        let ctx = LookupCtx::new_market_price_only(Utc.ymd(2021, 8, 31), &prices);
        let price = ctx.price(Measure::Price, &InstrumentRef::Stock(&stock))?;
        let price_delta = ctx.price(Measure::Delta, &InstrumentRef::Stock(&stock))?;
        let price_gamma = ctx.price(Measure::Gamma, &InstrumentRef::Stock(&stock))?;
        assert_abs_diff_eq!(price, 105.0);
        assert_abs_diff_eq!(price_delta, 1.0, epsilon = 0.0001);
        assert_abs_diff_eq!(price_gamma, 0.0, epsilon = 0.0001);
        Ok(())
    }

    #[test]
    fn price_call() -> Result<(), PricerError> {
        let stock = Stock::new(Ticker("AAPL".to_string()));
        let option = StockOption::new(stock, 100.0, Utc.ymd(2022, 8, 31), OptionType::Call);
        let prices = hashmap! {
            Ticker("AAPL".to_string()) => 100.0
        };
        let vols = hashmap! {
            Ticker("AAPL".to_string()) => 0.2,
        };
        let ctx = LookupCtx::new_from_prices_and_vols(Utc.ymd(2021, 8, 31), &prices, &vols);
        let price = ctx.price(Measure::Price, &InstrumentRef::Option(&option))?;
        let price_delta = ctx.price(Measure::Delta, &InstrumentRef::Option(&option))?;
        let price_gamma = ctx.price(Measure::Gamma, &InstrumentRef::Option(&option))?;
        assert_abs_diff_eq!(price, 7.965567455405804);
        assert_abs_diff_eq!(price_delta, 6.776394149722663);
        assert_abs_diff_eq!(price_gamma, 3.1275067301709396);
        Ok(())
    }
}
