# cosmwasm-simulate
Simulation tool of Cosmwasm smart contract

# Dependencies
```shell script
apt-get install protobuf-compiler
apt-get install python3-dev
pip install maturin
```

# Build
```shell script
cd python-bindings
maturin build
```

# Installation
```shell script
pip install target/wheels/cswimpy-0.1.0-cp38-cp38-manylinux_2_28_x86_64.whl
```
- The name of the actual `*.whl` file may differ by libpython version or OS, so check the full path via `ls target/wheels`.
- If `pip install` doesn't work, try `pip install --update pip`.

# TODO
- Gas calculation for querier/api
- Currently, newly instantiated contract addresses are made arbitrarily. Fix this so that it matches the CosmWasm standard.
- Reason `reply` for instantiate failures
- Currently, if bank messages trigger an error, it reverts the entire transaction, regardless of the presence of `reply`. Fix this.
- Implement atmoic commits.