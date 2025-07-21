use crate::contract::execute;
use crate::error::ContractError;
use crate::msg::{CombinedPriceResponse, ExecuteMsg};
use crate::state::{Config, FeeTier, FeeTierConfig, PairData, TokenData, CONFIG};
use crate::testing::mock_querier::{mock_dependencies_with_custom_querier, MockQuerier};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::Env;
use cosmwasm_std::{Addr, Coin, Uint128};
use neutron_std::types::cosmos::base::v1beta1::Coin as NeutronCoin;
use neutron_std::types::neutron::dex::{DepositRecord, MsgWithdrawalResponse, PairId};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use std::str::FromStr;

// Helper function to create a test config
fn setup_test_config(env: Env) -> Config {
    Config {
        oracle_contract: Addr::unchecked("oracle"),
        lp_denom: "factory/contract/lp".to_string(),
        pair_data: PairData {
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
            pair_id: "token0<>token1".to_string(),
        },
        total_shares: Uint128::zero(),
        whitelist: vec![Addr::unchecked("owner")],
        deposit_cap: Uint128::MAX,
        fee_tier_config: FeeTierConfig {
            fee_tiers: vec![
                FeeTier {
                    fee: 100,
                    percentage: 50,
                },
                FeeTier {
                    fee: 500,
                    percentage: 30,
                },
                FeeTier {
                    fee: 3000,
                    percentage: 20,
                },
            ],
        },
        timestamp_stale: 3600,
        last_executed: 0,
        pause_block: 0,
        paused: false,
        skew: 0i32,
        imbalance: 50,
        oracle_price_skew: 0i32,
        dynamic_spread_factor: 0i32,
        dynamic_spread_cap: 0i32,
    }
}

// Helper function to setup mock querier
fn setup_mock_querier() -> MockQuerier {
    let mut querier = MockQuerier::new();

    // Setup price response
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
fn test_dex_withdrawal_success() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance
    let contract_balance_0 = 1000000u128;
    let contract_balance_1 = 1000000u128;

    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(contract_balance_0, "token0"),
            Coin::new(contract_balance_1, "token1"),
        ],
    );

    // Setup active deposits
    let deposits = vec![
        DepositRecord {
            pair_id: Some(PairId {
                token0: "token0".to_string(),
                token1: "token1".to_string(),
            }),
            shares_owned: "500000".to_string(),
            center_tick_index: 0,
            lower_tick_index: 0,
            upper_tick_index: 0,
            fee: 100,
            total_shares: Some("500000".to_string()),
            pool: None,
        },
        DepositRecord {
            pair_id: Some(PairId {
                token0: "token0".to_string(),
                token1: "token1".to_string(),
            }),
            shares_owned: "500000".to_string(),
            center_tick_index: 0,
            lower_tick_index: 0,
            upper_tick_index: 0,
            fee: 500,
            total_shares: Some("500000".to_string()),
            pool: None,
        },
    ];
    querier.set_user_deposits_all_response(deposits);

    // Setup withdrawal simulation response
    let withdrawal_sim_response = MsgWithdrawalResponse {
        reserve0_withdrawn: "250000".to_string(),
        reserve1_withdrawn: "250000".to_string(),
        shares_burned: vec![NeutronCoin {
            denom: "lp_token".to_string(),
            amount: "500000".to_string(),
        }],
    };
    querier.set_withdrawal_simulation_response(withdrawal_sim_response);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex withdrawal
    let info = mock_info("owner", &[]);

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::DexWithdrawal {},
    )
    .unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 1);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_withdrawal");

    // Verify that withdrawal messages were created
    assert_eq!(res.messages.len(), 2); // One for each deposit

    // Check that the messages are withdrawal messages
    for msg in res.messages.iter() {
        match &msg.msg {
            cosmwasm_std::CosmosMsg::Any(any_msg) => {
                assert_eq!(any_msg.type_url, "/neutron.dex.MsgWithdrawal");
            }
            _ => panic!("Expected Any message with MsgWithdrawal type_url"),
        }
    }
}

#[test]
fn test_dex_withdrawal_no_active_deposits() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    // Setup empty deposits (no active deposits)
    querier.set_user_deposits_all_response(vec![]);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex withdrawal
    let info = mock_info("owner", &[]);

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::DexWithdrawal {},
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 1);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_withdrawal");

    // Verify that no withdrawal messages were created (no active deposits)
    assert_eq!(res.messages.len(), 0);
}

#[test]
fn test_dex_withdrawal_unauthorized() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let mut config = setup_test_config(env.clone());

    // Set whitelist to only include specific addresses
    config.whitelist = vec![Addr::unchecked("owner")];
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex withdrawal with unauthorized user
    let info = mock_info("unauthorized_user", &[]);

    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::DexWithdrawal {},
    )
    .unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_dex_withdrawal_with_funds() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex withdrawal with funds (should fail)
    let info = mock_info("owner", &[Coin::new(100u128, "token0")]);

    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::DexWithdrawal {},
    )
    .unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::FundsNotAllowed);
}

#[test]
fn test_dex_withdrawal_multiple_deposits_different_fees() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    // Setup active deposits with different fee tiers
    let deposits = vec![
        DepositRecord {
            pair_id: Some(PairId {
                token0: "token0".to_string(),
                token1: "token1".to_string(),
            }),
            shares_owned: "300000".to_string(),
            center_tick_index: 0,
            lower_tick_index: 0,
            upper_tick_index: 0,
            fee: 100,
            total_shares: Some("300000".to_string()),
            pool: None,
        },
        DepositRecord {
            pair_id: Some(PairId {
                token0: "token0".to_string(),
                token1: "token1".to_string(),
            }),
            shares_owned: "300000".to_string(),
            center_tick_index: 0,
            lower_tick_index: 0,
            upper_tick_index: 0,
            fee: 500,
            total_shares: Some("300000".to_string()),
            pool: None,
        },
        DepositRecord {
            pair_id: Some(PairId {
                token0: "token0".to_string(),
                token1: "token1".to_string(),
            }),
            shares_owned: "400000".to_string(),
            center_tick_index: 0,
            lower_tick_index: 0,
            upper_tick_index: 0,
            fee: 3000,
            total_shares: Some("400000".to_string()),
            pool: None,
        },
    ];
    querier.set_user_deposits_all_response(deposits);

    // Setup withdrawal simulation response
    let withdrawal_sim_response = MsgWithdrawalResponse {
        reserve0_withdrawn: "100000".to_string(),
        reserve1_withdrawn: "100000".to_string(),
        shares_burned: vec![NeutronCoin {
            denom: "lp_token".to_string(),
            amount: "500000".to_string(),
        }],
    };
    querier.set_withdrawal_simulation_response(withdrawal_sim_response);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex withdrawal
    let info = mock_info("owner", &[]);

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::DexWithdrawal {},
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 1);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_withdrawal");

    // Verify that withdrawal messages were created - one for each deposit
    assert_eq!(res.messages.len(), 3);

    // Check that the messages are withdrawal messages
    for msg in res.messages.iter() {
        match &msg.msg {
            cosmwasm_std::CosmosMsg::Any(any_msg) => {
                assert_eq!(any_msg.type_url, "/neutron.dex.MsgWithdrawal");
            }
            _ => panic!("Expected Any message with MsgWithdrawal type_url"),
        }
    }
}
