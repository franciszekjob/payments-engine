pub mod domain;
pub mod engine;
pub mod input;
pub mod output;

mod error;

pub use error::{AccountError, AppError, CommandError};

use std::io::{Read, Write};

use csv::{ReaderBuilder, Trim};

use crate::{
    domain::Command, engine::PaymentEngine, input::TransactionRow, output::write_accounts,
};

pub fn run<R: Read, W: Write>(input: R, output: W) -> Result<(), AppError> {
    let mut reader = ReaderBuilder::new().trim(Trim::All).from_reader(input);

    let mut engine = PaymentEngine::new();

    for result in reader.deserialize::<TransactionRow>() {
        let row = result?;
        let command = Command::try_from(row)?;

        engine.process(command);
    }

    write_accounts(&engine, output)?;

    Ok(())
}
