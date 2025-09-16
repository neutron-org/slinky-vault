//! # Cron Runner Contract
//!
//! This is intended to be called by the neutron Cron module
//! to perform rebalancing of multiple vault contracts.
//!
//! ## Overview
//!
//! The contract performs the following operations:
//! 1. Takes a list of vault addresses
//! 2. Calls `dex_withdrawal` on each vault to withdraw all active DEX positions
//! 3. Calls `dex_deposit` on each vault to redeploy liquidity
//!
//! ## Authorization
//!
//! The contract supports two types of authorized addresses:
//! - **Cron Module**: Can execute the main rebalancing function
//! - **Admin**: Can update configuration and manage the contract
//!

pub mod contract;
pub mod error;
pub mod msg;
pub mod state;
pub mod utils;
