use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Output};

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

fn file_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

fn run_binary(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_payments-engine"))
        .args(args)
        .output()
        .unwrap()
}
fn successful_accounts(output: Output) -> HashMap<u16, AccountOutput> {
    assert!(
        output.status.success(),
        "binary failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut reader = csv::Reader::from_reader(output.stdout.as_slice());
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
    let accounts = successful_accounts(run_binary(&["tests/data/multiple_clients.csv"]));

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
fn resolves_disputed_funds() {
    let output = run_binary(&["tests/data/dispute_resolve.csv"]);
    let accounts = successful_accounts(output);

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
    let output = run_binary(&["tests/data/chargeback_locked.csv"]);
    let accounts = successful_accounts(output);

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
fn ignores_unknown_dispute_and_continues_processing() {
    let output = run_binary(&["tests/data/unknown_dispute.csv"]);
    let accounts = successful_accounts(output);

    assert_eq!(
        accounts.get(&1),
        Some(&AccountOutput {
            client: 1,
            available: dec!(12),
            held: Decimal::ZERO,
            total: dec!(12),
            locked: false,
        })
    );
}

#[test]
fn missing_input_argument_returns_an_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_payments-engine"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing input file"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn nonexistent_input_file_returns_an_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_payments-engine"))
        .arg(file_path("does-not-exist.csv"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("No such file or directory"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
