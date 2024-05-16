extern crate alloc;

mod errors;
mod state;
mod types;

use alloc::{
    collections::{BTreeMap, VecDeque},
    vec::Vec,
};
use core::fmt::Error;
use errors::{
    GetBtcDepositAddressError, StakeError, UnlockTokensInQueueError, UnstakeError,
    UpdateBalanceError, VerifySignatureError, WithdrawBtcError,
};
use ic_cdk::{init, update};
use ic_ledger_types::{AccountIdentifier, Memo, Subaccount, Tokens};
use libsecp256k1::{Message, RecoveryId, Signature};
use sha3::Digest;
use state::{BtcStakingPoolState, Staker, UnstakeRequest};
use types::{
    GetBtcAddressArgs, InitArgs, StakeArgs, UnstakeArgs, UpdateBalanceArgs, UpdateBalanceResponse,
    UtxoStatus, WithdrawBtcArgs,
};

const DEFAULT_UNBONDING_PERIOD: u64 = 60 * 60 * 24 * 14 * 1000000; // 2 weeks, in nano seconds

#[init]
fn init(init_args: InitArgs) {
    state::replace_state(BtcStakingPoolState {
        ckbtc_minting_account: init_args.ckbtc_minting_account,
        ckbtc_ledger_account: init_args.ckbtc_ledger_account,
        otbtc_ledger_account: init_args.otbtc_ledger_account,
        stakers_map: BTreeMap::new(),
        unstaking_queue: VecDeque::new(),
        total_ckbtc_in_pool: 0,
        unbonding_period: DEFAULT_UNBONDING_PERIOD,
    });
}

#[update]
async fn get_btc_deposit_address(eth_address: String) -> Result<String, GetBtcDepositAddressError> {
    let args = GetBtcAddressArgs {
        owner: Some(ic_cdk::id()),
        subaccount: Some(
            convert_eth_address_to_subaccount(&eth_address)
                .map_err(|_| GetBtcDepositAddressError::InvalidEthereumAddress)?,
        ),
    };
    let ckbtc_minting_account = state::read_state(|state| state.ckbtc_minting_account);
    let (address,) = ic_cdk::call(ckbtc_minting_account, "get_btc_address", (args,))
        .await
        .map_err(|e| {
            GetBtcDepositAddressError::CkbtcMinterError(format!(
                "failed to call ckBTC minter: {:?}",
                e
            ))
        })?;
    Ok(address)
}

fn keccak256(input: &[u8]) -> [u8; 32] {
    // Create a new Keccak-256 hasher
    let mut hasher = sha3::Keccak256::new();
    // Update the hasher with the input data
    hasher.update(input);
    // Finalize the hasher and obtain the result
    let result = hasher.finalize();
    // Return the result as a fixed-size array
    result.into()
}

/// Convert a string (considered to be an Ethereum address) to a subaccount (with 32 bytes).
fn convert_eth_address_to_subaccount(eth_address: &str) -> Result<Subaccount, Error> {
    if eth_address.len() != 20 {
        return Err(Error::default());
    }
    let address_bytes = hex::decode(eth_address).map_err(|_| Error::default())?;
    Ok(Subaccount(keccak256(&address_bytes)))
}

#[update]
async fn update_balance(eth_address: String) -> Result<u64, UpdateBalanceError> {
    let args = UpdateBalanceArgs {
        owner: Some(ic_cdk::id()),
        subaccount: Some(
            convert_eth_address_to_subaccount(&eth_address)
                .map_err(|_| UpdateBalanceError::InvalidEthereumAddress)?,
        ),
    };
    // Update balance by calling the ckBTC minter canister.
    let ckbtc_minting_account = state::read_state(|state| state.ckbtc_minting_account);
    let (res,): (UpdateBalanceResponse,) =
        ic_cdk::call(ckbtc_minting_account, "update_balance", (args.clone(),))
            .await
            .map_err(|e| {
                UpdateBalanceError::CkbtcMinterError(format!(
                    "failed to call ckBTC minter: {:?}",
                    e
                ))
            })?;
    let mut balance = 0;
    for status in res.0 {
        match status {
            UtxoStatus::Minted { minted_amount, .. } => {
                balance += minted_amount;
            }
            _ => {}
        }
    }
    // Update the state.
    state::mutate_state(|state| {
        let staker = state
            .stakers_map
            .entry(eth_address.clone())
            .or_insert_with(|| Staker {
                eth_address: eth_address.clone(),
                subaccount: args.subaccount.clone().unwrap(),
                tx_nonce: 0,
                ckbtc_balance: 0,
                otbtc_balance: 0,
            });
        staker.ckbtc_balance += balance;
    });
    //
    Ok(balance)
}

#[update]
async fn stake(args: StakeArgs) -> Result<(), StakeError> {
    let subaccount = convert_eth_address_to_subaccount(&args.eth_address)
        .map_err(|_| StakeError::InvalidEthereumAddress)?;
    let mut staker = state::take_state(|state| {
        state
            .stakers_map
            .get(&args.eth_address)
            .ok_or(StakeError::LackOfStakerRecord)
            .cloned()
    })?;
    if staker.ckbtc_balance < args.amount {
        return Err(StakeError::NotEnoughCkbtcBalance);
    }
    // Verify the signature.
    if verify_signature(&staker, "stake", args.amount, &args.signature).is_err() {
        return Err(StakeError::InvalidSignature);
    }
    // Transfer ckBTC tokens to the main account.
    let transfer_args = ic_ledger_types::TransferArgs {
        memo: Memo(0),
        amount: Tokens::from_e8s(args.amount),
        fee: Tokens::from_e8s(10_000),
        // The subaccount of the account identifier that will be used to withdraw tokens and send them
        // to another account identifier. If set to None then the default subaccount will be used.
        // See the [Ledger doc](https://internetcomputer.org/docs/current/developer-docs/integrations/ledger/#accounts).
        from_subaccount: Some(subaccount.clone()),
        to: AccountIdentifier::new(&ic_cdk::id(), &Subaccount([0u8; 32])),
        created_at_time: None,
    };
    let ckbtc_ledger_account = state::read_state(|state| state.ckbtc_ledger_account);
    ic_ledger_types::transfer(ckbtc_ledger_account, transfer_args)
        .await
        .map_err(|e| StakeError::CkbtcLedgerError(format!("failed to call ckBTC ledger: {:?}", e)))?
        .map_err(|e| {
            StakeError::CkbtcTransferError(format!("ckBTC ledger transfer error {:?}", e))
        })?;
    // Mint otBTC tokens on the ledger.
    let transfer_args = ic_ledger_types::TransferArgs {
        memo: Memo(0),
        amount: Tokens::from_e8s(args.amount),
        fee: Tokens::from_e8s(10_000),
        // The subaccount of the account identifier that will be used to withdraw tokens and send them
        // to another account identifier. If set to None then the default subaccount will be used.
        // See the [Ledger doc](https://internetcomputer.org/docs/current/developer-docs/integrations/ledger/#accounts).
        from_subaccount: Some(subaccount.clone()),
        to: AccountIdentifier::new(&ic_cdk::id(), &subaccount.clone()),
        created_at_time: None,
    };
    let otbtc_ledger_account = state::read_state(|state| state.otbtc_ledger_account);
    ic_ledger_types::transfer(otbtc_ledger_account, transfer_args)
        .await
        .map_err(|e| StakeError::OtbtcLedgerError(format!("failed to call otBTC ledger: {:?}", e)))?
        .map_err(|e| {
            StakeError::OtbtcTransferError(format!("otBTC ledger transfer error {:?}", e))
        })?;
    // Change the state of the staking pool.
    state::mutate_state(|state| {
        staker.tx_nonce += 1;
        staker.ckbtc_balance -= args.amount;
        staker.otbtc_balance += args.amount;
        state.stakers_map.insert(args.eth_address.clone(), staker);
        state.total_ckbtc_in_pool += args.amount;
    });
    //
    Ok(())
}
fn verify_signature(
    staker: &Staker,
    action: &str,
    amount: u64,
    signature: &Vec<u8>,
) -> Result<(), VerifySignatureError> {
    if signature.len() != 65 {
        return Err(VerifySignatureError::InvalidSignatureLength);
    }
    let signing_string = format!("{}:{}:{}", staker.tx_nonce, action, amount);
    let message = Message::parse_slice(&keccak256(&signing_string.into_bytes()))
        .map_err(|e| VerifySignatureError::FailedParsingSigningMessage(format!("{:?}", e)))?;
    let recid = RecoveryId::parse(signature[64])
        .map_err(|e| VerifySignatureError::InvalidRecoveryIdInSignature(format!("{:?}", e)))?;
    let recoverable_signature = Signature::parse_standard_slice(&signature[..63])
        .map_err(|e| VerifySignatureError::FailedParsingSignature(format!("{:?}", e)))?;
    let pubkey = libsecp256k1::recover(&message, &recoverable_signature, &recid)
        .map_err(|e| VerifySignatureError::FailedRecoveringPublicKey(format!("{:?}", e)))?;
    let pubkey_bytes = pubkey.serialize();
    let hash = keccak256(&pubkey_bytes[1..]);
    let address = &hash[12..];
    if hex::encode(address) == staker.eth_address {
        Ok(())
    } else {
        Err(VerifySignatureError::SignerAddressMismatch)
    }
}

#[update]
async fn unstake(args: UnstakeArgs) -> Result<(), UnstakeError> {
    let subaccount = convert_eth_address_to_subaccount(&args.eth_address)
        .map_err(|_| UnstakeError::InvalidEthereumAddress)?;
    let mut staker = state::take_state(|state| {
        state
            .stakers_map
            .get(&args.eth_address)
            .ok_or(UnstakeError::LackOfStakerRecord)
            .cloned()
    })?;
    if staker.otbtc_balance < args.amount {
        return Err(UnstakeError::NotEnoughOtbtcBalance);
    }
    // Verify the signature.
    if verify_signature(&staker, "unstake", args.amount, &args.signature).is_err() {
        return Err(UnstakeError::InvalidSignature);
    }
    // Burn otBTC tokens on the ledger.
    let transfer_args = ic_ledger_types::TransferArgs {
        memo: Memo(0),
        amount: Tokens::from_e8s(args.amount),
        fee: Tokens::from_e8s(10_000),
        // The subaccount of the account identifier that will be used to withdraw tokens and send them
        // to another account identifier. If set to None then the default subaccount will be used.
        // See the [Ledger doc](https://internetcomputer.org/docs/current/developer-docs/integrations/ledger/#accounts).
        from_subaccount: Some(subaccount.clone()),
        to: AccountIdentifier::new(&ic_cdk::id(), &Subaccount([0u8; 32])),
        created_at_time: None,
    };
    let otbtc_ledger_account = state::read_state(|state| state.otbtc_ledger_account);
    ic_ledger_types::transfer(otbtc_ledger_account, transfer_args)
        .await
        .map_err(|e| {
            UnstakeError::OtbtcLedgerError(format!("failed to call otBTC ledger: {:?}", e))
        })?
        .map_err(|e| {
            UnstakeError::OtbtcTransferError(format!("otBTC ledger transfer error {:?}", e))
        })?;
    // Change the state
    state::mutate_state(|state| {
        staker.tx_nonce += 1;
        staker.otbtc_balance -= args.amount;
        state.stakers_map.insert(args.eth_address.clone(), staker);
        state.unstaking_queue.push_back(UnstakeRequest {
            eth_address: args.eth_address.clone(),
            amount: args.amount,
            unlock_time: ic_cdk::api::time() + state.unbonding_period,
        });
    });
    //
    Ok(())
}

#[update]
async fn unlock_tokens_in_queue() -> Result<(), UnlockTokensInQueueError> {
    let unstake_request =
        state::read_state(|state| state.unstaking_queue.front().map(|v| v.clone()));
    if let Some(request) = unstake_request {
        let staker = state::read_state(|state| {
            state
                .stakers_map
                .get(&request.eth_address)
                .ok_or(UnlockTokensInQueueError::LackOfStakerRecord)
                .cloned()
        })?;
        if ic_cdk::api::time() < request.unlock_time {
            return Err(UnlockTokensInQueueError::UnlockTimeNotReached);
        }
        // Transfer ckBTC tokens to the subaccount.
        let transfer_args = ic_ledger_types::TransferArgs {
            memo: Memo(0),
            amount: Tokens::from_e8s(request.amount),
            fee: Tokens::from_e8s(10_000),
            // The subaccount of the account identifier that will be used to withdraw tokens and send them
            // to another account identifier. If set to None then the default subaccount will be used.
            // See the [Ledger doc](https://internetcomputer.org/docs/current/developer-docs/integrations/ledger/#accounts).
            from_subaccount: None,
            to: AccountIdentifier::new(&ic_cdk::id(), &staker.subaccount.clone()),
            created_at_time: None,
        };
        let ckbtc_ledger_account = state::read_state(|state| state.ckbtc_ledger_account);
        ic_ledger_types::transfer(ckbtc_ledger_account, transfer_args)
            .await
            .map_err(|e| {
                UnlockTokensInQueueError::CkbtcLedgerError(format!(
                    "failed to call ckBTC ledger: {:?}",
                    e
                ))
            })?
            .map_err(|e| {
                UnlockTokensInQueueError::CkbtcTransferError(format!(
                    "ckBTC ledger transfer error {:?}",
                    e
                ))
            })?;
        // Change the state
        state::mutate_state(|state| {
            state.unstaking_queue.pop_front();
            state.total_ckbtc_in_pool -= request.amount;
            let staker = state
                .stakers_map
                .get_mut(&request.eth_address)
                .expect("staker not found, should not happen");
            staker.ckbtc_balance += request.amount;
        });
    }
    //
    Ok(())
}

#[update]
async fn withdraw_btc(args: WithdrawBtcArgs) -> Result<(), WithdrawBtcError> {
    let subaccount = convert_eth_address_to_subaccount(&args.eth_address)
        .map_err(|_| WithdrawBtcError::InvalidEthereumAddress)?;
    let mut staker = state::take_state(|state| {
        state
            .stakers_map
            .get(&args.eth_address)
            .ok_or(WithdrawBtcError::LackOfStakerRecord)
            .cloned()
    })?;
    if staker.ckbtc_balance < args.amount {
        return Err(WithdrawBtcError::NotEnoughCkbtcBalance);
    }
    // Verify the signature.
    if verify_signature(&staker, "withdraw_btc", args.amount, &args.signature).is_err() {
        return Err(WithdrawBtcError::InvalidSignature);
    }
    // Burn ckBTC tokens from the subaccount.
    let ckbtc_minting_account = state::read_state(|state| state.ckbtc_minting_account);
    let transfer_args = ic_ledger_types::TransferArgs {
        memo: Memo(0),
        amount: Tokens::from_e8s(args.amount),
        fee: Tokens::from_e8s(10_000),
        // The subaccount of the account identifier that will be used to withdraw tokens and send them
        // to another account identifier. If set to None then the default subaccount will be used.
        // See the [Ledger doc](https://internetcomputer.org/docs/current/developer-docs/integrations/ledger/#accounts).
        from_subaccount: Some(subaccount.clone()),
        to: AccountIdentifier::new(&ckbtc_minting_account, &Subaccount([0u8; 32])),
        created_at_time: None,
    };
    let ckbtc_ledger_account = state::read_state(|state| state.ckbtc_ledger_account);
    ic_ledger_types::transfer(ckbtc_ledger_account, transfer_args)
        .await
        .map_err(|e| {
            WithdrawBtcError::CkbtcLedgerError(format!("failed to call ckBTC ledger: {:?}", e))
        })?
        .map_err(|e| {
            WithdrawBtcError::CkbtcTransferError(format!("ckBTC ledger transfer error {:?}", e))
        })?;
    // Change the state
    state::mutate_state(|state| {
        staker.tx_nonce += 1;
        staker.ckbtc_balance -= args.amount;
        state.stakers_map.insert(args.eth_address.clone(), staker);
    });
    //
    Ok(())
}

ic_cdk::export_candid!();
