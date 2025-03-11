use crate::{
    error::{ContractError, ContractResult},
    state::{FeeTierConfig, TokenData, Config},
};
use cosmwasm_std::{Coin, Response, Uint128};
use neutron_std::types::neutron::util::precdec::PrecDec;
use prost::Message;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DepositOptions {
    pub token_a: Option<Coin>,
    pub token_b: Option<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ReceiveFunds {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {
    pub config: Config,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub whitelist: Vec<String>,
    pub token_a: TokenData,
    pub token_b: TokenData,
    pub fee_tier_config: FeeTierConfig,
    pub deposit_cap: Uint128,
    pub timestamp_stale: u64,
    pub paused: bool,
    pub oracle_contract: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ConfigUpdateMsg {
    pub whitelist: Option<Vec<String>>,
    pub max_blocks_old_token_a: Option<u64>,
    pub max_blocks_old_token_b: Option<u64>,
    pub deposit_cap: Option<Uint128>,
    pub timestamp_stale: Option<u64>,
    pub fee_tier_config: Option<FeeTierConfig>,
    pub paused: Option<bool>,
    pub skew: Option<bool>,
}

impl InstantiateMsg {
    pub fn validate(&self) -> ContractResult<()> {
        if self.whitelist.is_empty() {
            return Err(ContractError::EmptyValue {
                kind: "whitelist".to_string(),
            });
        }

        self.check_empty(self.token_a.denom.clone(), "token_a denom".to_string())?;
        self.check_empty(self.token_b.denom.clone(), "token_b denom".to_string())?;
        self.check_empty(
            self.token_a.pair.base.clone(),
            "token_a symbol (base)".to_string(),
        )?;
        self.check_empty(
            self.token_b.pair.base.clone(),
            "token_b symbol (base)".to_string(),
        )?;
        Self::validate_denom(&self.token_a.denom)?;
        Self::validate_denom(&self.token_b.denom)?;
        Self::validate_fee_tier_config(&self.fee_tier_config)?;

        if self.token_a.pair.quote == self.token_b.pair.quote && self.token_b.pair.quote != "USD" {
            return Err(ContractError::OnlySupportUsdQuote {
                quote0: self.token_a.pair.quote.clone(),
                quote1: self.token_b.pair.quote.clone(),
            });
        }
        Ok(())
    }

    pub fn validate_fee_tier_config(config: &FeeTierConfig) -> ContractResult<Response> {
        let mut total_percentage = 0u64;

        // Check each fee tier
        for tier in &config.fee_tiers {
            total_percentage += tier.percentage;
        }

        // Ensure total percentage is less than 100%
        if total_percentage > 100 {
            return Err(ContractError::InvalidFeeTier {
                reason: "Total fee tier percentages must be <= 100%".to_string(),
            });
        }

        Ok(Response::new())
    }
    fn validate_denom(denom: &str) -> ContractResult<Response> {
        let invalid_denom = |reason: &str| {
            Err(ContractError::InvalidIbcDenom {
                denom: String::from(denom),
                reason: reason.to_string(),
            })
        };
        // if it's an IBC denom
        if denom.len() >= 4 && denom.starts_with("ibc/") {
            // Step 1: Validate length
            if denom.len() != 68 {
                return invalid_denom("expected length of 68 chars");
            }

            // Step 2: Validate prefix
            if !denom.starts_with("ibc/") {
                return invalid_denom("expected prefix 'ibc/'");
            }

            // Step 3: Validate hash
            if !denom
                .chars()
                .skip(4)
                // c.is_ascii_hexdigit() could have been used here, but it allows lowercase characters
                .all(|c| matches!(c, '0'..='9' | 'A'..='F'))
            {
                return invalid_denom("invalid denom hash");
            }
        }
        Ok(Response::new())
    }
    pub fn check_empty(&self, input: String, kind: String) -> ContractResult<()> {
        if input.is_empty() {
            return Err(ContractError::EmptyValue { kind });
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // deposit funds to use for market making
    Deposit {},
    // withdraw free unutilised funds
    Withdraw { amount: Uint128 },
    // cancels and withdraws all active and filled Limit orders
    DexDeposit {},
    DexWithdrawal {},
    // create the LP token
    CreateToken {},

    UpdateConfig {
        update: ConfigUpdateMsg,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetDeposits {},
    GetConfig {},
    GetPrices {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CombinedPriceResponse {
    pub token_0_price: PrecDec,
    pub token_1_price: PrecDec,
    pub price_0_to_1: PrecDec,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct DepositResult {
    pub amount0: Uint128,
    pub amount1: Uint128,
    pub tick_index: i64,
    pub fee: u64,
}

#[derive(Message, Clone, PartialEq)]
pub struct WithdrawPayload {
    #[prost(string, tag = "1")]
    pub sender: String,
    #[prost(string, tag = "2")]
    pub amount: String,
}
