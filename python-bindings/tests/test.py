from cwsimpy import Model
import json

if __name__ == "__main__":
    RPC_URL = "https://rpc.malaga-420.cosmwasm.com:443"
    RPC_BN = 2326474
    VAULT_ROUTER_ADDRESS = (
        "wasm1xp8prmlsx9erdkrk43qjtrw54755zwm9f4x52m8k3an6jgcaldpqpmsd23"
    )

    m = Model(RPC_URL, RPC_BN, "wasm")

    flashloan_msg = json.dumps(
        {
            "flash_loan": {
                "assets": [
                    {"info": {"native_token": {"denom": "umlg"}}, "amount": "5000"}
                ],
                "msgs": [],
            }
        }
    ).encode()
    funds = []
    logs = m.execute(VAULT_ROUTER_ADDRESS, flashloan_msg, funds)
    print(logs.get_log())
    print(logs.get_err_msg())
