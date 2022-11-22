from cwsimpy import Model
import json
import base64


def to_binary(msg):
    return base64.b64encode(json.dumps(msg).encode()).decode("ascii")


def decode_vec(v):
    v = bytearray(v)
    return v.decode("utf-8")


def test_swap():
    RPC_URL = "https://rpc.malaga-420.cosmwasm.com:443"
    RPC_BN = 2326474
    PAIR_ADDR = "wasm15le5evw4regnwf9lrjnpakr2075fcyp4n4yzpelvqcuevzkw2lss46hslz"

    m = Model(RPC_URL, RPC_BN, "wasm")
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
    logs = m.execute(PAIR_ADDR, swap_msg, [("umlg", 100)])


def get_contract_addr_from_instantiate_response(log, code_id):
    for e in log:
        events = json.loads(e)["events"]
        for event in events:
            if event["type"] == "instantiate":
                o = {x["key"]: x["value"] for x in event["attributes"]}
                if o["code_id"] == str(code_id):
                    return o["_contract_address"]
    raise Exception("not found")


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
    m.enable_code_coverage()

    CODE_PATH = "/home/procfs/cosmwasm-simulate/target/wasm32-unknown-unknown/release/test_contract_cov.wasm"
    with open(CODE_PATH, "rb") as f:
        code = f.read()
    m.add_custom_code(1337, code)

    imsg = json.dumps({}).encode()
    res = m.instantiate(1337, imsg, [])
    contract_addr = get_contract_addr_from_instantiate_response(res.get_log(), 1337)

    covs = res.get_code_coverage_for_address(contract_addr)
    with open("cov.profraw", "wb") as f:
        f.write(bytearray(covs[0]))
