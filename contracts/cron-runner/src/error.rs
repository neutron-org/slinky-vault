use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Empty value: {kind}")]
    EmptyValue { kind: String },

    #[error("Invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    #[error("Serialization error")]
    SerializationError,

    #[error("Invalid vault address: {addr}")]
    InvalidVaultAddress { addr: String },

    #[error("Duplicate vault address: {addr}")]
    DuplicateVaultAddress { addr: String },

    #[error("No vaults configured")]
    NoVaultsConfigured,

    #[error("Failed to create message for vault {vault}: {reason}")]
    MessageCreationError { vault: String, reason: String },
}

pub type ContractResult<T> = Result<T, ContractError>;
