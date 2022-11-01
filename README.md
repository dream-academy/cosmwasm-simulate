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
maturin build
```

# TODO

- Gas calculation for querier/api.
- Currently, newly instantiated contract addresses are made arbitrarily. Fix this so that it matches the CosmWasm standard.
- Reason `reply` for instantiate failures.
- Currently, if bank messages trigger an error, it reverts the entire transaction, regardless of the presence of `reply`. Fix this.

# Usage

## Model Creation

```python
RPC_URL = "https://rpc.malaga-420.cosmwasm.com:443"
RPC_BN = 2326474
m = Model(RPC_URL, RPC_BN, "wasm")
```

## Contract Execution

```python
flashloan_msg = json.dumps(
    {
        "flash_loan": {
            "assets": [
                {"info": {"native_token": {"denom": "umlg"}}, "amount": "10000"}
            ],
            "msgs": [
                {
                    "bank": {
                        "send": {
                            "to_address": VAULT_ROUTER_ADDRESS,
                            "amount": [{"amount": "100", "denom": "umlg"}],
                        }
                    }
                }
            ],
        }
    }
).encode()
funds = [("umlg", 33)]
logs = m.execute(VAULT_ROUTER_ADDRESS, flashloan_msg, funds)
for x in logs.get_log():
    print(x)
print(logs.get_err_msg())
```

## Cheat Balance

Equivalent to `vm.deal` in foundry

```python
m.cheat_bank_balance(VAULET_ADDRESS, ("umlg", 10**9))
```

## Cheat Message Sender

Equivalent to `vm.startPrank` in foundry.

```python
m.cheat_message_sender("wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj")
```

## Cheat Block Number / Timestamp

Equivalent to `vm.warp`, `vm.roll` in foundry.

```python
m.cheat_block_number(20000)
m.cheat_block_timestamp(1000000)
```

## Cheat Code

Equivalent to `vm.etch` in foundry.

```python
with open(WASMFILE_PATH, "rb") as f:
    wasm_code = f.read()
m.cheat_code(PAIR_ADDR, wasm_code)
```

## Printing

Add the file below to the contract.

```rust
use cosmwasm_std::Deps;
use serde::{Deserialize, Serialize};

const PRINTER_ADDR: &str = "supergodprinter";

#[derive(Serialize, Deserialize)]
struct PrintRequest {
    msg: String,
}

#[derive(Serialize, Deserialize)]
struct PrintResponse {
    ack: bool,
}

pub fn print(deps: Deps, msg: &str) {
    let msg = PrintRequest {
        msg: msg.to_string(),
    };
    let _: PrintResponse = deps.querier.query_wasm_smart(PRINTER_ADDR, &msg).unwrap();
}
```

Then, invoke the print function in the contract as follows. Below is an example with TerraSwap's pair.

```rust
ask_pool = pools[1].clone();

offer_decimal = pair_info.asset_decimals[0];
ask_decimal = pair_info.asset_decimals[1];
print(deps.as_ref(), &format!("ask_pool(1): {}", ask_pool));
```

Afterwards, compile the contract and use `cheat_code` to overwrite existing code with the code that contains `print`s. Then, execute the contract and extract `stdout` from the debug log returned.

```python
logs = m.execute(PAIR_ADDR, swap_msg, [("umlg", 100)])
print("stdout1: {}".format(logs.get_stdout()))
```
