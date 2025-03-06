use std::fmt;
use std::str::FromStr;

use crate::error::ContractError;
use crate::msg::{
    CombinedPriceResponse, DepositResult, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use test_case::test_case;

use crate::utils::{get_deposit_data, price_to_tick_index};
use cosmwasm_std::{Decimal, Int128, Uint128};
use neutron_std::types::neutron::util::precdec::PrecDec;

// (total_available_0, total_available_1, expected_amount_0, expected_amount_1, tick_index, fee, token_0_price, token_1_price, price_0_to_1, base_deposit_percentage, expected_result)
// imbalance = 1900000 - 950000 / 2 = 475000 -> total = 50000 t0 , (100000 + 475000) t1
#[test_case(1000000, 2000000, 0, 0, 0, 0, "1", "1", "1", 5, 6, 6 => DepositResult { amount0: Uint128::new(50000), amount1: Uint128::new(575000), tick_index: 0, fee: 0 }; "imbalance case")]
#[test_case(1000000, 2000000, 0, 0, 0, 0, "1", "1", "1", 0, 6, 6 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(500000), tick_index: 0, fee: 0 }; "0% base deposit")]
#[test_case(1000000, 1000000, 0, 0, 0, 0, "1", "1", "1", 50, 6, 6 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: 0, fee: 0 }; "balanced case")]
#[test_case(1000000, 1000000, 0, 0, 0, 0, "2", "1", "2", 50, 6, 6 => DepositResult { amount0: Uint128::new(625000), amount1: Uint128::new(500000), tick_index: 0, fee: 0 }; "unequal token prices")]
#[test_case(1000000, 1000000, 0, 0, 0, 0, "1", "2", "0.5", 50, 6, 6 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(625000), tick_index: 0, fee: 0 }; "inverse unequal token prices")]
#[test_case(1000000, 1000000, 0, 0, 0, 0, "1", "2", "0.5", 100, 6, 6 => DepositResult { amount0: Uint128::new(1000000), amount1: Uint128::new(1000000), tick_index: 0, fee: 0 }; "100% deposit")]
#[test_case(0, 1000000, 1000000, 0, 0, 0, "1", "1", "1", 5, 6, 6 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(50000), tick_index: 0, fee: 0 }; "one token unavailable")]
#[test_case(0, 0, 1000000, 1000000, 0, 0, "1", "1", "1", 5, 6, 6 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(0), tick_index: 0, fee: 0 }; "both tokens unavailable")]
#[test_case(1000000, 1000000, 0, 1000000, 0, 0, "1", "1", "1", 5, 6, 6 => DepositResult { amount0: Uint128::new(50000), amount1: Uint128::new(575000), tick_index: 0, fee: 0 }; "expected amount for one token")]
#[test_case(1000000, 1000000, 1000000, 0, 0, 0, "1", "1", "1", 5, 6, 6 => DepositResult { amount0: Uint128::new(575000), amount1: Uint128::new(50000), tick_index: 0, fee: 0 }; "expected amount for other token")]
#[test_case(500000, 1000000, 500000, 0, 0, 0, "1", "1", "1", 0, 6, 6 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(0), tick_index: 0, fee: 0 }; "0% deposit with expected amount balanced")]
#[test_case(1000000, 1000000, 0, 1000000, 0, 0, "1", "1", "1", 0, 6, 6 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(500000), tick_index: 0, fee: 0 }; "0% deposit with expected amount imbalanced")]
#[test_case(500000, 1000000, 500000, 0, 0, 0, "1", "1", "1", 1, 6, 6 => DepositResult { amount0: Uint128::new(10000), amount1: Uint128::new(10000), tick_index: 0, fee: 0 }; "1% deposit with expected amount")]
// value 0 = 1000000
// value 1 = 1100000
// imbalance = 1100000 - 1000000 / 2 = 50000
// additional token 1 = 50000 / 1.1 = 45454.54 -> 45454
#[test_case(1000000, 1000000, 0, 0, 0, 0, "1", "1.1", "1", 0, 6, 6 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(45454), tick_index: 0, fee: 0 }; "slight price difference")]
// computed_amount_0 = 1000000 * 0.05 = 50000
// computed_amount_1 = 1000000 * 0.05 = 50000
// value 0 = 1000000 - 50000 = 950000 * 1 = 950000
// value 1 = 1000000 - 50000 = 950000 * 1.1 = 1045000
// imbalance = 1045000 - 950000 / 2 = 47500
// additional token 1 = 47500 / 1.1 = 43181.81 -> 43181
// total 0 = 50000
// total 1 = 50000 + 43181 = 93181
#[test_case(1000000, 1000000, 0, 0, 0, 0, "1", "1.1", "1", 5, 6, 6 => DepositResult { amount0: Uint128::new(50000), amount1: Uint128::new(93181), tick_index: 0, fee: 0 }; "slight price difference with 5% deposit")]
#[test_case(1000000, 1000000, 1000000, 1000000, 0, 0, "1", "1", "1", 1, 6, 6 => DepositResult { amount0: Uint128::new(20000), amount1: Uint128::new(20000), tick_index: 0, fee: 0 }; "expected amounts with 1% deposit")]
#[test_case(1000000, 1000000, 2000000, 2000000, 0, 0, "1", "1", "1", 100, 6, 6 => DepositResult { amount0: Uint128::new(1000000), amount1: Uint128::new(1000000), tick_index: 0, fee: 0 }; "capped deposit amounts")]
// computed_amount_0 = 1000000 * 0.1 = 100000
// computed_amount_1 = 1000000 * 0.1 = 100000
// value 0 = 1000000 - 100000 = 900000 * 1 = 900000
// value 1 = 1000000 - 100000 = 900000 * 200 = 180000000
// imbalance = 180000000 - 900000 / 2  = 89550000
// additional token 1 = 89550000 / 200 = 447750
// total 0 = 100000
// total 1 = 100000 + 447750 = 547750
#[test_case(1000000, 1000000, 0, 0, 0, 0, "1", "200", "1", 10, 6, 6 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(547750), tick_index: 0, fee: 0 }; "large price difference")]
fn test_get_deposit_data(
    total_available_0: u128,
    total_available_1: u128,
    expected_amount_0: u128,
    expected_amount_1: u128,
    tick_index: i64,
    fee: u64,
    token_0_price: &str,
    token_1_price: &str,
    price_0_to_1: &str,
    base_deposit_percentage: u64,
    decimals_0: u8,
    decimals_1: u8,
) -> DepositResult {
    let prices = CombinedPriceResponse {
        token_0_price: PrecDec::from_str(token_0_price).unwrap(),
        token_1_price: PrecDec::from_str(token_1_price).unwrap(),
        price_0_to_1: PrecDec::from_str(price_0_to_1).unwrap(),
    };

    get_deposit_data(
        Uint128::new(total_available_0),
        Uint128::new(total_available_1),
        tick_index,
        fee,
        &prices,
        base_deposit_percentage
    )
    .unwrap()
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
    
    assert_eq!(tick_above.abs(), tick_below.abs(), 
        "Tick indices should be symmetric around 1.0");

    // Test monotonicity
    let price1 = PrecDec::from_str("1.1").unwrap();
    let price2 = PrecDec::from_str("1.2").unwrap();
    
    let tick1 = price_to_tick_index(price1).unwrap();
    let tick2 = price_to_tick_index(price2).unwrap();
    
    assert!(tick1 > tick2, 
        "Tick index should decrease as price increases above 1.0");

    // Test precision handling
    let price_precise1 = PrecDec::from_str("0.000000000000000001").unwrap();
    let price_precise2 = PrecDec::from_str("0.000000000000000002").unwrap();
    
    let tick_precise1 = price_to_tick_index(price_precise1).unwrap();
    let tick_precise2 = price_to_tick_index(price_precise2).unwrap();
    
    println!("tick_precise1: {}, tick_precise2: {}", tick_precise1, tick_precise2);
    
    assert!(tick_precise1 >= tick_precise2, 
        "Should handle small price differences correctly");
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
        assert_eq!(tick, expected_tick, 
            "Failed for power of 10: {}", price_str);
    }

    // Test common price ratios
    let test_ratios = vec![
        ("1.5", -4055),    // 3:2 ratio
        ("2.0", -6932),    // 2:1 ratio
        ("3.0", -10987),   // 3:1 ratio
        ("4.0", -13864),   // 4:1 ratio
    ];

    for (price_str, expected_tick) in test_ratios {
        let price = PrecDec::from_str(price_str).unwrap();
        let tick = price_to_tick_index(price).unwrap();
        assert_eq!(tick, expected_tick, 
            "Failed for price ratio: {}", price_str);
    }
}
