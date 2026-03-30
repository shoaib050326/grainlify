use crate::{Escrow, EscrowStatus};
use soroban_sdk::{symbol_short, Env, Symbol};

const INV_CALLS: Symbol = symbol_short!("InvCalls");
#[cfg(test)]
const INV_DISABLED: Symbol = symbol_short!("InvOff");

fn record_call(env: &Env) {
    let calls: u32 = env.storage().instance().get(&INV_CALLS).unwrap_or(0);
    env.storage().instance().set(&INV_CALLS, &(calls + 1));
}

#[cfg(test)]
fn assert_enabled(env: &Env) {
    let disabled: bool = env.storage().instance().get(&INV_DISABLED).unwrap_or(false);
    if disabled {
        panic!("Invariant checks disabled");
    }
}

#[cfg(not(test))]
fn assert_enabled(_env: &Env) {}

/// Assert per-escrow invariants (INV-ESC-1 through INV-ESC-4).
///
/// Checks:
/// - INV-ESC-1: amount >= 0
/// - INV-ESC-2: remaining_amount >= 0
/// - INV-ESC-3: remaining_amount <= amount
/// - INV-ESC-4: Released => remaining_amount == 0
///
/// # Panics
/// Panics if any invariant is violated.
pub(crate) fn assert_escrow(env: &Env, escrow: &Escrow) {
    assert_enabled(env);
    record_call(env);
    if escrow.amount < 0 {
        panic!("Invariant violated: amount must be non-negative");
    }
    if escrow.remaining_amount < 0 {
        panic!("Invariant violated: remaining_amount must be non-negative");
    }
    if escrow.remaining_amount > escrow.amount {
        panic!("Invariant violated: remaining_amount cannot exceed amount");
    }
    if escrow.status == EscrowStatus::Released && escrow.remaining_amount != 0 {
        panic!("Invariant violated: released escrow must have zero remaining amount");
    }
}

/// Verify per-escrow invariants (INV-ESC-1 through INV-ESC-4) without panicking.
///
/// Returns `true` if all invariants hold, `false` otherwise.
///
/// Checks:
/// - INV-ESC-1: amount >= 0
/// - INV-ESC-2: remaining_amount >= 0
/// - INV-ESC-3: remaining_amount <= amount
/// - INV-ESC-4: Released => remaining_amount == 0
pub(crate) fn verify_escrow_invariants(escrow: &Escrow) -> bool {
    if escrow.amount < 0 {
        return false;
    }
    if escrow.remaining_amount < 0 {
        return false;
    }
    if escrow.remaining_amount > escrow.amount {
        return false;
    }
    if escrow.status == EscrowStatus::Released && escrow.remaining_amount != 0 {
        return false;
    }
    true
}

#[cfg(test)]
pub(crate) fn reset_test_state(env: &Env) {
    env.storage().instance().set(&INV_CALLS, &0_u32);
    env.storage().instance().set(&INV_DISABLED, &false);
}

#[cfg(test)]
pub(crate) fn set_disabled_for_test(env: &Env, disabled: bool) {
    env.storage().instance().set(&INV_DISABLED, &disabled);
}

#[cfg(test)]
pub(crate) fn call_count_for_test(env: &Env) -> u32 {
    env.storage().instance().get(&INV_CALLS).unwrap_or(0)
}