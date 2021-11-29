from pathlib import Path

import pandas as pd

from optpricerpy import Portfolio

_example_equity_trades_path = Path(__file__).parent / "example_equity_trades.csv"
_example_option_trades_path = Path(__file__).parent / "example_option_trades.csv"


def test_create_portfolio():
    p = Portfolio()
    p.add_equity("AAPL", 10)
    p.add_equity("AAPL", 20)
    assert p.nbr_equity_positions == 1
    assert p.nbr_option_trades == 0


def test_load_trades():
    df = pd.read_csv(_example_equity_trades_path)
    p = Portfolio()
    p.load_equity_trades(df)
    assert p.nbr_equity_positions == 1
    assert p.nbr_option_trades == 0


def test_option_trades():
    df = pd.read_csv(_example_option_trades_path)
    p = Portfolio()
    p.load_option_trades(df)
    assert p.nbr_equity_positions == 0
    assert p.nbr_option_trades == 3


def test_combination_portfolio():
    p = Portfolio()
    p.load_option_trades(pd.read_csv(_example_option_trades_path))
    p.load_equity_trades(pd.read_csv(_example_equity_trades_path))
    assert p.nbr_equity_positions == 1
    assert p.nbr_option_trades == 3


def test_position_image():
    p = Portfolio()
    p.load_option_trades(pd.read_csv(_example_option_trades_path))
    p.load_equity_trades(pd.read_csv(_example_equity_trades_path))
    df = p.position_image()
    print()
    print(df)
