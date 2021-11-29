from setuptools_rust import Binding, RustExtension


def build(setup_kwargs):
    """
    This function is mandatory in order to build the extensions.
    """
    setup_kwargs.update(
        {
            "rust_extensions": [
                RustExtension(
                    "optpricerpy._optpricerpy",
                    path="./rs/Cargo.toml",
                    binding=Binding.PyO3,
                    debug=False,
                )
            ],
            "include_package_data": True,
            "zip_safe": False,
        }
    )
