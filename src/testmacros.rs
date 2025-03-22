#[macro_export]
macro_rules! effects {
    (addr_deposit, $amount:expr) => {
        (
            Transaction::address_deposit($amount),
            Effects {
                address_delta: BalanceDelta($amount, 0),
                object_delta: BalanceDelta(0, 0),
            },
        )
    };

    (addr_withdraw, $amount:expr) => {
        (
            Transaction::address_withdraw($amount),
            Effects {
                address_delta: BalanceDelta(-$amount, 0),
                object_delta: BalanceDelta(0, 0),
            },
        )
    };

    (obj_deposit, $amount:expr) => {
        (
            Transaction::object_deposit($amount),
            Effects {
                address_delta: BalanceDelta(0, 0),
                object_delta: BalanceDelta($amount, 0),
            },
        )
    };

    (obj_withdraw, $amount:expr, $actual:expr) => {
        (
            Transaction::object_withdraw($amount),
            Effects {
                address_delta: BalanceDelta(0, 0),
                object_delta: BalanceDelta(-$actual, 0),
            },
        )
    };

    (obj_curse, $amount:expr) => {
        (
            Transaction::object_curse($amount),
            Effects {
                address_delta: BalanceDelta(0, 0),
                object_delta: BalanceDelta(0, $amount),
            },
        )
    };

    (obj_clawback, $amount:expr) => {
        (
            Transaction::object_clawback($amount),
            Effects {
                address_delta: BalanceDelta(0, 0),
                object_delta: BalanceDelta(-$amount, -$amount),
            },
        )
    };

    (addr_curse, $amount:expr) => {
        (
            Transaction::address_curse($amount),
            Effects {
                address_delta: BalanceDelta(0, $amount),
                object_delta: BalanceDelta(0, 0),
            },
        )
    };

    (addr_clawback, $amount:expr) => {
        (
            Transaction::address_clawback($amount),
            Effects {
                address_delta: BalanceDelta(-$amount, -$amount),
                object_delta: BalanceDelta(0, 0),
            },
        )
    };
}
