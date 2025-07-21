use std::str::FromStr;

use crate::msg::{CombinedPriceResponse, DepositResult};
use crate::state::FeeTier;
use test_case::test_case;

use crate::utils::{get_deposit_data, get_deposit_messages, price_to_tick_index};
use cosmwasm_std::Uint128;
use neutron_std::types::neutron::util::precdec::PrecDec;

// (total_available_0, total_available_1, tick_index, fee, token_0_price, token_1_price, price_0_to_1, base_deposit_percentage,
// skew, imbalance, oracle_price_skew, dynamic_spread_factor, dynamic_spread_cap)
// imbalance = 1900000 - 950000 / 2 = 475000 -> total = 50000 t0 , (100000 + 475000) t1
#[test_case(1000000, 2000000, 0, 0, "1", "1", "1", 5, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(50000), amount1: Uint128::new(600000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "imbalance case")]
#[test_case(1000000, 2000000, 0, 0, "1", "1", "1", 0, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(500000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "0% base deposit")]
#[test_case(1000000, 1000000,  0, 0, "1", "1", "1", 50, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "balanced case")]
#[test_case(1000000, 1000000, 0, 0, "2", "1", "2", 50, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(750000), amount1: Uint128::new(500000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "unequal token prices")]
#[test_case(1000000, 1000000, 0, 0, "1", "2", "0.5", 50, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(750000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "inverse unequal token prices")]
#[test_case(1000000, 1000000,  0, 0, "1", "1", "1", 100, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1000000), amount1: Uint128::new(1000000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "100% deposit")]
#[test_case(0, 1000000, 0, 0, "1", "1", "1", 5, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(550000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "one token unavailable")]
#[test_case(0, 0, 0, 0, "1", "1", "1", 5, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(0), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "both tokens unavailable")]
// value 0 = 1000000
// value 1 = 1100000
// imbalance = 1100000 - 1000000 / 2 = 50000
// additional token 1 = 50000 / 1.1 = 45454.54 -> 45454
#[test_case(1000000, 1000000, 0, 0, "1", "1.1", "1", 0, 0i32, 50u32, 0i32, 0i32, 0i32  => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(45454), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "slight price difference")]
// computed_amount_0 = 1000000 * 0.05 = 50000
// computed_amount_1 = 1000000 * 0.05 = 50000
// value 0 = 1000000 - 50000 = 950000 * 1 = 950000
// value 1 = 1000000 - 50000 = 950000 * 1.1 = 1045000
// imbalance = 1045000 - 950000 / 2 = 47500
// additional token 1 = 47500 / 1.1 = 43181.81 -> 43181
// total 0 = 50000
// total 1 = 50000 + 43181 = 93181
#[test_case(1000000, 1000000, 0, 0, "1", "1.1", "1", 5, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(50000), amount1: Uint128::new(95454), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "slight price difference with 5% deposit")]
#[test_case(1000000, 1000000, 0, 0, "1", "1", "1", 100, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1000000), amount1: Uint128::new(1000000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "capped deposit amounts")]
// computed_amount_0 = 1000000 * 0.1 = 100000
// computed_amount_1 = 1000000 * 0.1 = 100000
// value 0 = 1000000 - 100000 = 900000 * 1 = 900000
// value 1 = 1000000 - 100000 = 900000 * 200 = 180000000
// imbalance = 180000000 - 900000 / 2  = 89550000
// additional token 1 = 89550000 / 200 = 447750
// total 0 = 100000
// total 1 = 100000 + 447750 = 547750
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "large price difference")]
#[test_case(0, 1000000, 0, 10, "1", "1", "1", 10, 9, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(600000), tick_index: -9, fees: vec![FeeTier { fee: 10, percentage: 100 }] }; "large price difference with skew token0")]
#[test_case(1000000, 0, 0, 10, "1", "1", "1", 10, 9, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(600000), amount1: Uint128::new(0), tick_index: 9, fees: vec![FeeTier { fee: 10, percentage: 100 }] }; "large price difference with skew token1")]
#[test_case(10000000, 0, 0, 100, "1", "1", "1", 10, 99, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(6000000), amount1: Uint128::new(0), tick_index: 99, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test skew sequence -1")]
#[test_case(9000000, 1000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(4900000), amount1: Uint128::new(100000), tick_index: 79, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test skew sequence -2")]
#[test_case(8000000, 2000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(3800000), amount1: Uint128::new(200000), tick_index: 59, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test skew sequence -3")]
#[test_case(7000000, 3000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(2700000), amount1: Uint128::new(300000), tick_index: 40, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test skew sequence -4")]
#[test_case(6000000, 4000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1600000), amount1: Uint128::new(400000), tick_index: 20, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test skew sequence -5")]
#[test_case(5000000, 5000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: 0, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test skew sequence -6")]
#[test_case(10000000, 0, 0, 100, "1", "1", "1", 10, 99, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(6000000), amount1: Uint128::new(0), tick_index: 99, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test double skew sequence -1")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, 1i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 1, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew -1")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, 11i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 11, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew -2")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, 111i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 111, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew -3")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, 99999i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: 99999, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew -4")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, -1i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: -1, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew -5")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, -11i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: -11, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew -6")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, -111i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: -111, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew -7")]
#[test_case(1000000, 1000000, 0, 0, "1", "200", "1", 10, 0i32, 50u32, -99999i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(597500), tick_index: -99999, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew -8")]
#[test_case(9000000, 1000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 1i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(4900000), amount1: Uint128::new(100000), tick_index: 80, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test double skew sequence -2")]
#[test_case(8000000, 2000000, 0, 100, "1", "1", "1", 10, 99, 50u32, -2i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(3800000), amount1: Uint128::new(200000), tick_index: 57, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test double skew sequence -3")]
#[test_case(7000000, 3000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 4i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(2700000), amount1: Uint128::new(300000), tick_index: 44, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test double skew sequence -4")]
#[test_case(6000000, 4000000, 0, 100, "1", "1", "1", 10, 99, 50u32, -8i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1600000), amount1: Uint128::new(400000), tick_index: 12, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test double skew sequence -5")]
#[test_case(5000000, 5000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 33i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: 33, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test double skew sequence -6")]
#[test_case(5000000, 5000000, 0, 100, "1", "1", "1", 10, 99, 50u32, -99i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(500000), amount1: Uint128::new(500000), tick_index: -99, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test double skew sequence -7")]
#[test_case(1000000, 9000000, 0, 100, "1", "1", "1", 10, 99, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(4900000), tick_index: -79, fees: vec![FeeTier { fee: 100, percentage: 100 }] }; "test double skew sequence -8")]
#[test_case(1000000, 1000000, 0, 0, "1", "1", "1", 10, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(100000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static -1")]
#[test_case(1000000, 1000000, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(100000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static -2")]
#[test_case(1000000, 1000000, 0, 0, "1", "1", "1", 10, 100i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(100000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static -3")]
#[test_case(1000000, 1000000, 0, 0, "1", "1", "1", 10, 1000i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(100000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static -4")]
#[test_case(1000000, 1000000, 0, 0, "1", "1", "1", 10, 10000i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(100000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static -5")]
#[test_case(1111111, 1000000, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(166666), amount1: Uint128::new(100000), tick_index: 1, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced light left-heavy -1")]
#[test_case(1111111, 1000000, 0, 0, "1", "1", "1", 10, 100i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(166666), amount1: Uint128::new(100000), tick_index: 5, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced light left-heavy -2")]
#[test_case(1111111, 1000000, 0, 0, "1", "1", "1", 10, 1000i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(166666), amount1: Uint128::new(100000), tick_index: 53, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced light left-heavy -3")]
// imbalance @ 50% ((3000000 - 1000000) / (3000000 + 1000000)) = 0.5. skew @ 50%
#[test_case(3000000, 1000000, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1300000), amount1: Uint128::new(100000), tick_index: 5, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced more left-heavy -1")]
#[test_case(3000000, 1000000, 0, 0, "1", "1", "1", 10, 100i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1300000), amount1: Uint128::new(100000), tick_index: 50, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced more left-heavy -2")]
#[test_case(3000000, 1000000, 0, 0, "1", "1", "1", 10, 1000i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1300000), amount1: Uint128::new(100000), tick_index: 500, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced more left-heavy -3")]
// imbalance @ 50% ((3000000 - 1000000) / (3000000 + 1000000)) = 0.5. skew @ 50%f
#[test_case(3000000, 0, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1800000), amount1: Uint128::new(0), tick_index: 10, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced fully left-heavy -1")]
#[test_case(3000000, 0, 0, 0, "1", "1", "1", 10, 100i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1800000), amount1: Uint128::new(0), tick_index: 100, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced fully left-heavy -2")]
#[test_case(3000000, 0, 0, 0, "1", "1", "1", 10, 1000i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1800000), amount1: Uint128::new(0), tick_index: 1000, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced fully left-heavy -3")]
#[test_case(1000000, 1111111, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(166666), tick_index: -1, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced light right-heavy -1")]
#[test_case(1000000, 1111111, 0, 0, "1", "1", "1", 10, 100i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(166666), tick_index: -5, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced light right-heavy -2")]
#[test_case(1000000, 1111111, 0, 0, "1", "1", "1", 10, 1000i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(166666), tick_index: -53, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced light right-heavy -3")]
#[test_case(1000000, 3000000, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(1300000), tick_index: -5, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced more right-heavy -1")]
#[test_case(1000000, 3000000, 0, 0, "1", "1", "1", 10, 100i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(1300000), tick_index: -50, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced more right-heavy -2")]
#[test_case(1000000, 3000000, 0, 0, "1", "1", "1", 10, 1000i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(100000), amount1: Uint128::new(1300000), tick_index: -500, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced more right-heavy -3")]
#[test_case(0, 3000000, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(1800000), tick_index: -10, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced fully right-heavy -1")]
#[test_case(0, 3000000, 0, 0, "1", "1", "1", 10, 100i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(1800000), tick_index: -100, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced fully right-heavy -2")]
#[test_case(0, 3000000, 0, 0, "1", "1", "1", 10, 1000i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(1800000), tick_index: -1000, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "test oracle skew static imbalanced fully right-heavy -3")]
#[test_case(0, 3000000, 0, 0, "1", "1", "1", 10, 0i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(1800000), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "omni-skew-sequence-no-skews")]
#[test_case(0, 3000000, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(1800000), tick_index: -10, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "omni-skew-sequence-only-base-skew-b-dominant")]
#[test_case(3000000, 0, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 0i32 => DepositResult { amount0: Uint128::new(1800000), amount1: Uint128::new(0), tick_index: 10, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "omni-skew-sequence-only-base-skew-a-dominant")]
#[test_case(0, 3000000, 0, 0, "1", "1", "1", 10, 0i32, 50u32, 0i32, 0i32, 100i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(1800000), tick_index: -50, fees: vec![FeeTier { fee: 50, percentage: 100 }] }; "omni-skew-sequence-only-dynamic-skew-b-dominant")]
#[test_case(3000000, 0, 0, 0, "1", "1", "1", 10, 0i32, 50u32, 0i32, 0i32, 100i32 => DepositResult { amount0: Uint128::new(1800000), amount1: Uint128::new(0), tick_index: 50, fees: vec![FeeTier { fee: 50, percentage: 100 }] }; "omni-skew-sequence-only-dynamic-skew-a-dominant")]
#[test_case(0, 3000000, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 100i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(1800000), tick_index: -60, fees: vec![FeeTier { fee: 50, percentage: 100 }] }; "omni-skew-sequence-3-double-skew-b-dominant")]
#[test_case(3000000, 0, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 100i32 => DepositResult { amount0: Uint128::new(1800000), amount1: Uint128::new(0), tick_index: 60, fees: vec![FeeTier { fee: 50, percentage: 100 }] }; "omni-skew-sequence-4-double-skew-a-dominant")]
#[test_case(1, 299999, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 100i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(179998), tick_index: -60, fees: vec![FeeTier { fee: 50, percentage: 100 }] }; "omni-skew-sequence-edge-case-1")]
#[test_case(100, 100, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 100i32 => DepositResult { amount0: Uint128::new(10), amount1: Uint128::new(10), tick_index: 0, fees: vec![FeeTier { fee: 0, percentage: 100 }] }; "omni-skew-sequence-edge-case-1.5")]
#[test_case(299999, 1, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 100i32 => DepositResult { amount0: Uint128::new(179998), amount1: Uint128::new(0), tick_index: 60, fees: vec![FeeTier { fee: 50, percentage: 100 }] }; "omni-skew-sequence-edge-case-2")]
#[test_case(1, 299999, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, -1000i32, 300i32 => DepositResult { amount0: Uint128::new(0), amount1: Uint128::new(179998), tick_index: -160, fees: vec![FeeTier { fee: 150, percentage: 100 }] }; "omni-skew-sequence-edge-case-3")]
#[test_case(299999, 1, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 1000i32, 300i32 => DepositResult { amount0: Uint128::new(179998), amount1: Uint128::new(0), tick_index: 160, fees: vec![FeeTier { fee: 150, percentage: 100 }] }; "omni-skew-sequence-edge-case-4")]
// Fee of 10. perfectly balanced, skew cap 100.
// n = original tick index
// c = static tick that doesn't change price
//                  10    10
// ---------------|-----|-----|----------
//                c     n     c+20
// 100% imabalnced linarly, move fee to 60 (100/2 + base fee). fee adjusted by 50
//           60             60
// --|------------------|----------------|------
//   c-50              n               c+70
// Then move deposit index by the adjustement amount (100/2)
//                      50          50
// --------------|--------------|----------------|------
//                c            n+50             c+120
// add the full skew of 10
//                      50          50
// ----------------|--------------|----------------|------
//                c+10            n+60             c+130
#[test_case(300000, 0, 0, 0, "1", "1", "1", 10, 10i32, 50u32, 0i32, 0i32, 100i32 => DepositResult { amount0: Uint128::new(180000), amount1: Uint128::new(0), tick_index: 60, fees: vec![FeeTier { fee: 50, percentage: 100 }] }; "skew-wit-comment")]

fn test_get_deposit_data(
    total_available_0: u128,
    total_available_1: u128,
    tick_index: i64,
    fee: u64,
    token_0_price: &str,
    token_1_price: &str,
    price_0_to_1: &str,
    base_deposit_percentage: u64,
    skew: i32,
    imbalance: u32,
    oracle_price_skew: i32,
    dynamic_spread_factor: i32,
    dynamic_spread_cap: i32,
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
        vec![FeeTier {
            fee,
            percentage: 100,
        }],
        &prices,
        base_deposit_percentage,
        skew,
        imbalance,
        oracle_price_skew,
        dynamic_spread_factor,
        dynamic_spread_cap,
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
            skew: 0i32,
            imbalance: 50u32,
            oracle_price_skew: 0i32,
            dynamic_spread_factor: 0i32,
            dynamic_spread_cap: 0i32,
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
            &prices,
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
            &prices.clone(),
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
            &prices,
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
            &prices,
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

// dynamic_spread_cap, dynamic_spread_factor, imbalance_f64, fee_tiers
#[test_case(0, 0, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "zero spread cap")]
#[test_case(100, 50, 0.0, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "zero imbalance")]
// rounding error shoudl be ignored
#[test_case(100, 0, 0.000001, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "very small imbalance below epsilon")]
// Fee of 10. perfectly balanced, skew cap 100.
// n = original tick index
// c = static tick that doesn't change price
//         10          10
// --|----------|----------|----------
//   c          n         c+20
// 11% imabalnced linarly, move fee to 11. fee adjusted by 1
//          11         11
// -|-----------|-----------|------
//   c-1        n         c+21
// Then move deposit index by the adjustement amount (1)
//          11         11
// --|-----------|-----------|------
//   c            n         c+22
#[test_case(100, 0, 0.01, vec![FeeTier { fee: 10, percentage: 100 }] => (1, vec![FeeTier { fee: 11, percentage: 100 }]); "linear movement small imbalance -1")]
// Fee of 10. perfectly balanced, skew cap 100.
// n = original tick index
// c = static tick that doesn't change price
//          10    10
// -------|-----|-----|----------
//        c     n     c+20
// 10% imabalnced linarly, make imbalanced tick more expensive by 10% of the cap (10 bps)
// mincrease spread by 5, then move distribution by another 5.
// new fee is 15 (10 + 5), ticked inex moved by another 5
//          15     15
// -----|-------|-------|------
//   c-5        n       c+25
// Then move deposit index by the adjustement amount (-5)
//             15     15
// --------|-------|-------|------
//         c       n      c+30
// n and c remain identical, imbalanced index moved by 10
#[test_case(100, 0, 0.1, vec![FeeTier { fee: 10, percentage: 100 }] => (5, vec![FeeTier { fee: 15, percentage: 100 }]); "linear movement positive imbalance -1")]
#[test_case(100, 0, 0.2, vec![FeeTier { fee: 10, percentage: 100 }] => (10, vec![FeeTier { fee: 20, percentage: 100 }]); "linear movement positive imbalance -2")]
#[test_case(100, 0, 0.3, vec![FeeTier { fee: 10, percentage: 100 }] => (15, vec![FeeTier { fee: 25, percentage: 100 }]); "linear movement positive imbalance -3")]
#[test_case(100, 0, 0.4, vec![FeeTier { fee: 10, percentage: 100 }] => (20, vec![FeeTier { fee: 30, percentage: 100 }]); "linear movement positive imbalance -4")]
#[test_case(100, 0, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (25, vec![FeeTier { fee: 35, percentage: 100 }]); "linear movement positive imbalance -5")]
#[test_case(100, 0, 0.6, vec![FeeTier { fee: 10, percentage: 100 }] => (30, vec![FeeTier { fee: 40, percentage: 100 }]); "linear movement positive imbalance -6")]
#[test_case(100, 0, 0.7, vec![FeeTier { fee: 10, percentage: 100 }] => (35, vec![FeeTier { fee: 45, percentage: 100 }]); "linear movement positive imbalance -7")]
// Fee of 10. perfectly balanced, skew cap 100.
// n = original tick index
// c = static tick that doesn't change price
//                  10    10
// ---------------|-----|-----|----------
//                c     n     c+20
// 80% imabalnced linarly, move fee to 20. fee adjusted by 80
//           90               90
// --|------------------|----------------|------
//   c-80               n               c+100
// Then move deposit index by the adjustement amount (-80)
//            20          20
// -----------------|------------------|--------------------|------
//                  c                 n+80                 c+180
#[test_case(100, 0, 0.8, vec![FeeTier { fee: 10, percentage: 100 }] => (40, vec![FeeTier { fee: 50, percentage: 100 }]); "linear movement positive imbalance -8")]
#[test_case(100, 0, 0.9, vec![FeeTier { fee: 10, percentage: 100 }] => (45, vec![FeeTier { fee: 55, percentage: 100 }]); "linear movement positive imbalance -9")]
#[test_case(100, 0, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (50, vec![FeeTier { fee: 60, percentage: 100 }]); "linear movement positive imbalance -10")]
#[test_case(100, 0, -0.1, vec![FeeTier { fee: 10, percentage: 100 }] => (-5, vec![FeeTier { fee: 15, percentage: 100 }]); "linear movement negative imbalance -1")]
#[test_case(100, 0, -0.2, vec![FeeTier { fee: 10, percentage: 100 }] => (-10, vec![FeeTier { fee: 20, percentage: 100 }]); "linear movement negative imbalance -2")]
#[test_case(100, 0, -0.3, vec![FeeTier { fee: 10, percentage: 100 }] => (-15, vec![FeeTier { fee: 25, percentage: 100 }]); "linear movement negative imbalance -3")]
#[test_case(100, 0, -0.4, vec![FeeTier { fee: 10, percentage: 100 }] => (-20, vec![FeeTier { fee: 30, percentage: 100 }]); "linear movement negative imbalance -4")]
#[test_case(100, 0, -0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (-25, vec![FeeTier { fee: 35, percentage: 100 }]); "linear movement negative imbalance -5")]
#[test_case(100, 0, -0.6, vec![FeeTier { fee: 10, percentage: 100 }] => (-30, vec![FeeTier { fee: 40, percentage: 100 }]); "linear movement negative imbalance -6")]
#[test_case(100, 0, -0.7, vec![FeeTier { fee: 10, percentage: 100 }] => (-35, vec![FeeTier { fee: 45, percentage: 100 }]); "linear movement negative imbalance -7")]
#[test_case(100, 0, -0.8, vec![FeeTier { fee: 10, percentage: 100 }] => (-40, vec![FeeTier { fee: 50, percentage: 100 }]); "linear movement negative imbalance -8")]
#[test_case(100, 0, -0.9, vec![FeeTier { fee: 10, percentage: 100 }] => (-45, vec![FeeTier { fee: 55, percentage: 100 }]); "linear movement negative imbalance -9")]
#[test_case(100, 0, -1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (-50, vec![FeeTier { fee: 60, percentage: 100 }]); "linear movement negative imbalance -10")]
#[test_case(50, 0, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (25, vec![FeeTier { fee: 35, percentage: 100 }]); "maximum positive imbalance")]
#[test_case(50, 0, -1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (-25, vec![FeeTier { fee: 35, percentage: 100 }]); "maximum negative imbalance")]
#[test_case(100, 0, 0.2, vec![FeeTier { fee: 100, percentage: 50 }, FeeTier { fee: 200, percentage: 50 }] => (10, vec![FeeTier { fee: 110, percentage: 50 }, FeeTier { fee: 210, percentage: 50 }]); "multiple fee tiers")]
#[test_case(100, 0, 0.5, vec![FeeTier { fee: 1, percentage: 100 }] => (25, vec![FeeTier { fee: 26, percentage: 100 }]); "fee tier adjustment with low base fee")]
#[test_case(100, 0, 0.5, vec![FeeTier { fee: 0, percentage: 100 }] => (25, vec![FeeTier { fee: 25, percentage: 100 }]); "fee tier adjustment with zero base fee")]
#[test_case(100, 0, -0.5, vec![FeeTier { fee: 0, percentage: 100 }] => (-25, vec![FeeTier { fee: 25, percentage: 100 }]); "fee tier adjustment with zero base fee negative imbalance")]
// 9% imbalance with a cap of 10 implies a 0.9 bip delta, we don't have that persision. there will be no change in fee tier
#[test_case(10, 0, 0.09, vec![FeeTier { fee: 2, percentage: 100 }] => (0, vec![FeeTier { fee: 2, percentage: 100 }]); "small fee cap - positive imbalance -1")]
// 10% impabace wuth a cap of 10 implies a 1 bip delta, increase fee by 1,
#[test_case(10, 0, 0.1, vec![FeeTier { fee: 2, percentage: 100 }] => (1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance -2")]
// same as above should apply to any number between 0.1 and 0.2 exclusive
#[test_case(10, 0, 0.11, vec![FeeTier { fee: 2, percentage: 100 }] => (1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance -3")]
#[test_case(10, 0, 0.1999, vec![FeeTier { fee: 2, percentage: 100 }] => (1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -1")]
// a cap of 1 should only be realized at maximum imbalance
#[test_case(1, 0, 0.99, vec![FeeTier { fee: 2, percentage: 100 }] => (0, vec![FeeTier { fee: 2, percentage: 100 }]); "small fee cap - positive imbalance edge case -2")]
#[test_case(1, 0, 1.0, vec![FeeTier { fee: 2, percentage: 100 }] => (1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -3")]
#[test_case(1, 0, -1.0, vec![FeeTier { fee: 2, percentage: 100 }] => (-1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -4")]
// a cap of 2 should only be realized at half and max imbalance
#[test_case(2, 0, 0.4999, vec![FeeTier { fee: 2, percentage: 100 }] => (0, vec![FeeTier { fee: 2, percentage: 100 }]); "small fee cap - positive imbalance edge case -5")]
// anything over 0.5 should round up
#[test_case(2, 0, 0.5, vec![FeeTier { fee: 2, percentage: 100 }] => (1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -6")]
#[test_case(2, 0, -0.5, vec![FeeTier { fee: 2, percentage: 100 }] => (-1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -7")]
#[test_case(2, 0, 0.99, vec![FeeTier { fee: 2, percentage: 100 }] => (1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -8")]
#[test_case(2, 0, -0.99, vec![FeeTier { fee: 2, percentage: 100 }] => (-1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -9")]
#[test_case(2, 0, 1.0, vec![FeeTier { fee: 2, percentage: 100 }] => (1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -10")]
#[test_case(2, 0, -1.0, vec![FeeTier { fee: 2, percentage: 100 }] => (-1, vec![FeeTier { fee: 3, percentage: 100 }]); "small fee cap - positive imbalance edge case -11")]
// Logarithmic test
#[test_case(100, 0, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (25, vec![FeeTier { fee: 35, percentage: 100 }]); "logarithmic movement positive imbalance -1")]
// Edge Case Category 1: Parameter Validation
#[test_case(0, 0, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "zero spread cap should return no adjustment")]
#[test_case(100, 0, 0.0, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "zero imbalance should return no adjustment")]
#[test_case(100, 0, f64::EPSILON / 2.0, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "imbalance below epsilon should return no adjustment")]
#[test_case(100, 0, -f64::EPSILON / 2.0, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "negative imbalance below epsilon should return no adjustment")]
// Edge Case Category 2: Extreme Factor Values (using actual expected results)
#[test_case(100, -10000, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "very large negative factor produces no effect due to underflow")]
#[test_case(100, 10000, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (50, vec![FeeTier { fee: 60, percentage: 100 }]); "very large positive factor saturates to maximum")]
#[test_case(100, -1, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (25, vec![FeeTier { fee: 35, percentage: 100 }]); "factor -1 exponential case")]
#[test_case(100, 1, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (25, vec![FeeTier { fee: 35, percentage: 100 }]); "factor 1 logarithmic case")]
// Edge Case Category 3: Maximum Imbalance
#[test_case(100, 0, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (50, vec![FeeTier { fee: 60, percentage: 100 }]); "maximum positive imbalance linear")]
#[test_case(100, 0, -1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (-50, vec![FeeTier { fee: 60, percentage: 100 }]); "maximum negative imbalance linear")]
#[test_case(100, -100, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (50, vec![FeeTier { fee: 60, percentage: 100 }]); "maximum positive imbalance exponential")]
#[test_case(100, 100, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (50, vec![FeeTier { fee: 60, percentage: 100 }]); "maximum positive imbalance logarithmic")]
// Edge Case Category 4: Fee Tier Edge Cases
#[test_case(100, 0, 0.5, vec![FeeTier { fee: 0, percentage: 100 }] => (25, vec![FeeTier { fee: 25, percentage: 100 }]); "zero base fee with positive adjustment")]
#[test_case(50, 0, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (13, vec![FeeTier { fee: 23, percentage: 100 }]); "adjustment result with small cap")]
#[test_case(100, 0, -0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (-25, vec![FeeTier { fee: 35, percentage: 100 }]); "negative imbalance should increase fee")]
#[test_case(60, 0, -0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (-15, vec![FeeTier { fee: 25, percentage: 100 }]); "negative imbalance fee adjustment")]
// Edge Case Category 5: Multiple Fee Tiers
#[test_case(100, 0, 0.5, vec![FeeTier { fee: 1, percentage: 50 }, FeeTier { fee: 5, percentage: 30 }, FeeTier { fee: 10, percentage: 20 }] => (25, vec![FeeTier { fee: 26, percentage: 50 }, FeeTier { fee: 30, percentage: 30 }, FeeTier { fee: 35, percentage: 20 }]); "multiple fee tiers all adjusted equally")]
#[test_case(40, 0, 0.5, vec![FeeTier { fee: 0, percentage: 100 }, FeeTier { fee: 50, percentage: 0 }] => (10, vec![FeeTier { fee: 10, percentage: 100 }, FeeTier { fee: 60, percentage: 0 }]); "fee tiers with zero percentage")]
// Edge Case Category 6: Small Spread Caps (Precision Testing)
#[test_case(1, 0, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "spread cap 1 with 50% imbalance should round to 0")]
#[test_case(1, 0, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (1, vec![FeeTier { fee: 11, percentage: 100 }]); "spread cap 1 with 100% imbalance should adjust fee by 1")]
#[test_case(2, 0, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (1, vec![FeeTier { fee: 11, percentage: 100 }]); "spread cap 2 with 50% imbalance")]
#[test_case(3, 0, 0.33, vec![FeeTier { fee: 10, percentage: 100 }] => (0, vec![FeeTier { fee: 10, percentage: 100 }]); "spread cap 3 with 33% imbalance should round")]
// Edge Case Category 7: Convergence Testing (using actual results)
#[test_case(100, -50, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (50, vec![FeeTier { fee: 60, percentage: 100 }]); "exponential factor -50 at max imbalance")]
#[test_case(100, 0, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (50, vec![FeeTier { fee: 60, percentage: 100 }]); "linear factor 0 at max imbalance")]
#[test_case(100, 50, 1.0, vec![FeeTier { fee: 10, percentage: 100 }] => (50, vec![FeeTier { fee: 60, percentage: 100 }]); "logarithmic factor 50 at max imbalance")]
// Edge Case Category 8: Boundary Conditions for Curve Types
#[test_case(100, -1, 0.1, vec![FeeTier { fee: 10, percentage: 100 }] => (5, vec![FeeTier { fee: 15, percentage: 100 }]); "exponential factor -1 small imbalance")]
#[test_case(100, 1, 0.1, vec![FeeTier { fee: 10, percentage: 100 }] => (5, vec![FeeTier { fee: 15, percentage: 100 }]); "logarithmic factor 1 small imbalance")]
#[test_case(100, -100, 0.1, vec![FeeTier { fee: 10, percentage: 100 }] => (1, vec![FeeTier { fee: 11, percentage: 100 }]); "exponential factor -100 very small imbalance")]
#[test_case(100, 100, 0.1, vec![FeeTier { fee: 10, percentage: 100 }] => (8, vec![FeeTier { fee: 18, percentage: 100 }]); "logarithmic factor 100 small imbalance")]
// Edge Case Category 9: Fee Underflow Protection
#[test_case(200, 0, -0.5, vec![FeeTier { fee: 50, percentage: 100 }] => (-50, vec![FeeTier { fee: 100, percentage: 100 }]); "large negative imbalance should not underflow fee")]
#[test_case(1000, 0, -0.1, vec![FeeTier { fee: 10, percentage: 100 }] => (-50, vec![FeeTier { fee: 60, percentage: 100 }]); "small negative imbalance with large cap")]
// Edge Case Category 10: Extreme Precision Cases
#[test_case(1000000, 0, 0.000001, vec![FeeTier { fee: 100, percentage: 100 }] => (1, vec![FeeTier { fee: 101, percentage: 100 }]); "very large cap with tiny imbalance")]
#[test_case(1, 0, 0.999999, vec![FeeTier { fee: 100, percentage: 100 }] => (0, vec![FeeTier { fee: 100, percentage: 100 }]); "tiny cap with near-max imbalance should round to 0 tick adjustment")]
// Edge Case Category 11: Special Mathematical Cases
#[test_case(100, -100, 0.5, vec![FeeTier { fee: 10, percentage: 100 }] => (13, vec![FeeTier { fee: 23, percentage: 100 }]); "exponential with very large negative factor")]
#[test_case(50, 0, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (25, vec![FeeTier { fee: 30, percentage: 100 }]); "small cap at maximum imbalance")]
#[test_case(2, 0, 1.0, vec![FeeTier { fee: 1, percentage: 100 }] => (1, vec![FeeTier { fee: 2, percentage: 100 }]); "very small cap at maximum imbalance")]
// feel-good tests based on formula outputs:
// dynamic_spread_cap = 300
// validate curve at spread factor +/-(1, 10, 30, 100, 1000, 3000)
// at imabalnce ratios: +/1[0, 0.01, 0.1, 0,2, 0,49, 0.5, 0.75, 0.9, 0.99, 1.0]
/// Exponential case: factor < 0 (slow then fast):
/// g(x) = (1 - (1-x)^(1+q)) * c  where q = |factor|/100
///
/// Logarithmic case: factor > 0 (fast then slow):
/// h(x) = (1 - e^(-x*n)) * c / (1 - e^(-n))  where n = factor/100
///

/// Logarithmic case:
/// //factor 1 - positive imabalnce
#[test_case(300, 1, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 1")]
#[test_case(300, 1, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (2, vec![FeeTier { fee: 7, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 2")]
#[test_case(300, 1, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (15, vec![FeeTier { fee: 20, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 3")]
#[test_case(300, 1, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (30, vec![FeeTier { fee: 35, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 4")]
#[test_case(300, 1, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (74, vec![FeeTier { fee: 79, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 5")]
#[test_case(300, 1, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (75, vec![FeeTier { fee: 80, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 6")]
#[test_case(300, 1, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (113, vec![FeeTier { fee: 118, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 7")]
#[test_case(300, 1, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (135, vec![FeeTier { fee: 140, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 8")]
#[test_case(300, 1, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 9")]
#[test_case(300, 1, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1 case 10")]
//factor 10 - positive imabalnce
#[test_case(300, 10, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 1")]
#[test_case(300, 10, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (2, vec![FeeTier { fee: 7, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 2")]
#[test_case(300, 10, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (16, vec![FeeTier { fee: 21, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 3")]
#[test_case(300, 10, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (31, vec![FeeTier { fee: 36, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 4")]
#[test_case(300, 10, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (75, vec![FeeTier { fee: 80, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 5")]
#[test_case(300, 10, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (77, vec![FeeTier { fee: 82, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 6")]
#[test_case(300, 10, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (114, vec![FeeTier { fee: 119, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 7")]
#[test_case(300, 10, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (136, vec![FeeTier { fee: 141, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 8")]
#[test_case(300, 10, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 9")]
#[test_case(300, 10, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 10 case 10")]
//factor 30 - positive imabalnce
#[test_case(300, 30, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 1")]
#[test_case(300, 30, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (2, vec![FeeTier { fee: 7, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 2")]
#[test_case(300, 30, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (17, vec![FeeTier { fee: 22, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 3")]
#[test_case(300, 30, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (34, vec![FeeTier { fee: 39, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 4")]
#[test_case(300, 30, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (79, vec![FeeTier { fee: 84, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 5")]
#[test_case(300, 30, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (81, vec![FeeTier { fee: 86, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 6")]
#[test_case(300, 30, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (117, vec![FeeTier { fee: 122, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 7")]
#[test_case(300, 30, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (137, vec![FeeTier { fee: 142, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 8")]
#[test_case(300, 30, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 9")]
#[test_case(300, 30, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 30 case 10")]
//factor 100 - positive imabalnce
#[test_case(300, 100, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 1")]
#[test_case(300, 100, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (2, vec![FeeTier { fee: 7, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 2")]
#[test_case(300, 100, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (23, vec![FeeTier { fee: 28, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 3")]
#[test_case(300, 100, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (43, vec![FeeTier { fee: 48, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 4")]
#[test_case(300, 100, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (92, vec![FeeTier { fee: 97, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 5")]
#[test_case(300, 100, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (93, vec![FeeTier { fee: 98, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 6")]
#[test_case(300, 100, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (125, vec![FeeTier { fee: 130, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 7")]
#[test_case(300, 100, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (141, vec![FeeTier { fee: 146, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 8")]
#[test_case(300, 100, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 9")]
#[test_case(300, 100, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 100 case 10")]
//factor 1000 - positive imabalnce
#[test_case(300, 1000, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 1")]
#[test_case(300, 1000, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (14, vec![FeeTier { fee: 19, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 2")]
#[test_case(300, 1000, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (95, vec![FeeTier { fee: 100, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 3")]
#[test_case(300, 1000, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (130, vec![FeeTier { fee: 135, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 4")]
#[test_case(300, 1000, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 5")]
#[test_case(300, 1000, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 6")]
#[test_case(300, 1000, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 7")]
#[test_case(300, 1000, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 8")]
#[test_case(300, 1000, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 9")]
#[test_case(300, 1000, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-positive-imbalance 1000 case 10")]
/// factor 1 - negative imabalnce
#[test_case(300, 1, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 1")]
#[test_case(300, 1, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (-2, vec![FeeTier { fee: 7, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 2")]
#[test_case(300, 1, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-15, vec![FeeTier { fee: 20, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 3")]
#[test_case(300, 1, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-30, vec![FeeTier { fee: 35, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 4")]
#[test_case(300, 1, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-74, vec![FeeTier { fee: 79, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 5")]
#[test_case(300, 1, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-75, vec![FeeTier { fee: 80, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 6")]
#[test_case(300, 1, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-113, vec![FeeTier { fee: 118, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 7")]
#[test_case(300, 1, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-135, vec![FeeTier { fee: 140, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 8")]
#[test_case(300, 1, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 9")]
#[test_case(300, 1, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1 case 10")]
//factor 10 - negative imabalnce
#[test_case(300, 10, -0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 1")]
#[test_case(300, 10, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (-2, vec![FeeTier { fee: 7, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 2")]
#[test_case(300, 10, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-16, vec![FeeTier { fee: 21, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 3")]
#[test_case(300, 10, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-31, vec![FeeTier { fee: 36, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 4")]
#[test_case(300, 10, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-75, vec![FeeTier { fee: 80, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 5")]
#[test_case(300, 10, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-77, vec![FeeTier { fee: 82, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 6")]
#[test_case(300, 10, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-114, vec![FeeTier { fee: 119, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 7")]
#[test_case(300, 10, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-136, vec![FeeTier { fee: 141, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 8")]
#[test_case(300, 10, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 9")]
#[test_case(300, 10, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 10 case 10")]
//factor 30 - negative imabalnce
#[test_case(300, 30, -0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 1")]
#[test_case(300, 30, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (-2, vec![FeeTier { fee: 7, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 2")]
#[test_case(300, 30, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-17, vec![FeeTier { fee: 22, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 3")]
#[test_case(300, 30, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-34, vec![FeeTier { fee: 39, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 4")]
#[test_case(300, 30, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-79, vec![FeeTier { fee: 84, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 5")]
#[test_case(300, 30, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-81, vec![FeeTier { fee: 86, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 6")]
#[test_case(300, 30, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-117, vec![FeeTier { fee: 122, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 7")]
#[test_case(300, 30, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-137, vec![FeeTier { fee: 142, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 8")]
#[test_case(300, 30, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 9")]
#[test_case(300, 30, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 30 case 10")]
//factor 100 - negative imabalnce
#[test_case(300, 100, -0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 1")]
#[test_case(300, 100, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (-2, vec![FeeTier { fee: 7, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 2")]
#[test_case(300, 100, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-23, vec![FeeTier { fee: 28, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 3")]
#[test_case(300, 100, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-43, vec![FeeTier { fee: 48, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 4")]
#[test_case(300, 100, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-92, vec![FeeTier { fee: 97, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 5")]
#[test_case(300, 100, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-93, vec![FeeTier { fee: 98, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 6")]
#[test_case(300, 100, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-125, vec![FeeTier { fee: 130, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 7")]
#[test_case(300, 100, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-141, vec![FeeTier { fee: 146, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 8")]
#[test_case(300, 100, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 9")]
#[test_case(300, 100, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 100 case 10")]
//factor 1000 - negative imabalnce
#[test_case(300, 1000, -0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 1")]
#[test_case(300, 1000, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (-14, vec![FeeTier { fee: 19, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 2")]
#[test_case(300, 1000, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-95, vec![FeeTier { fee: 100, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 3")]
#[test_case(300, 1000, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-130, vec![FeeTier { fee: 135, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 4")]
#[test_case(300, 1000, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 5")]
#[test_case(300, 1000, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-149, vec![FeeTier { fee: 154, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 6")]
#[test_case(300, 1000, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 7")]
#[test_case(300, 1000, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 8")]
#[test_case(300, 1000, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 9")]
#[test_case(300, 1000, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Logarithmic-sequence-positive-factor-negative-imbalance 1000 case 10")]

/// exponential case:
/// actor 1 positive imabalnce
#[test_case(300, -1, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 1")]
#[test_case(300, -1, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (1, vec![FeeTier { fee: 6, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 2")]
#[test_case(300, -1, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (15, vec![FeeTier { fee: 20, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 3")]
#[test_case(300, -1, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (30, vec![FeeTier { fee: 35, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 4")]
#[test_case(300, -1, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (73, vec![FeeTier { fee: 78, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 5")]
#[test_case(300, -1, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (74, vec![FeeTier { fee: 79, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 6")]
#[test_case(300, -1, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (112, vec![FeeTier { fee: 117, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 7")]
#[test_case(300, -1, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (135, vec![FeeTier { fee: 140, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 8")]
#[test_case(300, -1, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (148, vec![FeeTier { fee: 153, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 9")]
#[test_case(300, -1, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1 case 10")]
//factor 10 positive imabalnce
#[test_case(300, -10, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 1")]
#[test_case(300, -10, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (1, vec![FeeTier { fee: 6, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 2")]
#[test_case(300, -10, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (12, vec![FeeTier { fee: 17, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 3")]
#[test_case(300, -10, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (26, vec![FeeTier { fee: 31, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 4")]
#[test_case(300, -10, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (68, vec![FeeTier { fee: 73, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 5")]
#[test_case(300, -10, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (70, vec![FeeTier { fee: 75, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 6")]
#[test_case(300, -10, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (109, vec![FeeTier { fee: 114, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 7")]
#[test_case(300, -10, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (134, vec![FeeTier { fee: 139, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 8")]
#[test_case(300, -10, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (148, vec![FeeTier { fee: 153, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 9")]
#[test_case(300, -10, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 10 case 10")]
//factor 30 positive imabalnce
#[test_case(300, -30, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 1")]
#[test_case(300, -30, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 2")]
#[test_case(300, -30, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (8, vec![FeeTier { fee: 13, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 3")]
#[test_case(300, -30, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (19, vec![FeeTier { fee: 24, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 4")]
#[test_case(300, -30, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (59, vec![FeeTier { fee: 64, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 5")]
#[test_case(300, -30, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (61, vec![FeeTier { fee: 66, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 6")]
#[test_case(300, -30, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (103, vec![FeeTier { fee: 108, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 7")]
#[test_case(300, -30, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (131, vec![FeeTier { fee: 136, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 8")]
#[test_case(300, -30, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (148, vec![FeeTier { fee: 153, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 9")]
#[test_case(300, -30, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 30 case 9 10")]
//factor 100 positive imabalnce
#[test_case(300, -100, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 1")]
#[test_case(300, -100, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 2")]
#[test_case(300, -100, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (2, vec![FeeTier { fee: 7, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 3")]
#[test_case(300, -100, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (6, vec![FeeTier { fee: 11, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 4")]
#[test_case(300, -100, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (36, vec![FeeTier { fee: 41, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 5")]
#[test_case(300, -100, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (38, vec![FeeTier { fee: 43, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 6")]
#[test_case(300, -100, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (84, vec![FeeTier { fee: 89, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 7")]
#[test_case(300, -100, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (122, vec![FeeTier { fee: 127, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 8")]
#[test_case(300, -100, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (147, vec![FeeTier { fee: 152, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 9")]
#[test_case(300, -100, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 100 case 10")]
//factor 1000 positive imabalnce
#[test_case(300, -1000, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 1")]
#[test_case(300, -1000, 0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 2")]
#[test_case(300, -1000, 0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 3")]
#[test_case(300, -1000, 0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 4")]
#[test_case(300, -1000, 0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 5")]
#[test_case(300, -1000, 0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 6")]
#[test_case(300, -1000, 0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (6, vec![FeeTier { fee: 11, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 7")]
#[test_case(300, -1000, 0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (47, vec![FeeTier { fee: 52, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 8")]
#[test_case(300, -1000, 0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (134, vec![FeeTier { fee: 139, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 9")]
#[test_case(300, -1000, 1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-positive-imbalance 1000 case 10")]
/// factor 1 negative imabalnce
#[test_case(300, -1, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 1")]
#[test_case(300, -1, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (-1, vec![FeeTier { fee: 6, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 2")]
#[test_case(300, -1, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-15, vec![FeeTier { fee: 20, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 3")]
#[test_case(300, -1, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-30, vec![FeeTier { fee: 35, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 4")]
#[test_case(300, -1, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-73, vec![FeeTier { fee: 78, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 5")]
#[test_case(300, -1, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-74, vec![FeeTier { fee: 79, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 6")]
#[test_case(300, -1, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-112, vec![FeeTier { fee: 117, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 7")]
#[test_case(300, -1, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-135, vec![FeeTier { fee: 140, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 8")]
#[test_case(300, -1, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-148, vec![FeeTier { fee: 153, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 9")]
#[test_case(300, -1, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1 case 10")]
//factor 10 negative imabalnce
#[test_case(300, -10, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 1")]
#[test_case(300, -10, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (-1, vec![FeeTier { fee: 6, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 2")]
#[test_case(300, -10, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-12, vec![FeeTier { fee: 17, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 3")]
#[test_case(300, -10, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-26, vec![FeeTier { fee: 31, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 4")]
#[test_case(300, -10, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-68, vec![FeeTier { fee: 73, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 5")]
#[test_case(300, -10, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-70, vec![FeeTier { fee: 75, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 6")]
#[test_case(300, -10, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-109, vec![FeeTier { fee: 114, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 7")]
#[test_case(300, -10, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-134, vec![FeeTier { fee: 139, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 8")]
#[test_case(300, -10, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-148, vec![FeeTier { fee: 153, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 9")]
#[test_case(300, -10, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 10 case 10")]
//factor 30 negative imabalnce
#[test_case(300, -30, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 1")]
#[test_case(300, -30, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 2")]
#[test_case(300, -30, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-8, vec![FeeTier { fee: 13, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 3")]
#[test_case(300, -30, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-19, vec![FeeTier { fee: 24, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 4")]
#[test_case(300, -30, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-59, vec![FeeTier { fee: 64, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 5")]
#[test_case(300, -30, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-61, vec![FeeTier { fee: 66, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 6")]
#[test_case(300, -30, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-103, vec![FeeTier { fee: 108, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 7")]
#[test_case(300, -30, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-131, vec![FeeTier { fee: 136, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 8")]
#[test_case(300, -30, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-148, vec![FeeTier { fee: 153, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 9")]
#[test_case(300, -30, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 30 case 10")]
//factor 100 negative imabalnce
#[test_case(300, -100, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 1")]
#[test_case(300, -100, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 2")]
#[test_case(300, -100, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (-2, vec![FeeTier { fee: 7, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 3")]
#[test_case(300, -100, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (-6, vec![FeeTier { fee: 11, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 4")]
#[test_case(300, -100, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (-36, vec![FeeTier { fee: 41, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 5")]
#[test_case(300, -100, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (-38, vec![FeeTier { fee: 43, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 6")]
#[test_case(300, -100, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-84, vec![FeeTier { fee: 89, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 7")]
#[test_case(300, -100, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-122, vec![FeeTier { fee: 127, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 8")]
#[test_case(300, -100, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-147, vec![FeeTier { fee: 152, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 9")]
#[test_case(300, -100, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 100 case 10")]
// factor 1000 negative imabalnce
#[test_case(300, -1000, 0.0, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 1")]
#[test_case(300, -1000, -0.01, vec![FeeTier { fee: 5, percentage: 100 }] =>  (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 2")]
#[test_case(300, -1000, -0.1, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 3")]
#[test_case(300, -1000, -0.2, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 4")]
#[test_case(300, -1000, -0.49, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 5")]
#[test_case(300, -1000, -0.5, vec![FeeTier { fee: 5, percentage: 100 }] => (0, vec![FeeTier { fee: 5, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 6")]
#[test_case(300, -1000, -0.75, vec![FeeTier { fee: 5, percentage: 100 }] => (-6, vec![FeeTier { fee: 11, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 7")]
#[test_case(300, -1000, -0.9, vec![FeeTier { fee: 5, percentage: 100 }] => (-47, vec![FeeTier { fee: 52, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 8")]
#[test_case(300, -1000, -0.99, vec![FeeTier { fee: 5, percentage: 100 }] => (-134, vec![FeeTier { fee: 139, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 9")]
#[test_case(300, -1000, -1.0, vec![FeeTier { fee: 5, percentage: 100 }] => (-150, vec![FeeTier { fee: 155, percentage: 100 }]); "Exponential-sequence-negative-factor-negative-imbalance 1000 case 10")]

fn test_calculate_dynamic_spread_adjustment_edge_cases(
    dynamic_spread_cap: i32,
    dynamic_spread_factor: i32,
    imbalance_f64: f64,
    fee_tiers: Vec<FeeTier>,
) -> (i64, Vec<FeeTier>) {
    use crate::utils::calculate_dynamic_spread_adjustment;

    println!("=== EDGE CASE TEST ===");
    println!("dynamic_spread_cap: {}", dynamic_spread_cap);
    println!("dynamic_spread_factor: {}", dynamic_spread_factor);
    println!("imbalance_f64: {}", imbalance_f64);
    println!("fee_tiers: {:?}", fee_tiers);

    let result = calculate_dynamic_spread_adjustment(
        dynamic_spread_factor,
        dynamic_spread_cap,
        imbalance_f64,
        fee_tiers,
    );

    println!("Result: {:?}", result);

    // Additional validation that should always hold
    let (tick_adj, modified_fees) = &result;

    // Tick adjustment should be reasonably bounded by spread cap
    assert!(
        tick_adj.abs() <= dynamic_spread_cap as i64,
        "Tick adjustment {} exceeds spread cap {}",
        tick_adj,
        dynamic_spread_cap
    );

    // All fees should be non-negative due to max(0, fee + adj) protection
    for fee_tier in modified_fees {
        assert!(
            fee_tier.fee >= 0,
            "Fee tier fee should never be negative: {}",
            fee_tier.fee
        );
    }

    result
}
