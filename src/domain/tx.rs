use rust_decimal::Decimal;

pub type ClientId = u16;
pub type TransactionId = u32;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Deposit {
        client: ClientId,
        tx: TransactionId,
        amount: Decimal,
    },
    Withdrawal {
        client: ClientId,
        tx: TransactionId,
        amount: Decimal,
    },
    Dispute {
        client: ClientId,
        tx: TransactionId,
    },
    Resolve {
        client: ClientId,
        tx: TransactionId,
    },
    Chargeback {
        client: ClientId,
        tx: TransactionId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositState {
    Settled,
    Disputed,
    ChargedBack,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionRecord {
    Deposit {
        client: ClientId,
        amount: Decimal,
        state: DepositState,
    },
    Withdrawal {
        client: ClientId,
        amount: Decimal,
    },
}

impl TransactionRecord {
    pub fn deposit(client: ClientId, amount: Decimal) -> Self {
        Self::Deposit {
            client,
            amount,
            state: DepositState::Settled,
        }
    }

    pub fn withdrawal(client: ClientId, amount: Decimal) -> Self {
        Self::Withdrawal { client, amount }
    }

    pub fn client(&self) -> ClientId {
        match self {
            Self::Deposit { client, .. } | Self::Withdrawal { client, .. } => *client,
        }
    }

    pub fn amount(&self) -> Decimal {
        match self {
            Self::Deposit { amount, .. } | Self::Withdrawal { amount, .. } => *amount,
        }
    }

    pub fn deposit_state(&self) -> Option<DepositState> {
        match self {
            Self::Deposit { state, .. } => Some(*state),
            Self::Withdrawal { .. } => None,
        }
    }

    pub fn dispute(&mut self) -> bool {
        match self {
            Self::Deposit { state, .. } if *state == DepositState::Settled => {
                *state = DepositState::Disputed;
                true
            }
            _ => false,
        }
    }

    pub fn resolve(&mut self) -> bool {
        match self {
            Self::Deposit { state, .. } if *state == DepositState::Disputed => {
                *state = DepositState::Settled;
                true
            }
            _ => false,
        }
    }

    pub fn chargeback(&mut self) -> bool {
        match self {
            Self::Deposit { state, .. } if *state == DepositState::Disputed => {
                *state = DepositState::ChargedBack;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::dec;

    use super::*;

    #[test]
    fn deposit_starts_settled_and_exposes_its_data() {
        let transaction = TransactionRecord::deposit(7, dec!(12.3456));

        assert_eq!(transaction.client(), 7);
        assert_eq!(transaction.amount(), dec!(12.3456));
        assert_eq!(transaction.deposit_state(), Some(DepositState::Settled));
    }

    #[test]
    fn deposit_can_be_disputed_and_resolved() {
        let mut transaction = TransactionRecord::deposit(1, dec!(10));

        assert!(!transaction.resolve());
        assert!(transaction.dispute());
        assert!(!transaction.dispute());
        assert_eq!(transaction.deposit_state(), Some(DepositState::Disputed));
        assert!(transaction.resolve());
        assert!(!transaction.resolve());
        assert_eq!(transaction.deposit_state(), Some(DepositState::Settled));
    }

    #[test]
    fn chargeback_is_only_allowed_for_a_disputed_deposit_and_is_final() {
        let mut transaction = TransactionRecord::deposit(1, dec!(10));

        assert!(!transaction.chargeback());
        assert!(transaction.dispute());
        assert!(transaction.chargeback());
        assert_eq!(transaction.deposit_state(), Some(DepositState::ChargedBack));
        assert!(!transaction.dispute());
        assert!(!transaction.resolve());
        assert!(!transaction.chargeback());
    }

    #[test]
    fn withdrawal_cannot_enter_the_dispute_lifecycle() {
        let mut transaction = TransactionRecord::withdrawal(2, dec!(5));

        assert_eq!(transaction.client(), 2);
        assert_eq!(transaction.amount(), dec!(5));
        assert_eq!(transaction.deposit_state(), None);
        assert!(!transaction.dispute());
        assert!(!transaction.resolve());
        assert!(!transaction.chargeback());
    }
}
