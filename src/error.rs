use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum CommandError {
    #[error("deposit transaction is missing an amount")]
    MissingDepositAmount,

    #[error("withdrawal transaction is missing an amount")]
    MissingWithdrawalAmount,
}

#[derive(Debug, Error, PartialEq)]
pub enum AccountError {
    #[error("account is locked")]
    Locked,

    #[error("amount must be greater than zero, got {0}")]
    NonPositiveAmount(Decimal),

    #[error("insufficient available funds: requested {requested}, available {available}")]
    InsufficientAvailable {
        available: Decimal,
        requested: Decimal,
    },

    #[error("insufficient held funds: requested {requested}, held {held}")]
    InsufficientHeld { held: Decimal, requested: Decimal },

    #[error("decimal arithmetic overflow")]
    ArithmeticOverflow,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Csv(#[from] csv::Error),

    #[error(transparent)]
    Command(#[from] CommandError),
}
