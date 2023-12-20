use std::{num::TryFromIntError, time::SystemTimeError};

use alloy_sol_types::{ContractError, SolInterface};
use edr_eth::{
    remote::{filter::SubscriptionType, jsonrpc, BlockSpec},
    Address, Bytes, SpecId, B256, U256,
};
use edr_evm::{
    blockchain::BlockchainError,
    hex,
    state::{AccountOverrideConversionError, StateError},
    Halt, MineBlockError, MinerTransactionError, OutOfGasError, TransactionCreationError,
    TransactionError,
};

use crate::data::CreationError;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Account override conversion error.
    #[error(transparent)]
    AccountOverrideConversionError(#[from] AccountOverrideConversionError),
    /// The transaction's gas price is lower than the next block's base fee,
    /// while automatically mining.
    #[error("Transaction gasPrice ({actual}) is too low for the next block, which has a baseFeePerGas of {expected}")]
    AutoMineGasPriceTooLow { expected: U256, actual: U256 },
    /// The transaction's max fee is lower than the next block's base fee, while
    /// automatically mining.
    #[error("Transaction maxFeePerGas ({actual}) is too low for the next block, which has a baseFeePerGas of {expected}")]
    AutoMineMaxFeeTooLow { expected: U256, actual: U256 },
    /// The transaction's priority fee is lower than the minimum gas price,
    /// while automatically mining.
    #[error("Transaction gas price is {actual}, which is below the minimum of {expected}")]
    AutoMinePriorityFeeTooLow { expected: U256, actual: U256 },
    /// The transaction nonce is too high, while automatically mining.
    #[error("Nonce too high. Expected nonce to be {expected} but got {actual}. Note that transactions can't be queued when automining.")]
    AutoMineNonceTooHigh { expected: u64, actual: u64 },
    /// The transaction nonce is too high, while automatically mining.
    #[error("Nonce too low. Expected nonce to be {expected} but got {actual}. Note that transactions can't be queued when automining.")]
    AutoMineNonceTooLow { expected: u64, actual: u64 },
    /// Blockchain error
    #[error(transparent)]
    Blockchain(#[from] BlockchainError),
    #[error(transparent)]
    Creation(#[from] CreationError),
    /// Block number or hash doesn't exist in blockchain
    #[error(
        "Received invalid block tag {block_spec}. Latest block number is {latest_block_number}"
    )]
    InvalidBlockNumberOrHash {
        block_spec: BlockSpec,
        latest_block_number: u64,
    },
    /// The block tag is not allowed in pre-merge hardforks.
    /// https://github.com/NomicFoundation/hardhat/blob/b84baf2d9f5d3ea897c06e0ecd5e7084780d8b6c/packages/hardhat-core/src/internal/hardhat-network/provider/modules/eth.ts#L1820
    #[error("The '{block_spec}' block tag is not allowed in pre-merge hardforks. You are using the '{spec:?}' hardfork.")]
    InvalidBlockTag { block_spec: BlockSpec, spec: SpecId },
    /// Invalid chain ID
    #[error("Invalid chainId ${actual} provided, expected ${expected} instead.")]
    InvalidChainId { expected: u64, actual: u64 },
    /// Invalid filter subscription type
    #[error("Subscription {filter_id} is not a {expected:?} subscription, but a {actual:?} subscription")]
    InvalidFilterSubscriptionType {
        filter_id: U256,
        expected: SubscriptionType,
        actual: SubscriptionType,
    },
    /// Invalid transaction index
    #[error("Transaction index '{0}' is too large")]
    InvalidTransactionIndex(U256),
    /// Invalid transaction request
    #[error("{0}")]
    InvalidTransactionInput(String),
    /// An error occurred while updating the mem pool.
    #[error(transparent)]
    MemPoolUpdate(StateError),
    /// An error occurred while mining a block.
    #[error(transparent)]
    MineBlock(#[from] MineBlockError<BlockchainError, StateError>),
    /// An error occurred while adding a pending transaction to the mem pool.
    #[error(transparent)]
    MinerTransactionError(#[from] MinerTransactionError<StateError>),
    /// Rlp decode error
    #[error(transparent)]
    RlpDecodeError(#[from] rlp::DecoderError),
    /// Unsupported RPC version
    #[error("unsupported JSON-RPC version: {0:?}")]
    RpcVersion(jsonrpc::Version),
    /// Error while running a transaction
    #[error(transparent)]
    RunTransaction(#[from] TransactionError<BlockchainError, StateError>),
    /// The `hardhat_setMinGasPrice` method is not supported when EIP-1559 is
    /// active.
    #[error("hardhat_setMinGasPrice is not supported when EIP-1559 is active")]
    SetMinGasPriceUnsupported,
    /// Serialization error
    #[error("Failed to serialize response: {0}")]
    Serialization(serde_json::Error),
    /// An error occurred while recovering a signature.
    #[error(transparent)]
    Signature(#[from] edr_eth::signature::SignatureError),
    /// State error
    #[error(transparent)]
    State(#[from] StateError),
    /// System time error
    #[error(transparent)]
    SystemTime(#[from] SystemTimeError),
    /// Timestamp lower than previous timestamp
    #[error("Timestamp {proposed} is lower than the previous block's timestamp {previous}")]
    TimestampLowerThanPrevious { proposed: u64, previous: u64 },
    /// Timestamp equals previous timestamp
    #[error("Timestamp {proposed} is equal to the previous block's timestamp. Enable the 'allowBlocksWithSameTimestamp' option to allow this")]
    TimestampEqualsPrevious { proposed: u64 },
    /// An error occurred while creating a pending transaction.
    #[error(transparent)]
    TransactionCreationError(#[from] TransactionCreationError),
    /// `eth_sendTransaction` failed and
    /// [`ProviderConfig::bail_on_call_failure`] was enabled
    #[error(transparent)]
    TransactionFailed(#[from] TransactionFailure),
    /// Failed to convert an integer type
    #[error("Could not convert the integer argument, due to: {0}")]
    TryFromIntError(#[from] TryFromIntError),
    /// The request hasn't been implemented yet
    #[error("Unimplemented: {0}")]
    Unimplemented(String),
    /// The address is not owned by this node.
    #[error("Unknown account {address}")]
    UnknownAddress { address: Address },
    /// Minimum required hardfork not met
    #[error("Feature is only available in post-{minimum:?} hardforks, the current hardfork is {actual:?}")]
    UnmetHardfork { actual: SpecId, minimum: SpecId },
}

impl From<ProviderError> for jsonrpc::Error {
    fn from(value: ProviderError) -> Self {
        #[allow(clippy::match_same_arms)]
        let (code, data) = match &value {
            ProviderError::AccountOverrideConversionError(_) => (-32000, None),
            ProviderError::AutoMineGasPriceTooLow { .. } => (-32000, None),
            ProviderError::AutoMineMaxFeeTooLow { .. } => (-32000, None),
            ProviderError::AutoMineNonceTooHigh { .. } => (-32000, None),
            ProviderError::AutoMineNonceTooLow { .. } => (-32000, None),
            ProviderError::AutoMinePriorityFeeTooLow { .. } => (-32000, None),
            ProviderError::Blockchain(_) => (-32000, None),
            ProviderError::Creation(_) => (-32000, None),
            ProviderError::InvalidBlockNumberOrHash { .. } => (-32000, None),
            ProviderError::InvalidBlockTag { .. } => (-32000, None),
            ProviderError::InvalidChainId { .. } => (-32000, None),
            ProviderError::InvalidFilterSubscriptionType { .. } => (-32000, None),
            ProviderError::InvalidTransactionIndex(_) => (-32000, None),
            ProviderError::InvalidTransactionInput(_) => (-32000, None),
            ProviderError::MemPoolUpdate(_) => (-32000, None),
            ProviderError::MineBlock(_) => (-32000, None),
            ProviderError::MinerTransactionError(_) => (-32000, None),
            ProviderError::RlpDecodeError(_) => (-32000, None),
            ProviderError::RpcVersion(_) => (-32000, None),
            ProviderError::RunTransaction(_) => (-32000, None),
            ProviderError::Serialization(_) => (-32000, None),
            ProviderError::SetMinGasPriceUnsupported => (-32000, None),
            ProviderError::Signature(_) => (-32000, None),
            ProviderError::State(_) => (-32000, None),
            ProviderError::SystemTime(_) => (-32000, None),
            ProviderError::TimestampLowerThanPrevious { .. } => (-32000, None),
            ProviderError::TimestampEqualsPrevious { .. } => (-32000, None),
            ProviderError::TransactionFailed(transaction_failure) => (
                -32000,
                Some(
                    serde_json::to_value(transaction_failure).expect("transaction_failure to json"),
                ),
            ),
            ProviderError::TransactionCreationError(_) => (-32000, None),
            ProviderError::TryFromIntError(_) => (-32000, None),
            ProviderError::Unimplemented(_) => (-32000, None),
            ProviderError::UnknownAddress { .. } => (-32000, None),
            ProviderError::UnmetHardfork { .. } => (-32602, None),
        };

        Self {
            code,
            message: value.to_string(),
            data,
        }
    }
}

/// Wrapper around [`revm_primitives::Halt`] to convert error messages to match
/// Hardhat.
#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionFailure {
    pub reason: TransactionFailureReason,
    pub data: Option<String>,
    pub transaction_hash: B256,
}

impl TransactionFailure {
    pub fn revert(output: Bytes, transaction_hash: B256) -> Self {
        let data = format!("0x{}", hex::encode(output.as_ref()));
        Self {
            reason: TransactionFailureReason::Revert(output),
            data: Some(data),
            transaction_hash,
        }
    }

    pub fn halt(halt: Halt, tx_hash: B256) -> Self {
        let reason = match halt {
            Halt::OpcodeNotFound | Halt::InvalidFEOpcode => {
                TransactionFailureReason::OpcodeNotFound
            }
            Halt::OutOfGas(error) => TransactionFailureReason::OutOfGas(error),
            halt => TransactionFailureReason::Inner(halt),
        };

        Self {
            reason,
            data: None,
            transaction_hash: tx_hash,
        }
    }
}

impl std::fmt::Display for TransactionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.reason {
            TransactionFailureReason::Inner(halt) => write!(f, "{halt:?}"),
            TransactionFailureReason::OpcodeNotFound => {
                write!(
                    f,
                    "VM Exception while processing transaction: invalid opcode"
                )
            }
            TransactionFailureReason::OutOfGas(_error) => write!(f, "out of gas"),
            TransactionFailureReason::Revert(output) => write!(f, "{}", revert_error(output)),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub enum TransactionFailureReason {
    Inner(Halt),
    OpcodeNotFound,
    OutOfGas(OutOfGasError),
    Revert(Bytes),
}

fn revert_error(output: &Bytes) -> String {
    if output.is_empty() {
        return "Transaction reverted without a reason".to_string();
    }

    match alloy_sol_types::GenericContractError::abi_decode(
        output.as_ref(),
        /* validate */ false,
    ) {
        Ok(contract_error) => {
            match contract_error {
                ContractError::CustomError(custom_error) => {
                    format!("VM Exception while processing transaction: reverted with an unrecognized custom error (return data: {custom_error})")
                }
                ContractError::Revert(revert) => {
                    format!("reverted with reason string '{}'", revert.reason())
                }
                ContractError::Panic(panic) => {
                    format!(
                        "VM Exception while processing transaction: reverted with panic code {} ({})",
                        serde_json::to_string(&panic.code).unwrap().replace('\"', ""),
                        panic_code_to_error_reason(panic.code.try_into().expect("panic code fits into u64"))
                    )
                }
            }
        }
        Err(decode_error) => match decode_error {
            alloy_sol_types::Error::TypeCheckFail { .. } => {
                format!("VM Exception while processing transaction: reverted with an unrecognized custom error (return data: 0x{})", hex::encode(output))
            }
            _ => unreachable!("Since we are not validating, no other error can occur"),
        },
    }
}

fn panic_code_to_error_reason(error_code: u64) -> &'static str {
    match error_code {
        0x1 => "Assertion error",
        0x11 => "Arithmetic operation underflowed or overflowed outside of an unchecked block",
        0x12 => "Division or modulo division by zero",
        0x21 => "Tried to convert a value into an enum, but the value was too big or negative",
        0x22 => "Incorrectly encoded storage byte array",
        0x31 => ".pop() was called on an empty array",
        0x32 => "Array accessed at an out-of-bounds or negative index",
        0x41 => "Too much memory was allocated, or an array was created that is too large",
        0x51 => "Called a zero-initialized variable of internal function type",
        _ => "Unknown panic code",
    }
}