use crate::error::ContractResult;
use crate::external_types::{QueryMsg as ApyQueryMsg, DropInstanceApy};
use cosmwasm_std::{Deps, WasmQuery, QueryRequest, to_json_binary, Addr, Decimal, CosmosMsg, WasmMsg};
use serde_json::{json, Value};

// Constants for fee tier calculation
const DAYS_IN_YEAR: u64 = 365;
const LN_1_0001: f64 = 0.00009999500033330835; // ln(1.0001)

#[derive(Clone, Debug)]
pub struct FeeTier {
    pub fee: u64,
    pub percentage: u64,
}

impl FeeTier {
    pub fn new(fee: u64, percentage: u64) -> Self {
        Self { fee, percentage }
    }
}

/// Calculate the base fee tier using the formula (r*t)/(2*ln(1.0001))
/// where r = APY (0.25 = 25% apy) and t = unbonding period in years (converted from days in config)
/// 
/// 
/// 
// # r = anualized rewards
// # t = unbonding period in years
// # RR = redemption Rate
// # the calculation comes from identifying the mid point as follows:
// #        A                             B
// #        |                             |
// #        |              M              |
// #        |              |              |
// #-----------------------------------------
// #     RRe^-rt   (target index)    RR (base index)
// #
// # index A: This is the target price to sell Dasst for Asset
// # 1.0001^i = RRe^-rt ->
// # i = ln(RRe^-rt) / ln(1.0001)
// #
// # index B: Price to sell Asset for Dasset
// # 1.0001^i = RR ->
// # i = ln(RR) / ln(1.0001)
// #
// # S: Spread: Difference between A and B:
// # index B - indeax A ->
// # (ln(RR) - ln(RRe^-rt)) / ln(1.0001) ->
// # (ln(RR/RRe^-rt)) / ln(1.0001) ->
// # (ln(1/e^-rt)) / ln(1.0001) ->
// # (ln(e^rt)) / ln(1.0001) ->
// # rt / ln(1.0001)
// #
// # M: adjustement needed to reach the mid point. 
// # M= S/2:
// # (rt / ln(1.0001)) / 2 ->
// # rt / 2ln(1.0001)
// #
// # It will also be the fee tier used for the deposit, as we want the follwing behavior:
// # assuming M = 40
// #        A ---- 40 ---- M ----- 40 ---- B          
// #        |              |               |
// #        |              |               |
// #        |              |               |
// #-------------------------------------------------
// #     RRe^-rt   (DEPOSIT INDEX)    RR (OLD INDEX)
// # the deposit index will be 40 ticks above or below the old index.
// # if the deposit index is above the mid point, the fee tier will be 40.
// # if the deposit index is below the mid point, the fee tier will be -40.
// # this will be the oracle price skew.
// # the total spread will remain S, but the target to buy ASSET and dASSET respectively will stay at  RRe^-rt and RR repectively
pub fn calculate_fee_tier(apy: Decimal, unbonding_days: u64, fee_dempening_amount: u64) -> ContractResult<u64> {
    // Convert APY from Decimal to f64
    let r: f64 = apy.to_string().parse()
        .map_err(|_| crate::error::ContractError::DecimalConversionError)?;
    
    // Convert unbonding period to years
    let t = unbonding_days as f64 / DAYS_IN_YEAR as f64;
    
    // Calculate fee tier: (r * t) / (2 * ln(1.0001))
    let fee_tier = (r * t) / (2.0 * LN_1_0001);
    
    // dempen by the dampening amount:
    let mut fee_tier_u64 = fee_tier.abs() as u64;

    if fee_tier_u64 > fee_dempening_amount {
        fee_tier_u64 -= fee_dempening_amount;
    } 

    // Return absolute value as u64 (always positive)
    Ok(fee_tier_u64)
}

/// Create fee tiers by adding spacings to the calculated base fee
pub fn create_fee_tiers(
    calculated_base_fee: u64,
    fee_tier_values: &[u64],
    percentages: &[u64],
) -> ContractResult<Vec<FeeTier>> {
    // Validate that percentages sum to 100
    let total_percentage: u64 = percentages.iter().sum();
    if total_percentage != 100 {
        return Err(crate::error::ContractError::InvalidFeeTier {
            reason: "Fee tier percentages must sum to 100".to_string(),
        });
    }
    
    // Validate that we have the same number of fee tier values and percentages
    if percentages.len() != fee_tier_values.len() {
        return Err(crate::error::ContractError::InvalidFeeTier {
            reason: "Number of percentages must match number of fee tier values".to_string(),
        });
    }
    
    let mut fee_tiers = Vec::new();
    
    // Create fee tiers by adding each fee tier value to the calculated base fee
    for (i, &fee_tier_value) in fee_tier_values.iter().enumerate() {
        let final_fee = calculated_base_fee + fee_tier_value;
        fee_tiers.push(FeeTier::new(final_fee, percentages[i]));
    }
    
    Ok(fee_tiers)
}

/// Create the update_config message for a vault contract
pub fn create_vault_update_message(
    vault_address: &str,
    fee_tiers: &[FeeTier],
    oracle_skew: i32,
    _sender: &str,
) -> ContractResult<CosmosMsg> {
    // Convert fee tiers to the format expected by the vault contract
    let fee_tier_list: Vec<Value> = fee_tiers
        .iter()
        .map(|tier| {
            json!({
                "fee": tier.fee,
                "percentage": tier.percentage
            })
        })
        .collect();
    
    let update_config_msg = json!({
        "update_config": {
            "update": {
                "fee_tier_config": {
                    "fee_tiers": fee_tier_list
                },
                "oracle_price_skew": oracle_skew
            }
        }
    });
    
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault_address.to_string(),
        msg: to_json_binary(&update_config_msg)?,
        funds: vec![],
    }))
}

/// Create dex_withdrawal message
pub fn create_dex_withdrawal_message(vault_address: &str) -> ContractResult<CosmosMsg> {
    let msg = json!({"dex_withdrawal": {}});
    
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault_address.to_string(),
        msg: to_json_binary(&msg)?,
        funds: vec![],
    }))
}

/// Create dex_deposit message  
pub fn create_dex_deposit_message(vault_address: &str) -> ContractResult<CosmosMsg> {
    let msg = json!({"dex_deposit": {}});
    
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault_address.to_string(),
        msg: to_json_binary(&msg)?,
        funds: vec![],
    }))
}

/// Validate asset configuration
pub fn validate_asset_config(asset: &crate::msg::AssetData) -> ContractResult<()> {
    if asset.unbonding_period == 0 || asset.unbonding_period > 365 {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: format!("Unbonding period must be between 1 and 365 days, got {}", asset.unbonding_period),
        });
    }

    // Validate that we have at least one fee spacing and percentage
    if asset.fee_spacings.is_empty() || asset.percentages.is_empty() {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: "Fee spacings and percentages cannot be empty".to_string(),
        });
    }

    // Validate that fee spacings and percentages have the same length
    if asset.fee_spacings.len() != asset.percentages.len() {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: format!(
                "Fee spacings length ({}) must match percentages length ({})",
                asset.fee_spacings.len(),
                asset.percentages.len()
            ),
        });
    }

    // Validate that percentages sum to 100
    let total_percentage: u64 = asset.percentages.iter().sum();
    if total_percentage != 100 {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: format!("Percentages must sum to 100, got {}", total_percentage),
        });
    }

    // Validate that no individual percentage is 0 or > 100
    for (i, &percentage) in asset.percentages.iter().enumerate() {
        if percentage == 0 {
            return Err(crate::error::ContractError::InvalidAssetConfig {
                reason: format!("Percentage at index {} cannot be 0", i),
            });
        }
        if percentage > 100 {
            return Err(crate::error::ContractError::InvalidAssetConfig {
                reason: format!("Percentage at index {} cannot exceed 100, got {}", i, percentage),
            });
        }
    }

    // Validate fee spacings are reasonable (should not be too large)
    for (i, &spacing) in asset.fee_spacings.iter().enumerate() {
        if spacing > 1000 {
            return Err(crate::error::ContractError::InvalidAssetConfig {
                reason: format!("Fee spacing at index {} is too large ({}), maximum allowed is 1000", i, spacing),
            });
        }
    }

    // Validate query period is reasonable (1 hour to 1 week)
    if asset.query_period_hours == 0 || asset.query_period_hours > 168 {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: format!("Query period must be between 1 and 168 hours, got {}", asset.query_period_hours),
        });
    }

    // Validate fee dampening amount is reasonable (should not be too large)
    if asset.fee_dempening_amount > 500 {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: format!("Fee dampening amount is too large ({}), maximum allowed is 500", asset.fee_dempening_amount),
        });
    }

    // Validate denom is not empty
    if asset.denom.trim().is_empty() {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: "Asset denom cannot be empty".to_string(),
        });
    }

    // Validate core contract is not empty
    if asset.core_contract.trim().is_empty() {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: "Core contract address cannot be empty".to_string(),
        });
    }

    // Validate vault address is not empty
    if asset.vault_address.trim().is_empty() {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: "Vault address cannot be empty".to_string(),
        });
    }

    Ok(())
}

/// Validate a list of assets
pub fn validate_assets(assets: &[crate::msg::AssetData]) -> ContractResult<()> {
    // Validate that we have at least one asset
    if assets.is_empty() {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: "At least one asset must be provided".to_string(),
        });
    }

    // Validate each asset
    for (i, asset) in assets.iter().enumerate() {
        validate_asset_config(asset).map_err(|e| {
            crate::error::ContractError::InvalidAssetConfig {
                reason: format!("Asset {} validation failed: {}", i, e),
            }
        })?;
    }

    // Check for duplicate vault addresses
    let mut vault_addresses = std::collections::HashSet::new();
    for asset in assets {
        if !vault_addresses.insert(&asset.vault_address) {
            return Err(crate::error::ContractError::InvalidAssetConfig {
                reason: format!("Duplicate vault address found: {}", asset.vault_address),
            });
        }
    }

    Ok(())
}

/// Validate instantiate message
pub fn validate_instantiate_msg(msg: &crate::msg::InstantiateMsg) -> ContractResult<()> {
    validate_assets(&msg.assets)?;

    // Validate apy contract address is not empty
    if msg.apy_contract.trim().is_empty() {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: "APY contract address cannot be empty".to_string(),
        });
    }

    // Validate whitelist is not empty
    if msg.whitelist.is_empty() {
        return Err(crate::error::ContractError::InvalidAssetConfig {
            reason: "At least one admin must be provided in whitelist".to_string(),
        });
    }

    // Validate whitelist addresses are not empty
    for (i, addr) in msg.whitelist.iter().enumerate() {
        if addr.trim().is_empty() {
            return Err(crate::error::ContractError::InvalidAssetConfig {
                reason: format!("Whitelist address at index {} cannot be empty", i),
            });
        }
    }

    Ok(())
}

/// Validate update config message
pub fn validate_update_config(update_config: &crate::msg::UpdateConfig) -> ContractResult<()> {
    if let Some(ref new_assets) = update_config.new_assets {
        validate_assets(new_assets)?;
    }

    // If new APY contract is provided, validate it's not empty
    if let Some(ref new_apy_contract) = update_config.new_apy_contract {
        if new_apy_contract.trim().is_empty() {
            return Err(crate::error::ContractError::InvalidAssetConfig {
                reason: "APY contract address cannot be empty".to_string(),
            });
        }
    }

    // If new whitelist is provided, validate it
    if let Some(ref new_whitelist) = update_config.new_whitelist {
        if new_whitelist.is_empty() {
            return Err(crate::error::ContractError::InvalidAssetConfig {
                reason: "At least one admin must be provided in whitelist".to_string(),
            });
        }

        for (i, addr) in new_whitelist.iter().enumerate() {
            if addr.trim().is_empty() {
                return Err(crate::error::ContractError::InvalidAssetConfig {
                    reason: format!("Whitelist address at index {} cannot be empty", i),
                });
            }
        }
    }

    Ok(())
}

/// Query APY from external APY contract
pub fn query_apy_contract(
    deps: &Deps,
    apy_contract: &Addr,
    instance: &str,
    time_span_hours: u64,
) -> ContractResult<Decimal> {
    let query_msg = ApyQueryMsg::GetApy {
        instance: instance.to_string(),
        time_span_hours,
    };
    
    let query_request = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: apy_contract.to_string(),
        msg: to_json_binary(&query_msg)?,
    });
    
    let result: DropInstanceApy = deps.querier.query(&query_request)?;
    Ok(result.apy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_calculate_fee_tier() {
        // Test with example values/
        // APY of 10% (0.1) with 21 day unbonding period
        let apy = Decimal::from_str("0.1").unwrap(); // 10% APY
        let unbonding_days = 21;
        let fee_dempening_amount = 0;
        
        let fee_tier = calculate_fee_tier(apy, unbonding_days, fee_dempening_amount).unwrap();
        
        // Expected calculation: (0.1 * 21/365) / (2 * ln(1.0001))
        // = (0.1 * 0.0575) / (2 * 0.00009999...)
        // = 0.00575 / 0.000199998...
        // ≈ 28.75 -> 28
        
        assert!(fee_tier == 28, "Expected fee tier around 28, got {}", fee_tier);
    }

    #[test]
    fn test_create_fee_tiers() {
        let calculated_base_fee = 30;
        let fee_tier_values = vec![0, 10]; // First tier uses calculated fee (30+0=30), second tier adds 10 (30+10=40)
        let percentages = vec![35, 65];
        
        let fee_tiers = create_fee_tiers(calculated_base_fee, &fee_tier_values, &percentages).unwrap();
        
        assert_eq!(fee_tiers.len(), 2);
        assert_eq!(fee_tiers[0].fee, 30); // calculated_base_fee + 0
        assert_eq!(fee_tiers[0].percentage, 35);
        assert_eq!(fee_tiers[1].fee, 40); // calculated_base_fee + 10
        assert_eq!(fee_tiers[1].percentage, 65);
    }

    #[test]
    fn test_create_fee_tiers_invalid_percentages() {
        let calculated_base_fee = 30;
        let fee_tier_values = vec![0, 10];
        let percentages = vec![35, 70]; // Sum is 105, not 100
        
        let result = create_fee_tiers(calculated_base_fee, &fee_tier_values, &percentages);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ContractError::InvalidFeeTier { reason } => {
                assert!(reason.contains("sum to 100"));
            }
            _ => panic!("Expected InvalidFeeTier error"),
        }
    }

    #[test]
    fn test_create_fee_tiers_mismatched_lengths() {
        let calculated_base_fee = 30;
        let fee_tier_values = vec![0, 10, 20]; // 3 fee tier values
        let percentages = vec![35, 65]; // 2 percentages, but should be 3 to match fee tier values
        
        let result = create_fee_tiers(calculated_base_fee, &fee_tier_values, &percentages);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ContractError::InvalidFeeTier { reason } => {
                assert!(reason.contains("match number of fee tier values"));
            }
            _ => panic!("Expected InvalidFeeTier error"),
        }
    }

    #[test]
    fn test_calculate_fee_tier_zero_apy() {
        // Test with zero APY
        let apy = Decimal::from_str("0").unwrap(); // 0% APY
        let unbonding_days = 21;
        let fee_dempening_amount = 0;
        
        let fee_tier = calculate_fee_tier(apy, unbonding_days, fee_dempening_amount).unwrap();
        
        // Expected calculation: (0 * 21/365) / (2 * ln(1.0001)) = 0
        assert_eq!(fee_tier, 0, "Expected fee tier to be 0 for zero APY, got {}", fee_tier);
    }

    #[test]
    fn test_calculate_fee_tier_various_apys() {
        let unbonding_days = 21;
        let fee_dempening_amount = 0;

        let test_cases = vec![
            ("0.001", 0),   // 0.1% APY  
            ("0.01", 2),   // 1% APY  
            ("0.05", 14),   // 5% APY  
            ("0.075", 21),  // 7.5% APY   
            ("0.10", 28),   // 10% APY 
            ("0.15", 43),   // 15% APY 
            ("0.20", 57),   // 20% APY 
            ("0.25", 71),   // 25% APY
            ("3.25", 934),   // 325% APY
            ("10.0", 2876),   // 1000% APY
            ("100.0", 28768),   // 10000% APY
        ];
        
        for (apy_str, expected) in test_cases {
            let apy = Decimal::from_str(apy_str).unwrap();
            let fee_tier = calculate_fee_tier(apy, unbonding_days, fee_dempening_amount).unwrap();
            
            println!("APY {} with {} days: calculated fee tier {}", apy_str, unbonding_days, fee_tier);
            
            assert!(
                fee_tier == expected,
                "APY {} with {} days: expected fee tier {}, got {}",
                apy_str, unbonding_days, expected, fee_tier
            );
        }
    }

    #[test]
    fn test_calculate_fee_tier_different_unbonding_periods() {
        let apy = Decimal::from_str("0.10").unwrap(); // 10% APY
        let fee_dempening_amount = 0;
        

        let test_cases = vec![
            (0, 0),     // 7 days  -> actual value is 9
            (7, 9),     // 7 days  -> actual value is 9
            (14, 19),   // 14 days -> actual value is 19  
            (21, 28),   // 21 days -> actual value is 28
            (28, 38),   // 28 days -> actual value is 38
            (30, 41),   // 30 days -> actual value is 41
        ];
        
        for (days, expected) in test_cases {
            let fee_tier = calculate_fee_tier(apy, days, fee_dempening_amount).unwrap();
            
            println!("APY 10% with {} days: calculated fee tier {}", days, fee_tier);
            
            assert!(
                fee_tier == expected,
                "APY 10% with {} days: expected fee tier {} got {}",
                days, expected, fee_tier
            );
        }
    }
    #[test]
    fn test_fee_tier_with_dempening() {
        let apy = Decimal::from_str("0.10").unwrap(); // 10% APY
        let fee_dempening_amount = 10;

        let test_cases = vec![
            // if dampening makes base fee negative, ignore dampening.
            (7, 9),     // 7 days  -> actual value is 9 - 10 = -1 -> 9 (ignore dampening effect)
            (14, 9),    // 14 days -> actual value is 19 - 10 = 9
            (21, 18),   // 21 days -> actual value is 28 - 10 = 18
            (28, 28),   // 28 days -> actual value is 38 - 10 = 28
            (30, 31),   // 30 days -> actual value is 41 - 10 = 31
        ];
        
        for (days, expected) in test_cases {
            let fee_tier = calculate_fee_tier(apy, days, fee_dempening_amount).unwrap();
            
            println!("APY 10% with {} days: calculated fee tier {}", days, fee_tier);
            
            assert!(
                fee_tier == expected,
                "APY 10% with {} days: expected fee tier {}, got {}",
                days, expected, fee_tier
            );
        }
    }
    #[test]
    fn test_create_fee_tiers_with_calculated_apy_bases() {
        // Test realistic scenarios with APY-calculated base fees
        let test_scenarios = vec![
            // Scenario 1: 5% APY -> base fee ~14
            (14, vec![0, 5, 10], vec![50, 30, 20]),
            // Scenario 2: 15% APY -> base fee ~43  
            (43, vec![0, 2, 7], vec![60, 25, 15]),
            // Scenario 3: 25% APY -> base fee ~72
            (72, vec![0, 1, 3, 8], vec![40, 30, 20, 10]),
        ];
        
        for (base_fee, fee_spacings, percentages) in test_scenarios {
            let fee_tiers = create_fee_tiers(base_fee, &fee_spacings, &percentages).unwrap();
            
            // Verify correct number of tiers
            assert_eq!(fee_tiers.len(), fee_spacings.len());
            
            // Verify fees are calculated correctly
            for (i, tier) in fee_tiers.iter().enumerate() {
                assert_eq!(tier.fee, base_fee + fee_spacings[i]);
                assert_eq!(tier.percentage, percentages[i]);
            }
            
            // Verify percentages sum to 100
            let total_percentage: u64 = fee_tiers.iter().map(|t| t.percentage).sum();
            assert_eq!(total_percentage, 100);
        }
    }

    #[test]
    fn test_extreme_apy_values() {
        let unbonding_days = 21;
        let fee_dempening_amount = 0;
        // Test very high APY
        let high_apy = Decimal::from_str("1.0").unwrap(); // 100% APY
        let high_fee_tier = calculate_fee_tier(high_apy, unbonding_days, fee_dempening_amount).unwrap();
        // Expected: (1.0 * 21/365) / (2 * ln(1.0001)) = 287
        assert_eq!(high_fee_tier, 287, "Expected high APY fee tier to be 287, got {}", high_fee_tier);
        
        // Test very low but non-zero APY
        let low_apy = Decimal::from_str("0.001").unwrap(); // 0.1% APY
        let low_fee_tier = calculate_fee_tier(low_apy, unbonding_days, fee_dempening_amount).unwrap();
        // Expected: (0.001 * 21/365) / (2 * ln(1.0001)) = 0
        assert_eq!(low_fee_tier, 0, "Expected low APY fee tier to be 0, got {}", low_fee_tier);
    }

    #[test]
    fn test_apy_fee_tier_linearity() {
        // Test that doubling APY approximately doubles the fee tier
        let unbonding_days = 21;
        let fee_dempening_amount = 0;
        let apy_5_percent = Decimal::from_str("0.05").unwrap();
        let apy_10_percent = Decimal::from_str("0.10").unwrap();
        
        let fee_tier_5 = calculate_fee_tier(apy_5_percent, unbonding_days, fee_dempening_amount).unwrap();
        let fee_tier_10 = calculate_fee_tier(apy_10_percent, unbonding_days, fee_dempening_amount).unwrap();
        
        // Test exact values: 5% APY = 14, 10% APY = 28 (exactly 2:1 ratio)
        assert_eq!(fee_tier_5, 14, "Expected 5% APY fee tier to be 14, got {}", fee_tier_5);
        assert_eq!(fee_tier_10, 28, "Expected 10% APY fee tier to be 28, got {}", fee_tier_10);
        assert_eq!(fee_tier_10, fee_tier_5 * 2, "Expected 10% APY to be exactly double 5% APY");
    }

    #[test]
    fn test_unbonding_period_fee_tier_linearity() {
        // Test that doubling unbonding period approximately doubles the fee tier
        let apy = Decimal::from_str("0.10").unwrap(); // 10% APY
        let fee_dempening_amount = 0;
        let fee_tier_21_days = calculate_fee_tier(apy, 21, fee_dempening_amount).unwrap();
        let fee_tier_42_days = calculate_fee_tier(apy, 42, fee_dempening_amount).unwrap();
        
        // Test exact values: 21 days = 28, 42 days = 57 (approximately 2:1 ratio)
        assert_eq!(fee_tier_21_days, 28, "Expected 21 day unbonding fee tier to be 28, got {}", fee_tier_21_days);
        assert_eq!(fee_tier_42_days, 57, "Expected 42 day unbonding fee tier to be 57, got {}", fee_tier_42_days);
    }

    #[test]
    fn test_full_apy_to_vault_update_flow() {
        // Test complete flow from APY to vault update message creation
        let test_scenarios = vec![
            // Low APY scenario
            ("0.05", 21, vec![0, 2, 5], vec![50, 30, 20]), // 5% APY
            // Medium APY scenario  
            ("0.10", 21, vec![0, 1, 3], vec![60, 25, 15]), // 10% APY
            // High APY scenario
            ("0.20", 21, vec![0, 1, 2, 5], vec![40, 30, 20, 10]), // 20% APY
        ];

        for (apy_str, unbonding_days, fee_spacings, percentages) in test_scenarios {
            let apy = Decimal::from_str(apy_str).unwrap();
            let fee_dempening_amount = 0;
            // Step 1: Calculate base fee tier from APY
            let base_fee = calculate_fee_tier(apy, unbonding_days, fee_dempening_amount).unwrap();
            assert!(base_fee > 0, "Base fee should be positive for non-zero APY");
            
            // Step 2: Create fee tiers
            let fee_tiers = create_fee_tiers(base_fee, &fee_spacings, &percentages).unwrap();
            assert_eq!(fee_tiers.len(), fee_spacings.len());
            
            // Step 3: Calculate oracle skew  
            let oracle_skew = (base_fee + 1) as i32;
            
            // Step 4: Create vault update message
            let vault_address = "neutron1test_vault_address";
            let sender = "neutron1test_sender";
            let update_msg = create_vault_update_message(
                vault_address,
                &fee_tiers,
                oracle_skew,
                sender,
            ).unwrap();
            
            // Verify message structure
            match update_msg {
                cosmwasm_std::CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute { contract_addr, msg: _, funds }) => {
                    assert_eq!(contract_addr, vault_address);
                    assert!(funds.is_empty());
                }
                _ => panic!("Expected WasmMsg::Execute"),
            }
            
            println!("APY {}: base_fee={}, oracle_skew={}, fee_tiers={:?}", 
                    apy_str, base_fee, oracle_skew, 
                    fee_tiers.iter().map(|t| (t.fee, t.percentage)).collect::<Vec<_>>());
        }
    }


    #[test]
    fn test_validate_asset_config_valid() {
        use crate::msg::AssetData;
        
        let valid_asset = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result = validate_asset_config(&valid_asset);
        assert!(result.is_ok(), "Valid asset should pass validation");
    }

    #[test]
    fn test_validate_asset_config_invalid_unbonding_period() {
        use crate::msg::AssetData;
        
        // Test zero unbonding period
        let mut asset = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 0,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result = validate_asset_config(&asset);
        assert!(result.is_err());
        
        // Test too large unbonding period
        asset.unbonding_period = 400;
        let result = validate_asset_config(&asset);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_asset_config_mismatched_lengths() {
        use crate::msg::AssetData;
        
        let asset = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10, 20], // 3 elements
            percentages: vec![35, 65],     // 2 elements - mismatch!
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result = validate_asset_config(&asset);
        assert!(result.is_err());
        
        if let Err(crate::error::ContractError::InvalidAssetConfig { reason }) = result {
            assert!(reason.contains("length"));
        } else {
            panic!("Expected InvalidAssetConfig error");
        }
    }

    #[test]
    fn test_validate_asset_config_invalid_percentages() {
        use crate::msg::AssetData;
        
        // Test percentages don't sum to 100
        let asset = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 70], // sums to 105, not 100
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result = validate_asset_config(&asset);
        assert!(result.is_err());

        // Test zero percentage
        let asset2 = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![0, 100], // one is 0
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result2 = validate_asset_config(&asset2);
        assert!(result2.is_err());

        // Test percentage > 100
        let asset3 = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0],
            percentages: vec![150], // > 100
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result3 = validate_asset_config(&asset3);
        assert!(result3.is_err());
    }

    #[test]
    fn test_validate_asset_config_empty_fields() {
        use crate::msg::AssetData;
        
        // Test empty denom
        let asset = AssetData {
            denom: "".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result = validate_asset_config(&asset);
        assert!(result.is_err());

        // Test empty core contract
        let asset2 = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result2 = validate_asset_config(&asset2);
        assert!(result2.is_err());

        // Test empty vault address
        let asset3 = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result3 = validate_asset_config(&asset3);
        assert!(result3.is_err());
    }

    #[test]
    fn test_validate_asset_config_extreme_values() {
        use crate::msg::AssetData;
        
        // Test too large fee spacing
        let asset = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 1500], // > 1000
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let result = validate_asset_config(&asset);
        assert!(result.is_err());

        // Test invalid query period
        let asset2 = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 200, // > 168
            fee_dempening_amount: 0,
        };

        let result2 = validate_asset_config(&asset2);
        assert!(result2.is_err());

        // Test too large fee dampening amount
        let asset3 = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 600, // > 500
        };

        let result3 = validate_asset_config(&asset3);
        assert!(result3.is_err());
    }

    #[test]
    fn test_validate_instantiate_msg_duplicates() {
        use crate::msg::{AssetData, InstantiateMsg};
        
        let asset1 = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let asset2 = AssetData {
            denom: "factory/neutron1test/udtest".to_string(), // Same denom!
            core_contract: "neutron1test_core2".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let msg = InstantiateMsg {
            assets: vec![asset1, asset2],
            apy_contract: "neutron1test_apy".to_string(),
            whitelist: vec!["neutron1admin1".to_string(), "neutron1admin2".to_string()],
        };

        let result = validate_instantiate_msg(&msg);
        assert!(result.is_err());
    }

    #[test] 
    fn test_validate_instantiate_msg_valid() {
        use crate::msg::{AssetData, InstantiateMsg};
        
        let asset1 = AssetData {
            denom: "factory/neutron1test/udtest1".to_string(),
            core_contract: "neutron1test_core1".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault1".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let asset2 = AssetData {
            denom: "factory/neutron1test/udtest2".to_string(),
            core_contract: "neutron1test_core2".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 5, 15],
            percentages: vec![50, 30, 20],
            vault_address: "neutron1test_vault2".to_string(),
            query_period_hours: 72,
            fee_dempening_amount: 10,
        };

        let msg = InstantiateMsg {
            assets: vec![asset1, asset2],
            apy_contract: "neutron1test_apy".to_string(),
            whitelist: vec!["neutron1admin1".to_string(), "neutron1admin2".to_string()],
        };

        let result = validate_instantiate_msg(&msg);
        assert!(result.is_ok(), "Valid instantiate message should pass validation");
    }


    #[test]
    fn test_validate_update_config_valid_assets_only() {
        use crate::msg::{AssetData, UpdateConfig};
        
        let asset = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let update_config = UpdateConfig {
            new_assets: Some(vec![asset]),
            new_apy_contract: None,
            new_whitelist: None,
        };

        let result = validate_update_config(&update_config);
        assert!(result.is_ok(), "Valid update config should pass validation");
    }

    #[test]
    fn test_validate_update_config_valid_apy_only() {
        use crate::msg::UpdateConfig;
        
        let update_config = UpdateConfig {
            new_assets: None,
            new_apy_contract: Some("neutron1test_apy".to_string()),
            new_whitelist: None,
        };

        let result = validate_update_config(&update_config);
        assert!(result.is_ok(), "Valid APY update should pass validation");
    }

    #[test]
    fn test_validate_update_config_empty_assets() {
        use crate::msg::UpdateConfig;
        
        let update_config = UpdateConfig {
            new_assets: Some(vec![]), // Empty assets
            new_apy_contract: None,
            new_whitelist: None,
        };

        let result = validate_update_config(&update_config);
        assert!(result.is_err(), "Empty assets should fail validation");
    }

    #[test]
    fn test_validate_update_config_empty_apy_contract() {
        use crate::msg::UpdateConfig;
        
        let update_config = UpdateConfig {
            new_assets: None,
            new_apy_contract: Some("".to_string()), // Empty APY contract
            new_whitelist: None,
        };

        let result = validate_update_config(&update_config);
        assert!(result.is_err(), "Empty APY contract should fail validation");
    }

    #[test]
    fn test_validate_update_config_invalid_assets() {
        use crate::msg::{AssetData, UpdateConfig};
        
        let invalid_asset = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 0, // Invalid unbonding period
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let update_config = UpdateConfig {
            new_assets: Some(vec![invalid_asset]),
            new_apy_contract: None,
            new_whitelist: None,
        };

        let result = validate_update_config(&update_config);
        assert!(result.is_err(), "Invalid asset should fail validation");
    }

    #[test]
    fn test_validate_update_config_both_fields() {
        use crate::msg::{AssetData, UpdateConfig};
        
        let asset = AssetData {
            denom: "factory/neutron1test/udtest".to_string(),
            core_contract: "neutron1test_core".to_string(),
            unbonding_period: 21,
            fee_spacings: vec![0, 10],
            percentages: vec![35, 65],
            vault_address: "neutron1test_vault".to_string(),
            query_period_hours: 24,
            fee_dempening_amount: 0,
        };

        let update_config = UpdateConfig {
            new_assets: Some(vec![asset]),
            new_apy_contract: Some("neutron1test_apy".to_string()),
            new_whitelist: None,
        };

        let result = validate_update_config(&update_config);
        assert!(result.is_ok(), "Valid update config with both fields should pass validation");
    }

    #[test]
    fn test_validate_update_config_new_whitelist() {
        use crate::msg::UpdateConfig;
        
        let new_whitelist = vec![
            "neutron1admin1".to_string(),
            "neutron1admin2".to_string(),
            "neutron1admin3".to_string(),
        ];

        let update_config = UpdateConfig {
            new_assets: None,
            new_apy_contract: None,
            new_whitelist: Some(new_whitelist),
        };

        let result = validate_update_config(&update_config);
        assert!(result.is_ok(), "Valid whitelist update should pass validation");
    }
}
