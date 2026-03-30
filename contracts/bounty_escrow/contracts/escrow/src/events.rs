use crate::{CapabilityAction, DisputeOutcome, DisputeReason, ReleaseType};
use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env, Symbol, String, Vec};

pub const EVENT_VERSION_V2: u32 = 2;

#[contracttype]
#[derive(Clone, Debug)]
pub struct BountyEscrowInitialized {
    pub version: u32,
    pub admin: Address,
    pub token: Address,
    pub timestamp: u64,
}

pub fn emit_bounty_initialized(env: &Env, event: BountyEscrowInitialized) {
    let topics = (symbol_short!("init"),);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FundsLocked {
    pub version: u32,
    pub bounty_id: u64,
    pub amount: i128,
    pub depositor: Address,
    pub deadline: u64,
}

pub fn emit_funds_locked(env: &Env, event: FundsLocked) {
    let topics = (symbol_short!("f_lock"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

/// Event for anonymous lock: only depositor commitment is emitted (no plaintext address).
#[contracttype]
#[derive(Clone, Debug)]
pub struct FundsLockedAnon {
    pub version: u32,
    pub bounty_id: u64,
    pub amount: i128,
    pub depositor_commitment: BytesN<32>,
    pub deadline: u64,
}

pub fn emit_funds_locked_anon(env: &Env, event: FundsLockedAnon) {
    let topics = (symbol_short!("lock_anon"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FundsReleased {
    pub version: u32,
    pub bounty_id: u64,
    pub amount: i128,
    pub recipient: Address,
    pub timestamp: u64,
}

pub fn emit_funds_released(env: &Env, event: FundsReleased) {
    let topics = (symbol_short!("f_rel"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

// ------------------------------------------------------------------------
// Scheduled release events
// ------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug)]
pub struct ScheduleCreated {
    pub bounty_id: u64,
    pub schedule_id: u64,
    pub amount: i128,
    pub recipient: Address,
    pub release_timestamp: u64,
    pub created_by: Address,
    pub timestamp: u64,
}

pub fn emit_schedule_created(env: &Env, event: ScheduleCreated) {
    let topics = (symbol_short!("sch_cr"), event.bounty_id, event.schedule_id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ScheduleReleased {
    pub bounty_id: u64,
    pub schedule_id: u64,
    pub amount: i128,
    pub recipient: Address,
    pub released_at: u64,
    pub released_by: Address,
    pub release_type: crate::ReleaseType,
}

pub fn emit_schedule_released(env: &Env, event: ScheduleReleased) {
    let topics = (symbol_short!("sch_rel"), event.bounty_id, event.schedule_id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FundsRefunded {
    pub version: u32,
    pub bounty_id: u64,
    pub amount: i128,
    pub refund_to: Address,
    pub timestamp: u64,
}

pub fn emit_funds_refunded(env: &Env, event: FundsRefunded) {
    let topics = (symbol_short!("f_ref"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

// ============================================================================
// Optional require-receipt for critical operations (Issue #677)
// ============================================================================

/// Outcome of a critical operation for receipt proof.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CriticalOperationOutcome {
    Released,
    Refunded,
}

/// Receipt (signed/committed proof of execution) for release or refund.
/// Emitted for each release/refund so users can prove completion off-chain;
/// optional on-chain verification via verify_receipt(receipt_id).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalOperationReceipt {
    /// Unique receipt id (monotonic counter).
    pub receipt_id: u64,
    /// Operation that was executed.
    pub outcome: CriticalOperationOutcome,
    /// Bounty that was released or refunded.
    pub bounty_id: u64,
    /// Amount transferred.
    pub amount: i128,
    /// Recipient (release) or refund_to (refund).
    pub party: Address,
    /// Ledger timestamp when the operation completed.
    pub timestamp: u64,
}

pub fn emit_operation_receipt(env: &Env, receipt: CriticalOperationReceipt) {
    let topics = (symbol_short!("receipt"), receipt.receipt_id);
    env.events().publish(topics, receipt.clone());
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeeOperationType {
    Lock,
    Release,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FeeCollected {
    pub version: u32,
    pub operation_type: FeeOperationType,
    pub amount: i128,
    pub fee_rate: i128,
    pub recipient: Address,
    pub timestamp: u64,
}

pub fn emit_fee_collected(env: &Env, event: FeeCollected) {
    let topics = (symbol_short!("fee"),);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BatchFundsLocked {
    pub version: u32,
    pub count: u32,
    pub total_amount: i128,
    pub timestamp: u64,
}

pub fn emit_batch_funds_locked(env: &Env, event: BatchFundsLocked) {
    let topics = (symbol_short!("b_lock"),);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FeeConfigUpdated {
    pub lock_fee_rate: i128,
    pub release_fee_rate: i128,
    pub fee_recipient: Address,
    pub fee_enabled: bool,
    pub timestamp: u64,
}

pub fn emit_fee_config_updated(env: &Env, event: FeeConfigUpdated) {
    let topics = (symbol_short!("fee_cfg"),);
    env.events().publish(topics, event.clone());
}

/// Event emitted when treasury destinations are updated
#[contracttype]
#[derive(Clone, Debug)]
pub struct TreasuryDistributionUpdated {
    pub destinations_count: u32,
    pub total_weight: u32,
    pub distribution_enabled: bool,
    pub timestamp: u64,
}

pub fn emit_treasury_distribution_updated(env: &Env, event: TreasuryDistributionUpdated) {
    let topics = (Symbol::new(env, "treasury_cfg"),);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MaintenanceModeChanged {
    pub enabled: bool,
    pub admin: Address,
    pub timestamp: u64,
}

pub fn emit_maintenance_mode_changed(env: &Env, event: MaintenanceModeChanged) {
    let topics = (symbol_short!("MaintSt"),);
    env.events().publish(topics, event.clone());
}

/// Event emitted when fees are distributed to treasury destinations
#[contracttype]
#[derive(Clone, Debug)]
pub struct TreasuryDistribution {
    pub version: u32,
    pub operation_type: FeeOperationType,
    pub total_amount: i128,
    pub distributions: Vec<TreasuryDistributionDetail>,
    pub timestamp: u64,
}

/// Detail for a single treasury distribution
#[contracttype]
#[derive(Clone, Debug)]
pub struct TreasuryDistributionDetail {
    pub destination_address: Address,
    pub region: String,
    pub amount: i128,
    pub weight: u32,
}

pub fn emit_treasury_distribution(env: &Env, event: TreasuryDistribution) {
    let topics = (Symbol::new(env, "treasury_dist"),);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BatchFundsReleased {
    pub version: u32,
    pub count: u32,
    pub total_amount: i128,
    pub timestamp: u64,
}

pub fn emit_batch_funds_released(env: &Env, event: BatchFundsReleased) {
    let topics = (symbol_short!("b_rel"),);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ApprovalAdded {
    pub bounty_id: u64,
    pub contributor: Address,
    pub approver: Address,
    pub timestamp: u64,
}

pub fn emit_approval_added(env: &Env, event: ApprovalAdded) {
    let topics = (symbol_short!("approval"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimCreated {
    pub bounty_id: u64, // use program_id+schedule_id equivalent in program-escrow
    pub recipient: Address,
    pub amount: i128,
    pub expires_at: u64,
    pub reason: DisputeReason,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimExecuted {
    pub bounty_id: u64,
    pub recipient: Address,
    pub amount: i128,
    pub claimed_at: u64,
    pub outcome: DisputeOutcome,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimCancelled {
    pub bounty_id: u64,
    pub recipient: Address,
    pub amount: i128,
    pub cancelled_at: u64,
    pub cancelled_by: Address,
    pub outcome: DisputeOutcome,
}

/// Event emitted when a claim ticket is issued to a bounty winner
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketIssued {
    pub ticket_id: u64,
    pub bounty_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
    pub expires_at: u64,
    pub issued_at: u64,
}

pub fn emit_ticket_issued(env: &Env, event: TicketIssued) {
    let topics = (symbol_short!("tkt_iss"), event.ticket_id);
    env.events().publish(topics, event.clone());
}

/// Event emitted when a beneficiary claims their reward using a ticket
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketClaimed {
    pub ticket_id: u64,
    pub bounty_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
    pub claimed_at: u64,
}

pub fn emit_ticket_claimed(env: &Env, event: TicketClaimed) {
    let topics = (symbol_short!("tkt_clm"), event.ticket_id);
    env.events().publish(topics, event.clone());
}

pub fn emit_pause_state_changed(env: &Env, event: crate::PauseStateChanged) {
    let topics = (symbol_short!("pause"), event.operation.clone());
    env.events().publish(topics, event);
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EmergencyWithdrawEvent {
    pub admin: Address,
    pub recipient: Address,
    pub amount: i128,
    pub timestamp: u64,
}

pub fn emit_emergency_withdraw(env: &Env, event: EmergencyWithdrawEvent) {
    let topics = (symbol_short!("em_wtd"),);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PromotionalPeriodCreated {
    pub id: u64,
    pub name: soroban_sdk::String,
    pub start_time: u64,
    pub end_time: u64,
    pub lock_fee_rate: i128,
    pub release_fee_rate: i128,
    pub is_global: bool,
    pub timestamp: u64,
}

pub fn emit_promotional_period_created(env: &Env, event: PromotionalPeriodCreated) {
    let topics = (symbol_short!("promo_c"), event.id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PromotionalPeriodUpdated {
    pub id: u64,
    pub enabled: bool,
    pub timestamp: u64,
}

pub fn emit_promotional_period_updated(env: &Env, event: PromotionalPeriodUpdated) {
    let topics = (symbol_short!("promo_u"), event.id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PromotionalPeriodActivated {
    pub id: u64,
    pub name: soroban_sdk::String,
    pub lock_fee_rate: i128,
    pub release_fee_rate: i128,
    pub timestamp: u64,
}

pub fn emit_promotional_period_activated(env: &Env, event: PromotionalPeriodActivated) {
    let topics = (symbol_short!("promo_a"), event.id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PromotionalPeriodExpired {
    pub id: u64,
    pub name: soroban_sdk::String,
    pub timestamp: u64,
}

pub fn emit_promotional_period_expired(env: &Env, event: PromotionalPeriodExpired) {
    let topics = (symbol_short!("promo_e"), event.id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityIssued {
    pub capability_id: u64,
    pub owner: Address,
    pub holder: Address,
    pub action: CapabilityAction,
    pub bounty_id: u64,
    pub amount_limit: i128,
    pub expires_at: u64,
    pub max_uses: u32,
    pub timestamp: u64,
}

pub fn emit_capability_issued(env: &Env, event: CapabilityIssued) {
    let topics = (symbol_short!("cap_new"), event.capability_id);
    env.events().publish(topics, event);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityUsed {
    pub capability_id: u64,
    pub holder: Address,
    pub action: CapabilityAction,
    pub bounty_id: u64,
    pub amount_used: i128,
    pub remaining_amount: i128,
    pub remaining_uses: u32,
    pub used_at: u64,
}

pub fn emit_capability_used(env: &Env, event: CapabilityUsed) {
    let topics = (symbol_short!("cap_use"), event.capability_id);
    env.events().publish(topics, event);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityRevoked {
    pub capability_id: u64,
    pub owner: Address,
    pub revoked_at: u64,
}

pub fn emit_capability_revoked(env: &Env, event: CapabilityRevoked) {
    let topics = (symbol_short!("cap_rev"), event.capability_id);
    env.events().publish(topics, event);
}

/// Emitted when the contract is deprecated or un-deprecated (kill switch / migration path).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeprecationStateChanged {
    pub deprecated: bool,
    pub migration_target: Option<Address>,
    pub admin: Address,
    pub timestamp: u64,
}

pub fn emit_deprecation_state_changed(env: &Env, event: DeprecationStateChanged) {
    let topics = (symbol_short!("deprec"),);
    env.events().publish(topics, event);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataUpdated {
    pub bounty_id: u64,
    pub repo_id: u64,
    pub issue_id: u64,
    pub bounty_type: soroban_sdk::String,
    pub reference_hash: Option<soroban_sdk::Bytes>,
    pub timestamp: u64,
}

pub fn emit_metadata_updated(env: &Env, bounty_id: u64, metadata: crate::EscrowMetadata) {
    let topics = (symbol_short!("meta_upd"), bounty_id);
    let event = MetadataUpdated {
        bounty_id,
        repo_id: metadata.repo_id,
        issue_id: metadata.issue_id,
        bounty_type: metadata.bounty_type,
        reference_hash: metadata.reference_hash,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, event);
}

// ==================== Event Batching (Issue #676) ====================
// Compact action summary for batch events. Indexers can decode a single
// EventBatch instead of N individual events during high-volume periods.
// action_type: 1=Lock, 2=Release, 3=Refund (u32 for Soroban contracttype)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSummary {
    pub bounty_id: u64,
    pub action_type: u32,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EventBatch {
    pub version: u32,
    pub batch_type: u32, // 1=lock, 2=release
    pub actions: soroban_sdk::Vec<ActionSummary>,
    pub total_amount: i128,
    pub timestamp: u64,
}

pub fn emit_event_batch(env: &Env, event: EventBatch) {
    let topics = (symbol_short!("ev_batch"), event.batch_type);
    env.events().publish(topics, event.clone());
}

// ==================== Owner Lock (Issue #675) ====================
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowLockedEvent {
    pub bounty_id: u64,
    pub locked_by: Address,
    pub locked_until: Option<u64>,
    pub reason: Option<soroban_sdk::String>,
    pub timestamp: u64,
}

pub fn emit_escrow_locked(env: &Env, event: EscrowLockedEvent) {
    let topics = (symbol_short!("esc_lock"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowUnlockedEvent {
    pub bounty_id: u64,
    pub unlocked_by: Address,
    pub timestamp: u64,
}

pub fn emit_escrow_unlocked(env: &Env, event: EscrowUnlockedEvent) {
    let topics = (symbol_short!("esc_unl"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

// ==================== Clone/Fork (Issue #678) ====================
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowClonedEvent {
    pub source_bounty_id: u64,
    pub new_bounty_id: u64,
    pub new_owner: Address,
    pub timestamp: u64,
}

pub fn emit_escrow_cloned(env: &Env, event: EscrowClonedEvent) {
    let topics = (symbol_short!("esc_clone"), event.new_bounty_id);
    env.events().publish(topics, event.clone());
}

// ==================== Archive on Completion (Issue #684) ====================
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowArchivedEvent {
    pub bounty_id: u64,
    pub reason: soroban_sdk::String, // e.g. "completed", "released", "refunded"
    pub archived_at: u64,
}

pub fn emit_escrow_archived(env: &Env, event: EscrowArchivedEvent) {
    let topics = (symbol_short!("esc_arch"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

// ==================== Renew / Rollover (Issue #679) ====================

/// Event emitted when an escrow is renewed (deadline extended, same bounty_id).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowRenewedEvent {
    pub bounty_id: u64,
    pub old_deadline: u64,
    pub new_deadline: u64,
    pub additional_amount: i128,
    pub cycle: u32,
    pub renewed_at: u64,
}

pub fn emit_escrow_renewed(env: &Env, event: EscrowRenewedEvent) {
    let topics = (symbol_short!("esc_rnw"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

/// Event emitted when a new escrow cycle is created, linked to a previous one.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCycleCreatedEvent {
    pub previous_bounty_id: u64,
    pub new_bounty_id: u64,
    pub cycle: u32,
    pub amount: i128,
    pub deadline: u64,
    pub created_at: u64,
}

pub fn emit_new_cycle_created(env: &Env, event: NewCycleCreatedEvent) {
    let topics = (symbol_short!("new_cyc"), event.new_bounty_id);
    env.events().publish(topics, event.clone());
}

// ==================== Frozen Balance (Issue #578) ====================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowFrozenEvent {
    pub bounty_id: u64,
    pub frozen_by: Address,
    pub reason: Option<soroban_sdk::String>,
    pub frozen_at: u64,
}

pub fn emit_escrow_frozen(env: &Env, event: EscrowFrozenEvent) {
    let topics = (symbol_short!("esc_frz"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowUnfrozenEvent {
    pub bounty_id: u64,
    pub unfrozen_by: Address,
    pub unfrozen_at: u64,
}

pub fn emit_escrow_unfrozen(env: &Env, event: EscrowUnfrozenEvent) {
    let topics = (symbol_short!("esc_ufrz"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressFrozenEvent {
    pub address: Address,
    pub frozen_by: Address,
    pub reason: Option<soroban_sdk::String>,
    pub frozen_at: u64,
}

pub fn emit_address_frozen(env: &Env, event: AddressFrozenEvent) {
    let topics = (symbol_short!("addr_frz"),);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressUnfrozenEvent {
    pub address: Address,
    pub unfrozen_by: Address,
    pub unfrozen_at: u64,
}

pub fn emit_address_unfrozen(env: &Env, event: AddressUnfrozenEvent) {
    let topics = (symbol_short!("addr_ufrz"),);
    env.events().publish(topics, event.clone());
}

// ------------------------------------------------------------------------
// Settlement Grace Period Events
// ------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug)]
pub struct SettlementGracePeriodEntered {
    pub version: u32,
    pub bounty_id: u64,
    pub grace_end_time: u64,
    pub settlement_type: Symbol,
    pub timestamp: u64,
}

pub fn emit_settlement_grace_period_entered(
    env: &Env,
    event: SettlementGracePeriodEntered,
) {
    let topics = (symbol_short!("grace_in"), event.bounty_id);
    env.events().publish(topics, event.clone());
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SettlementCompleted {
    pub version: u32,
    pub bounty_id: u64,
    pub amount: i128,
    pub recipient: Address,
    pub settlement_type: Symbol,
    pub timestamp: u64,
}

pub fn emit_settlement_completed(env: &Env, event: SettlementCompleted) {
    let topics = (Symbol::new(env, "settle_done"), event.bounty_id);
    env.events().publish(topics, event.clone());
}
