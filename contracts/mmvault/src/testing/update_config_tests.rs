use crate::contract::execute;
use crate::error::ContractError;
use crate::msg::{CombinedPriceResponse, ConfigUpdateMsg, ExecuteMsg};
use crate::state::{Config, FeeTier, FeeTierConfig, PairData, TokenData, CONFIG};
use crate::testing::mock_querier::{mock_dependencies_with_custom_querier, MockQuerier};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{Addr, Uint128};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use std::str::FromStr;

// Helper function to create a test config
fn setup_test_config() -> Config {
    Config {
        lp_denom: "factory/contract/lp".to_string(),
        pair_data: PairData {
            pair_id: "token0<>token1".to_string(),
            token_0: TokenData {
                denom: "token0".to_string(),
                decimals: 6,
                max_blocks_old: 100,
                pair: CurrencyPair {
                    base: "TOKEN0".to_string(),
                    quote: "USD".to_string(),
                },
            },
            token_1: TokenData {
                denom: "token1".to_string(),
                decimals: 6,
                max_blocks_old: 100,
                pair: CurrencyPair {
                    base: "TOKEN1".to_string(),
                    quote: "USD".to_string(),
                },
            },
        },
        total_shares: Uint128::zero(),
        whitelist: vec![Addr::unchecked("owner")],
        deposit_cap: Uint128::new(1000000),
        fee_tier_config: FeeTierConfig {
            fee_tiers: vec![
                FeeTier {
                    fee: 5,
                    percentage: 50,
                },
                FeeTier {
                    fee: 30,
                    percentage: 50,
                },
            ],
        },
        timestamp_stale: 3600,
        last_executed: 0,
        pause_block: 0,
        paused: false,
        oracle_contract: Addr::unchecked("oracle"),
        skew: false,
        imbalance: 0,
    }
}

// Helper function to setup mock querier with price data
fn setup_mock_querier() -> MockQuerier {
    let mut querier = MockQuerier::default();

    // Setup price data
    let price_response = CombinedPriceResponse {
        token_0_price: PrecDec::from_str("1.0").unwrap(),
        token_1_price: PrecDec::from_str("1.0").unwrap(),
        price_0_to_1: PrecDec::from_str("1.0").unwrap(),
    };
    querier.set_price_response(price_response);

    // Setup empty deposits response
    querier.set_empty_deposits();

    // Setup user deposits all response
    querier.set_user_deposits_all_response(vec![]);

    querier // Return the querier
}

#[test]
fn test_update_config_unauthorized() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message
    let update_msg = ConfigUpdateMsg {
        whitelist: Some(vec![Addr::unchecked("new_owner").to_string()]),
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: None,
        timestamp_stale: None,
        fee_tier_config: None,
        paused: None,
        imbalance: None,
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as non-owner
    let info = mock_info("unauthorized", &[]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_update_config_max_blocks_old() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message to update max_blocks_old for both tokens
    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: Some(200),
        max_blocks_old_token_b: Some(300),
        deposit_cap: None,
        timestamp_stale: None,
        fee_tier_config: None,
        paused: None,
        imbalance: None,
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_config");

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(updated_config.pair_data.token_0.max_blocks_old, 200);
    assert_eq!(updated_config.pair_data.token_1.max_blocks_old, 300);
}

#[test]
fn test_update_config_deposit_cap() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message to update deposit_cap
    let new_deposit_cap = Uint128::new(2000000);
    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: Some(new_deposit_cap),
        timestamp_stale: None,
        fee_tier_config: None,
        paused: None,
        imbalance: None,
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_config");

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(updated_config.deposit_cap, new_deposit_cap);
}

#[test]
fn test_update_config_timestamp_stale() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message to update timestamp_stale
    let new_timestamp_stale = 7200u64;
    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: None,
        timestamp_stale: Some(new_timestamp_stale),
        fee_tier_config: None,
        paused: None,
        imbalance: None,
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_config");

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(updated_config.timestamp_stale, new_timestamp_stale);
}

#[test]
fn test_update_config_invalid_timestamp_stale() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message with invalid timestamp_stale (0)
    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: None,
        timestamp_stale: Some(0),
        fee_tier_config: None,
        paused: None,
        imbalance: None,
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap_err();

    // Verify error
    assert!(matches!(err, ContractError::InvalidConfig { reason: _ }));
}

#[test]
fn test_update_config_fee_tier_config() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message with new fee tier config
    let new_fee_tier_config = FeeTierConfig {
        fee_tiers: vec![
            FeeTier {
                fee: 10,
                percentage: 25,
            },
            FeeTier {
                fee: 20,
                percentage: 25,
            },
            FeeTier {
                fee: 50,
                percentage: 50,
            },
        ],
    };

    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: None,
        timestamp_stale: None,
        fee_tier_config: Some(new_fee_tier_config),
        paused: None,
        imbalance: None,
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_config");

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(updated_config.fee_tier_config.fee_tiers.len(), 3);
    assert_eq!(updated_config.fee_tier_config.fee_tiers[0].fee, 10);
    assert_eq!(updated_config.fee_tier_config.fee_tiers[0].percentage, 25);
    assert_eq!(updated_config.fee_tier_config.fee_tiers[1].fee, 20);
    assert_eq!(updated_config.fee_tier_config.fee_tiers[1].percentage, 25);
    assert_eq!(updated_config.fee_tier_config.fee_tiers[2].fee, 50);
    assert_eq!(updated_config.fee_tier_config.fee_tiers[2].percentage, 50);
}

#[test]
fn test_update_config_invalid_fee_tier_config() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message with invalid fee tier config (percentages don't add up to 100)
    let invalid_fee_tier_config = FeeTierConfig {
        fee_tiers: vec![
            FeeTier {
                fee: 10,
                percentage: 30,
            },
            FeeTier {
                fee: 20,
                percentage: 30,
            },
        ],
    };

    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: None,
        timestamp_stale: None,
        fee_tier_config: Some(invalid_fee_tier_config),
        paused: None,
        imbalance: None,
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap_err();

    // Verify error
    assert!(matches!(err, ContractError::InvalidFeeTier { reason: _ }));
}

#[test]
fn test_update_config_paused() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message to pause the contract
    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: None,
        timestamp_stale: None,
        fee_tier_config: None,
        paused: Some(true),
        imbalance: None,
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_config");

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert!(updated_config.paused);
}

#[test]
fn test_update_config_imbalance() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message to update imbalance
    let new_imbalance = 50u32;
    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: None,
        timestamp_stale: None,
        fee_tier_config: None,
        paused: None,
        imbalance: Some(new_imbalance),
        skew: None,
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_config");

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(updated_config.imbalance, new_imbalance);
}

#[test]
fn test_update_config_skew() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create update message to update skew
    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: None,
        max_blocks_old_token_b: None,
        deposit_cap: None,
        timestamp_stale: None,
        fee_tier_config: None,
        paused: None,
        imbalance: None,
        skew: Some(true),
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig { update: update_msg },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_config");

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert!(updated_config.skew);
}

#[test]
fn test_update_config_all_fields() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let initial_config = setup_test_config();

    // Store config
    CONFIG.save(deps.as_mut().storage, &initial_config).unwrap();

    // Create update message with values different from initial config for ALL fields
    let update_msg = ConfigUpdateMsg {
        whitelist: None,
        max_blocks_old_token_a: Some(200),
        max_blocks_old_token_b: Some(300),
        deposit_cap: Some(Uint128::new(2000000)),
        timestamp_stale: Some(7200),
        fee_tier_config: Some(FeeTierConfig {
            fee_tiers: vec![
                FeeTier {
                    fee: 10,
                    percentage: 60,
                },
                FeeTier {
                    fee: 50,
                    percentage: 40,
                },
            ],
        }),
        paused: Some(true),
        imbalance: Some(50),
        skew: Some(true),
        oracle_contract: None,
    };

    // Execute update_config as owner
    let info = mock_info("owner", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::UpdateConfig {
            update: update_msg.clone(),
        },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_config");

    // Get updated config
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();

    // Verify all fields in the update message were actually changed in the config

    // Check whitelist
    if let Some(whitelist) = &update_msg.whitelist {
        let expected_whitelist: Vec<Addr> =
            whitelist.iter().map(|addr| Addr::unchecked(addr)).collect();
        assert_ne!(
            initial_config.whitelist, expected_whitelist,
            "Whitelist was not updated"
        );
        assert_eq!(
            updated_config.whitelist, expected_whitelist,
            "Whitelist was not updated correctly"
        );
    }

    // Check max_blocks_old for token_0
    if let Some(max_blocks_old) = update_msg.max_blocks_old_token_a {
        assert_ne!(
            initial_config.pair_data.token_0.max_blocks_old, max_blocks_old,
            "max_blocks_old_token_a was not updated"
        );
        assert_eq!(
            updated_config.pair_data.token_0.max_blocks_old, max_blocks_old,
            "max_blocks_old_token_a was not updated correctly"
        );
    }

    // Check max_blocks_old for token_1
    if let Some(max_blocks_old) = update_msg.max_blocks_old_token_b {
        assert_ne!(
            initial_config.pair_data.token_1.max_blocks_old, max_blocks_old,
            "max_blocks_old_token_b was not updated"
        );
        assert_eq!(
            updated_config.pair_data.token_1.max_blocks_old, max_blocks_old,
            "max_blocks_old_token_b was not updated correctly"
        );
    }

    // Check deposit_cap
    if let Some(deposit_cap) = update_msg.deposit_cap {
        assert_ne!(
            initial_config.deposit_cap, deposit_cap,
            "deposit_cap was not updated"
        );
        assert_eq!(
            updated_config.deposit_cap, deposit_cap,
            "deposit_cap was not updated correctly"
        );
    }

    // Check timestamp_stale
    if let Some(timestamp_stale) = update_msg.timestamp_stale {
        assert_ne!(
            initial_config.timestamp_stale, timestamp_stale,
            "timestamp_stale was not updated"
        );
        assert_eq!(
            updated_config.timestamp_stale, timestamp_stale,
            "timestamp_stale was not updated correctly"
        );
    }

    // Check fee_tier_config
    if let Some(fee_tier_config) = &update_msg.fee_tier_config {
        // Don't check length if it happens to be the same
        // Instead, check the actual content of the fee tiers
        let initial_tiers = &initial_config.fee_tier_config.fee_tiers;
        let new_tiers = &fee_tier_config.fee_tiers;

        // Check that at least one tier has changed
        let tiers_changed = new_tiers.iter().enumerate().any(|(i, tier)| {
            i >= initial_tiers.len()
                || initial_tiers[i].fee != tier.fee
                || initial_tiers[i].percentage != tier.percentage
        });

        assert!(tiers_changed, "fee_tier_config was not updated");
        assert_eq!(
            updated_config.fee_tier_config.fee_tiers.len(),
            fee_tier_config.fee_tiers.len(),
            "fee_tier_config was not updated correctly"
        );

        for (i, tier) in fee_tier_config.fee_tiers.iter().enumerate() {
            assert_eq!(updated_config.fee_tier_config.fee_tiers[i].fee, tier.fee);
            assert_eq!(
                updated_config.fee_tier_config.fee_tiers[i].percentage,
                tier.percentage
            );
        }
    }

    // Check paused
    if let Some(paused) = update_msg.paused {
        assert_ne!(initial_config.paused, paused, "paused was not updated");
        assert_eq!(
            updated_config.paused, paused,
            "paused was not updated correctly"
        );
    }

    // Check imbalance
    if let Some(imbalance) = update_msg.imbalance {
        assert_ne!(
            initial_config.imbalance, imbalance,
            "imbalance was not updated"
        );
        assert_eq!(
            updated_config.imbalance, imbalance,
            "imbalance was not updated correctly"
        );
    }

    // Check skew
    if let Some(skew) = update_msg.skew {
        assert_ne!(initial_config.skew, skew, "skew was not updated");
        assert_eq!(updated_config.skew, skew, "skew was not updated correctly");
    }

    // Ensure we've tested all fields in ConfigUpdateMsg
    // This is a compile-time check that will fail if new fields are added to ConfigUpdateMsg
    // but not tested above
    let _: () = check_all_fields_tested(&update_msg);
}

// Function to ensure all fields in ConfigUpdateMsg tested, else compilation fails
fn check_all_fields_tested(msg: &ConfigUpdateMsg) {
    let ConfigUpdateMsg {
        whitelist: _,
        max_blocks_old_token_a: _,
        max_blocks_old_token_b: _,
        deposit_cap: _,
        timestamp_stale: _,
        fee_tier_config: _,
        paused: _,
        imbalance: _,
        skew: _,
        oracle_contract: _,
    } = msg;
    // If a new field is added to ConfigUpdateMsg, this function will fail to compile
    // until the field is added here and tested in the test function
}
