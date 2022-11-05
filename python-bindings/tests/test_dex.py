from cwsimpy import Model
import json


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
    RPC_URL = "https://rpc.malaga-420.cosmwasm.com:443"
    RPC_BN = 2383460

    # user
    addr1 = "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj"
    # owner of factory/pair/token
    addr2 = "wasm1j5ad7ah3qte6tn9xnvvx6jlfm6uqsvxxqu5rfs"

    m = Model(RPC_URL, RPC_BN, "wasm")

    FACTORY_CODE_ID = 2385
    TOKEN_CODE_ID = 2386
    PAIR_CODE_ID = 2387

    m.cheat_message_sender(addr2)
    instantiate_msg = json.dumps(
        {
            "name": "DreamToken",
            "symbol": "DTK",
            "decimals": 6,
            "initial_balances": [{"address": addr2, "amount": str(10**20)}],
            "mint": {"minter": addr2, "cap": str(10**20)},
        }
    ).encode()

    res = m.instantiate(
        TOKEN_CODE_ID,
        instantiate_msg,
        [],
    )
    TOKEN_ADDR = get_contract_addr_from_instantiate_response(
        res.get_log(), TOKEN_CODE_ID
    )

    query_msg = json.dumps({"balance": {"address": addr2}}).encode()

    owner_balance = int(
        json.loads(bytearray(m.wasm_query(TOKEN_ADDR, query_msg)).decode("ascii"))[
            "balance"
        ]
    )
    assert owner_balance == 10**20

    instantiate_msg = json.dumps(
        {
            "pair_code_id": PAIR_CODE_ID,
            "token_code_id": TOKEN_CODE_ID,
        }
    ).encode()
    res = m.instantiate(
        FACTORY_CODE_ID,
        instantiate_msg,
        [],
    )
    FACTORY_ADDR = get_contract_addr_from_instantiate_response(
        res.get_log(), FACTORY_CODE_ID
    )

    query_msg = json.dumps({"config": {}}).encode()
    factory_owner = json.loads(
        bytearray(m.wasm_query(FACTORY_ADDR, query_msg)).decode("ascii")
    )["owner"]
    assert factory_owner == addr2

    execute_msg = json.dumps(
        {"add_native_token_decimals": {"denom": "umlg", "decimals": 6}}
    ).encode()
    res = m.execute(FACTORY_ADDR, execute_msg, [("umlg", 1)])
    assert res.get_err_msg() == ""

    execute_msg = json.dumps(
        {
            "create_pair": {
                "asset_infos": [
                    {"native_token": {"denom": "umlg"}},
                    {"token": {"contract_addr": TOKEN_ADDR}},
                ]
            }
        }
    ).encode()
    print(FACTORY_ADDR)
    print(TOKEN_ADDR)

    FACTORY_CODE_PATH = "/home/procfs/terraswap/target/wasm32-unknown-unknown/release/terraswap_factory.wasm"
    with open(FACTORY_CODE_PATH, "rb") as f:
        FACTORY_CODE = f.read()
    # m.cheat_code(FACTORY_ADDR, FACTORY_CODE)
    res = m.execute(FACTORY_ADDR, execute_msg, [])
    print(res.get_log())
    print(res.get_err_msg())
