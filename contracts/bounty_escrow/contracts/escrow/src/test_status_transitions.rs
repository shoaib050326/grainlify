use super::*;
use soroban_sdk::testutils::{Events, Ledger};
use soroban_sdk::{
    testutils::{Address as _, LedgerInfo, MockAuth, MockAuthInvoke},
    token, Address, Env, IntoVal, Symbol, TryIntoVal, Val,
};

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract = e.register_stellar_asset_contract_v2(admin.clone());
    let contract_address = contract.address();
    (
        token::Client::new(e, &contract_address),
        token::StellarAssetClient::new(e, &contract_address),
    )
}

fn create_escrow_contract<'a>(e: &Env) -> BountyEscrowContractClient<'a> {
    let contract_id = e.register_contract(None, BountyEscrowContract);
    BountyEscrowContractClient::new(e, &contract_id)
}

struct TestSetup<'a> {
    env: Env,
    #[allow(dead_code)]
    admin: Address,
    depositor: Address,
    contributor: Address,
    #[allow(dead_code)]
    token: token::Client<'a>,
    #[allow(dead_code)]
    token_admin: token::StellarAssetClient<'a>,
    escrow: BountyEscrowContractClient<'a>,
}

impl<'a> TestSetup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        let escrow = create_escrow_contract(&env);

        escrow.init(&admin, &token.address);
        token_admin.mint(&depositor, &1_000_000);

        Self {
            env,
            admin,
            depositor,
            contributor,
            token,
            token_admin,
            escrow,
        }
    }
}

struct RotationSetup<'a> {
    env: Env,
    admin: Address,
    pending_admin: Address,
    replacement_admin: Address,
    escrow: BountyEscrowContractClient<'a>,
}

impl<'a> RotationSetup<'a> {
    fn new() -> Self {
        let env = Env::default();
        let admin = Address::generate(&env);
        let pending_admin = Address::generate(&env);
        let replacement_admin = Address::generate(&env);
        let (token, _token_admin) = create_token_contract(&env, &admin);
        let escrow = create_escrow_contract(&env);

        authorize_contract_call(
            &env,
            &escrow,
            &admin,
            "init",
            (&admin, &token.address).into_val(&env),
        );
        escrow.init(&admin, &token.address);

        Self {
            env,
            admin,
            pending_admin,
            replacement_admin,
            escrow,
        }
    }

    fn authorize(&self, address: &Address, fn_name: &'static str, args: Val) {
        authorize_contract_call(&self.env, &self.escrow, address, fn_name, args);
    }
}

fn authorize_contract_call(
    env: &Env,
    escrow: &BountyEscrowContractClient<'_>,
    address: &Address,
    fn_name: &'static str,
    args: Val,
) {
    env.mock_auths(&[MockAuth {
        address,
        invoke: &MockAuthInvoke {
            contract: &escrow.address,
            fn_name,
            args,
            sub_invokes: &[],
        },
    }]);
}

#[test]
fn test_refund_eligibility_ineligible_before_deadline_without_approval() {
    let setup = TestSetup::new();
    let bounty_id = 99;
    let amount = 1_000;
    let deadline = setup.env.ledger().timestamp() + 500;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    let view = setup.escrow.get_refund_eligibility_view(&bounty_id);
    assert!(!view.eligible);
    assert_eq!(
        view.code,
        RefundEligibilityCode::IneligibleDeadlineNotPassed
    );
    assert_eq!(view.amount, 0);
    assert!(!view.approval_present);
}

#[test]
fn test_refund_eligibility_eligible_after_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 100;
    let amount = 1_200;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    let view = setup.escrow.get_refund_eligibility_view(&bounty_id);
    assert!(view.eligible);
    assert_eq!(view.code, RefundEligibilityCode::EligibleDeadlinePassed);
    assert_eq!(view.amount, amount);
    assert_eq!(view.recipient, Some(setup.depositor.clone()));
    assert!(!view.approval_present);
}

#[test]
fn test_refund_eligibility_eligible_with_admin_approval_before_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 101;
    let amount = 2_000;
    let deadline = setup.env.ledger().timestamp() + 1_000;
    let custom_recipient = Address::generate(&setup.env);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &custom_recipient, &RefundMode::Partial);

    let view = setup.escrow.get_refund_eligibility_view(&bounty_id);
    assert!(view.eligible);
    assert_eq!(view.code, RefundEligibilityCode::EligibleAdminApproval);
    assert_eq!(view.amount, 500);
    assert_eq!(view.recipient, Some(custom_recipient));
    assert!(view.approval_present);
}

#[test]
fn test_maintenance_mode_blocks_lock_but_not_release_or_refund_paths() {
    let setup = TestSetup::new();
    let bounty_id = 202;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup.escrow.set_maintenance_mode(&true);

    // Lock should be blocked (maintenance mode acts like lock pause).
    let res = setup
        .escrow
        .try_lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    assert!(matches!(res, Err(Ok(Error::FundsPaused))));

    // Existing escrow should still be able to release/refund (maintenance mode only affects lock).
    setup
        .escrow
        .set_maintenance_mode(&false);
    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_maintenance_mode(&true);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// Valid transitions: Locked → Released
#[test]
fn test_locked_to_released() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Locked
    );

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
}

// Valid transitions: Locked → Refunded
#[test]
fn test_locked_to_refunded() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Locked
    );

    setup.env.ledger().set_timestamp(deadline + 1);
    setup.escrow.refund(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Refunded
    );
}

// Valid transitions: Locked → PartiallyRefunded
#[test]
fn test_locked_to_partially_refunded() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Locked
    );

    // Approve partial refund before deadline
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &setup.depositor, &RefundMode::Partial);
    setup.escrow.refund(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::PartiallyRefunded
    );
}

// Valid transitions: PartiallyRefunded → Refunded
#[test]
fn test_partially_refunded_to_refunded() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // First partial refund
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &setup.depositor, &RefundMode::Partial);
    setup.escrow.refund(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::PartiallyRefunded
    );

    // Second refund completes it
    setup.env.ledger().set_timestamp(deadline + 1);
    setup.escrow.refund(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Refunded
    );
}

// Invalid transition: Released → Locked
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_released_to_locked_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

// Invalid transition: Released → Released
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_released_to_released_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// Invalid transition: Released → Refunded
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_released_to_refunded_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup.env.ledger().set_timestamp(deadline + 1);
    setup.escrow.refund(&bounty_id);
}

// Invalid transition: Released → PartiallyRefunded
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_released_to_partially_refunded_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup.env.ledger().set_timestamp(deadline + 1);
    setup
        .escrow
        .partial_release(&bounty_id, &setup.contributor, &500);
}

// Invalid transition: Refunded → Locked
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_refunded_to_locked_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set(LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });
    setup.escrow.refund(&bounty_id);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

// Invalid transition: Refunded → Released
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_refunded_to_released_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set(LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });
    setup.escrow.refund(&bounty_id);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// Invalid transition: Refunded → Refunded
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_refunded_to_refunded_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set(LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });
    setup.escrow.refund(&bounty_id);

    setup.escrow.refund(&bounty_id);
}

// Invalid transition: Refunded → PartiallyRefunded
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_refunded_to_partially_refunded_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set(LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });
    setup.escrow.refund(&bounty_id);

    setup
        .escrow
        .partial_release(&bounty_id, &setup.contributor, &100);
}

// Invalid transition: PartiallyRefunded → Locked
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_partially_refunded_to_locked_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &setup.depositor, &RefundMode::Partial);
    setup.escrow.refund(&bounty_id);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

// Invalid transition: PartiallyRefunded → Released
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_partially_refunded_to_released_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &setup.depositor, &RefundMode::Partial);
    setup.escrow.refund(&bounty_id);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// ============================================================================
// RISK FLAGS GOVERNANCE TESTS
// ============================================================================

#[test]
fn test_update_risk_flags_success() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Lock funds to create the initial escrow
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Verify initial risk flags are 0 (no metadata existed yet, fallback applied)
    assert_eq!(setup.escrow.get_risk_flags(&bounty_id), 0);

    // Update risk flags (e.g., HIGH_RISK = 1, UNDER_REVIEW = 2) -> Bitmask 3
    let new_flags = 3;
    setup.escrow.update_risk_flags(&bounty_id, &new_flags);

    // Verify flags persisted in the EscrowMetadata struct
    assert_eq!(setup.escrow.get_risk_flags(&bounty_id), new_flags);
    
    // Clear the flags
    setup.escrow.update_risk_flags(&bounty_id, &0);
    assert_eq!(setup.escrow.get_risk_flags(&bounty_id), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #202)")]
fn test_update_risk_flags_bounty_not_found() {
    let setup = TestSetup::new();
    let missing_bounty_id = 999;
    
    // Attempting to flag an escrow that does not exist should throw BountyNotFound (202)
    setup.escrow.update_risk_flags(&missing_bounty_id, &1);
}

#[test]
#[should_panic(expected = "Error(Contract, #202)")]
fn test_get_risk_flags_bounty_not_found() {
    let setup = TestSetup::new();
    let missing_bounty_id = 999;
    
    // Attempting to read flags from a missing escrow should fail
    setup.escrow.get_risk_flags(&missing_bounty_id);
}

// ============================================================================
// MAINTENANCE MODE HARDENING TESTS
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
fn test_maintenance_mode_halts_lock() {
    let setup = TestSetup::new();
    let reason = soroban_sdk::String::from_str(&setup.env, "Emergency upgrade");
    setup.escrow.set_maintenance_mode(&true, &Some(reason));
    
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;
    
    // Should panic with FundsPaused (18)
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
fn test_maintenance_mode_halts_release() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;
    
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    
    setup.escrow.set_maintenance_mode(&true, &None);
    
    // Should panic with FundsPaused (18)
    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
fn test_maintenance_mode_halts_refund() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;
    
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);
    
    setup.escrow.set_maintenance_mode(&true, &None);
    
    // Should panic with FundsPaused (18)
    setup.escrow.refund(&bounty_id);
}

#[test]
fn test_maintenance_mode_toggles_correctly() {
    let setup = TestSetup::new();
    let reason = soroban_sdk::String::from_str(&setup.env, "Routine sync");
    
    assert_eq!(setup.escrow.is_maintenance_mode(), false);
    
    setup.escrow.set_maintenance_mode(&true, &Some(reason));
    assert_eq!(setup.escrow.is_maintenance_mode(), true);
    
    setup.escrow.set_maintenance_mode(&false, &None);
    assert_eq!(setup.escrow.is_maintenance_mode(), false);
}

// ============================================================================
// CLAIM-WINDOW VALIDATION TESTS (Issue #1031)
// ============================================================================

/// Helper: lock a bounty and authorize a claim with a given window.
fn setup_claim_window_bounty(
    setup: &TestSetup,
    bounty_id: u64,
    amount: i128,
    claim_window_secs: u64,
) -> Address {
    let deadline = setup.env.ledger().timestamp() + 10_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&claim_window_secs);
    let recipient = Address::generate(&setup.env);
    setup.escrow.authorize_claim(&bounty_id, &recipient, &DisputeReason::Other);
    recipient
}

// --- set_claim_window ---

#[test]
fn test_set_claim_window_success() {
    let setup = TestSetup::new();
    // Should not panic; no return value to assert beyond no error.
    setup.escrow.set_claim_window(&3600_u64);
}

#[test]
fn test_set_claim_window_zero_disables_enforcement() {
    let setup = TestSetup::new();
    let bounty_id = 300;
    let amount = 1_000;
    // Set window to 0 — enforcement disabled.
    setup.escrow.set_claim_window(&0_u64);
    let deadline = setup.env.ledger().timestamp() + 10_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor, &DisputeReason::Other);
    // Advance time far past any window — should still succeed because window == 0.
    setup.env.ledger().set_timestamp(setup.env.ledger().timestamp() + 999_999);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
}

// --- validate_claim_window: no pending claim ---

#[test]
fn test_release_without_pending_claim_skips_window_check() {
    let setup = TestSetup::new();
    let bounty_id = 301;
    let amount = 1_000;
    let deadline = setup.env.ledger().timestamp() + 10_000;
    setup.escrow.set_claim_window(&60_u64);
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    // No authorize_claim called — no PendingClaim exists.
    // release_funds should succeed regardless of window.
    setup.escrow.release_funds(&bounty_id, &setup.contributor);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
}

// --- validate_claim_window: claim within window ---

#[test]
fn test_claim_within_window_succeeds() {
    let setup = TestSetup::new();
    let bounty_id = 302;
    let amount = 1_000;
    let recipient = setup_claim_window_bounty(&setup, bounty_id, amount, 3_600);
    // Still within the window — claim should succeed.
    setup.escrow.claim(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
    let _ = recipient; // used via authorize_claim
}

#[test]
fn test_release_within_window_succeeds() {
    let setup = TestSetup::new();
    let bounty_id = 303;
    let amount = 1_000;
    let _recipient = setup_claim_window_bounty(&setup, bounty_id, amount, 3_600);
    // Admin releases within the window — should succeed.
    setup.escrow.release_funds(&bounty_id, &setup.contributor);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
}

// --- validate_claim_window: claim at exact boundary ---

#[test]
fn test_claim_at_exact_window_boundary_succeeds() {
    let setup = TestSetup::new();
    let bounty_id = 304;
    let amount = 1_000;
    let window = 3_600_u64;
    let now = setup.env.ledger().timestamp();
    let deadline = now + 10_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&window);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor, &DisputeReason::Other);
    // Advance to exactly expires_at (now + window).
    setup.env.ledger().set_timestamp(now + window);
    // At the boundary (now == expires_at) the window is still valid.
    setup.escrow.claim(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
}

// --- validate_claim_window: expired window ---

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_claim_after_window_expires_fails() {
    let setup = TestSetup::new();
    let bounty_id = 305;
    let amount = 1_000;
    let window = 60_u64;
    let now = setup.env.ledger().timestamp();
    let deadline = now + 10_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&window);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor, &DisputeReason::Other);
    // Advance past the window.
    setup.env.ledger().set_timestamp(now + window + 1);
    // Should panic with DeadlineNotPassed (#6).
    setup.escrow.claim(&bounty_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_release_after_window_expires_fails() {
    let setup = TestSetup::new();
    let bounty_id = 306;
    let amount = 1_000;
    let window = 60_u64;
    let now = setup.env.ledger().timestamp();
    let deadline = now + 10_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&window);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor, &DisputeReason::Other);
    // Advance past the window.
    setup.env.ledger().set_timestamp(now + window + 1);
    // Should panic with DeadlineNotPassed (#6).
    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// --- validate_claim_window: window not configured ---

#[test]
fn test_release_with_no_window_configured_succeeds() {
    let setup = TestSetup::new();
    let bounty_id = 307;
    let amount = 1_000;
    let deadline = setup.env.ledger().timestamp() + 10_000;
    // No set_claim_window call — defaults to 0 (disabled).
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor, &DisputeReason::Other);
    // Advance time significantly — no window enforcement.
    setup.env.ledger().set_timestamp(setup.env.ledger().timestamp() + 999_999);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
}

// --- cancel then re-authorize ---

#[test]
fn test_cancel_expired_claim_then_authorize_new_window() {
    let setup = TestSetup::new();
    let bounty_id = 308;
    let amount = 1_000;
    let window = 60_u64;
    let now = setup.env.ledger().timestamp();
    let deadline = now + 10_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&window);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor, &DisputeReason::Other);
    // Expire the first window.
    setup.env.ledger().set_timestamp(now + window + 1);
    // Admin cancels the stale claim.
    setup.escrow.cancel_pending_claim(&bounty_id, &DisputeOutcome::CancelledByAdmin);
    // Re-authorize with a fresh window.
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor, &DisputeReason::Other);
    // Claim should now succeed within the new window.
    setup.escrow.claim(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
}

// --- isolation: window on one bounty does not affect another ---

#[test]
fn test_claim_window_isolation_between_bounties() {
    let setup = TestSetup::new();
    let bounty_a = 309;
    let bounty_b = 310;
    let amount = 1_000;
    let window = 60_u64;
    let now = setup.env.ledger().timestamp();
    let deadline = now + 10_000;

    setup.escrow.set_claim_window(&window);

    // Lock both bounties.
    setup.escrow.lock_funds(&setup.depositor, &bounty_a, &amount, &deadline);
    setup.escrow.lock_funds(&setup.depositor, &bounty_b, &amount, &deadline);

    // Authorize claim on bounty_a only.
    setup.escrow.authorize_claim(&bounty_a, &setup.contributor, &DisputeReason::Other);

    // Advance past the window for bounty_a.
    setup.env.ledger().set_timestamp(now + window + 1);

    // bounty_b has no pending claim — release should succeed.
    setup.escrow.release_funds(&bounty_b, &setup.contributor);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_b).status,
        EscrowStatus::Released
    );
}

// --- audit event emission ---

#[test]
fn test_set_claim_window_emits_event() {
    let setup = TestSetup::new();
    setup.escrow.set_claim_window(&7200_u64);
    let events = setup.env.events().all();
    let found = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1
            && topics
                .get(0)
                .map(|t| t == soroban_sdk::Symbol::new(&setup.env, "cw_set").into_val(&setup.env))
                .unwrap_or(false)
    });
    assert!(found, "ClaimWindowSet event not emitted");
}

#[test]
fn test_claim_window_validated_event_emitted_on_success() {
    let setup = TestSetup::new();
    let bounty_id = 311;
    let amount = 1_000;
    let _recipient = setup_claim_window_bounty(&setup, bounty_id, amount, 3_600);
    setup.escrow.claim(&bounty_id);
    let events = setup.env.events().all();
    let found = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1
            && topics
                .get(0)
                .map(|t| t == soroban_sdk::Symbol::new(&setup.env, "cw_ok").into_val(&setup.env))
                .unwrap_or(false)
    });
    assert!(found, "ClaimWindowValidated event not emitted");
}

#[test]
fn test_claim_window_expired_event_emitted_on_failure() {
    let setup = TestSetup::new();
    let bounty_id = 312;
    let amount = 1_000;
    let window = 60_u64;
    let now = setup.env.ledger().timestamp();
    let deadline = now + 10_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&window);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor, &DisputeReason::Other);
    setup.env.ledger().set_timestamp(now + window + 1);
    // Attempt claim — will fail, but the expired event should be emitted.
    let _ = setup.escrow.try_claim(&bounty_id);
    let events = setup.env.events().all();
    let found = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1
            && topics
                .get(0)
                .map(|t| t == soroban_sdk::Symbol::new(&setup.env, "cw_exp").into_val(&setup.env))
                .unwrap_or(false)
    });
    assert!(found, "ClaimWindowExpired event not emitted");
}

// ============================================================================
// FEE ROUTING INVARIANTS TESTS (Issue #50)
// ============================================================================

/// Helper: configure a 2% lock fee and 1% release fee on the global config.
fn setup_fee_config(setup: &TestSetup, fee_recipient: &Address) {
    setup.escrow.update_fee_config(
        &Some(200i128),  // 2% lock fee
        &Some(100i128),  // 1% release fee
        &None,
        &None,
        &Some(fee_recipient.clone()),
        &Some(true),
    );
}

// --- set_fee_routing: basic happy path ---

#[test]
fn test_set_fee_routing_treasury_only() {
    let setup = TestSetup::new();
    let bounty_id = 400u64;
    let amount = 10_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    let treasury = Address::generate(&setup.env);
    // treasury_bps = 10_000 (100%), no partner
    setup.escrow.set_fee_routing(&bounty_id, &treasury, &10_000i128, &None, &0i128);

    let routing = setup.escrow.get_fee_routing(&bounty_id).expect("routing must be set");
    assert_eq!(routing.treasury_recipient, treasury);
    assert_eq!(routing.treasury_bps, 10_000);
    assert!(routing.partner_recipient.is_none());
    assert_eq!(routing.partner_bps, 0);
}

#[test]
fn test_set_fee_routing_with_partner() {
    let setup = TestSetup::new();
    let bounty_id = 401u64;
    let amount = 10_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    let treasury = Address::generate(&setup.env);
    let partner = Address::generate(&setup.env);
    // 80% treasury, 20% partner
    setup.escrow.set_fee_routing(&bounty_id, &treasury, &8_000i128, &Some(partner.clone()), &2_000i128);

    let routing = setup.escrow.get_fee_routing(&bounty_id).expect("routing must be set");
    assert_eq!(routing.treasury_bps, 8_000);
    assert_eq!(routing.partner_recipient, Some(partner));
    assert_eq!(routing.partner_bps, 2_000);
}

// --- set_fee_routing: invariant violations ---

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_set_fee_routing_shares_dont_sum_to_10000_rejected() {
    let setup = TestSetup::new();
    let bounty_id = 402u64;
    let amount = 10_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    let treasury = Address::generate(&setup.env);
    let partner = Address::generate(&setup.env);
    // 70% + 20% = 90% — must be rejected (InvalidAmount = 13)
    setup.escrow.set_fee_routing(&bounty_id, &treasury, &7_000i128, &Some(partner), &2_000i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_set_fee_routing_treasury_only_wrong_bps_rejected() {
    let setup = TestSetup::new();
    let bounty_id = 403u64;
    let amount = 10_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    let treasury = Address::generate(&setup.env);
    // No partner but treasury_bps != 10_000 — must be rejected
    setup.escrow.set_fee_routing(&bounty_id, &treasury, &9_000i128, &None, &0i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #202)")]
fn test_set_fee_routing_bounty_not_found_rejected() {
    let setup = TestSetup::new();
    let treasury = Address::generate(&setup.env);
    // Bounty 999 does not exist
    setup.escrow.set_fee_routing(&999u64, &treasury, &10_000i128, &None, &0i128);
}

// --- get_fee_routing: returns None when not set ---

#[test]
fn test_get_fee_routing_returns_none_when_unset() {
    let setup = TestSetup::new();
    let bounty_id = 404u64;
    let amount = 10_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    assert!(setup.escrow.get_fee_routing(&bounty_id).is_none());
}

// --- fee routing invariant: treasury-only routing ---

#[test]
fn test_fee_routing_treasury_only_receives_full_fee_on_lock() {
    let setup = TestSetup::new();
    let bounty_id = 405u64;
    let gross = 100_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;

    let treasury = Address::generate(&setup.env);
    let fee_recipient = Address::generate(&setup.env);

    // Enable 2% lock fee globally
    setup_fee_config(&setup, &fee_recipient);

    // Lock first so bounty exists, then set routing
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &gross, &deadline);
    // fee = ceil(100_000 * 200 / 10_000) = 2_000 already went to fee_recipient (no routing yet)
    // Now set routing for a NEW bounty
    let bounty_id2 = 406u64;
    setup.token_admin.mint(&setup.depositor, &gross);
    setup.escrow.lock_funds(&setup.depositor, &bounty_id2, &gross, &deadline);
    setup.escrow.set_fee_routing(&bounty_id2, &treasury, &10_000i128, &None, &0i128);

    // Lock a third bounty that has routing pre-set — but routing is set after lock,
    // so we test release routing instead.
    let token_client = soroban_sdk::token::TokenClient::new(&setup.env, &setup.token.address);
    let treasury_balance_before = token_client.balance(&treasury);

    // Release bounty_id2 — release fee (1%) should go to treasury via per-bounty routing
    setup.escrow.release_funds(&bounty_id2, &setup.contributor);

    // release fee = ceil(net_amount * 100 / 10_000)
    // net_amount after 2% lock fee = 100_000 - 2_000 = 98_000
    // release fee = ceil(98_000 * 100 / 10_000) = 980
    let treasury_balance_after = token_client.balance(&treasury);
    assert_eq!(
        treasury_balance_after - treasury_balance_before,
        980,
        "treasury must receive the full release fee via per-bounty routing"
    );
}

#[test]
fn test_fee_routing_partner_split_invariant_holds() {
    let setup = TestSetup::new();
    let bounty_id = 407u64;
    let gross = 100_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;

    let treasury = Address::generate(&setup.env);
    let partner = Address::generate(&setup.env);
    let fee_recipient = Address::generate(&setup.env);

    setup_fee_config(&setup, &fee_recipient);
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &gross, &deadline);
    // Set 70/30 split for release fee
    setup.escrow.set_fee_routing(&bounty_id, &treasury, &7_000i128, &Some(partner.clone()), &3_000i128);

    let token_client = soroban_sdk::token::TokenClient::new(&setup.env, &setup.token.address);
    let treasury_before = token_client.balance(&treasury);
    let partner_before = token_client.balance(&partner);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    // net_amount = 100_000 - 2_000 = 98_000
    // release fee = ceil(98_000 * 100 / 10_000) = 980
    // treasury share = floor(980 * 7_000 / 10_000) = 686
    // partner share = 980 - 686 = 294
    let treasury_after = token_client.balance(&treasury);
    let partner_after = token_client.balance(&partner);

    let treasury_received = treasury_after - treasury_before;
    let partner_received = partner_after - partner_before;

    // Invariant: treasury + partner == total fee
    assert_eq!(
        treasury_received + partner_received,
        980,
        "fee routing invariant: treasury + partner must equal total release fee"
    );
    assert_eq!(treasury_received, 686, "treasury must receive 70% of fee");
    assert_eq!(partner_received, 294, "partner must receive 30% of fee");
}

// --- audit event emission ---

#[test]
fn test_set_fee_routing_emits_fee_routing_updated_event() {
    let setup = TestSetup::new();
    let bounty_id = 408u64;
    let amount = 10_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;
    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    let treasury = Address::generate(&setup.env);
    setup.escrow.set_fee_routing(&bounty_id, &treasury, &10_000i128, &None, &0i128);

    let events = setup.env.events().all();
    let found = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1
            && topics
                .get(0)
                .map(|t| t == soroban_sdk::Symbol::new(&setup.env, "fee_rte").into_val(&setup.env))
                .unwrap_or(false)
    });
    assert!(found, "FeeRoutingUpdated event must be emitted by set_fee_routing");
}

#[test]
fn test_fee_routing_emits_fee_routed_event_on_release() {
    let setup = TestSetup::new();
    let bounty_id = 409u64;
    let gross = 50_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;

    let treasury = Address::generate(&setup.env);
    let fee_recipient = Address::generate(&setup.env);
    setup_fee_config(&setup, &fee_recipient);

    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &gross, &deadline);
    setup.escrow.set_fee_routing(&bounty_id, &treasury, &10_000i128, &None, &0i128);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    let events = setup.env.events().all();
    let found = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1
            && topics
                .get(0)
                .map(|t| t == soroban_sdk::Symbol::new(&setup.env, "fee_rt").into_val(&setup.env))
                .unwrap_or(false)
    });
    assert!(found, "FeeRouted event must be emitted when per-bounty routing is active");
}

#[test]
fn test_fee_routing_emits_invariant_checked_event() {
    let setup = TestSetup::new();
    let bounty_id = 410u64;
    let gross = 50_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;

    let treasury = Address::generate(&setup.env);
    let fee_recipient = Address::generate(&setup.env);
    setup_fee_config(&setup, &fee_recipient);

    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &gross, &deadline);
    setup.escrow.set_fee_routing(&bounty_id, &treasury, &10_000i128, &None, &0i128);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    let events = setup.env.events().all();
    let found = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1
            && topics
                .get(0)
                .map(|t| t == soroban_sdk::Symbol::new(&setup.env, "fee_inv").into_val(&setup.env))
                .unwrap_or(false)
    });
    assert!(found, "FeeRoutingInvariantChecked event must be emitted");
}

// --- upgrade-safe schema version ---

#[test]
fn test_fee_routing_schema_version_set_on_init() {
    let setup = TestSetup::new();
    // FeeRoutingSchemaVersion must be written during init
    let events = setup.env.events().all();
    let found = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1
            && topics
                .get(0)
                .map(|t| t == soroban_sdk::Symbol::new(&setup.env, "fee_schm").into_val(&setup.env))
                .unwrap_or(false)
    });
    assert!(found, "FeeRoutingSchemaVersionSet event must be emitted during init");
}

// --- fallback: no per-bounty routing uses global path ---

#[test]
fn test_no_per_bounty_routing_falls_back_to_global() {
    let setup = TestSetup::new();
    let bounty_id = 411u64;
    let gross = 100_000i128;
    let deadline = setup.env.ledger().timestamp() + 1_000;

    let fee_recipient = Address::generate(&setup.env);
    setup_fee_config(&setup, &fee_recipient);

    setup.escrow.lock_funds(&setup.depositor, &bounty_id, &gross, &deadline);
    // No set_fee_routing call — global path should be used
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    let token_client = soroban_sdk::token::TokenClient::new(&setup.env, &setup.token.address);
    // release fee = ceil(98_000 * 100 / 10_000) = 980 → goes to fee_recipient
    assert_eq!(
        token_client.balance(&fee_recipient),
        // lock fee (2_000) + release fee (980)
        2_980,
        "global fee_recipient must receive both lock and release fees when no per-bounty routing"
    );
}
