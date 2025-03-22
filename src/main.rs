use std::cmp::min;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TransactionTarget {
    Address,
    Object,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TransactionKind {
    UserDeposit(u64),
    UserWithdraw(u64),

    Curse(u64),
    Clawback(u64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Transaction {
    kind: TransactionKind,
    target: TransactionTarget,
}

impl Transaction {
    fn is_clawback(&self) -> bool {
        matches!(self.kind, TransactionKind::Clawback(_))
    }

    fn into_delta(&self) -> BalanceDelta {
        match &self.kind {
            TransactionKind::UserDeposit(amount) => BalanceDelta(*amount as i64, 0),
            TransactionKind::UserWithdraw(amount) => BalanceDelta(-(*amount as i64), 0),
            TransactionKind::Curse(amount) => BalanceDelta(0, *amount as i64),
            // clawback takes both from the balance and the cursed amount.
            // Very important, otherwise the account would be permanently cursed.
            TransactionKind::Clawback(amount) => BalanceDelta(-(*amount as i64), -(*amount as i64)),
        }
    }

    fn address_deposit(amount: u64) -> Self {
        Self {
            kind: TransactionKind::UserDeposit(amount),
            target: TransactionTarget::Address,
        }
    }

    fn object_deposit(amount: u64) -> Self {
        Self {
            kind: TransactionKind::UserDeposit(amount),
            target: TransactionTarget::Object,
        }
    }

    fn address_withdraw(amount: u64) -> Self {
        Self {
            kind: TransactionKind::UserWithdraw(amount),
            target: TransactionTarget::Address,
        }
    }

    fn object_withdraw(amount: u64) -> Self {
        Self {
            kind: TransactionKind::UserWithdraw(amount),
            target: TransactionTarget::Object,
        }
    }

    fn object_curse(amount: u64) -> Self {
        Self {
            kind: TransactionKind::Curse(amount),
            target: TransactionTarget::Object,
        }
    }

    fn address_curse(amount: u64) -> Self {
        Self {
            kind: TransactionKind::Curse(amount),
            target: TransactionTarget::Address,
        }
    }

    fn object_clawback(amount: u64) -> Self {
        Self {
            kind: TransactionKind::Clawback(amount),
            target: TransactionTarget::Object,
        }
    }

    fn address_clawback(amount: u64) -> Self {
        Self {
            kind: TransactionKind::Clawback(amount),
            target: TransactionTarget::Address,
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
struct Effects {
    address_delta: BalanceDelta,
    object_delta: BalanceDelta,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
struct Balance(u64, u64);

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
struct BalanceDelta(i64, i64);

impl Balance {
    fn apply_delta(&mut self, delta: BalanceDelta) {
        let (b, c) = (self.0 as i64, self.1 as i64);

        let (b, c) = (b + delta.0, c + delta.1);

        assert!(b >= 0 && c >= 0);

        self.0 = b as u64;
        self.1 = c as u64;
    }

    fn check_limit(&self, transaction: &Transaction) -> bool {
        match &transaction.kind {
            // adding to a balance can never fail
            TransactionKind::UserDeposit(_) => true,
            TransactionKind::Curse(_) => true,

            TransactionKind::UserWithdraw(amount) => {
                let user_limit = self.0.saturating_sub(self.1);
                *amount <= user_limit
            }
            TransactionKind::Clawback(amount) => {
                let clawback_limit = min(self.0, self.1);
                *amount <= clawback_limit
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct State {
    address_state: Balance,
    object_state: Balance,
}

impl State {
    fn apply(&mut self, transaction: &Transaction) -> Effects {
        let transaction_delta = transaction.into_delta();

        match &transaction.target {
            TransactionTarget::Address => {
                self.address_state.apply_delta(transaction_delta);
                Effects {
                    address_delta: transaction_delta,
                    object_delta: BalanceDelta(0, 0),
                }
            }
            TransactionTarget::Object => {
                self.object_state.apply_delta(transaction_delta);
                Effects {
                    address_delta: BalanceDelta(0, 0),
                    object_delta: transaction_delta,
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct Executor {
    scheduled_transactions: Vec<Transaction>,

    state: State,
}

impl Executor {
    // Attempt to schedule a transaction and return false if it was rejected.
    fn schedule(&mut self, transaction: Transaction) -> Result<(), ()> {
        match (transaction.target, transaction.is_clawback()) {
            // Address transactions must be checked pre-scheduling
            (TransactionTarget::Address, _) => {
                if self.state.address_state.check_limit(&transaction) {
                    self.scheduled_transactions.push(transaction);
                    Ok(())
                } else {
                    Err(())
                }
            }

            // Non-clawback object transactions are checked at execution
            // (and can fail)
            (TransactionTarget::Object, false) => {
                self.scheduled_transactions.push(transaction);
                Ok(())
            }

            // Clawbacks from either addresses or objects are unsequenced,
            // so we must prove non-underflow.
            (target, true) => {
                let state = match target {
                    TransactionTarget::Address => &self.state.address_state,
                    TransactionTarget::Object => &self.state.object_state,
                };

                if state.check_limit(&transaction) {
                    self.scheduled_transactions.push(transaction);
                    Ok(())
                } else {
                    Err(())
                }
            }
        }
    }

    // Settle all scheduled transactions.
    fn settle(&mut self) -> Vec<(Transaction, Effects)> {
        // transactions are applied to next state, but checks are done against
        // the current state.
        let mut next_state = self.state;

        // Transactions are not scheduled without proof of no-underflow,
        // so settlement cannot fail.
        let ret = self
            .scheduled_transactions
            .drain(..)
            .map(|tx| {
                match (tx.target, tx.is_clawback()) {
                    // Address transactions as well as object clawbacks are proven at schedule
                    // time not to underflow
                    (TransactionTarget::Address, _) | (TransactionTarget::Object, true) => {
                        (tx, next_state.apply(&tx))
                    }

                    // User object transactions are checked at execution
                    (TransactionTarget::Object, false) => {
                        if self.state.object_state.check_limit(&tx) {
                            (tx, next_state.apply(&tx))
                        } else {
                            (tx, Effects::default())
                        }
                    }
                }
            })
            .collect();

        self.state = next_state;
        ret
    }
}

#[cfg(test)]
mod testmacros;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_withdraw() {
        let mut e = Executor::default();

        e.schedule(Transaction::address_deposit(100)).unwrap();
        // rejected, insufficient funds
        e.schedule(Transaction::address_withdraw(100)).unwrap_err();

        // Balance clears but withdraw is rejected because the deposit had not yet
        // settled.
        assert_eq!(
            e.settle(),
            vec![effects!(addr_deposit, /* infallible */ 100),]
        );
        assert_eq!(e.state.address_state, Balance(100, 0));

        e.schedule(Transaction::address_withdraw(100)).unwrap();

        // Now the withdraw clears because the deposit settled.
        assert_eq!(
            e.settle(),
            vec![effects!(addr_withdraw, /* infallible */ 100),]
        );
        assert_eq!(e.state.address_state, Balance(0, 0));
    }

    #[test]
    fn test_object_withdraw() {
        let mut e = Executor::default();

        // As with address withdraw, the deposit does not clear instantly.
        // However, the object withdraw is not checked at schedule time,
        // so scheduling succeeds.
        e.schedule(Transaction::object_deposit(100)).unwrap();
        e.schedule(Transaction::object_withdraw(100)).unwrap();
        assert_eq!(
            e.settle(),
            vec![
                effects!(obj_deposit, /* infallible */ 100),
                // object withdraw is checked at execution time, and
                // deposit has not settled, so we withdraw 0 of an attempted
                // 100.
                effects!(obj_withdraw, /* attempt */ 100, /* cleared */ 0)
            ]
        );
        assert_eq!(e.state.object_state, Balance(100, 0));

        // Now the deposit settles so a full withdraw is possible.
        e.schedule(Transaction::object_withdraw(100)).unwrap();
        assert_eq!(
            e.settle(),
            vec![effects!(
                obj_withdraw,
                /* attempted */ 100,
                /* cleared */ 100
            ),]
        );
        assert_eq!(e.state.object_state, Balance(0, 0));
    }

    #[test]
    fn test_object_clawback() {
        let mut e = Executor::default();

        e.schedule(Transaction::object_deposit(100)).unwrap();
        // Clawback is rejected because they have not yet cursed the object.
        e.schedule(Transaction::object_clawback(50)).unwrap_err();
        assert_eq!(
            e.settle(),
            vec![effects!(obj_deposit, /* infallible */ 100),]
        );
        assert_eq!(e.state.object_state, Balance(100, 0));

        // Now we curse 50 out of 100.
        e.schedule(Transaction::object_curse(50)).unwrap();
        assert_eq!(e.settle(), vec![effects!(obj_curse, /* infallible */ 50),]);
        assert_eq!(e.state.object_state, Balance(100, 50));

        // User can attempt to withdraw 60. it will fail at execution time.
        e.schedule(Transaction::object_withdraw(60)).unwrap();
        // 50 is okay though
        e.schedule(Transaction::object_withdraw(50)).unwrap();

        // Issuer cannot claw back 60 because they didn't curse enough.
        // Clawbacks are unsequenced so they are checked at schedule time.
        e.schedule(Transaction::object_clawback(60)).unwrap_err();

        // Issuer can claw back 50 though.
        e.schedule(Transaction::object_clawback(50)).unwrap();

        assert_eq!(
            e.settle(),
            vec![
                effects!(obj_withdraw, /* attempted */ 60, /* cleared */ 0),
                effects!(obj_withdraw, /* attempted */ 50, /* cleared */ 50),
                effects!(obj_clawback, /* infallable */ 50),
            ]
        );
        assert_eq!(e.state.object_state, Balance(0, 0));
    }

    #[test]
    fn test_address_clawback() {
        let mut e = Executor::default();

        e.schedule(Transaction::address_deposit(100)).unwrap();
        // cannot clawback before cursing
        e.schedule(Transaction::address_clawback(100)).unwrap_err();
        assert_eq!(
            e.settle(),
            vec![effects!(addr_deposit, /* infallible */ 100),]
        );
        assert_eq!(e.state.address_state, Balance(100, 0));

        // curse 50
        e.schedule(Transaction::address_curse(50)).unwrap();
        assert_eq!(e.settle(), vec![effects!(addr_curse, /* infallible */ 50),]);
        assert_eq!(e.state.address_state, Balance(100, 50));

        // user cannot withdraw 60
        e.schedule(Transaction::address_withdraw(60)).unwrap_err();
        // issuer cannot clawback 60
        e.schedule(Transaction::address_clawback(60)).unwrap_err();

        // but both can take out 50
        e.schedule(Transaction::address_clawback(50)).unwrap();
        e.schedule(Transaction::address_withdraw(50)).unwrap();

        assert_eq!(
            e.settle(),
            vec![
                effects!(addr_clawback, /* infallable */ 50),
                effects!(addr_withdraw, /* infallible */ 50),
            ]
        );
        assert_eq!(e.state.address_state, Balance(0, 0));

        // issuer can pre-emptively curse an account
        // Note: if we don't want this behavior, we can cap the curse amount to the balance
        // when settling.
        e.schedule(Transaction::address_curse(100)).unwrap();
        e.schedule(Transaction::address_deposit(110)).unwrap();
        assert_eq!(
            e.settle(),
            vec![
                effects!(addr_curse, /* infallible */ 100),
                effects!(addr_deposit, /* infallible */ 110),
            ]
        );
        assert_eq!(e.state.address_state, Balance(110, 100));

        // user cannot withdraw more than 10
        e.schedule(Transaction::address_withdraw(11)).unwrap_err();
        e.schedule(Transaction::address_withdraw(10)).unwrap();

        // issuer can clawback 50
        e.schedule(Transaction::address_clawback(50)).unwrap();

        assert_eq!(
            e.settle(),
            vec![
                effects!(addr_withdraw, /* infallible */ 10),
                effects!(addr_clawback, /* infallable */ 50),
            ]
        );
        // The remaining balance is still cursed.
        assert_eq!(e.state.address_state, Balance(50, 50));
    }
}
