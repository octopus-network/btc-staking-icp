use candid::{CandidType, Deserialize, Principal};
use ic_btc_interface::Utxo;
use ic_ledger_types::Subaccount;
use serde::Serialize;

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InitArgs {
    pub ckbtc_minting_account: Principal,
    pub ckbtc_ledger_account: Principal,
    pub otbtc_ledger_account: Principal,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GetBtcAddressArgs {
    pub owner: Option<Principal>,
    pub subaccount: Option<Subaccount>,
}

/// The argument of the [update_balance] endpoint.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UpdateBalanceArgs {
    /// The owner of the account on the ledger.
    /// The minter uses the caller principal if the owner is None.
    pub owner: Option<Principal>,
    /// The desired subaccount on the ledger, if any.
    pub subaccount: Option<Subaccount>,
}

/// The outcome of UTXO processing.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum UtxoStatus {
    /// The UTXO value does not cover the KYT check cost.
    ValueTooSmall(Utxo),
    /// The KYT check found issues with the deposited UTXO.
    Tainted(Utxo),
    /// The deposited UTXO passed the KYT check, but the minter failed to mint ckBTC on the ledger.
    /// The caller should retry the [update_balance] call.
    Checked(Utxo),
    /// The minter accepted the UTXO and minted ckBTC tokens on the ledger.
    Minted {
        /// The MINT transaction index on the ledger.
        block_index: u64,
        /// The minted amount (UTXO value minus fees).
        minted_amount: u64,
        /// The UTXO that caused the balance update.
        utxo: Utxo,
    },
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct UpdateBalanceResponse(pub Vec<UtxoStatus>);

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StakeArgs {
    pub eth_address: String,
    pub amount: u64,
    pub signature: Vec<u8>,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UnstakeArgs {
    pub eth_address: String,
    pub amount: u64,
    pub signature: Vec<u8>,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WithdrawBtcArgs {
    pub eth_address: String,
    pub amount: u64,
    pub signature: Vec<u8>,
}
