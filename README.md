# Payments Engine

A basic implementation of a payments engine that processes transactions from a CSV file and outputs account balances to another CSV file. The engine supports deposits, withdrawals, disputes, resolutions, and chargebacks.

## Assumptions

- Only successfully processed deposits can enter the dispute lifecycle. Withdrawals are not disputable.

- Dispute, resolve, and chargeback operations are ignored when the referenced transaction belongs to a different client.

- Repeated disputes for a transaction that is already disputed or charged back are ignored.

-  A resolved deposit may be disputed again.