
use crate::utils::{normalize_price};
use crate::error::ContractError;
use cosmwasm_std::{Int128};
use neutron_std::types::neutron::util::precdec::PrecDec;
use test_case::test_case;

#[test_case(Int128::new(1234567), 6 => Ok(PrecDec::from_str("1.234567").unwrap()); "positive number with 6 decimals")]
#[test_case(Int128::new(1234567), 2 => Ok(PrecDec::from_str("12345.67").unwrap()); "positive number with 2 decimals")]
#[test_case(Int128::new(1234567), 0 => Ok(PrecDec::from_str("1234567").unwrap()); "positive number with 0 decimals")]
#[test_case(Int128::new(1234567890098764321), 12 => Ok(PrecDec::from_str("1234567.890098764321").unwrap()); "large positive number")]
#[test_case(Int128::zero(), 6 => Ok(PrecDec::zero()); "zero")]
#[test_case(Int128::new(-1234567), 6 => Err(ContractError::PriceIsNegative); "negative number")]
fn test_normalize_price(
    input_price: Int128,
    input_decimals: u64,
) -> Result<PrecDec, ContractError> {
    normalize_price(input_price, input_decimals)
}
