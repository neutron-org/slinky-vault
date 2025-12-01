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

// Struct to represent a deposit scenario
struct DepositScenario {
    user: String,
    token0_amount: u128,
    token1_amount: u128,
    token0_price: String,
    token1_price: String,
    expected_shares: Option<u128>, // Optional for validation
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
        skew: 0i32,
        imbalance: 50u32,
        oracle_price_skew: 0i32,
        dynamic_spread_factor: 0i32,
        dynamic_spread_cap: 0i32,
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

// Helper function to calculate expected shares based on token amounts and prices
fn calculate_expected_shares(
    token0_amount: u128,
    token1_amount: u128,
    token0_price: &str,
    token1_price: &str,
) -> Uint256 {
    let price_0 = PrecDec::from_str(token0_price).unwrap();
    let price_1 = PrecDec::from_str(token1_price).unwrap();

    // Calculate total value in USD
    let token0_value = price_0
        .checked_mul(PrecDec::from_str(&token0_amount.to_string()).unwrap())
        .unwrap();
    let token1_value = price_1
        .checked_mul(PrecDec::from_str(&token1_amount.to_string()).unwrap())
        .unwrap();
    let total_value = token0_value.checked_add(token1_value).unwrap();

    // Convert to shares
    total_value
        .checked_mul(PrecDec::from_str(&SHARES_MULTIPLIER.to_string()).unwrap())
        .unwrap()
        .to_uint_floor()
}

// Function to execute a sequence of deposits with changing prices
fn execute_deposit_sequence(scenarios: Vec<DepositScenario>) {
    let env = mock_env();
    let mut config = setup_test_config(env.clone());

    // Set a high deposit cap to avoid cap issues
    config.deposit_cap = Uint128::new(u128::MAX / 2);

    // Initialize with first scenario's price
    let first = &scenarios[0];
    let mut querier = setup_mock_querier_with_price(&first.token0_price, &first.token1_price);

    // Set initial contract balance to 0
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![Coin::new(0u128, "token0"), Coin::new(0u128, "token1")],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    let mut total_token0 = 0u128;
    let mut total_token1 = 0u128;
    let mut previous_shares = Uint128::zero();

    // Execute each deposit scenario
    for (i, scenario) in scenarios.iter().enumerate() {
        println!(
            "Executing scenario {}: User {}, token0: {}, token1: {}, price0: {}, price1: {}",
            i + 1,
            scenario.user,
            scenario.token0_amount,
            scenario.token1_amount,
            scenario.token0_price,
            scenario.token1_price
        );

        // Update running totals
        total_token0 += scenario.token0_amount;
        total_token1 += scenario.token1_amount;

        // Create a new querier with updated price data
        let mut updated_querier =
            setup_mock_querier_with_price(&scenario.token0_price, &scenario.token1_price);

        // Set the contract balance to reflect cumulative deposits
        updated_querier.set_contract_balance(
            env.contract.address.as_ref(),
            vec![
                Coin::new(total_token0, "token0"),
                Coin::new(total_token1, "token1"),
            ],
        );

        // Add debug output to verify price data
        let price_response = updated_querier.get_price_response();

        // Update the deps with the new querier
        deps.querier = updated_querier;

        // Execute deposit
        let mut funds = vec![];
        if scenario.token0_amount > 0 {
            funds.push(Coin::new(scenario.token0_amount, "token0"));
        }
        if scenario.token1_amount > 0 {
            funds.push(Coin::new(scenario.token1_amount, "token1"));
        }
        let info = mock_info(&scenario.user, &funds);

        let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap();

        // Extract minted amount
        let minted = res
            .attributes
            .iter()
            .find(|attr| attr.key == "minted_amount")
            .map(|attr| Uint128::from_str(&attr.value).unwrap())
            .unwrap();

        // Get updated total shares
        let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
        let current_shares = updated_config.total_shares;

        println!("Minted shares: {}", minted);
        println!("Total shares after deposit: {}", current_shares);

        // Validate expected shares if provided
        if let Some(expected) = scenario.expected_shares {
            assert_eq!(
                minted,
                Uint128::from(expected),
                "Scenario {}: Expected {} shares but got {}",
                i + 1,
                expected,
                minted
            );
        } else {
            // If no expected shares provided, calculate them based on the formula
            let calculated_shares = calculate_expected_shares(
                scenario.token0_amount,
                scenario.token1_amount,
                &scenario.token0_price,
                &scenario.token1_price,
            );
            println!("Calculated shares: {}", calculated_shares);
        }

        // Validate that total shares increased by the minted amount
        assert_eq!(
            current_shares,
            previous_shares + minted,
            "Total shares should increase by exactly the minted amount"
        );

        previous_shares = current_shares;
    }
}

#[test]
fn test_sequential_deposits_equal_price() {
    // Test with equal prices that remain constant
    let scenarios = vec![
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 1000000,
            token1_amount: 1000000,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(2000000 * SHARES_MULTIPLIER as u128),
        },
        DepositScenario {
            user: "user2".to_string(),
            token0_amount: 500000,
            token1_amount: 500000,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(1000000 * SHARES_MULTIPLIER as u128),
        },
        DepositScenario {
            user: "user3".to_string(),
            token0_amount: 250000,
            token1_amount: 250000,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(500000 * SHARES_MULTIPLIER as u128),
        },
    ];

    execute_deposit_sequence(scenarios);
}

#[test]
fn test_sequential_deposits_changing_price() {
    // Test with prices that change between deposits
    let scenarios = vec![
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 1000000,
            token1_amount: 1000000,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(2000000 * SHARES_MULTIPLIER as u128),
        },
        // since we already deposited 1000000 token0 and 1000000 token1, the contract value should increase when we bump the price.
        // in this case we;re doubling the contract value so we should get the same shares as above
        DepositScenario {
            user: "user2".to_string(),
            token0_amount: 1000000,
            token1_amount: 1000000,
            token0_price: "2.0".to_string(), // token0 price doubles
            token1_price: "1.0".to_string(),
            expected_shares: Some(2000000 * SHARES_MULTIPLIER as u128),
        },
        // same as above, we hsould receive the same shares as the previous 2 deposits
        DepositScenario {
            user: "user3".to_string(),
            token0_amount: 1000000,
            token1_amount: 1000000,
            token0_price: "1.0".to_string(), // token0 price returns to original
            token1_price: "2.0".to_string(), // token1 price doubles
            expected_shares: Some(2000000 * SHARES_MULTIPLIER as u128),
        },
    ];

    execute_deposit_sequence(scenarios);
}

#[test]
fn test_sequential_deposits_imbalanced() {
    // Test with imbalanced token amounts
    let scenarios = vec![
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 1000000,
            token1_amount: 1000000,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(2000000 * SHARES_MULTIPLIER as u128),
        },
        DepositScenario {
            user: "user2".to_string(),
            token0_amount: 2000000,
            token1_amount: 500000,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(2500000 * SHARES_MULTIPLIER as u128),
        },
        DepositScenario {
            user: "user3".to_string(),
            token0_amount: 100000,
            token1_amount: 900000,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(1000000 * SHARES_MULTIPLIER as u128),
        },
    ];

    execute_deposit_sequence(scenarios);
}

#[test]
fn test_sequential_deposits_price_volatility() {
    // as long as we deposit the same amounts, the token price should not impact shares minted
    let token0_amount = 100e6 as f64;
    let token1_amount = 100e6 as f64;
    let initial_token0_price = 0.0000001 as f64;
    let initial_token1_price = 0.00001 as f64;
    let expected_shares = (((token0_amount * initial_token0_price)
        + (token1_amount * initial_token1_price))
        * SHARES_MULTIPLIER as f64) as u128;

    // Test with high price volatility
    let scenarios = vec![
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: token0_amount as u128,
            token1_amount: token1_amount as u128,
            token0_price: initial_token0_price.to_string(),
            token1_price: initial_token1_price.to_string(),
            expected_shares: Some(expected_shares),
        },
        DepositScenario {
            user: "user2".to_string(),
            token0_amount: token0_amount as u128,
            token1_amount: token1_amount as u128,
            token0_price: "50.0".to_string(), // Significant price increase
            token1_price: "0.05".to_string(), // Price decrease
            expected_shares: Some(expected_shares), // 5.0*1000000 + 0.5*1000000 = 5500000
        },
        DepositScenario {
            user: "user3".to_string(),
            token0_amount: token0_amount as u128,
            token1_amount: token1_amount as u128,
            token0_price: "0.1".to_string(), // Significant price decrease
            token1_price: "10.0".to_string(), // Significant price increase
            expected_shares: Some(expected_shares), // 0.1*1000000 + 10.0*1000000 = 10100000
        },
        DepositScenario {
            user: "user4".to_string(),
            token0_amount: token0_amount as u128,
            token1_amount: token1_amount as u128,
            token0_price: "0.0000001".to_string(), // Significant price decrease
            token1_price: "100.0".to_string(),     // Significant price increase
            expected_shares: Some(expected_shares), // 0.1*1000000 + 10.0*1000000 = 10100000
        },
    ];

    execute_deposit_sequence(scenarios);
}

// This function could be used for fuzzing in the future
#[allow(dead_code)]
fn generate_random_scenario(
    user: String,
    token0_amount_range: (u128, u128),
    token1_amount_range: (u128, u128),
    token0_price_range: (f64, f64),
    token1_price_range: (f64, f64),
) -> DepositScenario {
    // In a real fuzzing implementation, you would generate random values within the ranges
    // For now, we'll just use the minimum values
    DepositScenario {
        user,
        token0_amount: token0_amount_range.0,
        token1_amount: token1_amount_range.0,
        token0_price: token0_price_range.0.to_string(),
        token1_price: token1_price_range.0.to_string(),
        expected_shares: None,
    }
}

#[test]
fn test_sequential_deposits_small_initial() {
    // Test with imbalanced token amounts
    let scenarios = vec![
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 1,
            token1_amount: 0,
            token0_price: "0.0000001".to_string(),
            token1_price: "0.000000001".to_string(),
            expected_shares: Some(100 as u128),
        },
        // should mint 10x more than the prevous iteration
        DepositScenario {
            user: "user2".to_string(),
            token0_amount: 10,
            token1_amount: 0,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(1000),
        },
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 1,
            token1_amount: 0,
            token0_price: "0.0000001".to_string(),
            token1_price: "0.000000001".to_string(),
            expected_shares: Some(100 as u128),
        },
    ];

    execute_deposit_sequence(scenarios);
}
#[test]
fn test_sequential_deposits_large_initial() {
    // Test with imbalanced token amounts
    let scenarios = vec![
        // deposit wirth 1,000,000,000,000
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 1000000000000,
            token1_amount: 0,
            token0_price: "1".to_string(),
            token1_price: "1".to_string(),
            expected_shares: Some(1000000000000 * SHARES_MULTIPLIER as u128),
        },
        // should mint 10x more than the prevous iteration
        DepositScenario {
            user: "user2".to_string(),
            token0_amount: 100000000,
            token1_amount: 0,
            token0_price: "1.0".to_string(),
            token1_price: "1.0".to_string(),
            expected_shares: Some(100000000 * SHARES_MULTIPLIER as u128),
        },
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 1,
            token1_amount: 0,
            token0_price: "1".to_string(),
            token1_price: "1".to_string(),
            expected_shares: Some(1 * SHARES_MULTIPLIER as u128),
        },
    ];

    execute_deposit_sequence(scenarios);
}

#[test]
fn test_sequential_deposits_multiple_rounding() {
    // Test with imbalanced token amounts
    let scenarios = vec![
        // correctly returns rounded-down mint amount
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 50,
            token1_amount: 50,
            token0_price: "0.999999999999999".to_string(),
            token1_price: "0.999999999999999".to_string(),
            expected_shares: Some(99999999999),
        },
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 1000,
            token1_amount: 0,
            token0_price: "0.999999999999999".to_string(),
            token1_price: "0.999999999999999".to_string(),
            expected_shares: Some(999999999990),
        },
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 5000,
            token1_amount: 5000,
            token0_price: "0.999999999999999".to_string(),
            token1_price: "0.999999999999999".to_string(),
            expected_shares: Some(9999999999900),
        },
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 50000,
            token1_amount: 50000,
            token0_price: "0.999999999999999".to_string(),
            token1_price: "0.999999999999999".to_string(),
            expected_shares: Some(99999999999000),
        },
    ];

    execute_deposit_sequence(scenarios);
}

#[test]
fn test_sequential_deposits_multiple_rounding_small_shares() {
    // Test with imbalanced token amounts
    let scenarios = vec![
        // correctly returns rounded-down mint amount
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 100,
            token1_amount: 0,
            token0_price: "0.00000000001".to_string(),
            token1_price: "0.999999999999999".to_string(),
            expected_shares: Some(1),
        },
        DepositScenario {
            user: "user1".to_string(),
            token0_amount: 199,
            token1_amount: 0,
            token0_price: "0.00000000001".to_string(),
            token1_price: "0.999999999999999".to_string(),
            expected_shares: Some(1),
        },
    ];

    execute_deposit_sequence(scenarios);
}
