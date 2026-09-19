use std::collections::HashMap;

use payments_engine::{AppError, CommandError, run};
use rust_decimal::{Decimal, dec};
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct AccountOutput {
    client: u16,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

fn run_to_bytes(input: &str) -> Vec<u8> {
    let mut output = Vec::new();
    run(input.as_bytes(), &mut output).unwrap();
    output
}

fn run_to_accounts(input: &str) -> HashMap<u16, AccountOutput> {
    let output = run_to_bytes(input);
    let mut reader = csv::Reader::from_reader(output.as_slice());

    reader
        .deserialize::<AccountOutput>()
        .map(|row| {
            let row = row.unwrap();
            (row.client, row)
        })
        .collect()
}

#[test]
fn processes_deposits_and_withdrawals_for_multiple_clients() {
    let accounts = run_to_accounts(
        "type,client,tx,amount\n\
         deposit,2,1,20\n\
         deposit,1,2,10\n\
         withdrawal,2,3,7\n\
         withdrawal,1,4,3\n",
    );

    assert_eq!(
        accounts.get(&1),
        Some(&AccountOutput {
            client: 1,
            available: dec!(7),
            held: Decimal::ZERO,
            total: dec!(7),
            locked: false,
        })
    );
    assert_eq!(
        accounts.get(&2),
        Some(&AccountOutput {
            client: 2,
            available: dec!(13),
            held: Decimal::ZERO,
            total: dec!(13),
            locked: false,
        })
    );
}

#[test]
fn dispute_holds_funds_until_resolved() {
    let accounts = run_to_accounts(
        "type,client,tx,amount\n\
         deposit,1,1,100\n\
         dispute,1,1,\n\
         withdrawal,1,2,40\n\
         resolve,1,1,\n",
    );

    assert_eq!(
        accounts.get(&1),
        Some(&AccountOutput {
            client: 1,
            available: dec!(100),
            held: Decimal::ZERO,
            total: dec!(100),
            locked: false,
        })
    );
}

#[test]
fn chargeback_locks_account_and_ignores_later_payments() {
    let accounts = run_to_accounts(
        "type,client,tx,amount\n\
         deposit,1,1,100\n\
         dispute,1,1,\n\
         chargeback,1,1,\n\
         deposit,1,2,50\n\
         withdrawal,1,3,10\n",
    );

    assert_eq!(
        accounts.get(&1),
        Some(&AccountOutput {
            client: 1,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            total: Decimal::ZERO,
            locked: true,
        })
    );
}

#[test]
fn accepts_whitespace_and_preserves_four_decimal_places() {
    let output = run_to_bytes(
        "type, client, tx, amount\n\
           deposit, 1, 1, 12.3456\n",
    );
    let mut reader = csv::Reader::from_reader(output.as_slice());
    let record = reader.records().next().unwrap().unwrap();

    assert_eq!(&record[0], "1");
    assert_eq!(&record[1], "12.3456");
    assert_eq!(&record[3], "12.3456");
}

#[test]
fn unknown_dispute_is_ignored() {
    let accounts = run_to_accounts(
        "type,client,tx,amount\n\
     deposit,1,1,10\n\
     dispute,1,999,\n\
     deposit,1,2,5\n",
    );

    assert_eq!(
        accounts.get(&1),
        Some(&AccountOutput {
            client: 1,
            available: dec!(15),
            held: Decimal::ZERO,
            total: dec!(15),
            locked: false,
        })
    );
}

#[test]
fn missing_payment_amount_returns_an_error() {
    let mut deposit_output = Vec::new();
    let deposit_error = run(
        "type,client,tx,amount\ndeposit,1,1,\n".as_bytes(),
        &mut deposit_output,
    )
    .unwrap_err();
    assert!(matches!(
        deposit_error,
        AppError::Command(CommandError::MissingDepositAmount)
    ));

    let mut withdrawal_output = Vec::new();
    let withdrawal_error = run(
        "type,client,tx,amount\nwithdrawal,1,1,\n".as_bytes(),
        &mut withdrawal_output,
    )
    .unwrap_err();
    assert!(matches!(
        withdrawal_error,
        AppError::Command(CommandError::MissingWithdrawalAmount)
    ));
}
