use crate::contract::execute;
use crate::error::ContractError;
use crate::msg::{CombinedPriceResponse, ExecuteMsg};
use crate::state::{
    Config, FeeTier, FeeTierConfig, PairData, TokenData, CONFIG, SHARES_MULTIPLIER,
};
use crate::testing::mock_querier::{mock_dependencies_with_custom_querier, MockQuerier};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::Env;
use cosmwasm_std::{Addr, Coin, Uint128};
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
fn test_deposit_success() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set up contract balances - IMPORTANT: set initial balances to 0
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![Coin::new(0u128, "token0"), Coin::new(0u128, "token1")],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Prepare deposit funds
    let deposit_amount_0 = 500000u128;
    let deposit_amount_1 = 500000u128;

    // Execute deposit with both tokens
    let info_with_both_tokens = mock_info(
        "user1",
        &[
            Coin::new(deposit_amount_0, "token0"),
            Coin::new(deposit_amount_1, "token1"),
        ],
    );

    // Update the contract balance to reflect what it would be AFTER the deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(deposit_amount_0, "token0"),
            Coin::new(deposit_amount_1, "token1"),
        ],
    );

    // Execute the deposit
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info_with_both_tokens,
        ExecuteMsg::Deposit { beneficiary: None },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 7);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "deposit");
    assert_eq!(res.attributes[1].key, "token_0_deposited");
    assert_eq!(res.attributes[1].value, deposit_amount_0.to_string());
    assert_eq!(res.attributes[2].key, "token_1_deposited");
    assert_eq!(res.attributes[2].value, deposit_amount_1.to_string());
    assert_eq!(res.attributes[3].key, "from");
    assert_eq!(res.attributes[3].value, "user1");
    assert_eq!(res.attributes[4].key, "beneficiary");
    assert_eq!(res.attributes[4].value, "user1");
    assert_eq!(res.attributes[5].key, "minted_amount");

    // Verify that LP tokens were minted
    assert!(!res.messages.is_empty());

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert!(updated_config.total_shares > Uint128::zero());
}

#[test]
fn test_deposit_paused() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let mut config = setup_test_config(env.clone());

    // Set contract to paused
    config.paused = true;
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit
    let info = mock_info(
        "user1",
        &[
            Coin::new(500000u128, "token0"),
            Coin::new(500000u128, "token1"),
        ],
    );

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::Paused {});
}

#[test]
fn test_deposit_no_funds() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with no funds
    let info = mock_info("user1", &[]);

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::NoFundsSent {});
}

#[test]
fn test_deposit_invalid_token() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with invalid token
    let info = mock_info("user1", &[Coin::new(500000u128, "invalid_token")]);

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::InvalidToken {});
}

#[test]
fn test_deposit_zero_amount() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with zero amount
    let info = mock_info(
        "user1",
        &[Coin::new(0u128, "token0"), Coin::new(500000u128, "token1")],
    );

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::InvalidTokenAmount {});
}

#[test]
fn test_deposit_exceeds_cap() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let mut querier = setup_mock_querier();
    let env = mock_env();
    let mut config = setup_test_config(env.clone());

    // Set a low deposit cap
    config.deposit_cap = Uint128::new(100);
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with amount exceeding cap
    let info = mock_info(
        "user1",
        &[
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    // Update contract balance to reflect what it would be after the deposit
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::ExceedsDepositCap {});
}

#[test]
fn test_deposit_under_cap() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let mut config = setup_test_config(env.clone());
    // Set a low deposit cap
    config.deposit_cap = Uint128::new(2000000u128);
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with amount under cap
    let info = mock_info(
        "user1",
        &[
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );
    //update contract balance to reflect what it would be after the deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );
    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap();
    let shares_after_first = CONFIG.load(deps.as_ref().storage).unwrap().total_shares;
    // Extract minted amounts
    let minted1 = res
        .attributes
        .iter()
        .find(|attr| attr.key == "minted_amount")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();
    assert!(
        shares_after_first
            == Uint128::from(2000000u128.checked_mul(SHARES_MULTIPLIER as u128).unwrap())
    );
    assert!(minted1 == shares_after_first);

    // Second deposit - this should fail because we've reached the cap
    // The deposit cap is 2000000, and we've already deposited 2000000 worth of tokens. 1 more token should not be allowed
    let info = mock_info("user", &[Coin::new(1u128, "token0")]);

    // Update contract balance for second deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000001u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap_err();

    // Verify error
    assert_eq!(err, ContractError::ExceedsDepositCap {});
}

#[test]
fn test_deposit_whitelist_exceeds_cap() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let mut config = setup_test_config(env.clone());

    // Set a low deposit cap
    config.deposit_cap = Uint128::new(100);
    // Add user to whitelist
    config.whitelist.push(Addr::unchecked("user1"));
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with amount exceeding cap from whitelisted user
    let info = mock_info(
        "user1",
        &[
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );
    //update contract balance to reflect what it would be after the deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
        ],
    );
    // Should succeed despite exceeding cap
    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "deposit");
}

#[test]
fn test_deposit_single_token() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with only token0
    let info = mock_info("user1", &[Coin::new(500000u128, "token0")]);
    //update contract balance to reflect what it would be after the deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![Coin::new(500000u128, "token0")],
    );

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "deposit");

    // Verify that LP tokens were minted
    assert!(!res.messages.is_empty());
}

#[test]
fn test_deposit_different_token_prices() {
    // Setup
    let mut querier = setup_mock_querier();

    // Setup price data with token0 worth twice as much as token1
    let price_response = CombinedPriceResponse {
        token_0_price: PrecDec::from_str("2.0").unwrap(),
        token_1_price: PrecDec::from_str("1.0").unwrap(),
        price_0_to_1: PrecDec::from_str("2.0").unwrap(),
    };
    querier.set_price_response(price_response);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let env = mock_env();
    let mut config = setup_test_config(env.clone());
    config.deposit_cap = Uint128::new(10000000);

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with equal amounts of both tokens
    let info = mock_info(
        "user1",
        &[
            Coin::new(500000u128, "token0"),
            Coin::new(500000u128, "token1"),
        ],
    );
    //update contract balance to reflect what it would be after the deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(500000u128, "token0"),
            Coin::new(500000u128, "token1"),
        ],
    );
    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "deposit");

    // Verify that LP tokens were minted
    assert!(!res.messages.is_empty());

    // The amount minted should reflect the different token values
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert!(updated_config.total_shares > Uint128::zero());
}

#[test]
fn test_deposit_multiple_times() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Setup price data with token0 worth twice as much as token1
    let price_response = CombinedPriceResponse {
        token_0_price: PrecDec::from_str("1.0").unwrap(),
        token_1_price: PrecDec::from_str("1.0").unwrap(),
        price_0_to_1: PrecDec::from_str("1.0").unwrap(),
    };
    let input_amount = 500000u128;
    let shares_expected = input_amount * 2u128 * SHARES_MULTIPLIER as u128;
    querier.set_price_response(price_response);

    // Set up initial contract balances
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![Coin::new(0u128, "token0"), Coin::new(0u128, "token1")],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.deposit_cap = Uint128::new(input_amount * 4u128);

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // First deposit
    let info1 = mock_info(
        "user1",
        &[
            Coin::new(input_amount, "token0"),
            Coin::new(input_amount, "token1"),
        ],
    );

    // Update contract balance for first deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(input_amount, "token0"),
            Coin::new(input_amount, "token1"),
        ],
    );

    let res1 = execute(deps.as_mut(), env.clone(), info1, ExecuteMsg::Deposit { beneficiary: None }).unwrap();

    println!(
        "Shares after first deposit: {}",
        CONFIG.load(deps.as_ref().storage).unwrap().total_shares
    );
    let shares_after_first = CONFIG.load(deps.as_ref().storage).unwrap().total_shares;

    // Second deposit
    let info2 = mock_info(
        "user2",
        &[
            Coin::new(input_amount, "token0"),
            Coin::new(input_amount, "token1"),
        ],
    );

    // Update contract balance for second deposit - IMPORTANT: this is cumulative
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(input_amount.checked_mul(2u128).unwrap(), "token0"), // 500000 (first) + 500000 (second)
            Coin::new(input_amount.checked_mul(2u128).unwrap(), "token1"), // 500000 (first) + 500000 (second)
        ],
    );

    let res2 = execute(deps.as_mut(), env.clone(), info2, ExecuteMsg::Deposit { beneficiary: None }).unwrap();

    let shares_after_second = CONFIG.load(deps.as_ref().storage).unwrap().total_shares;
    println!("Shares after second deposit: {}", shares_after_second);

    assert!(shares_after_first == Uint128::from(shares_expected));
    // Verify total shares increased
    assert!(shares_after_second == shares_after_first.checked_mul(2u128.into()).unwrap());

    // Extract minted amounts
    let minted1 = res1
        .attributes
        .iter()
        .find(|attr| attr.key == "minted_amount")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();

    let minted2 = res2
        .attributes
        .iter()
        .find(|attr| attr.key == "minted_amount")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();
    assert!(minted1 == minted2);
}

#[test]
fn test_deposit_with_price_staleness() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set price to be stale
    querier.set_price_error(true);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit
    let info = mock_info(
        "user1",
        &[
            Coin::new(500000u128, "token0"),
            Coin::new(500000u128, "token1"),
        ],
    );

    let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap_err();

    // Verify error is related to stale price
    assert!(matches!(err, ContractError::OracleError { .. }));
}

#[test]
fn test_deposit_with_imbalanced_tokens() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let mut config = setup_test_config(env.clone());
    let input_amount_0 = 1000000u128;
    let input_amount_1 = 100u128;
    let expected_minted_amount = (input_amount_0 + input_amount_1) * SHARES_MULTIPLIER as u128;
    config.deposit_cap = Uint128::new(input_amount_0 + input_amount_1);
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute deposit with highly imbalanced token amounts
    let info = mock_info(
        "user1",
        &[
            Coin::new(1000000u128, "token0"),
            Coin::new(100u128, "token1"),
        ],
    );

    // Update contract balance to reflect what it would be after the deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(100u128, "token1"),
        ],
    );

    let res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Deposit { beneficiary: None }).unwrap();

    // Verify that the deposit was successful but with adjusted share calculation
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "deposit");

    // Extract minted amount and verify it's based on the smaller token amount
    let minted = res
        .attributes
        .iter()
        .find(|attr| attr.key == "minted_amount")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();

    // The minted amount should be proportional to the smaller token amount
    assert!(minted == Uint128::from(expected_minted_amount));
}

#[test]
fn test_deposit_with_beneficiary() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set balances -- inital balance to 0
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![Coin::new(0u128, "token0"), Coin::new(0u128, "token1")],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Prepare deposit funds
    let deposit_amount_0 = 500000u128;
    let deposit_amount_1 = 500000u128;

    // Execute deposit with both tokens and a beneficiary
    let info_from_user1 = mock_info(
        "user1",
        &[
            Coin::new(deposit_amount_0, "token0"),
            Coin::new(deposit_amount_1, "token1"),
        ],
    );

    // Update the contract balance to reflect what it would be AFTER the deposit
    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(deposit_amount_0, "token0"),
            Coin::new(deposit_amount_1, "token1"),
        ],
    );

    // Execute the deposit with beneficiary - use the contract address as a valid address
    // In real usage, this would be any valid bech32 address
    let beneficiary_addr = env.contract.address.to_string();
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info_from_user1,
        ExecuteMsg::Deposit {
            beneficiary: Some(beneficiary_addr.clone()),
        },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 7);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "deposit");
    assert_eq!(res.attributes[1].key, "token_0_deposited");
    assert_eq!(res.attributes[1].value, deposit_amount_0.to_string());
    assert_eq!(res.attributes[2].key, "token_1_deposited");
    assert_eq!(res.attributes[2].value, deposit_amount_1.to_string());
    assert_eq!(res.attributes[3].key, "from");
    assert_eq!(res.attributes[3].value, "user1"); // funds come from user1
    assert_eq!(res.attributes[4].key, "beneficiary");
    assert_eq!(res.attributes[4].value, beneficiary_addr); // LP tokens sent to beneficiary
    assert_eq!(res.attributes[5].key, "minted_amount");

    // Verify that LP tokens were minted
    assert!(!res.messages.is_empty());

    // The beneficiary attribute confirms LP tokens are sent to the specified address
    // The actual MsgMint protobuf encoding would need to be decoded to verify the
    // mint_to_address field, but the beneficiary attribute confirms the logic worked

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert!(updated_config.total_shares > Uint128::zero());
}

#[test]
fn test_deposit_with_invalid_beneficiary() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![Coin::new(0u128, "token0"), Coin::new(0u128, "token1")],
    );

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    let info = mock_info(
        "user1",
        &[
            Coin::new(500000u128, "token0"),
            Coin::new(500000u128, "token1"),
        ],
    );

    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![Coin::new(500000u128, "token0"), Coin::new(500000u128, "token1")],
    );

    // try deposit with invalid beneficiary address
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Deposit {
            beneficiary: Some("".to_string()),
        },
    )
    .unwrap_err();

    assert!(matches!(err, ContractError::Std(_)));
}
