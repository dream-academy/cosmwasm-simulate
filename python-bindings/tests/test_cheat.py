from cwsimpy import Model
import json
import base64


def to_binary(msg):
    return base64.b64encode(json.dumps(msg).encode()).decode("ascii")


def decode_vec(v):
    v = bytearray(v)
    return v.decode("utf-8")


if __name__ == "__main__":
    FACTORY_ADDR = "wasm1hczjykytm4suw4586j5v42qft60gc4j307gf7cxuazfg7jxt4h4sjvp7rx"
    TOKEN_ADDR = "wasm124v54ngky9wxhx87t252x4xfgujmdsu7uhjdugtkkqt39nld0e6st7e64h"
    PAIR_ADDR = "wasm15le5evw4regnwf9lrjnpakr2075fcyp4n4yzpelvqcuevzkw2lss46hslz"
    LPTOKEN_ADDR = "wasm147ntaasx8mcx6a8jk7cvpyvus8r80garfnue4qrzrl0whk9ftntqpld03t"
    MY_ADDRESS = "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj"

    RPC_URL = "https://rpc.malaga-420.cosmwasm.com:443"
    RPC_BN = 2326474

    m = Model(RPC_URL, RPC_BN, "wasm")
    m.cheat_message_sender(MY_ADDRESS)

    WASMFILE_PATH = "/home/procfs/terraswap/target/wasm32-unknown-unknown/release/terraswap_pair.wasm"
    with open(WASMFILE_PATH, "rb") as f:
        wasm_code = f.read()
    m.cheat_code(PAIR_ADDR, wasm_code)

    balance_query_msg = json.dumps({"balance": {"address": MY_ADDRESS}}).encode()
    swap_msg = json.dumps(
        {
            "swap": {
                "offer_asset": {
                    "info": {"native_token": {"denom": "umlg"}},
                    "amount": "100",
                },
                "belief_price": None,
                "max_spread": None,
                "to": None,
            }
        }
    ).encode()
    bal1 = int(
        json.loads(decode_vec(m.query(TOKEN_ADDR, balance_query_msg)))["balance"]
    )
    logs = m.execute(PAIR_ADDR, swap_msg, [("umlg", 100)])
    print("stdout1: {}".format(logs.get_stdout()))
    bal2 = int(
        json.loads(decode_vec(m.query(TOKEN_ADDR, balance_query_msg)))["balance"]
    )
    print("got tokens: {}".format(bal2 - bal1))

    swap_msg = json.dumps(
        {
            "send": {
                "contract": PAIR_ADDR,
                "amount": "10",
                "msg": to_binary(
                    {
                        "swap": {
                            "belief_price": None,
                            "max_spread": None,
                            "to": MY_ADDRESS,
                        }
                    }
                ),
            }
        }
    ).encode()
    logs = m.execute(TOKEN_ADDR, swap_msg, [])
    print("stdout2: {}".format(logs.get_stdout()))
    bal3 = int(
        json.loads(decode_vec(m.query(TOKEN_ADDR, balance_query_msg)))["balance"]
    )
    print("spent tokens: {}".format(bal2 - bal3))
