use statrs::distribution::{ContinuousCDF, Normal};

use crate::instrument::OptionType;

use super::modelerror::ModelError;

#[derive(Debug)]
pub struct BlackScholesParams {
    pub spot: f64,
    pub strike: f64,
    pub tte: f64,
    pub r: f64,
    pub vol: f64,
}

impl BlackScholesParams {
    pub fn new(spot: f64, strike: f64, tte: f64, r: f64, vol: f64) -> Self {
        Self {
            spot,
            strike,
            tte,
            r,
            vol,
        }
    }
}

fn call_price(d1: f64, d2: f64, pars: BlackScholesParams) -> Result<f64, ModelError> {
    let n = Normal::new(0.0, 1.0).expect("failed to make normal dist");
    let p = pars.spot * n.cdf(d1) - pars.strike * f64::exp(-pars.r * pars.tte) * n.cdf(d2);
    Ok(p)
}

fn put_price(d1: f64, d2: f64, pars: BlackScholesParams) -> Result<f64, ModelError> {
    let n = Normal::new(0.0, 1.0).expect("failed to make normal dist");
    let p = pars.strike * f64::exp(-pars.r * pars.tte) * n.cdf(-d2) - pars.spot * n.cdf(-d1);
    Ok(p)
}

pub fn black_scholes(pars: BlackScholesParams, option_type: OptionType) -> Result<f64, ModelError> {
    let log_sk = f64::ln(pars.spot / pars.strike);
    let scaled_vol = pars.vol * f64::sqrt(pars.tte);
    let d1 = (log_sk + (pars.r + 0.5 * f64::powi(pars.vol, 2)) * pars.tte) / scaled_vol;
    let d2 = (log_sk + (pars.r - 0.5 * f64::powi(pars.vol, 2)) * pars.tte) / scaled_vol;
    match option_type {
        OptionType::Call => call_price(d1, d2, pars),
        OptionType::Put => put_price(d1, d2, pars),
    }
}

#[cfg(test)]
mod tests {
    use crate::model::modelerror::ModelError;

    use super::{black_scholes, BlackScholesParams, OptionType};
    use approx::assert_abs_diff_eq;

    #[test]
    fn atm_call() -> Result<(), ModelError> {
        let pars = BlackScholesParams::new(100.0, 100.0, 1.0, 0.0, 0.2);
        let p = black_scholes(pars, OptionType::Call)?;
        assert_abs_diff_eq!(p, 7.965567455405804);
        Ok(())
    }
}
