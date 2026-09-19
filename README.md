# Payments Engine

A basic implementation of a payments engine that processes transactions from a CSV file and outputs account balances to another CSV file. The engine supports deposits, withdrawals, disputes, resolutions, and chargebacks.

## Usage

To run the payments engine, use the following command:

```bash
cargo run -- input_file.csv > output_file.csv
```

## Testing

The project contains unit, integration and e2e tests. To run all tests, use the following command:

```bash
cargo test
```

## Assumptions

- Only successfully processed deposits can enter the dispute lifecycle. Withdrawals are not disputable.

- Dispute, resolve, and chargeback operations are ignored when the referenced transaction belongs to a different client.

- Repeated disputes for a transaction that is already disputed or charged back are ignored.

- A resolved deposit may be disputed again.

- Once an account is locked, all later commands for that account are ignored. This also means that another active dispute may remain held after a chargeback locks the account.

## Design decisions

- Amounts are stored using `Decimal` to avoid floating-point precision issues. We could store them as scaled integers (where 1 represents 0.0001) to avoid the overhead of `Decimal`, but this would require additional parsing and formatting logic. I would only consider changing this if profiling showed that decimal operations were a real bottleneck.

- The `total` balance is not stored directly but is calculated as `available + held`.

- The CSV is processed one row at a time instead of loading an entire file into memory. Processing takes `O(n)` time and `O(c + t)` memory, where `c` is the number of clients and `t` is the number of successful transactions.

-  I process commands from a single CSV sequentially because their order can change the result. Although different clients are independent, the work performed for each row is small, so adding threads, queues, and synchronization would probably add more complexity than useful performance.

- A dispute always holds the full original deposit amount. If part of that deposit was already withdrawn, the available balance may become negative. This represents funds that the client spent before the deposit was reversed.

## Follow-ups

- If this engine was used with many concurrent sources (e.g. TCP streams), I would process all operations for the same client sequentially, while still processing different clients in parallel. For instance, a fixed (configurable) number of sharded workers seems like a good approach. Ofc we do not want one worker per client, because app may have millions of clients. I would also use bounded queues to prevent uncontrolled memory growth.

- The current engine stores successful withdrawals even though only deposits can be disputed. Memory usage could be reduced by retaining only deposit records. If duplicate payment IDs still needed to be detected, a separate `HashSet<TransactionId>` could store the IDs without keeping complete withdrawal records.
