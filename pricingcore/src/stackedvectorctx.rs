use std::sync::Arc;

use crate::{
    error::PricerError,
    portfolio::Position,
    positioncalculator::PositionScaler,
    pricingctx::{Axis, InstrumentRef, Measure, PricingCtx, VectorResult, VectorizedPricingCtx},
    transform::{ShiftItem, TransformAlignment, TransformDefinition},
};
use ndarray::{Array, Dimension};

pub struct ShiftedExecutor {
    pub base_ctx: Arc<PricingCtx>,
    shifts: Vec<ShiftItem>,
}

impl ShiftedExecutor {
    fn new(base_ctx: &Arc<PricingCtx>, shifts: Vec<ShiftItem>) -> Self {
        Self {
            base_ctx: base_ctx.clone(),
            shifts,
        }
    }

    pub fn process(
        &self,
        measure: Measure,
        instrument: &InstrumentRef,
        v: f64,
    ) -> Result<f64, PricerError> {
        let mut v = v;
        v = self.base_ctx.process_shifts(measure, instrument, v)?;
        for si in self.shifts.iter() {
            if si.shift_target.is_target(measure) && si.instrument_filter.accept(instrument) {
                v = (1.0 + si.rel_shift) * v + si.abs_shift;
            }
        }
        Ok(v)
    }
}

pub struct StackedVectorizedPricingCtx {
    axis: Axis,
    base_ctx: Arc<PricingCtx>,
    parent: Option<Arc<StackedVectorizedPricingCtx>>,
    transform_def: TransformDefinition,
    axes: Vec<Axis>,
    shape: Vec<usize>,
}

impl StackedVectorizedPricingCtx {
    pub fn new(
        axis: Axis,
        base_ctx: &Arc<PricingCtx>,
        parent: Option<&Arc<StackedVectorizedPricingCtx>>,
        transform_def: TransformDefinition,
    ) -> Self {
        let axes = {
            let mut parent_axes = if let Some(p) = parent {
                Vec::from(p.axes())
            } else {
                vec![]
            };
            parent_axes.push(axis);
            parent_axes
        };
        let shape = axes.iter().map(|a| a.dim).collect::<Vec<usize>>();
        Self {
            axis,
            base_ctx: base_ctx.clone(),
            parent: parent.cloned(),
            transform_def,
            axes,
            shape,
        }
    }

    pub fn shift_base_ctx(
        ctx: &Arc<PricingCtx>,
        transform_def: TransformDefinition,
    ) -> Arc<StackedVectorizedPricingCtx> {
        let axis = Axis::new_base_axis(transform_def.len());
        Arc::new(StackedVectorizedPricingCtx::new(
            axis,
            ctx,
            None,
            transform_def,
        ))
    }

    pub fn shift_vector_ctx(
        ctx: &Arc<StackedVectorizedPricingCtx>,
        transform_def: TransformDefinition,
        alignment: TransformAlignment,
    ) -> Arc<StackedVectorizedPricingCtx> {
        let axis = match alignment {
            TransformAlignment::Stacked => ctx.axis(),
            TransformAlignment::Orthogonal => {
                Axis::new_from_parent(ctx.axis(), transform_def.len())
            }
        };
        Arc::new(StackedVectorizedPricingCtx::new(
            axis,
            &ctx.base_ctx,
            Some(ctx),
            transform_def,
        ))
    }

    fn fill_ctx_vec<'a, 'b: 'a>(&'b self, v: &mut Vec<&'a StackedVectorizedPricingCtx>) {
        if let Some(p) = &self.parent {
            p.fill_ctx_vec(v);
        }
        v.push(self);
    }

    fn ctx_vec<'a, 'b: 'a>(&'b self) -> Vec<&'a StackedVectorizedPricingCtx> {
        let mut v = vec![];
        self.fill_ctx_vec(&mut v);
        v
    }

    fn fill_shift_item_vec(
        ctxs: &[&StackedVectorizedPricingCtx],
        ax: Axis,
        i: usize,
        v: &mut Vec<ShiftItem>,
    ) -> Result<(), PricerError> {
        for ctx in ctxs.iter() {
            if ax == ctx.axis {
                let si = ctx.transform_def.shift_item_at(i)?;
                v.push(si);
            }
        }
        Ok(())
    }
}

impl VectorizedPricingCtx for StackedVectorizedPricingCtx {
    fn price_position(
        self: &Arc<Self>,
        measure: Measure,
        position: &Position,
    ) -> Result<VectorResult, PricerError> {
        let mut vectorres = self.price(measure, &InstrumentRef::from(&position.instrument))?;
        let position_scaler = PositionScaler::new(measure, position);
        vectorres.arr.mapv_inplace(|v| position_scaler.scale(v));
        Ok(vectorres)
    }

    fn price(
        self: &Arc<Self>,
        measure: Measure,
        instrument: &InstrumentRef,
    ) -> Result<VectorResult, PricerError> {
        let mut res = Array::from_elem(self.shape(), f64::NAN);
        let ctxv = self.ctx_vec();
        for (coord, v) in res.indexed_iter_mut() {
            let mut shifts = vec![];
            for (ax, i) in self.axes().iter().zip(coord.as_array_view()) {
                StackedVectorizedPricingCtx::fill_shift_item_vec(&ctxv, *ax, *i, &mut shifts)?;
            }
            let sliced_ctx = Arc::new(PricingCtx::ShiftedExecutor(ShiftedExecutor::new(
                &self.base_ctx,
                shifts,
            )));
            let x = sliced_ctx.price(measure, instrument)?;
            *v = x
        }
        let vres = VectorResult::new(res);
        Ok(vres)
    }

    fn axes(&self) -> &[Axis] {
        &self.axes
    }

    fn shape(&self) -> &[usize] {
        &self.shape
    }

    fn axis(&self) -> Axis {
        self.axis
    }
}

#[cfg(test)]
mod tests {

    use crate::error::PricerError;
    use crate::instrument::OptionType;
    use crate::pricingctx::VectorizedPricingCtx;
    use approx;
    use approx::assert_abs_diff_eq;

    use chrono::{TimeZone, Utc};
    use maplit::hashmap;
    use ndarray::array;

    use crate::transform::{InstrumentFilter, RiskFactor};
    use crate::{
        instrument::{Stock, StockOption, Ticker},
        lookupctx::LookupCtx,
        pricingctx::{InstrumentRef, Measure},
        stackedvectorctx::StackedVectorizedPricingCtx,
        transform::TransformDefinition,
    };

    #[test]
    fn price_call_shift_equites_up_down_vol_up_down_orto() -> Result<(), PricerError> {
        let stock = Stock::new(Ticker("AAPL".to_string()));
        let option = StockOption::new(stock, 100.0, Utc.ymd(2022, 8, 31), OptionType::Call);
        let prices = hashmap! {
            Ticker("AAPL".to_string()) => 100.0
        };
        let vols = hashmap! {
            Ticker("AAPL".to_string()) => 0.2,
        };
        let ctx = LookupCtx::new_from_prices_and_vols(Utc.ymd(2021, 8, 31), &prices, &vols);
        let mut vctx = StackedVectorizedPricingCtx::shift_base_ctx(
            &ctx,
            TransformDefinition::new_vector_measure_transform(
                Measure::Price,
                InstrumentFilter::RiskFactorFilter(RiskFactor::Equity),
                vec![0.0, 0.0, 0.0],
                vec![-0.05, 0.0, 0.05],
            )?,
        );
        vctx = StackedVectorizedPricingCtx::shift_vector_ctx(
            &vctx,
            TransformDefinition::new_vector_measure_transform(
                Measure::Vol,
                InstrumentFilter::Passthrough,
                vec![-0.1, 0.0, 0.2],
                vec![0.0, 0.0, 0.0],
            )?,
            crate::transform::TransformAlignment::Orthogonal,
        );
        let price = vctx.price(Measure::Price, &InstrumentRef::Option(&option))?;
        println!("{:?}", price);

        assert_abs_diff_eq!(
            price.arr,
            array![
                [1.8880632480607211, 5.519541063676968, 13.080826657108283],
                [3.987761167674492, 7.965567455405804, 15.851941887820608],
                [7.064019137898839, 10.905593471555477, 18.867330893095627],
            ]
            .into_dyn(),
        );
        Ok(())
    }

    #[test]
    fn price_call_shift_equites_up_down() -> Result<(), PricerError> {
        let stock = Stock::new(Ticker("AAPL".to_string()));
        let option = StockOption::new(stock, 100.0, Utc.ymd(2022, 8, 31), OptionType::Call);
        let prices = hashmap! {
            Ticker("AAPL".to_string()) => 100.0
        };
        let vols = hashmap! {
            Ticker("AAPL".to_string()) => 0.2,
        };
        let ctx = LookupCtx::new_from_prices_and_vols(Utc.ymd(2021, 8, 31), &prices, &vols);
        let transform_def = TransformDefinition::new_vector_measure_transform(
            Measure::Price,
            InstrumentFilter::RiskFactorFilter(RiskFactor::Equity),
            vec![0.0, 0.0, 0.0],
            vec![-0.05, 0.0, 0.05],
        )?;
        let vctx = StackedVectorizedPricingCtx::shift_base_ctx(&ctx, transform_def);
        let price = vctx.price(Measure::Price, &InstrumentRef::Option(&option))?;
        println!("{:?}", price);

        assert_abs_diff_eq!(
            price.arr,
            array![5.519541063676968, 7.965567455405804, 10.905593471555477].into_dyn(),
        );
        Ok(())
    }

    #[test]
    fn price_call_shift_uprice() -> Result<(), PricerError> {
        let stock = Stock::new(Ticker("AAPL".to_string()));
        let option = StockOption::new(stock, 100.0, Utc.ymd(2022, 8, 31), OptionType::Call);
        let prices = hashmap! {
            Ticker("AAPL".to_string()) => 100.0
        };
        let vols = hashmap! {
            Ticker("AAPL".to_string()) => 0.2,
        };
        let ctx = LookupCtx::new_from_prices_and_vols(Utc.ymd(2021, 8, 31), &prices, &vols);
        let transform_def = TransformDefinition::new_scalar_measure_transform(
            Measure::UnderlyingPrice,
            InstrumentFilter::Passthrough,
            0.0,
            0.05,
        )?;
        let vctx = StackedVectorizedPricingCtx::shift_base_ctx(&ctx, transform_def);
        let price = vctx.price(Measure::Price, &InstrumentRef::Option(&option))?;
        let price_delta = vctx.price(Measure::Delta, &InstrumentRef::Option(&option))?;
        assert_abs_diff_eq!(price.arr, array![10.905593471555477].into_dyn());
        assert_abs_diff_eq!(price_delta.arr, array![6.108762769889156].into_dyn());
        Ok(())
    }
}
