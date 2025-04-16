use std::str::FromStr;

use crate::msg::{CombinedPriceResponse, DepositResult};
use test_case::test_case;

use crate::utils::{get_deposit_data, get_deposit_messages, price_to_tick_index};
use cosmwasm_std::Uint128;
use neutron_std::types::neutron::util::precdec::PrecDec;

// (total_available_0, total_available_1, tick_index, fee, token_0_price, token_1_price, price_0_to_1, base_deposit_percentage, expected_result)
// imbalance = 1900000 - 950000 / 2 = 475000 -> total = 50000 t0 , (100000 + 475000) t1
#[test_case(1000000, 2000000, 0, 0, "1", "1", "1", 5, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(50000), amount1: Uint128::new(600000), tick_index: 0, fee: 0 }; "imbalance case")]
#[test_case(1000000, 2000000, 0, 0, "1", "1", "1", 0, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(500000), tick_index: 0, fee: 0 }; "0% base deposit")]
#[test_case(1000000, 1000000,  0, 0, "1", "1", "1", 50, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: 0, fee: 0 }; "balanced case")]
#[test_case(1000000, 1000000, 0, 0, "2", "1", "2", 50, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(750000), amount1: Uint128::new(500000), tick_index: 0, fee: 0 }; "unequal token prices")]
#[test_case(1000000, 1000000, 0, 0, "1", "2", "0.5", 50, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(750000), tick_index: 0, fee: 0 }; "inverse unequal token prices")]
#[test_case(1000000, 1000000,  0, 0, "1", "1", "1", 100, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(1000000), amount1: Uint128::new(1000000), tick_index: 0, fee: 0 }; "100% deposit")]
#[test_case(0, 1000000, 0, 0, "1", "1", "1", 5, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(550000), tick_index: 0, fee: 0 }; "one token unavailable")]
#[test_case(0, 0, 0, 0, "1", "1", "1", 5, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(0), tick_index: 0, fee: 0 }; "both tokens unavailable")]
// value 0 = 1000000
// value 1 = 1100000
// imbalance = 1100000 - 1000000 / 2 = 50000
// additional token 1 = 50000 / 1.1 = 45454.54 -> 45454
#[test_case(1000000, 1000000, 0, 0, "1", "1.1", "1", 0, false, 50u32, 0i32  => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(45454), tick_index: 0, fee: 0 }; "slight price difference")]
// computed_amount_0 = 1000000 * 0.05 = 50000
// computed_amount_1 = 1000000 * 0.05 = 50000
// value 0 = 1000000 - 50000 = 950000 * 1 = 950000
// value 1 = 1000000 - 50000 = 950000 * 1.1 = 1045000
// imbalance = 1045000 - 950000 / 2 = 47500
// additional token 1 = 47500 / 1.1 = 43181.81 -> 43181
// total 0 = 50000
// total 1 = 50000 + 43181 = 93181
#[test_case(1000000, 1000000, 0, 0, "1", "1.1", "1", 5, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(50000), amount1: Uint128::new(95454), tick_index: 0, fee: 0 }; "slight price difference with 5% deposit")]
#[test_case(1000000, 1000000, 0, 0, "1", "1", "1", 100, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(1000000), amount1: Uint128::new(1000000), tick_index: 0, fee: 0 }; "capped deposit amounts")]
// computed_amount_0 = 1000000 * 0.1 = 100000
// computed_amount_1 = 1000000 * 0.1 = 100000
// value 0 = 1000000 - 100000 = 900000 * 1 = 900000
// value 1 = 1000000 - 100000 = 900000 * 200 = 180000000
// imbalance = 180000000 - 900000 / 2  = 89550000
// additional token 1 = 89550000 / 200 = 447750
// total 0 = 100000
// total 1 = 100000 + 447750 = 547750
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 0, fee: 0 }; "large price difference")]
#[test_case(0, 1000000, 0, 10, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(600000), tick_index: -9, fee: 10 }; "large price difference with skew token0")]
#[test_case(1000000, 0, 0, 10, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(600000), amount1: Uint128::new(0), tick_index: 9, fee: 10 }; "large price difference with skew token1")]
#[test_case(10000000, 0, 0, 100, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(6000000), amount1: Uint128::new(0), tick_index: 99, fee: 100 }; "test skew sequence -1")]
#[test_case(9000000, 1000000, 0, 100, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(4900000), amount1: Uint128::new(100000), tick_index: 79, fee: 100 }; "test skew sequence -2")]
#[test_case(8000000, 2000000, 0, 100, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(3800000), amount1: Uint128::new(200000), tick_index: 59, fee: 100 }; "test skew sequence -3")]
#[test_case(7000000, 3000000, 0, 100, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(2700000), amount1: Uint128::new(300000), tick_index: 40, fee: 100 }; "test skew sequence -4")]
#[test_case(6000000, 4000000, 0, 100, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(1600000), amount1: Uint128::new(400000), tick_index: 20, fee: 100 }; "test skew sequence -5")]
#[test_case(5000000, 5000000, 0, 100, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: 0, fee: 100 }; "test skew sequence -6")]
#[test_case(10000000, 0, 0, 100, "1", "1", "1", 10, true, 50u32, 0i32 => DepositResult { amount0: Uint128::new(6000000), amount1: Uint128::new(0), tick_index: 99, fee: 100 }; "test double skew sequence -1")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, 1i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 1, fee: 0 }; "test oracle skew -1")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, 11i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 11, fee: 0 }; "test oracle skew -2")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, 111i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 111, fee: 0 }; "test oracle skew -3")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, 99999i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 99999, fee: 0 }; "test oracle skew -4")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, -1i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: -1, fee: 0 }; "test oracle skew -5")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, -11i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: -11, fee: 0 }; "test oracle skew -6")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, -111i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: -111, fee: 0 }; "test oracle skew -7")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, false, 50u32, -99999i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: -99999, fee: 0 }; "test oracle skew -8")]
#[test_case(9000000, 1000000, 0, 100, "1", "1", "1", 10, true, 50u32, 1i32 => DepositResult { amount0: Uint128::new(4900000), amount1: Uint128::new(100000), tick_index: 80, fee: 100 }; "test double skew sequence -2")]
#[test_case(8000000, 2000000, 0, 100, "1", "1", "1", 10, true, 50u32, -2i32 => DepositResult { amount0: Uint128::new(3800000), amount1: Uint128::new(200000), tick_index: 57, fee: 100 }; "test double skew sequence -3")]
#[test_case(7000000, 3000000, 0, 100, "1", "1", "1", 10, true, 50u32, 4i32 => DepositResult { amount0: Uint128::new(2700000), amount1: Uint128::new(300000), tick_index: 44, fee: 100 }; "test double skew sequence -4")]
#[test_case(6000000, 4000000, 0, 100, "1", "1", "1", 10, true, 50u32, -8i32 => DepositResult { amount0: Uint128::new(1600000), amount1: Uint128::new(400000), tick_index: 12, fee: 100 }; "test double skew sequence -5")]
#[test_case(5000000, 5000000, 0, 100, "1", "1", "1", 10, true, 50u32, 33i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: 33, fee: 100 }; "test double skew sequence -6")]
#[test_case(5000000, 5000000, 0, 100, "1", "1", "1", 10, true, 50u32, -99i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: -99, fee: 100 }; "test double skew sequence -7")]

fn test_get_deposit_data(
    total_available_0: u128,
    total_available_1: u128,
    tick_index: i64,
    fee: u64,
    token_0_price: &str,
    token_1_price: &str,
    price_0_to_1: &str,
    base_deposit_percentage: u64,
    skew: bool,
    imbalance: u32,
    oracle_price_skew: i32,
) -> DepositResult {
    let prices = CombinedPriceResponse {
        token_0_price: PrecDec::from_str(token_0_price).unwrap(),
        token_1_price: PrecDec::from_str(token_1_price).unwrap(),
        price_0_to_1: PrecDec::from_str(price_0_to_1).unwrap(),
    };

    println!("Input values:");
    println!("  total_available_0: {}", total_available_0);
    println!("  total_available_1: {}", total_available_1);
    println!("  prices: {:?}", prices);
    println!("  base_deposit_percentage: {}", base_deposit_percentage);

    let result = get_deposit_data(
        Uint128::new(total_available_0),
        Uint128::new(total_available_1),
        tick_index,
        fee,
        &prices,
        base_deposit_percentage,
        skew,
        imbalance,
        oracle_price_skew,
    )
    .unwrap();

    println!("Result: {:?}", result);
    result
}

#[test_case(PrecDec::from_str("123456791234567.000000000000000000").unwrap() => -324485; "large positive number with decimals")]
#[test_case(PrecDec::from_str("123456791234567").unwrap() => -324485; "large positive number without decimals")]
#[test_case(PrecDec::from_str("12345").unwrap() => -94215; "medium positive number")]
#[test_case(PrecDec::from_str("11.0").unwrap() => -23980; "small positive number greater than 1")]
#[test_case(PrecDec::from_str("2.0").unwrap() => -6932; "number 2")]
#[test_case(PrecDec::from_str("1.10").unwrap() => -953; "slightly above 1")]
#[test_case(PrecDec::from_str("1.0").unwrap() => 0; "exactly 1")]
#[test_case(PrecDec::from_str("0.9").unwrap() => 1054; "slightly below 1")]
#[test_case(PrecDec::from_str("0.5").unwrap() => 6932; "0.5")]
#[test_case(PrecDec::from_str("0.1").unwrap() => 23027; "0.1")]
#[test_case(PrecDec::from_str("0.01").unwrap() => 46054; "0.01")]
#[test_case(PrecDec::from_str("0.0011").unwrap() => 68128; "small fraction")]
#[test_case(PrecDec::from_str("0.000123").unwrap() => 90038; "smaller fraction")]
#[test_case(PrecDec::from_str("0.00000009234").unwrap() => 161986; "tiny fraction")]
#[test_case(PrecDec::from_str("0.000000000000123").unwrap() => 297281; "tinier fraction")]
#[test_case(PrecDec::from_str("0.999999999999999999").unwrap() => 0; "slightly below 1 with max precision")]
fn test_price_to_tick_index(price: PrecDec) -> i64 {
    price_to_tick_index(price).unwrap()
}

#[test]
fn test_price_to_tick_index_properties() {
    // Test symmetry around 1.0
    let price_above = PrecDec::from_str("2.0").unwrap();
    let price_below = PrecDec::from_str("0.5").unwrap();

    let tick_above = price_to_tick_index(price_above).unwrap();
    let tick_below = price_to_tick_index(price_below).unwrap();

    assert_eq!(
        tick_above.abs(),
        tick_below.abs(),
        "Tick indices should be symmetric around 1.0"
    );

    // Test monotonicity
    let price1 = PrecDec::from_str("1.1").unwrap();
    let price2 = PrecDec::from_str("1.2").unwrap();

    let tick1 = price_to_tick_index(price1).unwrap();
    let tick2 = price_to_tick_index(price2).unwrap();

    assert!(
        tick1 > tick2,
        "Tick index should decrease as price increases above 1.0"
    );

    // Test precision handling
    let price_precise1 = PrecDec::from_str("0.000000000000000001").unwrap();
    let price_precise2 = PrecDec::from_str("0.000000000000000002").unwrap();

    let tick_precise1 = price_to_tick_index(price_precise1).unwrap();
    let tick_precise2 = price_to_tick_index(price_precise2).unwrap();

    println!(
        "tick_precise1: {}, tick_precise2: {}",
        tick_precise1, tick_precise2
    );

    assert!(
        tick_precise1 >= tick_precise2,
        "Should handle small price differences correctly"
    );
}

#[test]
fn test_price_to_tick_index_special_values() {
    // Test powers of 10
    let test_powers = vec![
        ("10.0", -23027),
        ("100.0", -46054),
        ("0.1", 23027),
        ("0.01", 46054),
    ];

    for (price_str, expected_tick) in test_powers {
        let price = PrecDec::from_str(price_str).unwrap();
        let tick = price_to_tick_index(price).unwrap();
        assert_eq!(tick, expected_tick, "Failed for power of 10: {}", price_str);
    }

    // Test common price ratios
    let test_ratios = vec![
        ("1.5", -4055),  // 3:2 ratio
        ("2.0", -6932),  // 2:1 ratio
        ("3.0", -10987), // 3:1 ratio
        ("4.0", -13864), // 4:1 ratio
    ];

    for (price_str, expected_tick) in test_ratios {
        let price = PrecDec::from_str(price_str).unwrap();
        let tick = price_to_tick_index(price).unwrap();
        assert_eq!(tick, expected_tick, "Failed for price ratio: {}", price_str);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::CombinedPriceResponse;
    use crate::state::{Config, FeeTier, FeeTierConfig, PairData, TokenData};
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{Addr, CosmosMsg, Uint128};
    use neutron_std::types::neutron::dex::MsgDeposit;
    use neutron_std::types::neutron::util::precdec::PrecDec;
    use neutron_std::types::slinky::types::v1::CurrencyPair;
    use prost::Message;

    // Helper function to create a test config
    fn setup_test_config() -> Config {
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
                        fee: 100,
                        percentage: 60,
                    },
                    FeeTier {
                        fee: 500,
                        percentage: 30,
                    },
                    FeeTier {
                        fee: 3000,
                        percentage: 10,
                    },
                ],
            },
            last_executed: 0,
            timestamp_stale: 1000,
            paused: false,
            pause_block: 0,
            skew: false,
            imbalance: 50u32,
            oracle_price_skew: 0i32,
        }
    }

    // Helper function to create test prices
    fn setup_test_prices() -> CombinedPriceResponse {
        CombinedPriceResponse {
            token_0_price: PrecDec::from_ratio(1u128, 1u128),
            token_1_price: PrecDec::from_ratio(1u128, 1u128),
            price_0_to_1: PrecDec::from_ratio(1u128, 1u128),
        }
    }

    #[test]
    fn test_get_deposit_messages_zero_balances() {
        let env = mock_env();
        let config = setup_test_config();
        let prices = setup_test_prices();
        let tick_index = 0;

        // Test with zero balances
        let token0_balance = Uint128::zero();
        let token1_balance = Uint128::zero();

        let messages = get_deposit_messages(
            &env,
            config,
            tick_index,
            prices,
            token0_balance,
            token1_balance,
        )
        .unwrap();

        // Should return an empty vector since there are no tokens to deposit
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_get_deposit_messages_equal_balances() {
        let env = mock_env();
        let mut config = setup_test_config();
        let prices = setup_test_prices();
        let tick_index = 0;

        // Test with equal balances
        let token0_balance = Uint128::new(1000000);
        let token1_balance = Uint128::new(1000000);

        // Make sure we have at least one fee tier in the config
        if config.fee_tier_config.fee_tiers.is_empty() {
            config.fee_tier_config.fee_tiers = vec![FeeTier {
                fee: 100,
                percentage: 100,
            }];
        }

        // Print debug information
        println!("Fee tiers: {:?}", config.fee_tier_config.fee_tiers);

        let messages = get_deposit_messages(
            &env,
            config.clone(),
            tick_index,
            prices.clone(),
            token0_balance,
            token1_balance,
        )
        .unwrap();

        // Print more debug information
        println!("Number of messages: {}", messages.len());

        // Should return at least one message
        assert!(
            !messages.is_empty(),
            "Expected at least one deposit message"
        );

        // Verify the first message is for the first fee tier
        if let CosmosMsg::Any(any_msg) = &messages[0] {
            println!("Message type_url: {}", any_msg.type_url);
            let deposit_msg = MsgDeposit::decode(any_msg.value.as_slice()).unwrap();

            println!("Deposit message: {:?}", deposit_msg);

            // First fee tier should use its percentage of the tokens
            assert_eq!(deposit_msg.tick_indexes_a_to_b[0], 0);

            // Check that amounts are approximately correct based on the fee tier percentage
            let amount_a: Uint128 = deposit_msg.amounts_a[0].parse().unwrap();
            let amount_b: Uint128 = deposit_msg.amounts_b[0].parse().unwrap();

            assert!(amount_a > Uint128::zero());
            assert!(amount_b > Uint128::zero());
        } else {
            panic!("Expected Any message, got: {:?}", &messages[0]);
        }
    }

    #[test]
    fn test_get_deposit_messages_uneven_balances() {
        let env = mock_env();
        let config = setup_test_config();
        let prices = setup_test_prices();
        let tick_index = 0;

        // Test with uneven balances
        let token0_balance = Uint128::new(2000000);
        let token1_balance = Uint128::new(1000000);

        let messages = get_deposit_messages(
            &env,
            config,
            tick_index,
            prices,
            token0_balance,
            token1_balance,
        )
        .unwrap();

        // Should return at least one message
        assert!(
            !messages.is_empty(),
            "Expected at least one deposit message"
        );

        // Verify the messages are properly formatted
        for (i, msg) in messages.iter().enumerate() {
            if let CosmosMsg::Any(any_msg) = msg {
                let deposit_msg = MsgDeposit::decode(any_msg.value.as_slice()).unwrap();

                println!("Message {}: {:?}", i, deposit_msg);

                // Each message should have valid amounts
                for (j, amount_a) in deposit_msg.amounts_a.iter().enumerate() {
                    let amount_a_uint: Uint128 = amount_a.parse().unwrap();
                    let amount_b_uint: Uint128 = deposit_msg.amounts_b[j].parse().unwrap();

                    assert!(
                        amount_a_uint > Uint128::zero() || amount_b_uint > Uint128::zero(),
                        "Expected at least one non-zero amount in message {}, position {}",
                        i,
                        j
                    );
                }
            } else {
                panic!("Expected Any message, got: {:?}", msg);
            }
        }
    }

    #[test]
    fn test_get_deposit_messages_different_prices() {
        let env = mock_env();
        let config = setup_test_config();
        let mut prices = setup_test_prices();
        // Set token0 to be worth twice as much as token1
        prices.token_0_price = PrecDec::from_ratio(2u128, 1u128);
        prices.price_0_to_1 = PrecDec::from_ratio(2u128, 1u128);

        let tick_index = 0;

        // Equal token amounts but different values due to price
        let token0_balance = Uint128::new(1000000);
        let token1_balance = Uint128::new(1000000);

        let messages = get_deposit_messages(
            &env,
            config.clone(),
            tick_index,
            prices,
            token0_balance,
            token1_balance,
        )
        .unwrap();

        // Should return at least one message
        assert!(
            !messages.is_empty(),
            "Expected at least one deposit message"
        );

        // Verify the messages are properly formatted
        for (i, msg) in messages.iter().enumerate() {
            if let CosmosMsg::Any(any_msg) = msg {
                let deposit_msg = MsgDeposit::decode(any_msg.value.as_slice()).unwrap();

                println!("Message {}: {:?}", i, deposit_msg);

                // Each message should have valid tick indexes and fees
                for j in 0..deposit_msg.tick_indexes_a_to_b.len() {
                    // The tick index should be the one we provided
                    assert_eq!(deposit_msg.tick_indexes_a_to_b[j], tick_index);

                    // The fee should be one of the configured fees
                    assert!(
                        config
                            .fee_tier_config
                            .fee_tiers
                            .iter()
                            .any(|tier| tier.fee == deposit_msg.fees[j]),
                        "Fee {} not found in config",
                        deposit_msg.fees[j]
                    );
                }
            } else {
                panic!("Expected Any message, got: {:?}", msg);
            }
        }
    }
}
