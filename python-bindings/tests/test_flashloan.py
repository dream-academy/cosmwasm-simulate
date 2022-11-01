from cwsimpy import Model
import json

if __name__ == "__main__":
    RPC_URL = "https://rpc.malaga-420.cosmwasm.com:443"
    RPC_BN = 2326474
    VAULT_ROUTER_ADDRESS = (
        "wasm1xp8prmlsx9erdkrk43qjtrw54755zwm9f4x52m8k3an6jgcaldpqpmsd23"
    )
    VAULET_ADDRESS = "wasm1fedmcgtsvmymyr6jssgar0h7uhhcuxhr7ygjjw5q2epgzef3jy0svcr5jx"

    my_addr = "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj"
    m = Model(RPC_URL, RPC_BN, "wasm")
    m.cheat_bank_balance(VAULET_ADDRESS, ("umlg", 10**9))

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
