use std::io::Write;

use rust_decimal::Decimal;
use serde::Serialize;

use crate::domain::{Account, ClientId};
use crate::engine::PaymentEngine;

#[derive(Debug, Serialize, PartialEq)]
pub struct AccountRow {
    client: ClientId,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

impl AccountRow {
    pub fn from_account(client: ClientId, account: &Account) -> Self {
        Self {
            client,
            available: account.available(),
            held: account.held(),
            total: account.total(),
            locked: account.is_locked(),
        }
    }
}

pub fn write_accounts<W: Write>(engine: &PaymentEngine, writer: W) -> csv::Result<()> {
    let mut csv = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(writer);

    csv.write_record(["client", "available", "held", "total", "locked"])?;

    for (client, account) in engine.accounts() {
        csv.serialize(AccountRow::from_account(client, account))?;
    }

    csv.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rust_decimal::dec;
    use serde::Deserialize;

    use super::*;
    use crate::domain::Command;

    #[derive(Debug, Deserialize, PartialEq)]
    struct ParsedAccountRow {
        client: ClientId,
        available: Decimal,
        held: Decimal,
        total: Decimal,
        locked: bool,
    }

    #[test]
    fn empty_engine_writes_header() {
        let engine = PaymentEngine::new();
        let mut output = Vec::new();

        write_accounts(&engine, &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "client,available,held,total,locked\n"
        );
    }

    #[test]
    fn writes_account_balances_as_csv() {
        let mut engine = PaymentEngine::new();
        engine.process(Command::Deposit {
            client: 1,
            tx: 1,
            amount: dec!(10),
        });

        let mut output = Vec::new();
        write_accounts(&engine, &mut output).unwrap();

        let mut reader = csv::Reader::from_reader(output.as_slice());
        let rows = reader
            .deserialize::<ParsedAccountRow>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            rows,
            vec![ParsedAccountRow {
                client: 1,
                available: dec!(10),
                held: Decimal::ZERO,
                total: dec!(10),
                locked: false,
            }]
        );
    }

    #[test]
    fn preserves_four_decimal_places_in_csv_output() {
        let mut engine = PaymentEngine::new();
        engine.process(Command::Deposit {
            client: 1,
            tx: 1,
            amount: dec!(12.3456),
        });

        let mut output = Vec::new();
        write_accounts(&engine, &mut output).unwrap();

        let mut reader = csv::Reader::from_reader(output.as_slice());
        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(&record[1], "12.3456");
        assert_eq!(&record[3], "12.3456");
    }
}
