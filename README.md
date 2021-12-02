# OptionPricer: python-rust interop case study [WIP] #

[This readme is very much a work-in-progress. The code needs some finishing
touches but can serve as inspiration while this document is being finalized.]

`OptionPricer` is a small example/case study project intending to show how to
do practical language interop between python and rust for data-centric 
workloads.

`OptionPricer` as it's name suggests implements a toy pricing engine for 
financial options in rust. This choice is due to the nature of the required
calculations which are most easily expressed as a recursive-descent evaluation 
of a tree of financial components. This setup does not easily lend itself to 
the kind of homogenous calculation flows that are naturally expressible in 
`numpy`. It is also tedious to implement this in something like `numba` due 
to the limited support for high level data structures and recursions 
(though this perception might stem from a lack of familiarity with `numba`s 
capabilities by the author). 

## Motivation ##
There are many tutorials and examples on the web showing how to do language 
interop between python and some lower level language. Often these examples
shows how to call small functions such as the fibonacci function or string
manipulation functions in the target language. While these examples perfectly
shows how to technically perform the interop the author found a gap
in the available materials when it comes to how to structure a green field 
native extension that plays nice with a modern data workflow using libraries
such as `pandas` or `xarray` and a notebook-centric iterative developement. 

### Target audience ###
The target audience for this material is the working data scientist or quant
feeling frustration with the performance of python. It is not perfect guide for
writing polished native extensions distributed to end-users (but may very well
be a good starting point.)

Over time, many python users feel a naggin doubt that the choice of language for
their workload was the wrong one. Either due to (1.) the lack of handholding the 
language offers in terms of compiler safety or (2.) due to performance issues.

While rewriting in something like julia is possible, the REPL-based development
workflow is not always suitable or wanted. Julia has a great ecosystem for many
data tasks but lack the breadth of the python ecosystem and working with python
libraries via PyCall is not an entirely frictionless activity when it comes to
project management and code reuse between julia and python proper.

Rewriting in something like dotnet or java allows for high performance but at 
the high cost of losing the tried and tested ecosystem of python. There are 
attempts on both the jvm and in dotnet to offer dataframe abstractions but none
are as powerful or easy to use as pandas. The same goes for ndarray packages and
more. 

What this project attempts to show is that a mixed language solution is both
feasible and practical. Using a powerful language like rust allows the user to
express solutions in complex analytical domains using high level constructs
while retaining readability and type safety.

## Acknowledgement ##
There is not much novel in this example other than showing and motivating the 
complete end-to-end functionality. It stands on the shoulders of many wonderful
OSS projects such as `pyo3`, `apache arrow`, `rayon`, `xarray` and many more.

A special shoutout goes to the amazing `pandera` library. `pandera` makes 
working with dataframes a joy due to the powerful validations you can apply to
your api boundaries. Taking dataframes as arguments or returning them from an 
api is usually a very tedious task in terms of testing, validations and
documentation. `pandera` streamlines this process by allowing you to explicitly
state your data schemas upfront and securing the shape of data in and out of
your functions. 

## Organization ##
This project is made up by two parts. The actual pricing engine is developed
as a rust crate at [pricingcore](pricingcore/).
The python extension module is developed in [optpricerpy](optpricerpy/) and
uses a mixture of python and rust gluecode using the `pyo3` interop crate. The 
rust glue acts as a bridge between python and the calculation engine.  

## Pricing primer ##
The point of the project is not a innovate or do incredible things with regards
to asset pricing. Rather, the pricing is just a fitting example of some
computation that fulfils the following characteristics:
 1. expensive enough that doing the computation in pure python would be 
    prohibitly time consuming
 2. complex enough that just expressing it vectorized using `pandas`/`numpy`
    would be tedious or downright impossible

### High level aim ###
The aim of the project in terms of functionality is the ability to price a 
mixed portfolio of equity options and equites. By price I mean calculate
relevant metrics for the positions in the portfolio, such as the 
`net present value`, `equity exposure` or sensitivities such as `delta` or
`gamma`. 

In addition to these metrics, the module also allows the pricing to be 
transformed by different scenarios applied upon underlying parameters. 
The simplest such scenario would be what is called a ladder. An example of a
ladder would be to calculate for instance `equity exposure` while applying a 
vector of shifts to some underlying 
parameter such as `underlying_price`. Shifting the input underlying price up and
down 5% using the relative shift vector `[-0.05, 0.05]` would return an
`equity exposure` result of the same dimensionality as the applied shift.  

In addition to these simple 1d ladders, the engine allows shifts to be applied 
either stacked upon each other or orthogonally. 
 * An example of a stacked shift
could be a crash scenario, where shifts are applied individually to the
underlying stocks (and other risk factors) to create or recreate a movement in
the market (such as the 2008 crash or the pandemic) and see how it affects the 
portfolio. 
 * An example of an orthogonal shift would be a "ladder" in two dimensions
 such as shifting underlying prices using relative shifts in one axis while at 
 the same time shifting volatility using absolute percentage point shifts in 
 another. The end result would be a 2d matrix showing the change in metric value
 of the portfolio for different combinations of price and volatility 
 perturbations. 

The user may stack or orthogonally combine shifts freely as long as the 
dimensions align. That is, each orthogonal axis can be made up of arbitrary
many layers of stacked shifts. 

### Options ###
An option contract gives the holder the right but not the obligation to 
excercise some predetermined transaction in the future. This project concerns
itself with the absolute simplest text-book variants of european equity call 
and put options. 

A european call option on a given stock gives the holder the right but 
not the obligation to buy the stock at a predetermined price (the strike, `K`)
at some point in the future (the excercise date `T`). If the price of the stock
at time `T` is higher than `K` then it is worthwhile to exercise the option.

A european put option on the other hand gives the holder the right to sell a 
stock at a given price `K` at time `T`.  

For more information on options and financial instruments the reader is adviced
to study [Hull](https://www.goodreads.com/book/show/100827.Options_Futures_and_Other_Derivatives). If one can stomach a little bit more 
mathematics [Shreve](https://www.goodreads.com/book/show/232559.Stochastic_Calculus_Models_for_Finance_II?ac=1&from_search=true&qid=ZNNHAZhlOF&rank=1)
is wonderful.

### Option pricing and metrics ###
The price of an option can be calculated based on mathematical models (see the
links in the preceeding section for details). The model uses input data from 
the market and spits what price must hold given the assumptions of the model. 
This is somewhat different from machine learning models for instance, where we 
are only interested in prediction of some target variable. The financial models
painstakingly details a series of assumptions on how the market works and 
derives a unique price that must hold under those assumptions.

The most famous pricing model is the Black-Scholes model and it is this model 
we will use in the pricing engine. 

## Installation ##
The project is easily installed for consumption. A recent stable rust compiler
is all that is required (currently developed using `1.56.1`). Simply issue 
```
pip install .
```
in `<REPOPATH>/optpricerpy` with a virtualenv activated. This requires a recent
version of `pip`.

For development the project uses [poetry](https://python-poetry.org/). Make sure
that a recent version of poetry is installed. In a terminal navigate to

```
<REPOPATH>/optpricerpy
```

and issue the command 

```
poetry install
```

This will 
 1. create a local virtualenv in `<REPOPATH>/optpricerpy/.venv`
 2. install all required python dependencies, including development dependencies
 3. build the rust extension module inplace

To make sure everything works issue

```
poetry run pytest
```

in order to run the unittests.
Run

```
poetry run tox
```
to run code formatting, typechecking and unittests. 

A nice development experience is using vscode with the [python/pylance](https://marketplace.visualstudio.com/items?itemName=ms-python.python)
extension for python and [rust-analyzer](https://rust-analyzer.github.io/) for 
the rust parts. Opening `<REPOPATH>` for editing allows for simultaneous
development of both the python and the rust parts. 

## Usage guide ##
TODO
(In the meantime look at the example in the [notebooks](notebooks/))
