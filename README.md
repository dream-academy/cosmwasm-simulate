# cosmwasm-simulate
Simulation tool of Cosmwasm smart contract

# Dependencies
```shell script
apt-get install protobuf-compiler
```

# Build
```shell script
cargo build
```

# TODO
- Gas calculation for querier/api
- Currently, newly instantiated contract addresses are made arbitrarily. Fix this so that it matches the CosmWasm standard.
- Reason `reply` for instantiate failures