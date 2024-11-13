use thiserror::Error;
use cosmwasm_std::{StdError, Uint128};

pub type ContractResult<T> = core::result::Result<T, ContractError>;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("field {kind} should not be empty")]
    EmptyValue { kind: String },

    #[error("BadTokenA")]
    BadTokenA,

    #[error("BadTokenB")]
    BadTokenB,

    #[error("denom {denom} is not a correct IBC denom: {reason}")]
    InvalidIbcDenom { denom: String, reason: String },
   
    #[error( "Market {symbol}, {quote} not found in {location}")]
    UnsupportedMarket { symbol: String, quote: String, location: String},

    #[error( "Market {symbol}, {quote} not enabled in {location}")]
    DisabledMarket { symbol: String, quote: String, location: String},
    
    #[error( "Market {symbol}, {quote} did not return an block height")]
    PriceAgeUnavailable { symbol: String, quote: String},

    #[error("Market {symbol}, {quote} did not return a block height")]
    PriceNotAvailable { symbol: String, quote: String },
    
    #[error( "Market {symbol}, {quote} returned a nil price")]
    PriceIsNil { symbol: String, quote: String},

    #[error( "Market {symbol}, {quote} is older than {max_blocks} blocks")]
    PriceTooOld { symbol: String, quote: String, max_blocks: u64},

    #[error("input for {input} is invalid: {reason}")]
    MalformedInput { input: String, reason: String },

    #[error("Only USD quote currency supported. Quote Currencies provided: {quote0}, {quote1}")]
    OnlySupportUsdQuote { quote0: String, quote1: String},

    #[error("Invalid DEX deposit base fee: {fee}")]
    InvalidBaseFee  { fee: u64 },

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

    #[error("Msg sender must be the contract owner")]
    Unauthorized,

    #[error("No funds available")]
    NoFundsAvailable,

    #[error("Funds cannot be received here")]
    FundsNotAllowed,

    #[error("failed to convert uint to int. value of coin amount as Uint128 exceeds max possible Int128 amount")]
    ConversionError,

    #[error("Price is invalid")]
    InvalidPrice,

    #[error("Insufficient balance for Deposit: available: {available}, required: {required}")]
    InsufficientFunds { available: Uint128, required: Uint128},

    #[error("Liquidity exists but tick index was not returned")]
    TickIndexDoesNotExist,

    #[error("Liquidity exists but cannot be retreived")]
    LiquidityNotFound,

}

