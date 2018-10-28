# Blockchain gateway

This microservice handles communication with Bitcoin and Ethereum nodes

## Dependencies (not implemented yet)

- RabbitMQ
- Bitcoind
- Parity Ethereum


# Caveats
1. Currently, when there are many stq transfers in one tx, this ether tx fee is allocated to each transfer.
This doesn't affect our system, as we only care for our witdrawal tx fees, which are always 1 to 1.

2. Stq tx fees are in ether (wei) units.
