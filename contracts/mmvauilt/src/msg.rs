use crate::{
    error::{ContractError, ContractResult},
    state::TokenData,
};
use cosmwasm_std::{Coin, Decimal, Response, Uint128};
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
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub owner: String,
    pub token_a: TokenData,
    pub token_b: TokenData,
    pub max_block_old: u64,
    pub base_fee: u64,
    pub base_deposit_percentage: u64,
}

impl InstantiateMsg {
    pub fn validate(&self) -> ContractResult<()> {
        self.check_empty(self.owner.clone(), "beneficiary".to_string())?;
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

        if self.max_block_old <= 0 {
            return Err(ContractError::MalformedInput {
                input: "max_block_stale".to_string(),
                reason: "must be >=1".to_string(),
            });
        }
        Self::validate_denom(&self.token_a.denom)?;
        Self::validate_denom(&self.token_b.denom)?;
        Self::validate_base_fee(self.base_fee)?;
        Self::validate_base_deposit_percentage(self.base_deposit_percentage)?;

        if self.token_a.pair.quote == self.token_b.pair.quote && self.token_b.pair.quote != "USD" {
            return Err(ContractError::OnlySupportUsdQuote {
                quote0: self.token_a.pair.quote.clone(),
                quote1: self.token_b.pair.quote.clone(),
            });
        }
        Ok(())
    }

    pub fn validate_base_fee(fee: u64) -> ContractResult<Response> {
        // TODO: GET FROM DEX, for now Define the allowed fees array
        let allowed_fees: [u64; 12] = [0, 1, 2, 3, 4, 5, 10, 20, 50, 100, 150, 200];

        // Check if the fee is in the allowed_fees array
        if !allowed_fees.contains(&fee) {
            return Err(ContractError::InvalidBaseFee { fee });
        }

        // If fee is valid, return Ok with an empty response
        Ok(Response::new())
    }
    pub fn validate_base_deposit_percentage(percentage: u64) -> ContractResult<Response> {
        if percentage > 100 {
            return Err(ContractError::InvalidDepositPercentage { percentage });
        }

        // If percentage is valid, return Ok with an empty response
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
            return Err(ContractError::EmptyValue { kind: kind });
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
    Withdraw {},
    // // cancels and withdraws all active and filled Limit orders
    DexDeposit {},
    DexWithdrawal {},
    // // pauses all deposit functionality
    // Pause {},
    // // helper to atomically purge and withdraw
    // PurgeAnddWithdraw {},
    // // helper to atomically purge and pause
    // PurgeAndPause {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetFormated {},
    GetDeposits {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CombinedPriceResponse {
    pub token_0_price: Decimal,
    pub token_1_price: Decimal,
    pub price_0_to_1: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct DepositResult {
    pub amount0: Uint128,
    pub amount1: Uint128,
    pub tick_index: i64,
    pub fee: u64,
}
