from cwsimpy import Model
import json
import sys
import os
import base64
import itertools
from graphviz import Digraph

TERRASWAP_FACTORY_ADDR = (
    "terra1466nf3zuxpya8q9emxukd7vftaf6h4psr0a07srl5zw74zh84yjqxl5qul"
)
TOKEN_CODE_ID = 4
OWNER = "terra10s4ujq0m8sfmtgp9v4035g4rhwn22h3ygv5dg0"

RPC_URL = "http://5.9.66.60:26657"
RPC_BN = 2554297


def get_contract_addr_from_instantiate_response(log, code_id):
    for e in log:
        events = json.loads(e)["events"]
        for event in events:
            if event["type"] == "instantiate":
                o = {x["key"]: x["value"] for x in event["attributes"]}
                if o["code_id"] == str(code_id):
                    return o["_contract_address"]
    raise Exception("not found")


def get_contract_addr_from_create_pair_response(log):
    for e in log:
        attributes = json.loads(e)["attributes"]
        for attribute in attributes:
            if attribute["key"] == "pair_contract_addr":
                pair_contract_addr = attribute["value"]
            elif attribute["key"] == "liquidity_token_addr":
                liquidity_token_addr = attribute["value"]
    return pair_contract_addr, liquidity_token_addr


def get_factory_pair_check_response(log):
    pair_addr = json.loads(log)["contract_addr"]
    liquidity_addr = json.loads(log)["liquidity_token"]
    return pair_addr, liquidity_addr


def prettify_call_trace(call_graph, call_graph_labels, dir_path):
    d = Digraph()
    nodes = set()
    for src in call_graph:
        nodes.add(src)
        for dst in call_graph[src]:
            nodes.add(dst)
    for node in nodes:
        d.node(str(node), call_graph_labels[node])
        if node in call_graph:
            for dst in call_graph[node]:
                d.edge(str(node), str(dst))
    d.format = "svg"
    d.render(directory=dir_path)


class Test:
    def __init__(self):
        self.m = Model(RPC_URL, RPC_BN, "terra")
        self.m.cheat_message_sender(OWNER)

    def add_pair(self, token1_addr, token2_addr):
        execute_msg = json.dumps(
            {
                "create_pair": {
                    "asset_infos": [
                        {"token": {"contract_addr": token1_addr}},
                        {"token": {"contract_addr": token2_addr}},
                    ]
                }
            }
        ).encode()
        res = self.m.execute(TERRASWAP_FACTORY_ADDR, execute_msg, [])
        return res

    def create_token(self, token_name, token_symbol):
        instantiate_msg = json.dumps(
            {
                "name": token_name,
                "symbol": token_symbol,
                "decimals": 6,
                "initial_balances": [{"address": OWNER, "amount": str(2**128 - 1)}],
                "mint": {"minter": OWNER, "cap": str(2**128 - 1)},
            }
        ).encode()
        res = self.m.instantiate(TOKEN_CODE_ID, instantiate_msg, [])
        return get_contract_addr_from_instantiate_response(res.get_log(), TOKEN_CODE_ID)


def testFactoryAddPairs():
    t = Test()
    token_addrs = []
    token_addr1 = t.create_token("token", "TKZ")
    print("[+] New token created: {}".format(token_addr1))
    token_addr2 = t.create_token("token", "TKZ")
    print("[+] New token created: {}".format(token_addr2))

    res = t.add_pair(token_addr1, token_addr2)
    pair_addr, lptoken_addr = get_contract_addr_from_create_pair_response(res.get_log())
    print("[+] New pair created: {}".format(pair_addr))

    call_graph, call_graph_labels = res.get_call_trace()
    prettify_call_trace(call_graph, call_graph_labels, "callgraphs")


if __name__ == "__main__":
    testFactoryAddPairs()
