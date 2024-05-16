use candid::CandidType;

#[derive(CandidType, Debug)]
pub enum GetBtcDepositAddressError {
    /// The specified address is not a valid Ethereum address.
    InvalidEthereumAddress,
    /// The call to the CKBTC minter canister failed.
    CkbtcMinterError(String),
}

#[derive(CandidType, Debug)]
pub enum UpdateBalanceError {
    /// The specified address is not a valid Ethereum address.
    InvalidEthereumAddress,
    /// The call to the CKBTC minter canister failed.
    CkbtcMinterError(String),
}

#[derive(CandidType, Debug)]
pub enum StakeError {
    /// The specified address is not a valid Ethereum address.
    InvalidEthereumAddress,
    /// Staker record not found.
    LackOfStakerRecord,
    /// The specified amount is larger than the available balance.
    NotEnoughCkbtcBalance,
    /// The signature is invalid.
    InvalidSignature,
    /// The call to the ckBTC ledger canister failed.
    CkbtcLedgerError(String),
    /// The transfer on the ckBTC ledger canister failed.
    CkbtcTransferError(String),
    /// The call to the otBTC ledger canister failed.
    OtbtcLedgerError(String),
    /// The transfer on the otBTC ledger canister failed.
    OtbtcTransferError(String),
}

#[derive(CandidType, Debug)]
pub enum UnstakeError {
    /// The specified address is not a valid Ethereum address.
    InvalidEthereumAddress,
    /// Staker record not found.
    LackOfStakerRecord,
    /// The specified amount is larger than the available balance.
    NotEnoughOtbtcBalance,
    /// The signature is invalid.
    InvalidSignature,
    /// The call to the otBTC ledger canister failed.
    OtbtcLedgerError(String),
    /// The transfer on the otBTC ledger canister failed.
    OtbtcTransferError(String),
}

#[derive(CandidType, Debug)]
pub enum UnlockTokensInQueueError {
    /// Staker record not found.
    LackOfStakerRecord,
    /// The withdrawal time has not been reached yet.
    UnlockTimeNotReached,
    /// The call to the ckBTC ledger canister failed.
    CkbtcLedgerError(String),
    /// The transfer on the ckBTC ledger canister failed.
    CkbtcTransferError(String),
}

#[derive(CandidType, Debug)]
pub enum WithdrawBtcError {
    /// The specified address is not a valid Ethereum address.
    InvalidEthereumAddress,
    /// Staker record not found.
    LackOfStakerRecord,
    /// The specified amount is larger than the available balance.
    NotEnoughCkbtcBalance,
    /// The signature is invalid.
    InvalidSignature,
    /// The call to the ckBTC ledger canister failed.
    CkbtcLedgerError(String),
    /// The transfer on the ckBTC ledger canister failed.
    CkbtcTransferError(String),
}

#[derive(CandidType, Debug)]
pub enum VerifySignatureError {
    InvalidSignatureLength,
    FailedParsingSigningMessage(String),
    InvalidRecoveryIdInSignature(String),
    FailedParsingSignature(String),
    FailedRecoveringPublicKey(String),
    SignerAddressMismatch,
}
