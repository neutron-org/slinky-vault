use crate::contract::execute;
use crate::error::ContractError;
use crate::msg::{CombinedPriceResponse, ExecuteMsg};
use crate::state::{
    Config, FeeTier, FeeTierConfig, PairData, TokenData, CONFIG, SHARES_MULTIPLIER,
};
use crate::testing::mock_querier::{mock_dependencies_with_custom_querier, MockQuerier};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{Addr, Coin, Env, Uint128, Uint256};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use std::str::FromStr;

// Struct to represent a withdrawal scenario
struct WithdrawScenario {
    user: String,
    withdraw_amount: u128,
    token0_price: String,
    token1_price: String,
    expected_token0: Option<u128>, // Optional for validation
    expected_token1: Option<u128>, // Optional for validation
}

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

// Helper function to setup mock querier with specific price data
fn setup_mock_querier_with_price(token0_price: &str, token1_price: &str) -> MockQuerier {
    let mut querier = MockQuerier::default();

    // Calculate price ratio
    let price_0 = PrecDec::from_str(token0_price).unwrap();
    let price_1 = PrecDec::from_str(token1_price).unwrap();
    let price_ratio = if price_1.is_zero() {
        PrecDec::from_str("0.0").unwrap()
    } else {
        price_0.checked_div(price_1).unwrap()
    };

    // Setup price data
    let price_response = CombinedPriceResponse {
        token_0_price: price_0,
        token_1_price: price_1,
        price_0_to_1: price_ratio,
    };
    querier.set_price_response(price_response);

    // Setup empty deposits response
    querier.set_empty_deposits();
    querier.set_user_deposits_all_response(vec![]);

    querier
}

// Function to execute a sequence of withdrawals with changing prices
fn execute_withdraw_sequence(
    scenarios: Vec<WithdrawScenario>,
    initial_token0: u128,
    initial_token1: u128,
    initial_shares: u128,
) {
    let env = mock_env();
    let mut config = setup_test_config(env.clone());

    // Set initial total shares based on initial token amounts and a 1:1 price
    config.total_shares = Uint128::new(initial_shares);

    // Initialize with first scenario's price
    let first = &scenarios[0];
    let mut querier = setup_mock_querier_with_price(&first.token0_price, &first.token1_price);

    // Set initial contract balance
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(initial_token0, "token0"),
            Coin::new(initial_token1, "token1"),
            Coin::new(initial_shares, "factory/contract/lp"),
        ],
    );

    // Set LP token supply
    querier.set_supply("factory/contract/lp", initial_shares);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    let mut remaining_token0 = initial_token0;
    let mut remaining_token1 = initial_token1;
    let mut remaining_shares = initial_shares;

    // Execute each withdrawal scenario
    for (i, scenario) in scenarios.iter().enumerate() {
        println!(
            "Executing scenario {}: User {}, withdraw amount: {}, price0: {}, price1: {}",
            i + 1,
            scenario.user,
            scenario.withdraw_amount,
            scenario.token0_price,
            scenario.token1_price
        );

        // Create a new querier with updated price data
        let mut updated_querier =
            setup_mock_querier_with_price(&scenario.token0_price, &scenario.token1_price);

        // Set the contract balance to reflect remaining tokens
        updated_querier.set_contract_balance(
            env.contract.address.as_ref(),
            vec![
                Coin::new(remaining_token0, "token0"),
                Coin::new(remaining_token1, "token1"),
                Coin::new(remaining_shares, "factory/contract/lp"),
            ],
        );

        // Set LP token supply
        updated_querier.set_supply("factory/contract/lp", remaining_shares);

        // Update deps with new querier
        deps = mock_dependencies_with_custom_querier(updated_querier);

        // Update config with remaining shares
        let mut updated_config = config.clone();
        updated_config.total_shares = Uint128::new(remaining_shares);
        CONFIG.save(deps.as_mut().storage, &updated_config).unwrap();

        // Execute withdrawal
        let info = mock_info(
            &scenario.user,
            &[Coin::new(scenario.withdraw_amount, "factory/contract/lp")],
        );

        let res = execute(
            deps.as_mut(),
            env.clone(),
            info,
            ExecuteMsg::Withdraw {
                amount: Uint128::new(scenario.withdraw_amount),
            },
        );

        // Verify the result
        match res {
            Ok(response) => {
                println!("Withdrawal successful");

                // Validate expected token amounts if provided
                if let Some(expected_token0) = scenario.expected_token0 {
                    assert_eq!(
                        expected_token0, expected_token0,
                        "Expected token0 withdrawal amount doesn't match actual"
                    );
                }

                if let Some(expected_token1) = scenario.expected_token1 {
                    assert_eq!(
                        expected_token1, expected_token1,
                        "Expected token1 withdrawal amount doesn't match actual"
                    );
                }

                // Verify messages in response
                assert!(!response.messages.is_empty(), "No messages in response");

                // First message should be burn LP tokens
                match &response.messages[0].msg {
                    cosmwasm_std::CosmosMsg::Any(any_msg) => {
                        assert_eq!(
                            any_msg.type_url, "/osmosis.tokenfactory.v1beta1.MsgBurn",
                            "First message should be MsgBurn"
                        );
                    }
                    _ => panic!("Expected Any message with MsgBurn type_url"),
                }

                // Check for token transfer messages
                let mut found_token0_transfer = false;
                let mut found_token1_transfer = false;

                for msg in &response.messages {
                    if let cosmwasm_std::CosmosMsg::Bank(bank_msg) = &msg.msg {
                        if let cosmwasm_std::BankMsg::Send { to_address, amount } = bank_msg {
                            if to_address == &scenario.user {
                                for coin in amount {
                                    if coin.denom == "token0" {
                                        found_token0_transfer = true;
                                        if let Some(expected) = scenario.expected_token0 {
                                            println!("expected: {}", expected);
                                            println!("coin.amount: {}", coin.amount);
                                            assert_eq!(
                                                coin.amount,
                                                Uint128::new(expected),
                                                "Token0 transfer amount doesn't match expected"
                                            );
                                        }
                                    } else if coin.denom == "token1" {
                                        found_token1_transfer = true;
                                        if let Some(expected) = scenario.expected_token1 {
                                            println!("expected: {}", expected);
                                            println!("coin.amount: {}", coin.amount);
                                            assert_eq!(
                                                coin.amount,
                                                Uint128::new(expected),
                                                "Token1 transfer amount doesn't match expected"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Verify transfers were found if tokens were expected
                if scenario.expected_token0.is_some() && scenario.expected_token0.unwrap() > 0 {
                    assert!(
                        found_token0_transfer,
                        "No token0 transfer found in messages"
                    );
                }

                if scenario.expected_token1.is_some() && scenario.expected_token1.unwrap() > 0 {
                    assert!(
                        found_token1_transfer,
                        "No token1 transfer found in messages"
                    );
                }
            }
            Err(e) => {
                panic!("Withdrawal failed: {:?}", e);
            }
        }

        // Update remaining amounts
        remaining_token0 -= scenario.expected_token0.unwrap();
        remaining_token1 -= scenario.expected_token1.unwrap();
        remaining_shares -= scenario.withdraw_amount;
        // Update config for next iteration
        config = updated_config;
    }
}

#[test]
fn test_sequential_withdrawals_equal_prices() {
    // Initial token amounts
    let initial_token0 = 1000000u128;
    let initial_token1 = 1000000u128;
    let initial_shares = 2000000000000000u128;
    // Define withdrawal scenarios
    let scenarios = vec![
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 500000000000000u128, // 25% of total shares
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(250000u128), // 25% of token0
            expected_token1: Some(250000u128), // 25% of token1
        },
        // remaining amount of shares is 1500000000000000
        // remaining amount of token0 is 750000
        // remaining amount of token1 is 750000
        // 33% of remaining shares is 500000000000000
        // 33% of remaining token0 is 250000
        // 33% of remaining token1 is 250000
        WithdrawScenario {
            user: "user2".to_string(),
            withdraw_amount: 500000000000000u128, // 33% of remaining shares
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(250000u128), // 33% of remaining token0
            expected_token1: Some(250000u128), // 33% of remaining token1
        },
        // remaining amount of shares is 1000000000000000
        // remaining amount of token0 is 500000
        // remaining amount of token1 is 500000
        // 50% of remaining shares is 500000000000000
        // 50% of remaining token0 is 250000
        // 50% of remaining token1 is 250000
        WithdrawScenario {
            user: "user2".to_string(),
            withdraw_amount: 500000000000000u128, // 50% of remaining shares
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(250000u128), // 50% of remaining token0
            expected_token1: Some(250000u128), // 50% of remaining token1
        },
        // remaining amount of shares is 500000000000000
        // remaining amount of token0 is 250000
        // remaining amount of token1 is 250000
        // 100% of remaining shares is 500000000000000
        // 100% of remaining token0 is 250000
        // 100% of remaining token1 is 250000
        WithdrawScenario {
            user: "user2".to_string(),
            withdraw_amount: 500000000000000u128, // 100% of remaining shares
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(250000u128), // 100% of remaining token0
            expected_token1: Some(250000u128), // 100% of remaining token1
        },
    ];

    execute_withdraw_sequence(scenarios, initial_token0, initial_token1, initial_shares);
}

#[test]
fn test_sequential_withdrawals_changing_prices() {
    // even though prices are changing. the total percentage of tokens returned should be the same
    let initial_token0 = 1000000u128;
    let initial_token1 = 1000000u128;
    let initial_shares = 2000000000000000u128;
    // Define withdrawal scenarios
    let scenarios = vec![
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 500000000000000u128, // 25% of total shares
            token0_price: "0.12345".to_string(),
            token1_price: "123.0".to_string(),
            expected_token0: Some(250000u128), // 25% of token0
            expected_token1: Some(250000u128), // 25% of token1
        },
        // remaining amount of shares is 1500000000000000
        // remaining amount of token0 is 750000
        // remaining amount of token1 is 750000
        // 33% of remaining shares is 500000000000000
        // 33% of remaining token0 is 250000
        // 33% of remaining token1 is 250000
        WithdrawScenario {
            user: "user2".to_string(),
            withdraw_amount: 500000000000000u128, // 33% of remaining shares
            token0_price: "11122.0".to_string(),
            token1_price: "12.0".to_string(),
            expected_token0: Some(250000u128), // 33% of remaining token0
            expected_token1: Some(250000u128), // 33% of remaining token1
        },
        // remaining amount of shares is 1000000000000000
        // remaining amount of token0 is 500000
        // remaining amount of token1 is 500000
        // 50% of remaining shares is 500000000000000
        // 50% of remaining token0 is 250000
        // 50% of remaining token1 is 250000
        WithdrawScenario {
            user: "user2".to_string(),
            withdraw_amount: 500000000000000u128, // 50% of remaining shares
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(250000u128), // 50% of remaining token0
            expected_token1: Some(250000u128), // 50% of remaining token1
        },
        // remaining amount of shares is 500000000000000
        // remaining amount of token0 is 250000
        // remaining amount of token1 is 250000
        // 100% of remaining shares is 500000000000000
        // 100% of remaining token0 is 250000
        // 100% of remaining token1 is 250000
        WithdrawScenario {
            user: "user2".to_string(),
            withdraw_amount: 500000000000000u128, // 100% of remaining shares
            token0_price: "9.9".to_string(),
            token1_price: "4.11".to_string(),
            expected_token0: Some(250000u128), // 100% of remaining token0
            expected_token1: Some(250000u128), // 100% of remaining token1
        },
    ];

    execute_withdraw_sequence(scenarios, initial_token0, initial_token1, initial_shares);
}

#[test]
fn test_sequential_withdrawals_extreme_prices() {
    // Initial token amounts
    let initial_shares = 1000000u128;
    let initial_token0 = 1000000u128;
    let initial_token1 = 1000000u128;

    // Define withdrawal scenarios with extreme price changes
    let scenarios = vec![
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 250000u128, // 25% of total shares
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(250000u128), // 25% of token0
            expected_token1: Some(250000u128), // 25% of token1
        },
        WithdrawScenario {
            user: "user2".to_string(),
            withdraw_amount: 250000u128,        // 33% of remaining shares
            token0_price: "1000.0".to_string(), // Extreme price increase
            token1_price: "0.00000000001".to_string(), // Extreme price decrease
            expected_token0: Some(250000u128),  // Still 33% of remaining token0
            expected_token1: Some(250000u128),  // Still 33% of remaining token1
        },
        WithdrawScenario {
            user: "user3".to_string(),
            withdraw_amount: 500000u128, // 100% of remaining shares
            token0_price: "0.0000000001".to_string(), // Extreme price decrease
            token1_price: "1000000.0".to_string(), // Extreme price increase
            expected_token0: Some(500000u128), // 100% of remaining token0
            expected_token1: Some(500000u128), // 100% of remaining token1
        },
    ];

    execute_withdraw_sequence(scenarios, initial_token0, initial_token1, initial_shares);
}

#[test]
fn test_sequential_withdrawals_rounding() {
    // Initial token amounts - imbalanced
    let initial_token0 = 1;
    let initial_token1 = 1;
    let initial_shares = 1000000;
    // Define withdrawal scenarios
    let scenarios = vec![
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 999999, // will not return the tokens
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(0),
            expected_token1: Some(0),
        },
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 1, // one share remaining should return the token
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(1),
            expected_token1: Some(1),
        },
    ];

    execute_withdraw_sequence(scenarios, initial_token0, initial_token1, initial_shares);
}

#[test]
fn test_sequential_withdrawals_multiple_rounding() {
    // Initial token amounts - imbalanced
    let initial_token0 = 10;
    let initial_token1 = 10;
    let initial_shares = 1000000;
    // Define withdrawal scenarios
    let scenarios = vec![
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 199999, // will return 1 token
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(1),
            expected_token1: Some(1),
        },
        // remaining shares is 800001
        // remaining tokens are 9
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 80000, // will return 0 token
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(0),
            expected_token1: Some(0),
        },
        // remaining shares is 720001
        // remaining tokens are 9
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 80000, // will return 0 tokens
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(0),
            expected_token1: Some(0),
        },
        // remaining shares is 640001
        // remaining tokens are 9
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 60000, // will return 0 tokens
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(0),
            expected_token1: Some(0),
        },
        // remaining shares is 580001
        // remaining tokens are 9
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 50000, // will return 0 tokens
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(0),
            expected_token1: Some(0),
        },
        // remaining shares is 580001 - 50000 = 530001
        // remaining tokens are 9
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 48000, // will return 0 tokens
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(0),
            expected_token1: Some(0),
        },
        // remaining shares is 530001 - 48000 = 482001
        // remaining tokens are 9
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 482000, // will return 8 token, 1 share remainder
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(8),
            expected_token1: Some(8),
        },
        // remaining shares is 530001 - 48000 = 482001
        // remaining tokens are 9
        WithdrawScenario {
            user: "user1".to_string(),
            withdraw_amount: 1, // single share remainder will return 1 token
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_token0: Some(1), // 25% of token0
            expected_token1: Some(1), // 25% of token1
        },
    ];

    execute_withdraw_sequence(scenarios, initial_token0, initial_token1, initial_shares);
}
