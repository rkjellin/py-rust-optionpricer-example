from dataclasses import dataclass
from datetime import date
from enum import Enum, unique
from typing import Optional, Union

import numpy as np
import pandera as pan
import pandera.typing as pat
import pyarrow as pa
import xarray as xr

import optpricerpy._optpricerpy as _opt
from optpricerpy.portfolio import Portfolio


class ScalarPricingResultSchema(pan.SchemaModel):
    """Schema describing a scalar/non-scenario pricing result."""

    trade_id: pat.Index[str] = pan.Field(coerce=True, nullable=False)
    valuation_date: pat.Index[pat.DateTime] = pan.Field(coerce=True, nullable=False)
    measure: pat.Index[str] = pan.Field(coerce=True, nullable=False)
    measure_value: pat.Series[float] = pan.Field(coerce=True, nullable=True)

    class Config:
        multiindex_strict = True
        multiindex_coerce = True


class VectorPricingResultScheme(pan.SchemaModel):
    """Schema describing pricing result vectorized over 1 axis."""

    trade_id: pat.Series[str] = pan.Field(coerce=True, nullable=False)
    valuation_date: pat.Series[pat.DateTime] = pan.Field(coerce=True, nullable=False)
    measure: pat.Series[str] = pan.Field(coerce=True, nullable=False)
    rel_shift: pat.Series[float] = pan.Field(coerce=True, nullable=True)
    abs_shift: pat.Series[float] = pan.Field(coerce=True, nullable=True)
    measure_value: pat.Series[float] = pan.Field(coerce=True, nullable=True)


class MarketDataSchema(pan.SchemaModel):
    """Schema describing data format for loading market data from python."""

    ticker: pat.Index[str] = pan.Field(check_name=True, coerce=True, nullable=False)
    date: pat.Index[pat.DateTime] = pan.Field(
        check_name=True, coerce=True, nullable=False
    )
    spot: pat.Series[float] = pan.Field(coerce=True, nullable=False)
    vol: pat.Series[float] = pan.Field(coerce=True, nullable=False)

    class Config:
        multiindex_strict = True
        multiindex_coerce = True


class MarketData:
    """
    A MarketData object acts as a store for individual market data items
    such as volatilities or spot prices.
    """

    core_marketdata: _opt.MarketData

    def __init__(self) -> None:
        self.core_marketdata = _opt.MarketData()

    def __str__(self) -> str:
        return str(self.core_marketdata)

    @pan.check_types
    def load_market_data(self, market_data: pat.DataFrame[MarketDataSchema]):
        """Batch-load market data."""
        pa_df = pa.Table.from_pandas(market_data)
        for batch in pa_df.to_batches():
            self.core_marketdata.load_market_data(batch)


@unique
class Measure(str, Enum):
    PRICE = "price"
    UNDERLYING_PRICE = "underlying_price"
    VOL = "vol"
    EXPOSURE = "exposure"


@unique
class RiskFactorFilter(str, Enum):
    """A specific risk factor."""

    EQUITY = "equity"


@dataclass
class TickerFilter:
    ticker: str


@unique
class Alignment(str, Enum):
    """
    The Alignment enum determines of a scenario shift should introduce
    a new axis or be applied on top of the previous shift.
    """

    STACKED = "stacked"
    ORTHOGONAL = "orthogonal"


@dataclass
class ScenarioShift:
    """
    A ScenarioShift defines a vectorized shift as a series of relative
    and absolute shifts applied to the `target_measure`.
    The `target_filter` is used to limit the set of target nodes affected.
    """

    target_measure: Measure
    target_filter: Optional[Union[RiskFactorFilter, TickerFilter]]
    rel_shifts: list[float]
    abs_shifts: list[float]
    name: Optional[str] = None

    def axis_name(self) -> str:
        return self.name or f"shift_{self.target_measure}"


@dataclass
class ScenarioDefinition:
    shifts: list[tuple[ScenarioShift, Alignment]]

    def axes(self) -> list[str]:
        res = []
        for i, (si, align) in enumerate(self.shifts):
            if i == 0 or align == Alignment.ORTHOGONAL:
                res.append(si.axis_name())
        return res


def _target_filter_2_core(
    target_filter: Optional[Union[RiskFactorFilter, TickerFilter]]
) -> Optional[_opt.FilterDef]:
    if not target_filter:
        return _opt.FilterDef.new_passthrough()
    if isinstance(target_filter, RiskFactorFilter):
        return _opt.FilterDef("risk_factor_filter", target_filter.value)
    elif isinstance(target_filter, TickerFilter):
        return _opt.FilterDef("ticker_filter", target_filter.ticker)
    else:
        raise NotImplementedError(target_filter)


def _scenario_shift_2_core(shift: ScenarioShift) -> _opt.ScenarioShift:
    insfilter = _target_filter_2_core(shift.target_filter)
    return _opt.ScenarioShift(
        shift.target_measure, insfilter, shift.abs_shifts, shift.rel_shifts
    )


def _scenario_def_2_core(
    scenario_def: ScenarioDefinition,
) -> _opt.ScenarioDefinition:
    shifts = []
    for (shift, alignment) in scenario_def.shifts:
        core_shift = _scenario_shift_2_core(shift)
        is_ortho = alignment == Alignment.ORTHOGONAL
        shifts.append((core_shift, is_ortho))
    return _opt.ScenarioDefinition(shifts)


class PricingEngine:
    """
    A pricing engine defines an entry point for issuing pricing
    requests on a portfolio.
    """

    core_pricing_engine: _opt.PricingEngine

    def __init__(self) -> None:
        self.core_pricing_engine = _opt.PricingEngine()

    @pan.check_types
    def price_portfolio(
        self,
        measures: list[Measure],
        valuation_dates: list[date],
        portfolio: Portfolio,
        market_data: MarketData,
    ) -> pat.DataFrame[ScalarPricingResultSchema]:
        """
        Issue a pricing request for a list of regular pricing measures per a
        specific `valuation_date`.
        """
        pa_df = self.core_pricing_engine.price(
            measures,
            valuation_dates,
            portfolio.core_portfolio,
            market_data.core_marketdata,
        )
        return pa_df.to_pandas().set_index(["trade_id", "valuation_date", "measure"])

    @pan.check_types
    def price_portfolio_ladder_scenario(
        self,
        measures: list[Measure],
        valuation_dates: list[date],
        portfolio: Portfolio,
        market_data: MarketData,
        ladder_definition: ScenarioShift,
    ) -> pat.DataFrame[VectorPricingResultScheme]:
        """
        Issue a pricing request for a list of pricing measures per a
        specific series of `valuation_dates` shifted by the `ladder_definition`.
        """
        pa_df = self.core_pricing_engine.price_ladder(
            measures,
            valuation_dates,
            portfolio.core_portfolio,
            market_data.core_marketdata,
            _scenario_shift_2_core(ladder_definition),
        )
        return pa_df.to_pandas()

    def price_portfolio_scenario(
        self,
        measures: list[Measure],
        valuation_dates: list[date],
        portfolio: Portfolio,
        market_data: MarketData,
        scenario_definition: ScenarioDefinition,
    ) -> xr.Dataset:
        """
        Issue a pricing request for a list of pricing measures per a
        specific series of `valuation_dates` shifted by the `scenario_definition`.

        The method returns an xarray dataset with a valuation_date dimension,
        trade_id dimension and 1 additional dimension per axis in the scenario.
        """
        position_ids = portfolio.position_ids()
        axes = scenario_definition.axes()
        axes.insert(0, "trade_id")
        axes.insert(0, "valuation_date")
        arrs = {
            measure.value: (
                axes,
                self.core_pricing_engine.price_portfolio_scenario(
                    measure,
                    valuation_dates,
                    portfolio.core_portfolio,
                    market_data.core_marketdata,
                    _scenario_def_2_core(scenario_definition),
                ),
            )
            for measure in measures
        }
        return xr.Dataset(
            arrs,
            coords={"valuation_date": valuation_dates, "trade_id": position_ids},
        )

    def price_portfolio_2d_matrix_scenario(
        self,
        measures: list[Measure],
        valuation_dates: list[date],
        portfolio: Portfolio,
        market_data: MarketData,
        *,
        x_shift: ScenarioShift,
        y_shift: ScenarioShift,
    ) -> xr.Dataset:
        """
        Issue a pricing request for a list of pricing measures per a
        specific series of `valuation_dates`, shifted in 2 dimensions defined by
        `x_shift` and `y_shift` respectively.

        The method returns an xarray dataset with a valuation_date dimension,
        trade_id dimension and 2 additional scenario dimensions.
        """
        scenario_def = ScenarioDefinition(
            shifts=[(x_shift, Alignment.ORTHOGONAL), (y_shift, Alignment.ORTHOGONAL)]
        )
        return self.price_portfolio_scenario(
            measures, valuation_dates, portfolio, market_data, scenario_def
        )
