from enum import unique
from typing import Annotated

import pandas as pd
import pandera as pan
import pandera.typing as pat
import pyarrow as pa

import optpricerpy._optpricerpy as _opt


class EquityTradeSchema(pan.SchemaModel):
    """Schema describing required stock trade data"""

    ticker: pat.Series[str] = pan.Field(coerce=True, nullable=False)
    size: pat.Series[float] = pan.Field(coerce=True, nullable=False)


class OptionTradeSchema(pan.SchemaModel):
    """Schema describing required option trade data"""

    ticker: pat.Series[str] = pan.Field(coerce=True, nullable=False)
    size: pat.Series[float] = pan.Field(coerce=True, nullable=False)
    strike: pat.Series[float] = pan.Field(coerce=True, nullable=False)
    option_type: pat.Series[pat.Category] = pan.Field(
        coerce=True, nullable=True, dtype_kwargs={"categories": ["call", "put"]}
    )

    expiry: pat.Series[pat.DateTime] = pan.Field(coerce=True, nullable=False)


class PositionImageSchema(pan.SchemaModel):
    """Schema describing a position image"""

    trade_id: pat.Index[str] = pan.Field(
        check_name=True, coerce=True, nullable=False, unique=True
    )
    ticker: pat.Series[str] = pan.Field(coerce=True, nullable=False)
    strike: pat.Series[float] = pan.Field(coerce=True, nullable=True)
    expiry: pat.Series[pat.DateTime] = pan.Field(coerce=True, nullable=True)
    option_type: pat.Series[pat.Category] = pan.Field(
        coerce=True, nullable=True, dtype_kwargs={"categories": ["call", "put"]}
    )

    class Config:
        coerce = True
        strict = True


class Portfolio:
    """
    A portfolio is an object carrying a number of positions.
    """

    core_portfolio: _opt.Portfolio

    def __init__(self) -> None:
        self.core_portfolio = _opt.Portfolio()

    def position_ids(self) -> list[str]:
        return self.core_portfolio.position_ids()

    @property
    def nbr_equity_positions(self) -> int:
        return self.core_portfolio.equity_pos_len()

    @property
    def nbr_option_trades(self) -> int:
        return self.core_portfolio.option_trades_len()

    def __str__(self) -> str:
        return f"Portfolio[nbr_equity_positions={self.nbr_equity_positions}, nbr_option_trades={self.nbr_option_trades}]"

    def add_equity(self, ticker: str, size: float):
        self.core_portfolio.add_equity(ticker, size)

    @pan.check_types
    def position_image(self) -> pat.DataFrame[PositionImageSchema]:
        """Return the full position image in a cross-asset wide data set."""
        return self.core_portfolio.position_image().to_pandas().set_index("trade_id")

    @pan.check_types
    def load_equity_trades(self, trades: pat.DataFrame[EquityTradeSchema]):
        """Batch-load a number of stock trades into the portfolio."""
        pa_trades = pa.Table.from_pandas(trades)
        for batch in pa_trades.to_batches():
            self.core_portfolio.load_equity_trades(batch)

    @pan.check_types
    def load_option_trades(self, trades: pat.DataFrame[OptionTradeSchema]):
        """
        Batch-load a number of option trades into the portfolio. Currently option trades
        are considered OTC and each trade will result in a unique position in the portfolio.
        """

        # Categorical dictionary array type is lost in rust arrow conversion.
        # Use naive string representation in the meantime. Much more expensive than u8 etc but
        # trade loading is a small cost compared to pricing.
        trades["option_type"] = trades["option_type"].astype(str)
        pa_trades = pa.Table.from_pandas(trades)
        for batch in pa_trades.to_batches():
            self.core_portfolio.load_option_trades(batch)
