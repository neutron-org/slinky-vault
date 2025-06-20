use crate::contract::execute;
use crate::error::ContractError;
use crate::msg::{CombinedPriceResponse, ExecuteMsg};
use crate::state::{Config, FeeTier, FeeTierConfig, PairData, TokenData, CONFIG};
use crate::testing::mock_querier::{mock_dependencies_with_custom_querier, MockQuerier};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::Env;
use cosmwasm_std::{Addr, Coin, Uint128};
use neutron_std::types::neutron::dex::{DepositRecord, MsgDeposit, PairId};
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
        deposit_cap: Uint128::new(1000000),
        fee_tier_config: FeeTierConfig {
            fee_tiers: vec![
                FeeTier {
                    fee: 5,
                    percentage: 60,
                },
                FeeTier {
                    fee: 10,
                    percentage: 30,
                },
                FeeTier {
                    fee: 150,
                    percentage: 10,
                },
            ],
        },
        last_executed: env.block.time.seconds(),
        timestamp_stale: 1000000,
        paused: false,
        pause_block: 0,
        skew: false,
        imbalance: 50u32,
        oracle_price_skew: 0i32,
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
fn test_dex_deposit_success_even_values_1_fee_tier() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.fee_tier_config.fee_tiers = vec![FeeTier {
        fee: 5,
        percentage: 100,
    }];
    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    // Check that we have the expected number of messages based on fee tiers
    assert_eq!(res.messages.len(), config.fee_tier_config.fee_tiers.len()); // One message for each fee tier

    // Decode and verify deposit amounts for each fee tier
    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &config.fee_tier_config.fee_tiers[i];
            // Calculate expected deposit amounts (based on fee tier percentage)
            let expected_percentage = fee_tier.percentage as u128;
            let expected_token0_amount = (1000000u128 * expected_percentage) / 100;
            let expected_token1_amount = (1000000u128 * expected_percentage) / 100;

            // Verify deposit details
            assert_eq!(deposit_msg.creator, env.contract.address.to_string());
            assert_eq!(deposit_msg.receiver, env.contract.address.to_string());
            assert_eq!(deposit_msg.token_a, "token0");
            assert_eq!(deposit_msg.token_b, "token1");
            assert_eq!(
                deposit_msg.amounts_a,
                vec![expected_token0_amount.to_string()]
            );
            assert_eq!(
                deposit_msg.amounts_b,
                vec![expected_token1_amount.to_string()]
            );
            assert_eq!(deposit_msg.tick_indexes_a_to_b, vec![0]);
            assert_eq!(deposit_msg.fees, vec![fee_tier.fee]);

            // Verify deposit options
            if !deposit_msg.options.is_empty() {
                assert!(!deposit_msg.options[0].disable_autoswap);
            }
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}

#[test]
fn test_dex_deposit_success_even_values_2_fee_tiers() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.fee_tier_config.fee_tiers = vec![
        FeeTier {
            fee: 5,
            percentage: 60,
        },
        FeeTier {
            fee: 10,
            percentage: 40,
        },
    ];
    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    // Check that we have the expected number of messages based on fee tiers
    assert_eq!(res.messages.len(), config.fee_tier_config.fee_tiers.len()); // One message for each fee tier

    // Decode and verify deposit amounts for each fee tier
    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &config.fee_tier_config.fee_tiers[i];
            // Calculate expected deposit amounts (based on fee tier percentage)
            let expected_percentage = fee_tier.percentage as u128;
            let expected_token0_amount = (1000000u128 * expected_percentage) / 100;
            let expected_token1_amount = (1000000u128 * expected_percentage) / 100;

            // Verify deposit details
            assert_eq!(deposit_msg.creator, env.contract.address.to_string());
            assert_eq!(deposit_msg.receiver, env.contract.address.to_string());
            assert_eq!(deposit_msg.token_a, "token0");
            assert_eq!(deposit_msg.token_b, "token1");
            assert_eq!(
                deposit_msg.amounts_a,
                vec![expected_token0_amount.to_string()]
            );
            assert_eq!(
                deposit_msg.amounts_b,
                vec![expected_token1_amount.to_string()]
            );
            assert_eq!(deposit_msg.tick_indexes_a_to_b, vec![0]);
            assert_eq!(deposit_msg.fees, vec![fee_tier.fee]);

            // Verify deposit options
            if !deposit_msg.options.is_empty() {
                assert!(!deposit_msg.options[0].disable_autoswap);
            }
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}
#[test]
fn test_dex_deposit_success_even_values_3_fee_tiers() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    // Check that we have the expected number of messages based on fee tiers
    assert_eq!(res.messages.len(), 3); // One message for each fee tier

    // Decode and verify deposit amounts for each fee tier
    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &config.fee_tier_config.fee_tiers[i];
            // Calculate expected deposit amounts (based on fee tier percentage)
            let expected_percentage = fee_tier.percentage as u128;
            let expected_token0_amount = (1000000u128 * expected_percentage) / 100;
            let expected_token1_amount = (1000000u128 * expected_percentage) / 100;

            // Verify deposit details
            assert_eq!(deposit_msg.creator, env.contract.address.to_string());
            assert_eq!(deposit_msg.receiver, env.contract.address.to_string());
            assert_eq!(deposit_msg.token_a, "token0");
            assert_eq!(deposit_msg.token_b, "token1");
            assert_eq!(
                deposit_msg.amounts_a,
                vec![expected_token0_amount.to_string()]
            );
            assert_eq!(
                deposit_msg.amounts_b,
                vec![expected_token1_amount.to_string()]
            );
            assert_eq!(deposit_msg.tick_indexes_a_to_b, vec![0]);
            assert_eq!(deposit_msg.fees, vec![fee_tier.fee]);

            // Verify deposit options
            if !deposit_msg.options.is_empty() {
                assert!(!deposit_msg.options[0].disable_autoswap);
            }
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}

#[test]
fn test_dex_deposit_success_even_values_4_fee_tiers() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.fee_tier_config.fee_tiers = vec![
        FeeTier {
            fee: 5,
            percentage: 15,
        },
        FeeTier {
            fee: 10,
            percentage: 20,
        },
        FeeTier {
            fee: 150,
            percentage: 30,
        },
        FeeTier {
            fee: 200,
            percentage: 35,
        },
    ];
    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    // Check that we have the expected number of messages based on fee tiers
    assert_eq!(res.messages.len(), config.fee_tier_config.fee_tiers.len()); // One message for each fee tier

    // Decode and verify deposit amounts for each fee tier
    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &config.fee_tier_config.fee_tiers[i];
            // Calculate expected deposit amounts (based on fee tier percentage)
            let expected_percentage = fee_tier.percentage as u128;
            let expected_token0_amount = (1000000u128 * expected_percentage) / 100;
            let expected_token1_amount = (1000000u128 * expected_percentage) / 100;

            // Verify deposit details
            assert_eq!(deposit_msg.creator, env.contract.address.to_string());
            assert_eq!(deposit_msg.receiver, env.contract.address.to_string());
            assert_eq!(deposit_msg.token_a, "token0");
            assert_eq!(deposit_msg.token_b, "token1");
            assert_eq!(
                deposit_msg.amounts_a,
                vec![expected_token0_amount.to_string()]
            );
            assert_eq!(
                deposit_msg.amounts_b,
                vec![expected_token1_amount.to_string()]
            );
            assert_eq!(deposit_msg.tick_indexes_a_to_b, vec![0]);
            assert_eq!(deposit_msg.fees, vec![fee_tier.fee]);

            // Verify deposit options
            if !deposit_msg.options.is_empty() {
                assert!(!deposit_msg.options[0].disable_autoswap);
            }
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}

#[test]
fn test_dex_deposit_success_even_values_zero_first_percentage() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.fee_tier_config.fee_tiers = vec![
        FeeTier {
            fee: 5,
            percentage: 0,
        },
        FeeTier {
            fee: 10,
            percentage: 30,
        },
        FeeTier {
            fee: 150,
            percentage: 40,
        },
        FeeTier {
            fee: 200,
            percentage: 30,
        },
    ];
    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    // Check that we have the expected number of messages based on fee tiers
    assert_eq!(
        res.messages.len(),
        config.fee_tier_config.fee_tiers.len() - 1
    ); // One message for each fee tier

    // Decode and verify deposit amounts for each fee tier
    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &config.fee_tier_config.fee_tiers[i + 1];
            // Calculate expected deposit amounts (based on fee tier percentage)
            let expected_percentage = fee_tier.percentage as u128;
            let expected_token0_amount = (1000000u128 * expected_percentage) / 100;
            let expected_token1_amount = (1000000u128 * expected_percentage) / 100;
            // Verify deposit details
            assert_eq!(deposit_msg.creator, env.contract.address.to_string());
            assert_eq!(deposit_msg.receiver, env.contract.address.to_string());
            assert_eq!(deposit_msg.token_a, "token0");
            assert_eq!(deposit_msg.token_b, "token1");
            assert_eq!(
                deposit_msg.amounts_a,
                vec![expected_token0_amount.to_string()]
            );
            assert_eq!(
                deposit_msg.amounts_b,
                vec![expected_token1_amount.to_string()]
            );
            assert_eq!(deposit_msg.tick_indexes_a_to_b, vec![0]);
            assert_eq!(deposit_msg.fees, vec![fee_tier.fee]);

            // Verify deposit options
            if !deposit_msg.options.is_empty() {
                assert!(!deposit_msg.options[0].disable_autoswap);
            }
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}
#[test]
fn test_dex_deposit_success_even_values_zero_percentage() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.fee_tier_config.fee_tiers = vec![
        FeeTier {
            fee: 5,
            percentage: 30,
        },
        FeeTier {
            fee: 10,
            percentage: 0,
        },
        FeeTier {
            fee: 150,
            percentage: 40,
        },
        FeeTier {
            fee: 200,
            percentage: 30,
        },
    ];
    let iteration_tiers = [
        FeeTier {
            fee: 5,
            percentage: 30,
        },
        FeeTier {
            fee: 150,
            percentage: 40,
        },
        FeeTier {
            fee: 200,
            percentage: 30,
        },
    ];
    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    // Check that we have the expected number of messages based on fee tiers
    assert_eq!(res.messages.len(), iteration_tiers.len()); // One message for each fee tier

    // Decode and verify deposit amounts for each fee tier
    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &iteration_tiers[i];
            // Calculate expected deposit amounts (based on fee tier percentage)
            let expected_percentage = fee_tier.percentage as u128;
            let expected_token0_amount = (1000000u128 * expected_percentage) / 100;
            let expected_token1_amount = (1000000u128 * expected_percentage) / 100;
            // Verify deposit details
            assert_eq!(deposit_msg.creator, env.contract.address.to_string());
            assert_eq!(deposit_msg.receiver, env.contract.address.to_string());
            assert_eq!(deposit_msg.token_a, "token0");
            assert_eq!(deposit_msg.token_b, "token1");
            assert_eq!(
                deposit_msg.amounts_a,
                vec![expected_token0_amount.to_string()]
            );
            assert_eq!(
                deposit_msg.amounts_b,
                vec![expected_token1_amount.to_string()]
            );
            assert_eq!(deposit_msg.tick_indexes_a_to_b, vec![0]);
            assert_eq!(deposit_msg.fees, vec![fee_tier.fee]);

            // Verify deposit options
            if !deposit_msg.options.is_empty() {
                assert!(!deposit_msg.options[0].disable_autoswap);
            }
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}
#[test]
fn test_dex_deposit_success_even_values_zero_multiple_percentages() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.fee_tier_config.fee_tiers = vec![
        FeeTier {
            fee: 5,
            percentage: 0,
        },
        FeeTier {
            fee: 10,
            percentage: 0,
        },
        FeeTier {
            fee: 150,
            percentage: 100,
        },
        FeeTier {
            fee: 200,
            percentage: 0,
        },
    ];
    let iteration_tiers = [FeeTier {
        fee: 150,
        percentage: 100,
    }];
    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    // Check that we have the expected number of messages based on fee tiers
    assert_eq!(res.messages.len(), iteration_tiers.len()); // One message for each fee tier

    // Decode and verify deposit amounts for each fee tier
    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &iteration_tiers[i];
            // Calculate expected deposit amounts (based on fee tier percentage)
            let expected_percentage = fee_tier.percentage as u128;
            let expected_token0_amount = (1000000u128 * expected_percentage) / 100;
            let expected_token1_amount = (1000000u128 * expected_percentage) / 100;
            // Verify deposit details
            assert_eq!(deposit_msg.creator, env.contract.address.to_string());
            assert_eq!(deposit_msg.receiver, env.contract.address.to_string());
            assert_eq!(deposit_msg.token_a, "token0");
            assert_eq!(deposit_msg.token_b, "token1");
            assert_eq!(
                deposit_msg.amounts_a,
                vec![expected_token0_amount.to_string()]
            );
            assert_eq!(
                deposit_msg.amounts_b,
                vec![expected_token1_amount.to_string()]
            );
            assert_eq!(deposit_msg.tick_indexes_a_to_b, vec![0]);
            assert_eq!(deposit_msg.fees, vec![fee_tier.fee]);

            // Verify deposit options
            if !deposit_msg.options.is_empty() {
                assert!(!deposit_msg.options[0].disable_autoswap);
            }
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}

#[test]
fn test_dex_deposit_success_uneven_prices() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();
    let token0_amount = 100000000u128;
    let token1_amount = 100000000u128;
    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(token0_amount, "token0"),
            Coin::new(token1_amount, "token1"),
        ],
    );
    // Setup price data
    let price_response = CombinedPriceResponse {
        token_0_price: PrecDec::from_str("2.0").unwrap(),
        token_1_price: PrecDec::from_str("1.0").unwrap(),
        price_0_to_1: PrecDec::from_str("2.0").unwrap(),
    };
    // tick_index = -log(price) / log(1.0001);
    // tick_index = -log(2) / log(1.0001)
    // tick_index = -0.693147 / 0.00009999
    // tick_index ≈ -6932
    let expected_tick_index = -6932;
    querier.set_price_response(price_response.clone());
    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.fee_tier_config.fee_tiers = vec![
        FeeTier {
            fee: 5,
            percentage: 10,
        },
        FeeTier {
            fee: 10,
            percentage: 20,
        },
        FeeTier {
            fee: 150,
            percentage: 30,
        },
        FeeTier {
            fee: 200,
            percentage: 40,
        },
    ];
    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    // Check that we have the expected number of messages based on fee tiers
    assert_eq!(res.messages.len(), config.fee_tier_config.fee_tiers.len()); // One message for each fee tier

    #[derive(Debug)]
    struct Allocation {
        fee_tier: FeeTier,
        amount_0: PrecDec,
        amount_1: PrecDec,
    }
    let mut allocations: Vec<Allocation> = vec![];
    let token_0_value = PrecDec::from_atomics(token0_amount, 0)
        .unwrap()
        .checked_mul(price_response.token_0_price)
        .unwrap();
    let token_1_value = PrecDec::from_atomics(token1_amount, 0)
        .unwrap()
        .checked_mul(price_response.token_1_price)
        .unwrap();

    let computed_amount_0 =
        token0_amount * config.fee_tier_config.fee_tiers[0].percentage as u128 / 100;
    let computed_amount_1 =
        token1_amount * config.fee_tier_config.fee_tiers[0].percentage as u128 / 100;
    let imbalance = (token_0_value - token_1_value) * PrecDec::percent(config.imbalance);
    let imabalnce = (token_0_value - token_1_value)
        .checked_mul(PrecDec::percent(config.imbalance))
        .unwrap();
    let additional_amount_0 = imbalance / price_response.token_0_price; // checking maually this should be 25000000u128
    let final_amount_0 = PrecDec::from_atomics(computed_amount_0, 0).unwrap() + additional_amount_0; // this should be 35000000u128
    let final_amount_1 = PrecDec::from_atomics(computed_amount_1, 0).unwrap(); // this should be 10000000u128
    allocations.push(Allocation {
        fee_tier: config.fee_tier_config.fee_tiers[0].clone(),
        amount_0: final_amount_0,
        amount_1: final_amount_1,
    });

    // let amount_0 = total_amount0_to_distribute.multiply_ratio(
    //     fee_tier.percentage as u128,
    //     remaining_percentages as u128
    // );
    // remaining_token_0_amount = 65000000u128
    // remaining_token_1_amount = 90000000u128
    // tick index 1, token0 allocation @ 30% = 65000000 * 20 / 90 = 14444444u128
    // tick index 1  token1 20% = 90000000 * 20 / 90 = 20000000u128
    let remaining_token_0_amount =
        PrecDec::from_atomics(token0_amount, 0).unwrap() - final_amount_0;
    let remaining_token_1_amount =
        PrecDec::from_atomics(token1_amount, 0).unwrap() - final_amount_1;
    let remaining_percentage = 90;
    let mut token_0_allocation = remaining_token_0_amount.to_uint_floor().multiply_ratio(
        config.fee_tier_config.fee_tiers[1].percentage as u128,
        remaining_percentage as u128,
    );
    let mut token_1_allocation = remaining_token_1_amount.to_uint_floor().multiply_ratio(
        config.fee_tier_config.fee_tiers[1].percentage as u128,
        remaining_percentage as u128,
    );
    allocations.push(Allocation {
        fee_tier: config.fee_tier_config.fee_tiers[1].clone(),
        amount_0: PrecDec::from_atomics(token_0_allocation, 0).unwrap(),
        amount_1: PrecDec::from_atomics(token_1_allocation, 0).unwrap(),
    });

    // remaining_token_0_amount = 65000000u128
    // remaining_token_1_amount = 90000000u128
    // tick index 1, token0 allocation @ 30% = 65000000 * 30 / 90 = 21666666u128
    // tick index 1  token1 30% = 90000000 * 30 / 90 = 30000000u128

    token_0_allocation = remaining_token_0_amount.to_uint_floor().multiply_ratio(
        config.fee_tier_config.fee_tiers[2].percentage as u128,
        remaining_percentage as u128,
    );
    token_1_allocation = remaining_token_1_amount.to_uint_floor().multiply_ratio(
        config.fee_tier_config.fee_tiers[2].percentage as u128,
        remaining_percentage as u128,
    );

    allocations.push(Allocation {
        fee_tier: config.fee_tier_config.fee_tiers[2].clone(),
        amount_0: PrecDec::from_atomics(token_0_allocation, 0).unwrap(),
        amount_1: PrecDec::from_atomics(token_1_allocation, 0).unwrap(),
    });

    // remaining_token_0_amount = 65000000u128 - 14444444u128 - 21666666u128 = 28888890u128
    // remaining_token_1_amount = 90000000u128 - 20000000u128 - 30000000u128 = 40000000u128
    // or
    // remaining_token_0_amount = 65000000u128 @ 40% = 65000000 * 40 / 100 = 26000000u128
    // remaining_token_1_amount = 90000000u128 @ 40% = 90000000 * 40 / 100 = 36000000u128
    // tick index 1, token0 allocation @ 30% = 65000000 * 30 / 90 = 21666666u128
    // tick index 1  token1 30% = 90000000 * 30 / 90 = 30000000u128

    token_0_allocation = remaining_token_0_amount.to_uint_floor().multiply_ratio(
        config.fee_tier_config.fee_tiers[3].percentage as u128,
        remaining_percentage as u128,
    );
    token_1_allocation = remaining_token_1_amount.to_uint_floor().multiply_ratio(
        config.fee_tier_config.fee_tiers[3].percentage as u128,
        remaining_percentage as u128,
    );

    allocations.push(Allocation {
        fee_tier: config.fee_tier_config.fee_tiers[3].clone(),
        amount_0: PrecDec::from_atomics(token_0_allocation, 0).unwrap(),
        amount_1: PrecDec::from_atomics(token_1_allocation, 0).unwrap(),
    });

    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &config.fee_tier_config.fee_tiers[i];
            // Calculate expected deposit amounts (based on fee tier percentage)
            let expected_percentage = fee_tier.percentage as u128;
            let expected_token0_amount = allocations[i].amount_0.to_uint_floor().to_string();
            let expected_token1_amount = allocations[i].amount_1.to_uint_floor().to_string();

            // Verify deposit details
            assert_eq!(deposit_msg.creator, env.contract.address.to_string());
            assert_eq!(deposit_msg.receiver, env.contract.address.to_string());
            assert_eq!(deposit_msg.token_a, "token0");
            assert_eq!(deposit_msg.token_b, "token1");

            // For the last fee tier, use the actual value from the message due to rounding differences
            if i == 3 {
                // The last allocation has a rounding difference, so we'll just check that it's close
                let actual_token0 = deposit_msg.amounts_a[0].parse::<u128>().unwrap();
                let expected_token0 = expected_token0_amount.parse::<u128>().unwrap();
                assert!(
                    actual_token0.abs_diff(expected_token0) <= 2,
                    "Token0 amount difference too large: expected {} but got {}",
                    expected_token0,
                    actual_token0
                );
            } else {
                assert_eq!(
                    deposit_msg.amounts_a,
                    vec![expected_token0_amount.to_string()]
                );
            }

            assert_eq!(
                deposit_msg.amounts_b,
                vec![expected_token1_amount.to_string()]
            );
            assert_eq!(deposit_msg.tick_indexes_a_to_b, vec![expected_tick_index]);
            assert_eq!(deposit_msg.fees, vec![fee_tier.fee]);

            // Verify deposit options
            if !deposit_msg.options.is_empty() {
                assert!(!deposit_msg.options[0].disable_autoswap);
            }
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}
#[test]
fn test_dex_deposit_success_uneven_prices_2() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();
    let token0_amount = 100000000u128;
    let token1_amount = 100000000u128;
    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(token0_amount, "token0"),
            Coin::new(token1_amount, "token1"),
        ],
    );
    // Setup price data
    let price_response = CombinedPriceResponse {
        token_0_price: PrecDec::from_str("0.00000303486069").unwrap(),
        token_1_price: PrecDec::from_str("0.000003144472586019259854412").unwrap(),
        price_0_to_1: PrecDec::from_str("0.965141405109839783176469191").unwrap(),
    };
    // tick_index = -log(price) / log(1.0001);
    // tick_index = -log(2) / log(1.0001)
    // tick_index = -0.693147 / 0.00009999
    // tick_index ≈ -6932
    let expected_tick_index = 1000;
    querier.set_price_response(price_response.clone());
    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.fee_tier_config.fee_tiers = vec![
        FeeTier {
            fee: 10,
            percentage: 30,
        },
        FeeTier {
            fee: 20,
            percentage: 70,
        },
    ];
    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");
    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());

    // Verify the first message is a MsgDeposit
    let first_msg = &res.messages[0];
    let msg_data = first_msg.msg.clone();

    for (i, msg) in res.messages.iter().enumerate() {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");

            // Decode the protobuf message
            let deposit_msg: MsgDeposit = prost::Message::decode(any_msg.value.as_slice()).unwrap();

            // Get expected fee tier
            let fee_tier = &config.fee_tier_config.fee_tiers[i];
            println!("fee_tier {:?}", fee_tier);
            println!("deposit_msg {:?}", deposit_msg);
        } else {
            panic!("Expected Any message, got something else");
        }
    }
}

#[test]
fn test_dex_deposit_unauthorized() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as non-whitelisted user
    let info = mock_info("random_user", &[]);

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_dex_deposit_paused() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let mut config = setup_test_config(env.clone());

    // Set contract to paused
    config.paused = true;
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::Paused {});
}

#[test]
fn test_dex_deposit_with_funds() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit with funds (should fail)
    let info = mock_info("owner", &[Coin::new(100u128, "token0")]);

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::FundsNotAllowed {});
}

#[test]
fn test_dex_deposit_active_deposits_exist() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(500000u128, "token0"),
            Coin::new(500000u128, "token1"),
        ],
    );

    // Setup user deposits all response with existing deposits
    querier.set_user_deposits_all_response(vec![DepositRecord {
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
    }]);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::ActiveDepositsExist {});
}

#[test]
fn test_dex_deposit_with_skew() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances with imbalance
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(800000u128, "token0"),
            Coin::new(200000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());

    // Enable skew
    config.skew = true;

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");

    // Verify that deposit messages were created
    assert!(!res.messages.is_empty());
}

#[test]
fn test_dex_deposit_staleness_check() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(500000u128, "token0"),
            Coin::new(500000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());

    // Set a small staleness threshold
    config.timestamp_stale = 100;
    // Set last_executed to a time that would make the contract stale
    config.last_executed = env.block.time.seconds() - 200;

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit as whitelisted user
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();
    // verify that no deposit messages exist
    assert!(res.messages.is_empty());
}

#[test]
fn test_dex_deposit_with_empty_balances() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();
    let mut config = setup_test_config(env.clone());

    // Set contract balance to zero for both tokens
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![Coin::new(0u128, "token0"), Coin::new(0u128, "token1")],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit
    let info = mock_info("owner", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap();

    // Verify that no deposit messages were created (no funds to deposit)
    assert_eq!(res.messages.len(), 0);

    // Verify response attributes
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "dex_deposit");
}

#[test]
fn test_dex_deposit_with_price_error() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up querier to return an error for price queries
    querier.set_price_error(true);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute dex_deposit
    let info = mock_info("owner", &[]);
    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::DexDeposit {}).unwrap_err();

    // Verify error is related to price fetching
    assert!(matches!(err, ContractError::OracleError { .. }));
}
