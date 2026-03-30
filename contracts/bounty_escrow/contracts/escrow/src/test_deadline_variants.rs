#![cfg(test)]
//! # Bounty Escrow Deadline Variant Tests
//!
//! This module validates deadline semantics using Soroban ledger timestamps.

use crate::{BountyEscrowContract, BountyEscrowContractClient, Error, EscrowStatus, RefundMode};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

const NO_DEADLINE: u64 = u64::MAX;

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract = e.register_stellar_asset_contract_v2(admin.clone());
    let addr = contract.address();
    (
        token::Client::new(e, &addr),
        token::StellarAssetClient::new(e, &addr),
    )
}

fn create_escrow_contract<'a>(e: &Env) -> BountyEscrowContractClient<'a> {
    let id = e.register_contract(None, BountyEscrowContract);
    BountyEscrowContractClient::new(e, &id)
}

struct Setup<'a> {
    env: Env,
    _admin: Address,
    depositor: Address,
    contributor: Address,
    token: token::Client<'a>,
    escrow: BountyEscrowContractClient<'a>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        let escrow = create_escrow_contract(&env);

        escrow.init(&admin, &token.address);
        token_admin.mint(&depositor, &10_000_000);

        Self {
            env,
            _admin: admin,
            depositor,
            contributor,
            token,
            escrow,
        }
    }
}

#[test]
fn test_zero_deadline_immediately_refundable() {
    let s = Setup::new();
    let deadline = 0_u64;
    s.escrow.lock_funds(&s.depositor, &10, &1_000, &deadline);

    let before = s.token.balance(&s.depositor);
    s.escrow.refund(&10);

    let info = s.escrow.get_escrow_info(&10);
    assert_eq!(info.deadline, deadline);
    assert_eq!(info.status, EscrowStatus::Refunded);
    assert_eq!(s.token.balance(&s.depositor), before + 1_000);
}

#[test]
fn test_future_deadline_refund_blocked_before_expiry() {
    let s = Setup::new();
    let deadline = s.env.ledger().timestamp() + 10_000;
    s.escrow.lock_funds(&s.depositor, &11, &1_000, &deadline);

    let result = s.escrow.try_refund(&11);
    assert_eq!(result.unwrap_err().unwrap(), Error::DeadlineNotPassed);

    let info = s.escrow.get_escrow_info(&11);
    assert_eq!(info.status, EscrowStatus::Locked);
    assert_eq!(s.token.balance(&s.escrow.address), 1_000);
}

#[test]
fn test_future_deadline_refund_succeeds_after_expiry() {
    let s = Setup::new();
    let now = s.env.ledger().timestamp();
    let deadline = now + 500;
    s.escrow.lock_funds(&s.depositor, &12, &1_200, &deadline);

    s.env.ledger().set_timestamp(deadline + 1);

    let before = s.token.balance(&s.depositor);
    s.escrow.refund(&12);

    let info = s.escrow.get_escrow_info(&12);
    assert_eq!(info.status, EscrowStatus::Refunded);
    assert_eq!(s.token.balance(&s.depositor), before + 1_200);
    assert_eq!(s.token.balance(&s.escrow.address), 0);
}

#[test]
fn test_future_deadline_early_refund_with_admin_approval() {
    let s = Setup::new();
    let deadline = s.env.ledger().timestamp() + 86_400;
    s.escrow.lock_funds(&s.depositor, &13, &2_000, &deadline);

    s.escrow
        .approve_refund(&13, &2_000, &s.depositor, &RefundMode::Full);

    let before = s.token.balance(&s.depositor);
    s.escrow.refund(&13);

    let info = s.escrow.get_escrow_info(&13);
    assert_eq!(info.status, EscrowStatus::Refunded);
    assert_eq!(s.token.balance(&s.depositor), before + 2_000);
}

#[test]
fn test_future_deadline_release_unaffected_by_deadline() {
    let s = Setup::new();
    let deadline = s.env.ledger().timestamp() + 86_400;
    s.escrow.lock_funds(&s.depositor, &14, &3_000, &deadline);

    s.escrow.release_funds(&14, &s.contributor);

    let info = s.escrow.get_escrow_info(&14);
    assert_eq!(info.status, EscrowStatus::Released);
    assert_eq!(s.token.balance(&s.contributor), 3_000);
}

#[test]
fn test_no_deadline_is_stored_verbatim() {
    let s = Setup::new();
    s.escrow.lock_funds(&s.depositor, &20, &1_000, &NO_DEADLINE);

    let info = s.escrow.get_escrow_info(&20);
    assert_eq!(info.deadline, NO_DEADLINE);
    assert_eq!(info.status, EscrowStatus::Locked);
}

#[test]
fn test_no_deadline_refund_blocked_without_approval() {
    let s = Setup::new();
    s.escrow.lock_funds(&s.depositor, &21, &1_000, &NO_DEADLINE);

    let result = s.escrow.try_refund(&21);
    assert_eq!(result.unwrap_err().unwrap(), Error::DeadlineNotPassed);
}

#[test]
fn test_no_deadline_refund_blocked_even_after_large_time_advance() {
    let s = Setup::new();
    s.escrow.lock_funds(&s.depositor, &22, &1_000, &NO_DEADLINE);
    s.env.ledger().set_timestamp(100 * 365 * 24 * 3600);

    let result = s.escrow.try_refund(&22);
    assert_eq!(result.unwrap_err().unwrap(), Error::DeadlineNotPassed);
}

#[test]
fn test_no_deadline_refund_succeeds_with_admin_approval() {
    let s = Setup::new();
    s.escrow.lock_funds(&s.depositor, &23, &1_500, &NO_DEADLINE);
    s.escrow
        .approve_refund(&23, &1_500, &s.depositor, &RefundMode::Full);

    let before = s.token.balance(&s.depositor);
    s.escrow.refund(&23);

    let info = s.escrow.get_escrow_info(&23);
    assert_eq!(info.status, EscrowStatus::Refunded);
    assert_eq!(s.token.balance(&s.depositor), before + 1_500);
    assert_eq!(s.token.balance(&s.escrow.address), 0);
}

#[test]
fn test_no_deadline_partial_refund_with_admin_approval() {
    let s = Setup::new();
    s.escrow.lock_funds(&s.depositor, &24, &2_000, &NO_DEADLINE);
    s.escrow
        .approve_refund(&24, &800, &s.depositor, &RefundMode::Partial);

    s.escrow.refund(&24);

    let info = s.escrow.get_escrow_info(&24);
    assert_eq!(info.status, EscrowStatus::PartiallyRefunded);
    assert_eq!(info.remaining_amount, 1_200);
    assert_eq!(s.token.balance(&s.escrow.address), 1_200);
}

#[test]
fn test_no_deadline_release_succeeds() {
    let s = Setup::new();
    s.escrow.lock_funds(&s.depositor, &25, &2_500, &NO_DEADLINE);

    s.escrow.release_funds(&25, &s.contributor);

    let info = s.escrow.get_escrow_info(&25);
    assert_eq!(info.status, EscrowStatus::Released);
    assert_eq!(s.token.balance(&s.contributor), 2_500);
    assert_eq!(s.token.balance(&s.escrow.address), 0);
}

#[test]
fn test_mixed_deadline_refund_independence() {
    let s = Setup::new();
    let future = s.env.ledger().timestamp() + 1_000;
    s.escrow.lock_funds(&s.depositor, &32, &1_000, &future);
    s.escrow.lock_funds(&s.depositor, &33, &1_000, &NO_DEADLINE);

    s.env.ledger().set_timestamp(future + 1);

    assert!(s.escrow.try_refund(&32).is_ok());
    assert_eq!(
        s.escrow.try_refund(&33).unwrap_err().unwrap(),
        Error::DeadlineNotPassed
    );
}
