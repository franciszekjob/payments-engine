use rust_decimal::Decimal;

use crate::AccountError;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Account {
    available: Decimal,
    held: Decimal,
    locked: bool,
}

impl Account {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn available(&self) -> Decimal {
        self.available
    }

    pub fn held(&self) -> Decimal {
        self.held
    }

    pub fn total(&self) -> Decimal {
        self.available + self.held
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub(crate) fn deposit(&mut self, amount: Decimal) -> Result<(), AccountError> {
        self.validate_mutation(amount)?;
        let available = self
            .available
            .checked_add(amount)
            .ok_or(AccountError::ArithmeticOverflow)?;
        available
            .checked_add(self.held)
            .ok_or(AccountError::ArithmeticOverflow)?;

        self.available = available;

        Ok(())
    }

    pub(crate) fn withdraw(&mut self, amount: Decimal) -> Result<(), AccountError> {
        self.validate_mutation(amount)?;

        if self.available < amount {
            return Err(AccountError::InsufficientAvailable {
                available: self.available,
                requested: amount,
            });
        }

        self.available = self
            .available
            .checked_sub(amount)
            .ok_or(AccountError::ArithmeticOverflow)?;

        Ok(())
    }

    pub(crate) fn hold(&mut self, amount: Decimal) -> Result<(), AccountError> {
        self.validate_mutation(amount)?;

        // A dispute holds the full original transaction amount even when the
        // client has since spent some of it, so available funds may go negative.
        let available = self
            .available
            .checked_sub(amount)
            .ok_or(AccountError::ArithmeticOverflow)?;
        let held = self
            .held
            .checked_add(amount)
            .ok_or(AccountError::ArithmeticOverflow)?;

        self.available = available;
        self.held = held;

        Ok(())
    }

    pub(crate) fn release(&mut self, amount: Decimal) -> Result<(), AccountError> {
        self.validate_mutation(amount)?;

        if self.held < amount {
            return Err(AccountError::InsufficientHeld {
                held: self.held,
                requested: amount,
            });
        }

        let held = self
            .held
            .checked_sub(amount)
            .ok_or(AccountError::ArithmeticOverflow)?;
        let available = self
            .available
            .checked_add(amount)
            .ok_or(AccountError::ArithmeticOverflow)?;

        self.held = held;
        self.available = available;

        Ok(())
    }

    pub(crate) fn chargeback(&mut self, amount: Decimal) -> Result<(), AccountError> {
        self.validate_mutation(amount)?;

        if self.held < amount {
            return Err(AccountError::InsufficientHeld {
                held: self.held,
                requested: amount,
            });
        }

        self.held = self
            .held
            .checked_sub(amount)
            .ok_or(AccountError::ArithmeticOverflow)?;
        self.locked = true;

        Ok(())
    }

    fn validate_mutation(&self, amount: Decimal) -> Result<(), AccountError> {
        if self.locked {
            return Err(AccountError::Locked);
        }

        if amount <= Decimal::ZERO {
            return Err(AccountError::NonPositiveAmount(amount));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::{Decimal, dec};

    use super::*;

    #[test]
    fn new_account_has_zero_balances_and_is_unlocked() {
        let account = Account::new();

        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
        assert!(!account.is_locked());
    }

    #[test]
    fn deposit_and_withdraw_update_available_funds() {
        let mut account = Account::new();

        account.deposit(dec!(10)).unwrap();
        account.withdraw(dec!(4)).unwrap();

        assert_eq!(account.available(), dec!(6));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(6));
    }

    #[test]
    fn withdrawal_with_insufficient_funds_does_not_mutate_account() {
        let mut account = Account::new();
        account.deposit(dec!(3)).unwrap();

        let error = account.withdraw(dec!(4)).unwrap_err();

        assert_eq!(
            error,
            AccountError::InsufficientAvailable {
                available: dec!(3),
                requested: dec!(4),
            }
        );
        assert_eq!(account.total(), dec!(3));
    }

    #[test]
    fn hold_can_make_available_funds_negative() {
        let mut account = Account::new();
        account.deposit(dec!(10)).unwrap();
        account.withdraw(dec!(8)).unwrap();

        account.hold(dec!(10)).unwrap();

        assert_eq!(account.available(), dec!(-8));
        assert_eq!(account.held(), dec!(10));
        assert_eq!(account.total(), dec!(2));
    }

    #[test]
    fn release_returns_held_funds_to_available() {
        let mut account = Account::new();
        account.deposit(dec!(10)).unwrap();
        account.hold(dec!(10)).unwrap();

        account.release(dec!(10)).unwrap();

        assert_eq!(account.available(), dec!(10));
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), dec!(10));
    }

    #[test]
    fn chargeback_removes_held_funds_and_locks_account() {
        let mut account = Account::new();
        account.deposit(dec!(10)).unwrap();
        account.hold(dec!(10)).unwrap();

        account.chargeback(dec!(10)).unwrap();

        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
        assert!(account.is_locked());
        assert_eq!(account.deposit(dec!(1)), Err(AccountError::Locked));
    }

    #[test]
    fn non_positive_amounts_are_rejected_without_mutation() {
        let mut account = Account::new();

        assert_eq!(
            account.deposit(Decimal::ZERO),
            Err(AccountError::NonPositiveAmount(Decimal::ZERO))
        );
        assert_eq!(
            account.withdraw(dec!(-1)),
            Err(AccountError::NonPositiveAmount(dec!(-1)))
        );
        assert_eq!(account, Account::new());
    }

    #[test]
    fn deposit_rejects_an_overflowing_total_without_mutation() {
        let amount: Decimal = "7000000000000000000000000000.0".parse().unwrap();
        let mut account = Account::new();
        account.deposit(amount).unwrap();
        account.hold(amount).unwrap();

        for _ in 0..10 {
            account.deposit(amount).unwrap();
        }

        let before = account.clone();
        assert_eq!(
            account.deposit(amount),
            Err(AccountError::ArithmeticOverflow)
        );
        assert_eq!(account, before);
    }
}
