use rust_decimal::Decimal;
use serde::Deserialize;

use crate::CommandError;
use crate::domain::{ClientId, Command, TransactionId};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct TransactionRow {
    #[serde(rename = "type")]
    pub kind: TransactionType,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Option<Decimal>,
}

impl TryFrom<TransactionRow> for Command {
    type Error = CommandError;

    fn try_from(row: TransactionRow) -> Result<Self, Self::Error> {
        let TransactionRow {
            kind,
            client,
            tx,
            amount,
        } = row;

        match kind {
            TransactionType::Deposit => Ok(Command::Deposit {
                client,
                tx,
                amount: amount.ok_or(CommandError::MissingDepositAmount)?,
            }),
            TransactionType::Withdrawal => Ok(Command::Withdrawal {
                client,
                tx,
                amount: amount.ok_or(CommandError::MissingWithdrawalAmount)?,
            }),
            TransactionType::Dispute => Ok(Command::Dispute { client, tx }),
            TransactionType::Resolve => Ok(Command::Resolve { client, tx }),
            TransactionType::Chargeback => Ok(Command::Chargeback { client, tx }),
        }
    }
}

#[cfg(test)]
mod tests {
    use csv::{ReaderBuilder, Trim};
    use rust_decimal::Decimal;

    use super::*;

    fn parse_row(csv: &str) -> TransactionRow {
        ReaderBuilder::new()
            .trim(Trim::All)
            .from_reader(csv.as_bytes())
            .deserialize()
            .next()
            .unwrap()
            .unwrap()
    }

    #[test]
    fn payment_row_deserializes_and_converts_to_command() {
        let row = parse_row("type, client, tx, amount\n deposit, 1, 42, 12.3456\n");

        assert_eq!(
            Command::try_from(row).unwrap(),
            Command::Deposit {
                client: 1,
                tx: 42,
                amount: Decimal::new(123_456, 4),
            }
        );
    }

    #[test]
    fn dispute_does_not_require_an_amount() {
        let row = parse_row("type,client,tx,amount\ndispute,1,42,\n");

        assert_eq!(
            Command::try_from(row).unwrap(),
            Command::Dispute { client: 1, tx: 42 }
        );
    }

    #[test]
    fn payment_without_amount_is_rejected() {
        let row = parse_row("type,client,tx,amount\nwithdrawal,1,42,\n");

        assert_eq!(
            Command::try_from(row),
            Err(CommandError::MissingWithdrawalAmount)
        );
    }

    #[test]
    fn deposit_without_amount_is_rejected() {
        let row = parse_row("type,client,tx,amount\ndeposit,1,42,\n");

        assert_eq!(
            Command::try_from(row),
            Err(CommandError::MissingDepositAmount)
        );
    }
}
