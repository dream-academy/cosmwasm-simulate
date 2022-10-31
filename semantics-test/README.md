# CW-Semantics-Test
This contract is used to test the submessage semantics of CosmWasm.

# Flow
The main contract is `caller`, and it interacts with multiple `callee`s. Also, `callee`s can interact with other `callee`s as well. The implementation of `caller` and `callee` tests nested submessages extensively.

# Caller
## Instantiation
It instantiates 5 callees.

## Execution
`caller` implements methods called `caller_call`_{*} where * is the testcase number. For each testcase, it sends testcases to `callee`s.

## Reply
Upon reply, it logs the reply value as well as success/failure.

# Callee
## Instantiation
Sets instance id (a number)

## Execution
The API prototype is `callee_call(Option<(String, Binary)>)`. If the argument is `None`, it does nothing. If it is not `None`, it calls another `callee` where its address is `String` and the message is `Binary`.

## Reply
Upon reply, it logs the reply value as well as success/failure.