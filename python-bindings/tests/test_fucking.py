from cwsimpy import Model
import json
import string
import base64


def get_token_balance(m, addr):
    token_balance = json.dumps({"balance": {"address": addr}}).encode()

    res = bytearray(m.wasm_query(wooz3k_token, token_balance)).decode("ascii")
    return res


def get_bank_balance(m, addr):
    token_balance = json.dumps({"balance": {"address": addr, "denom": "umlg"}}).encode()

    res = bytearray(m.bank_query(token_balance)).decode("ascii")
    return res


def to_binary(msg):
    return base64.b64encode(msg.encode()).decode("ascii")


if __name__ == "__main__":
    RPC_URL = "https://rpc.malaga-420.cosmwasm.com:443"
    RPC_BN = None
    VAULT_ROUTER_ADDRESS = (
        "wasm18wcf03ytrdskpp9ujs2egtv764e7vlxeaesn9x7efyaawaegxhpq2qv29c"
    )

    wooz3k_token = "wasm1qjc0zatks55gxyyvljjl8umrp7v0hvxpay6ef03467xhme9kwy6s7um8lt"
    wooz3k_pair = "wasm1taueqyzzeq27tr5mh50e2nsxqu55tut88u5rpl099fc65njqa8wq57ygnk"
    wooz3k_factory = "wasm1gzs0yla3tftkrexp8rdpk3s66m0d9kz0qluslcd04mgzyx2mhtnsuq0wv8"
    lptoken_addr = "wasm1rkhcftp36ap9hk4d2kcn6jp7gdfrgk5mj5re3p4nz8he3q2eqlpsnx9m54"
    wooz3k_router = "wasm1lyey5durmd0mv7md0z0ke8xs9sdzq84qghv4x82g3gsd0pyfpyrqd6jkwe"

    my_addr = "wasm1j5ad7ah3qte6tn9xnvvx6jlfm6uqsvxxqu5rfs"

    jot_bob = "wasm1w20htrmu36l2lvrxf2an54h2gwlwr3ydz50exd"

    m = Model(RPC_URL, RPC_BN, "wasm")
    m.cheat_message_sender(my_addr)

    # filename = "/mnt/c/Users/wooz3k/Desktop/terraswap/artifacts/terraswap_router.wasm"
    # with open(filename, "rb") as f:
    #    wasm_code = f.read()
    # m.cheat_code(wooz3k_router, wasm_code)

    print(
        "before pair token amount : {}".format(  # router swap logic test
            get_token_balance(m, wooz3k_pair)
        )
    )
    print("before pair bank amount : {}".format(get_bank_balance(m, wooz3k_pair)))

    token_transfer = json.dumps(
        {"transfer": {"recipient": jot_bob, "amount": "999000"}}
    ).encode()

    res = m.execute(wooz3k_token, token_transfer, [])

    m.cheat_message_sender(jot_bob)

    token_transfer = json.dumps(
        {"transfer": {"recipient": wooz3k_router, "amount": "999000"}}
    ).encode()

    # res = m.execute(wooz3k_token, token_transfer, [])

    m.cheat_message_sender(my_addr)

    sub_msg = base64.b64encode(
        json.dumps(
            {
                "execute_swap_operations": {
                    "operations": [
                        {
                            "terra_swap": {
                                "offer_asset_info": {
                                    "token": {"contract_addr": wooz3k_token}
                                },
                                "ask_asset_info": {"native_token": {"denom": "umlg"}},
                            }
                        }
                    ],
                    "minimum_receive": "100",
                }
            }
        ).encode()
    ).decode()

    send_msg = json.dumps(
        {
            "send": {
                "contract": wooz3k_router,
                "amount": "1",
                "msg": sub_msg,
            }
        }
    ).encode()

    res = m.execute(wooz3k_token, send_msg, [("umlg", 100)])
    print(res.get_log())

    print("after pair token amount : {}".format(get_token_balance(m, wooz3k_pair)))
    print("after pair bank amount : {}".format(get_bank_balance(m, wooz3k_pair)))
