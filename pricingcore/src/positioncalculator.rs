use crate::{
    portfolio::Position,
    pricingctx::{Measure, PositionScaling},
};

pub struct PositionScaler<'a> {
    measure: Measure,
    position: &'a Position,
}

impl<'a> PositionScaler<'a> {
    pub fn new(measure: Measure, position: &'a Position) -> Self {
        Self { measure, position }
    }

    pub fn scale(&self, v: f64) -> f64 {
        let scale_factor = match self.measure.position_scaling() {
            PositionScaling::NoScaling => 1.0,
            PositionScaling::Linear => self.position.size,
        };
        scale_factor * v
    }
}
