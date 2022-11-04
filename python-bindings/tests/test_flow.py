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

    CODE_PATH = "/home/procfs/cosmwasm-simulate/target/wasm32-unknown-unknown/release/callee.wasm"
    with open(CODE_PATH, "rb") as f:
        code = f.read()
    msg = json.dumps(
        {
            "flow": {}
        }
    ).encode()
    m.cheat_code(PAIR_ADDR, code)
    logs = m.execute(PAIR_ADDR, msg, [])
    print(logs.get_log())
    print(logs.get_err_msg())