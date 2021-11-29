use rand::distributions::Distribution;
use statrs::distribution::Exp;

mod dispatcher;
pub mod error;
mod genericpricer;
pub mod instrument;
pub mod lookupctx;
mod marketdataproviders;
mod model;
mod optionpricer;
pub mod portfolio;
mod positioncalculator;
pub mod pricingctx;
pub mod stackedvectorctx;
mod stockpricer;
pub mod transform;

pub fn gen_rand_pricingcore() -> f64 {
    let mut rng = rand::thread_rng();
    let n = Exp::new(0.5).unwrap();
    n.sample(&mut rng)
}
