#!/bin/sh
grcov -b artifacts/test_contract_cov.o -s . -t html -o cov_report