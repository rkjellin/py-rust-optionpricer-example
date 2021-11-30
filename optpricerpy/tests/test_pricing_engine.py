from datetime import date
from pathlib import Path

import pandas as pd
from pandas.testing import assert_frame_equal

from optpricerpy.portfolio import Portfolio
from optpricerpy.pricing_engine import (
    Alignment,
    MarketData,
    Measure,
    PricingEngine,
    RiskFactorFilter,
    ScenarioDefinition,
    ScenarioShift,
)

_market_data_path = Path(__file__).parent / "example_marketdata.csv"
_example_equity_trades_path = Path(__file__).parent / "example_equity_trades.csv"
_example_option_trades_path = Path(__file__).parent / "example_option_trades.csv"
_expected_pricing_result = Path(__file__).parent / "expected_pricing_result.csv"


def test_load_marketdata():
    df = pd.read_csv(_market_data_path).set_index(["ticker", "date"])
    md = MarketData()
    md.load_market_data(df)


def test_pricing_engine_price():
    df = pd.read_csv(_market_data_path).set_index(["ticker", "date"])
    md = MarketData()
    md.load_market_data(df)

    p = Portfolio()
    p.load_option_trades(pd.read_csv(_example_option_trades_path))
    p.load_equity_trades(pd.read_csv(_example_equity_trades_path))

    pricing_engine = PricingEngine()
    df = pricing_engine.price_portfolio([Measure.PRICE], date(2021, 12, 1), p, md)
    assert_frame_equal(
        df,
        pd.read_csv(_expected_pricing_result).set_index(["trade_id", "measure"]),
        check_like=True,
    )


def test_pricing_engine_portfolio_ladder():
    df = pd.read_csv(_market_data_path).set_index(["ticker", "date"])
    md = MarketData()
    md.load_market_data(df)

    p = Portfolio()
    p.load_option_trades(pd.read_csv(_example_option_trades_path))
    p.load_equity_trades(pd.read_csv(_example_equity_trades_path))

    pricing_engine = PricingEngine()
    arr = pricing_engine.price_portfolio_ladder_scenario(
        [Measure.PRICE],
        date(2021, 12, 1),
        p,
        md,
        ScenarioShift(
            Measure.VOL,
            None,
            rel_shifts=[-0.05, 0, 0.05],
            abs_shifts=[0.0, 0.0, 0.0],
        ),
    )
    print()
    print(arr)


def test_pricing_engine_portfolio_2d_scenario():
    df = pd.read_csv(_market_data_path).set_index(["ticker", "date"])
    md = MarketData()
    md.load_market_data(df)

    p = Portfolio()
    p.load_option_trades(pd.read_csv(_example_option_trades_path))
    p.load_equity_trades(pd.read_csv(_example_equity_trades_path))

    pricing_engine = PricingEngine()
    arr = pricing_engine.price_portfolio_2d_matrix_scenario(
        [Measure.PRICE],
        date(2021, 12, 1),
        p,
        md,
        x_shift=ScenarioShift(
            Measure.PRICE,
            RiskFactorFilter.EQUITY,
            rel_shifts=[-0.05, 0, 0.05],
            abs_shifts=[0.0, 0.0, 0.0],
        ),
        y_shift=ScenarioShift(
            Measure.VOL,
            None,
            rel_shifts=[-0.05, 0, 0.05],
            abs_shifts=[0.0, 0.0, 0.0],
        ),
    )
