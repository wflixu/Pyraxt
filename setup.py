import sys

sys.stderr.write(
    """
===============================
Unsupported installation method
===============================
axon doesn't support installation with `python setup.py install`.
Please use `python -m pip install .` instead.
"""
)
sys.exit(1)
