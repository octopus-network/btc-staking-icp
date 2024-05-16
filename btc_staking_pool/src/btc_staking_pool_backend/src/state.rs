use alloc::collections::{BTreeMap, VecDeque};
use candid::{CandidType, Deserialize, Principal};
use ic_ledger_types::Subaccount;
use serde::Serialize;
use std::cell::RefCell;

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct Staker {
    pub eth_address: String,
    pub subaccount: Subaccount,
    pub tx_nonce: u64,
    pub ckbtc_balance: u64,
    pub otbtc_balance: u64,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct UnstakeRequest {
    pub eth_address: String,
    pub amount: u64,
    pub unlock_time: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BtcStakingPoolState {
    pub ckbtc_minting_account: Principal,
    pub ckbtc_ledger_account: Principal,
    pub otbtc_ledger_account: Principal,
    pub stakers_map: BTreeMap<String, Staker>,
    pub unstaking_queue: VecDeque<UnstakeRequest>,
    pub total_ckbtc_in_pool: u64,
    pub unbonding_period: u64,
}

thread_local! {
    static __STATE: RefCell<Option<BtcStakingPoolState>> = RefCell::default();
}

/// Take the current state.
///
/// After calling this function the state won't be initialized anymore.
/// Panics if there is no state.
pub fn take_state<F, R>(f: F) -> R
where
    F: FnOnce(BtcStakingPoolState) -> R,
{
    __STATE.with(|s| f(s.take().expect("State not initialized!")))
}

/// Mutates (part of) the current state using `f`.
///
/// Panics if there is no state.
pub fn mutate_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut BtcStakingPoolState) -> R,
{
    __STATE.with(|s| f(s.borrow_mut().as_mut().expect("State not initialized!")))
}

/// Read (part of) the current state using `f`.
///
/// Panics if there is no state.
pub fn read_state<F, R>(f: F) -> R
where
    F: FnOnce(&BtcStakingPoolState) -> R,
{
    __STATE.with(|s| f(s.borrow().as_ref().expect("State not initialized!")))
}

/// Replaces the current state.
pub fn replace_state(state: BtcStakingPoolState) {
    __STATE.with(|s| {
        *s.borrow_mut() = Some(state);
    });
}
