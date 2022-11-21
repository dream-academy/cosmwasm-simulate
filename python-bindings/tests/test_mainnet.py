from cwsimpy import Model
import json
import base64

if __name__ == "__main__":
    RPC_URL = "http://51.81.155.97:26657"
    m = Model("RPC", RPC_URL, None, "terra")
    print(m)
