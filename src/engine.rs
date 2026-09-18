use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::domain::{Account, ClientId, Command, DepositState, TransactionId, TransactionRecord};

#[derive(Default)]
pub struct PaymentEngine {
    accounts: HashMap<ClientId, Account>,
    transactions: HashMap<TransactionId, TransactionRecord>,
}

impl PaymentEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process(&mut self, command: Command) {
        match command {
            Command::Deposit { client, tx, amount } => {
                self.deposit(client, tx, amount);
            }
            Command::Withdrawal { client, tx, amount } => {
                self.withdraw(client, tx, amount);
            }
            Command::Dispute { client, tx } => {
                self.dispute(client, tx);
            }
            Command::Resolve { client, tx } => {
                self.resolve(client, tx);
            }
            Command::Chargeback { client, tx } => {
                self.chargeback(client, tx);
            }
        }
    }

    pub fn accounts(&self) -> impl Iterator<Item = (ClientId, &Account)> {
        self.accounts
            .iter()
            .map(|(&client, account)| (client, account))
    }

    fn deposit(&mut self, client: ClientId, tx: TransactionId, amount: Decimal) {
        // Spec guarantees that transaction IDs are unique, but a guard is added
        // to ensure that in future this assumption is not violated.
        if self.transactions.contains_key(&tx) {
            return;
        }

        let account = self.accounts.entry(client).or_default();

        if account.deposit(amount).is_err() {
            return;
        }

        self.transactions
            .insert(tx, TransactionRecord::deposit(client, amount));
    }

    fn withdraw(&mut self, client: ClientId, tx: TransactionId, amount: Decimal) {
        // Spec guarantees that transaction IDs are unique, but a guard is added
        // to ensure that in future this assumption is not violated.
        if self.transactions.contains_key(&tx) {
            return;
        }

        let account = self.accounts.entry(client).or_default();

        if account.withdraw(amount).is_err() {
            return;
        }

        self.transactions
            .insert(tx, TransactionRecord::withdrawal(client, amount));
    }

    fn dispute(&mut self, client: ClientId, tx: TransactionId) {
        let amount = match self.validated_deposit_amount(client, tx, DepositState::Settled) {
            Some(amount) => amount,
            None => return,
        };
        let Some(account) = self.accounts.get_mut(&client) else {
            return;
        };

        if account.hold(amount).is_err() {
            return;
        }

        if let Some(transaction) = self.transactions.get_mut(&tx) {
            transaction.dispute();
        }
    }

    fn resolve(&mut self, client: ClientId, tx: TransactionId) {
        let amount = match self.validated_deposit_amount(client, tx, DepositState::Disputed) {
            Some(amount) => amount,
            None => return,
        };
        let Some(account) = self.accounts.get_mut(&client) else {
            return;
        };

        if account.release(amount).is_err() {
            return;
        }

        if let Some(transaction) = self.transactions.get_mut(&tx) {
            transaction.resolve();
        }
    }

    fn chargeback(&mut self, client: ClientId, tx: TransactionId) {
        let amount = match self.validated_deposit_amount(client, tx, DepositState::Disputed) {
            Some(amount) => amount,
            None => return,
        };
        let Some(account) = self.accounts.get_mut(&client) else {
            return;
        };

        if account.chargeback(amount).is_err() {
            return;
        }

        if let Some(transaction) = self.transactions.get_mut(&tx) {
            transaction.chargeback();
        }
    }

    fn validated_deposit_amount(
        &self,
        client: ClientId,
        tx: TransactionId,
        expected_state: DepositState,
    ) -> Option<Decimal> {
        // Check if the transaction exists and client is the owner of the transaction.
        match self.transactions.get(&tx) {
            Some(TransactionRecord::Deposit {
                client: owner,
                amount,
                state,
            }) if *owner == client && *state == expected_state => Some(*amount),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal::dec;

    #[test]
    fn process_handles_full_transaction_lifecycle() {
        let mut engine = PaymentEngine::new();

        engine.process(Command::Deposit {
            client: 1,
            tx: 1,
            amount: dec!(100),
        });
        engine.process(Command::Withdrawal {
            client: 1,
            tx: 2,
            amount: dec!(20),
        });
        engine.process(Command::Dispute { client: 1, tx: 1 });
        engine.process(Command::Resolve { client: 1, tx: 1 });
        engine.process(Command::Dispute { client: 1, tx: 1 });
        engine.process(Command::Chargeback { client: 1, tx: 1 });

        let (_, account) = engine.accounts().find(|(client, _)| *client == 1).unwrap();
        assert_eq!(account.available(), dec!(-20));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(-20));
        assert!(account.is_locked());
    }

    // deposit

    #[test]
    fn deposit_creates_account() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));

        assert!(engine.accounts.contains_key(&1));
    }

    #[test]
    fn deposit_increases_available_and_total() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn deposit_does_not_change_held() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.held(), Decimal::ZERO);
    }

    #[test]
    fn deposit_on_locked_account_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);

        engine.deposit(1, 2, dec!(50));

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
    }

    #[test]
    fn multiple_deposits_accumulate() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.deposit(1, 2, dec!(50));

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(150));
        assert_eq!(account.total(), dec!(150));
    }

    // withdrawal

    #[test]
    fn withdrawal_decreases_available_and_total() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.withdraw(1, 2, dec!(40));

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(60));
        assert_eq!(account.total(), dec!(60));
    }

    #[test]
    fn withdrawal_of_exact_available_balance_succeeds() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.withdraw(1, 2, dec!(100));

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
    }

    #[test]
    fn withdrawal_with_insufficient_funds_does_not_change_balance() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.withdraw(1, 2, dec!(150));

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn withdrawal_for_new_client_creates_empty_account_and_fails() {
        let mut engine = PaymentEngine::new();
        engine.withdraw(1, 1, dec!(100));

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
    }

    #[test]
    fn withdrawal_on_locked_account_is_ignored() {
        let mut engine = PaymentEngine::new();

        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);

        engine.withdraw(1, 2, dec!(50));

        let account = engine.accounts.get(&1).unwrap();
        assert!(account.is_locked());
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
    }

    // dispute

    #[test]
    fn dispute_moves_available_to_held() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), dec!(100));
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn dispute_unknown_transaction_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 2);

        // The account should remain unchanged since the dispute was for a non-existent transaction.
        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn dispute_wrong_client_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(2, 1);

        // The account should remain unchanged since the dispute was for a transaction not owned by the client.
        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn withdrawal_cannot_be_disputed() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.withdraw(1, 2, dec!(50));
        engine.dispute(1, 2);

        // The account should remain unchanged since the dispute was for a withdrawal transaction.
        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(50));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(50));
    }

    #[test]
    fn already_disputed_transaction_cannot_be_disputed_again() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.dispute(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), dec!(100));
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn charged_back_transaction_cannot_be_disputed() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);
        engine.dispute(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
    }

    #[test]
    fn dispute_can_make_available_negative() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(10));
        engine.withdraw(1, 2, dec!(8));
        engine.dispute(1, 1);

        // The account should have negative available funds since the dispute moved the deposit
        // amount to held, leaving the withdrawal amount as a negative available balance.
        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(-8));
        assert_eq!(account.held(), dec!(10));
        assert_eq!(account.total(), dec!(2));
    }

    // resolve

    #[test]
    fn resolve_releases_held_funds() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.resolve(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn resolve_unknown_transaction_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.resolve(1, 2);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn resolve_transaction_not_under_dispute_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.resolve(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn resolve_wrong_client_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.resolve(2, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), dec!(100));
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn resolve_charged_back_transaction_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);
        engine.resolve(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(0));
        assert_eq!(account.held(), dec!(0));
        assert_eq!(account.total(), dec!(0));
    }

    #[test]
    fn resolved_transaction_can_be_disputed_again() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.resolve(1, 1);
        engine.dispute(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), dec!(100));
        assert_eq!(account.total(), dec!(100));
    }

    // chargeback

    #[test]
    fn chargeback_removes_held_funds_from_total() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
    }

    #[test]
    fn chargeback_keeps_available_unchanged() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(10));
        engine.withdraw(1, 2, dec!(7));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(-7));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(-7));
    }

    #[test]
    fn chargeback_locks_account() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert!(account.is_locked());
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
    }

    #[test]
    fn chargeback_unknown_transaction_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.chargeback(1, 2);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn chargeback_transaction_not_under_dispute_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.chargeback(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), dec!(100));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn chargeback_wrong_client_is_ignored() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(2, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), dec!(100));
        assert_eq!(account.total(), dec!(100));
    }

    #[test]
    fn chargeback_is_terminal() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);

        // After a chargeback, no further actions should be possible.
        engine.resolve(1, 1);
        engine.dispute(1, 1);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
        assert!(account.is_locked());
    }

    #[test]
    fn chargeback_leaves_other_active_disputes_held_on_locked_account() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.deposit(1, 2, dec!(50));
        engine.dispute(1, 1);
        engine.dispute(1, 2);

        engine.chargeback(1, 1);
        engine.resolve(1, 2);
        engine.chargeback(1, 2);

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), dec!(50));
        assert_eq!(account.total(), dec!(50));
        assert!(account.is_locked());
        assert_eq!(
            engine.transactions.get(&2).unwrap().deposit_state(),
            Some(DepositState::Disputed)
        );
    }

    #[test]
    fn operations_after_chargeback_do_not_modify_account() {
        let mut engine = PaymentEngine::new();
        engine.deposit(1, 1, dec!(100));
        engine.dispute(1, 1);
        engine.chargeback(1, 1);

        engine.deposit(1, 2, dec!(50));
        engine.withdraw(1, 3, dec!(25));

        let account = engine.accounts.get(&1).unwrap();
        assert!(account.is_locked());
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
    }
}
