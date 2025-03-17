use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

pub type ContractResult<T> = core::result::Result<T, ContractError>;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("field {kind} should not be empty")]
    EmptyValue { kind: String },

    #[error("BadTokenA")]
    BadTokenA,

    #[error("Failed to decode response data")]
    DecodingError,

    #[error("Current block is on hold due to a stale price")]
    BlockOnHold,

    #[error("No response data from place limit order")]
    NoResponseData,

    #[error("Contract Already Has Active Deposits")]
    ActiveDepositsExist,

    #[error("BadTokenB")]
    BadTokenB,

    #[error("denom {denom} is not a correct IBC denom: {reason}")]
    InvalidIbcDenom { denom: String, reason: String },

    #[error("Market {symbol}, {quote} not found in {location}")]
    UnsupportedMarket {
        symbol: String,
        quote: String,
        location: String,
    },

    #[error("Market {symbol}, {quote} not enabled in {location}")]
    DisabledMarket {
        symbol: String,
        quote: String,
        location: String,
    },

    #[error("Market {symbol}, {quote} did not return an block height")]
    PriceAgeUnavailable { symbol: String, quote: String },

    #[error("Market {symbol}, {quote} did not return a block height")]
    PriceNotAvailable { symbol: String, quote: String },

    #[error("Market {symbol}, {quote} returned a nil price")]
    PriceIsNil { symbol: String, quote: String },

    #[error("Market {symbol}, {quote} is older than {max_blocks} blocks")]
    PriceTooOld {
        symbol: String,
        quote: String,
        max_blocks: u64,
    },

    #[error("input for {input} is invalid: {reason}")]
    MalformedInput { input: String, reason: String },

    #[error("Only USD quote currency supported. Quote Currencies provided: {quote0}, {quote1}")]
    OnlySupportUsdQuote { quote0: String, quote1: String },

    #[error("Invalid DEX deposit base fee: {fee}")]
    InvalidBaseFee { fee: u64 },

    #[error("Invalid deposit percentage: {percentage}. Normal range is [0-100]")]
    InvalidDepositPercentage { percentage: u64 },

    #[error("Too many decimals from oracle responce, exceeds u32 allowance")]
    TooManyDecimals,

    #[error("Price cannot be negative")]
    PriceIsNegative,

    #[error("Failed to convert value to Decimal")]
    DecimalConversionError,

    #[error("No funds sent with deposit function")]
    NoFundsSent,

    #[error("Attempted deposit of invalid token")]
    InvalidToken,

    #[error("Attempted deposit of invalid token amount")]
    InvalidTokenAmount,

    #[error("Attempted withdraw of invalid token amount")]
    InvalidWithdrawAmount,

    #[error("Cannot withdraw zero amount")]
    ZeroBurnAmount,

    #[error("LP token already created")]
    TokenAlreadyCreated,

    #[error("Msg sender must be the contract owner")]
    Unauthorized,

    #[error("No funds available")]
    NoFundsAvailable,

    #[error("Funds cannot be received here")]
    FundsNotAllowed,

    #[error("Only LP tokens can be used for withdrawals")]
    OnlyLpTokenAllowed,

    #[error("failed to convert uint to int. value of coin amount as Uint128 exceeds max possible Int128 amount")]
    ConversionError,

    #[error("Price is invalid")]
    InvalidPrice,

    #[error("No reply data")]
    InvalidFeeTier { reason: String },

    #[error("No reply data")]
    InvalidConfig { reason: String },
    
    #[error("Timestamp is stale")]
    StaleTimestamp,

    #[error("SubMsg failed")]
    SubMsgFailure { reason: String },

    #[error("No reply data")]
    NoReplyData,

    #[error("Failed to parse uint128")]
    ParseError,

    #[error("Contract is paused")]
    Paused,

    #[error("Deposit would exceed the deposit cap")]
    ExceedsDepositCap,

    #[error("Incorrect token or token amount provided")]
    LpTokenError,

    #[error("Insufficient funds for withdrawal")]
    InsufficientFundsForWithdrawal,

    #[error("Insufficient balance for Deposit: available: {available}, required: {required}")]
    InsufficientFunds {
        available: Uint128,
        required: Uint128,
    },

    #[error("Serialization error")]
    SerializationError,

    #[error("Liquidity exists but tick index was not returned")]
    TickIndexDoesNotExist,

    #[error("Liquidity exists but cannot be retreived")]
    LiquidityNotFound,

    #[error("Unknown reply id: {id}")]
    UnknownReplyId { id: u64 },

    #[error("Overflow error")]
    Overflow(cosmwasm_std::OverflowError),

    #[error("PrecDec division error")]
    CheckedDiv(cosmwasm_std::CheckedFromRatioError),

    #[error("Custom error: {0}")]
    CustomError(String),
}

impl From<cosmwasm_std::OverflowError> for ContractError {
    fn from(err: cosmwasm_std::OverflowError) -> Self {
        ContractError::Overflow(err)
    }
}

impl From<cosmwasm_std::CheckedFromRatioError> for ContractError {
    fn from(err: cosmwasm_std::CheckedFromRatioError) -> Self {
        ContractError::CheckedDiv(err)
    }
}