from cwsimpy import Model
import json
import base64

if __name__ == "__main__":
    RPC_URL = "http://5.9.66.60:26657"
    FACTORY_ADDR = "terra1466nf3zuxpya8q9emxukd7vftaf6h4psr0a07srl5zw74zh84yjqxl5qul"
    ROUTER_ADDR = "terra13ehuhysn5mqjeaheeuew2gjs785f6k7jm8vfsqg3jhtpkwppcmzqcu7chk"
    m = Model(RPC_URL, 2540362, "terra")
    msg = json.dumps(
        {
            "pairs": {
                "start_after": None,
                "limit": None,
            }
        }
    ).encode()
    res = m.wasm_query(FACTORY_ADDR, msg)
    print(bytearray(res).decode("utf-8"))
