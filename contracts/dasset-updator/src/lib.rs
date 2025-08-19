//! # DAsset Updator Contract
//!
//! This CosmWasm contract replicates the functionality of the Python script
//! for updating vault configurations based on APY data.
//!
//! ## Overview
//!
//! The contract performs the following operations:
//! 1. Queries APY data from an external APY contract for each configured dAsset
//! 2. If APY is zero or too small: only performs dex_withdrawal (skips update and deposit)
//! 3. If APY is non-zero:
//!    - Calculates the optimal fee tier using the formula: `(r*t)/(2*ln(1.0001))`
//!      where `r` is the APY and `t` is the unbonding period in years
//!    - Creates fee tier configurations with custom spacings and percentage distributions
//!    - Updates vault contracts with new fee tiers and oracle price skew
//!    - Performs full sequence: dex_withdrawal, update_config, and dex_deposit for all vaults
//!
//! ## Usage
//!
//! 1. Instantiate the contract with asset configurations and APY contract address
//! 2. Call `RunVaultUpdate{}` to update all configured vaults based on current APY data
//!
//! ## Configuration
//!
//! Each asset is configured with:
//! - `denom`: The asset denomination (e.g., "dATOM")
//! - `core_contract`: The core contract address for APY queries
//! - `unbonding_period`: Unbonding period in days
//! - `fee_spacings`: Fee tier values to add to the calculated base fee
//! - `percentages`: Distribution percentages across fee tiers (must sum to 100)
//! - `vault_address`: The vault contract address to update
//! - `query_period_hours`: Hours for APY calculation period
//! - `fee_dempening_amount`: Amount to add to the calculated base fee
//!
//! ## Fee Tier Calculation
//!
//! The contract calculates a base fee using the APY and unbonding period, then creates
//! fee tiers by adding each value in `fee_spacings` to this calculated base fee.
//! For example, if the calculated base fee is 30 and fee_spacings is [0, 10], 
//! the resulting fee tiers will be [30, 40].

pub mod contract;
pub mod error;
pub mod external_types;
pub mod msg;
pub mod state;
pub mod utils;
