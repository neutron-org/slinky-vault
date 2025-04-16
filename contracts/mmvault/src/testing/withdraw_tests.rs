use crate::contract::execute;
use crate::error::ContractError;
use crate::msg::{CombinedPriceResponse, ExecuteMsg};
use crate::state::{
    Config, FeeTier, FeeTierConfig, PairData, TokenData, CONFIG, SHARES_MULTIPLIER,
};
use crate::testing::mock_querier::{mock_dependencies_with_custom_querier, MockQuerier};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::Env;
use cosmwasm_std::{Addr, Binary, Coin, Uint128};
use neutron_std::types::neutron::dex::DepositRecord;
use neutron_std::types::neutron::dex::PairId;
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
        total_shares: Uint128::new(1000000 * SHARES_MULTIPLIER as u128), // Initial shares
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
fn test_withdraw_success_no_active_deposits() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance
    let contract_balance_0 = 1000000u128;
    let contract_balance_1 = 1000000u128;
    let contract_balance_lp = (contract_balance_0 + contract_balance_1)
        .checked_mul(SHARES_MULTIPLIER as u128)
        .unwrap();

    // Set up the mock querier with the LP token supply
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(contract_balance_0, "token0"),
            Coin::new(contract_balance_1, "token1"),
            Coin::new(contract_balance_lp, "factory/contract/lp"),
        ],
    );

    // Add LP token supply to the querier
    querier.set_supply("factory/contract/lp", contract_balance_lp);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.total_shares = Uint128::from(contract_balance_lp);

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute withdraw
    let withdraw_amount = contract_balance_lp; // 100% of total shares
    let info = mock_info(
        "user1",
        &[Coin::new(withdraw_amount, "factory/contract/lp")],
    );

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Withdraw {
            amount: Uint128::from(withdraw_amount),
        },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 5);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "withdrawal");

    // Verify that tokens were sent to user
    assert_eq!(res.messages.len(), 3);

    // Check first message is a burn message for LP tokens
    match &res.messages[0].msg {
        cosmwasm_std::CosmosMsg::Any(any_msg) => {
            assert_eq!(any_msg.type_url, "/osmosis.tokenfactory.v1beta1.MsgBurn");
        }
        _ => panic!("Expected Any message with MsgBurn type_url"),
    }

    // Check second message is a bank send of token0
    match &res.messages[1].msg {
        cosmwasm_std::CosmosMsg::Bank(bank_msg) => match bank_msg {
            cosmwasm_std::BankMsg::Send { to_address, amount } => {
                assert_eq!(to_address, "user1");
                assert_eq!(amount.len(), 1);
                assert_eq!(amount[0].denom, "token0");
                assert_eq!(amount[0].amount, Uint128::new(contract_balance_0));
            }
            _ => panic!("Expected BankMsg::Send"),
        },
        _ => panic!("Expected Bank message"),
    }

    // Check third message is a bank send of token1
    match &res.messages[2].msg {
        cosmwasm_std::CosmosMsg::Bank(bank_msg) => match bank_msg {
            cosmwasm_std::BankMsg::Send { to_address, amount } => {
                assert_eq!(to_address, "user1");
                assert_eq!(amount.len(), 1);
                assert_eq!(amount[0].denom, "token1");
                assert_eq!(amount[0].amount, Uint128::new(contract_balance_1));
            }
            _ => panic!("Expected BankMsg::Send"),
        },
        _ => panic!("Expected Bank message"),
    }

    // Verify config was updated
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        updated_config.total_shares,
        config.total_shares - Uint128::from(withdraw_amount)
    );
}

#[test]
fn test_withdraw_with_active_deposits() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(1000000u128, "token0"),
            Coin::new(1000000u128, "token1"),
            Coin::new(1000000u128, "factory/contract/lp"),
        ],
    );

    // Setup active deposits
    let deposits = vec![
        DepositRecord {
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
        },
        DepositRecord {
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
        },
    ];
    querier.set_user_deposits_all_response(deposits);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute withdraw
    let withdraw_amount = Uint128::new(500000 * SHARES_MULTIPLIER as u128);
    let info = mock_info(
        "user1",
        &[Coin::new(withdraw_amount, "factory/contract/lp")],
    );

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Withdraw {
            amount: withdraw_amount,
        },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 1);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "withdrawal");

    // Verify that submessages were created for withdrawal
    assert_eq!(res.messages.len(), 2);

    // Verify both messages are MsgWithdrawal
    for msg in &res.messages {
        match &msg.msg {
            cosmwasm_std::CosmosMsg::Any(any_msg) => {
                assert_eq!(any_msg.type_url, "/neutron.dex.MsgWithdrawal");
            }
            _ => panic!("Expected Any message with MsgWithdrawal type_url"),
        }
    }

    // Verify first message has reply_on: Never
    assert_eq!(res.messages[0].reply_on, cosmwasm_std::ReplyOn::Never);

    // Verify second message has reply_on: Success
    assert_eq!(res.messages[1].reply_on, cosmwasm_std::ReplyOn::Success);
}

#[test]
fn test_withdraw_more_than_total_shares() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let mut config = setup_test_config(env.clone());
    config.total_shares = Uint128::new(1000000 * SHARES_MULTIPLIER as u128);
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute withdraw with amount exceeding total shares
    let withdraw_amount = config.total_shares + Uint128::new(1);
    let info = mock_info(
        "user1",
        &[Coin::new(withdraw_amount, "factory/contract/lp")],
    );

    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Withdraw {
            amount: withdraw_amount,
        },
    )
    .unwrap_err();

    // Verify error (should be an overflow error when subtracting from total_shares)
    assert_eq!(err, ContractError::InvalidWithdrawAmount);
}

#[test]
fn test_withdraw_zero_amount() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute withdraw with zero amount
    let withdraw_amount = Uint128::zero();
    let info = mock_info(
        "user1",
        &[Coin::new(withdraw_amount, "factory/contract/lp")],
    );

    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Withdraw {
            amount: withdraw_amount,
        },
    )
    .unwrap_err();

    // Should succeed but not do anything meaningful
    assert_eq!(err, ContractError::ZeroBurnAmount);
}

#[test]
fn test_withdraw_proportional_amounts() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance with uneven amounts
    let contract_balance_0 = 20000000u128;
    let contract_balance_1 = 10000000u128;
    let contract_balance_lp = 30000000u128 * SHARES_MULTIPLIER as u128;

    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(contract_balance_0, "token0"),
            Coin::new(contract_balance_1, "token1"),
            Coin::new(contract_balance_lp, "factory/contract/lp"),
        ],
    );

    // Add LP token supply to the querier
    querier.set_supply("factory/contract/lp", contract_balance_lp);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.total_shares = Uint128::from(contract_balance_lp);

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute withdraw of 50% of shares
    let withdraw_amount = contract_balance_lp.checked_div(3u128).unwrap();
    let info = mock_info(
        "user1",
        &[Coin::new(withdraw_amount, "factory/contract/lp")],
    );

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Withdraw {
            amount: Uint128::from(withdraw_amount),
        },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "withdrawal");

    // Extract withdrawn amounts
    let withdraw_amount_0 = res
        .attributes
        .iter()
        .find(|attr| attr.key == "withdraw_amount_0")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();

    let withdraw_amount_1 = res
        .attributes
        .iter()
        .find(|attr| attr.key == "withdraw_amount_1")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();

    // Verify proportional withdrawal (50% of each token)
    assert_eq!(withdraw_amount_0, Uint128::new(contract_balance_0 / 3));
    assert_eq!(withdraw_amount_1, Uint128::new(contract_balance_1 / 3));
}

#[test]
fn test_withdraw_different_token_prices() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Setup price data with token0 worth twice as much as token1
    let price_response = CombinedPriceResponse {
        token_0_price: PrecDec::from_str("2.0").unwrap(),
        token_1_price: PrecDec::from_str("1.0").unwrap(),
        price_0_to_1: PrecDec::from_str("2.0").unwrap(),
    };
    querier.set_price_response(price_response);

    // Set contract balance
    let contract_balance_0 = 500000u128;
    let contract_balance_1 = 1000000u128;
    let contract_balance_lp = 1000000u128 * SHARES_MULTIPLIER as u128;

    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(contract_balance_0, "token0"),
            Coin::new(contract_balance_1, "token1"),
            Coin::new(contract_balance_lp, "factory/contract/lp"),
        ],
    );

    // Add LP token supply to the querier
    querier.set_supply("factory/contract/lp", contract_balance_lp);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.total_shares = Uint128::from(contract_balance_lp);

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Execute withdraw
    let withdraw_amount = Uint128::new(500000 * SHARES_MULTIPLIER as u128); // 50% of total shares
    let info = mock_info(
        "user1",
        &[Coin::new(withdraw_amount.u128(), "factory/contract/lp")],
    );

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Withdraw {
            amount: withdraw_amount,
        },
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "withdrawal");

    // Extract withdrawn amounts
    let withdraw_amount_0 = res
        .attributes
        .iter()
        .find(|attr| attr.key == "withdraw_amount_0")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();

    let withdraw_amount_1 = res
        .attributes
        .iter()
        .find(|attr| attr.key == "withdraw_amount_1")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();

    // Verify proportional withdrawal (50% of each token)
    assert_eq!(withdraw_amount_0, Uint128::new(contract_balance_0 / 2));
    assert_eq!(withdraw_amount_1, Uint128::new(contract_balance_1 / 2));
}

#[test]
fn test_multiple_withdrawals() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance
    let initial_balance = 1000000u128;
    let initial_lp_balance = initial_balance * SHARES_MULTIPLIER as u128;

    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(initial_balance, "token0"),
            Coin::new(initial_balance, "token1"),
            Coin::new(initial_lp_balance, "factory/contract/lp"),
        ],
    );

    // Add LP token supply to the querier
    querier.set_supply("factory/contract/lp", initial_lp_balance);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.total_shares = Uint128::from(initial_lp_balance);
    let initial_shares = config.total_shares;

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // First withdrawal (25%)
    let withdraw_amount_1 = initial_shares / Uint128::new(4); // 25% of total shares
    let info = mock_info(
        "user1",
        &[Coin::new(withdraw_amount_1.u128(), "factory/contract/lp")],
    );

    let res1 = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::Withdraw {
            amount: withdraw_amount_1,
        },
    )
    .unwrap();

    // Update contract balance after first withdrawal
    let remaining_balance = initial_balance * 3 / 4; // 75% remaining
    let remaining_lp_balance = initial_lp_balance * 3 / 4;

    deps.querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(remaining_balance, "token0"),
            Coin::new(remaining_balance, "token1"),
            Coin::new(remaining_lp_balance, "factory/contract/lp"),
        ],
    );

    // Update LP token supply
    deps.querier
        .set_supply("factory/contract/lp", remaining_lp_balance);

    // Second withdrawal (another 25% of original)
    let withdraw_amount_2 = initial_shares / Uint128::new(4);
    let info2 = mock_info(
        "user1",
        &[Coin::new(withdraw_amount_2.u128(), "factory/contract/lp")],
    );

    let res2 = execute(
        deps.as_mut(),
        env.clone(),
        info2,
        ExecuteMsg::Withdraw {
            amount: withdraw_amount_2,
        },
    )
    .unwrap();

    // Verify config was updated correctly
    let updated_config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        updated_config.total_shares,
        initial_shares - withdraw_amount_1 - withdraw_amount_2
    );

    // Extract withdrawn amounts from first withdrawal
    let withdraw_amount_0_first = res1
        .attributes
        .iter()
        .find(|attr| attr.key == "withdraw_amount_0")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();

    // Extract withdrawn amounts from second withdrawal
    let withdraw_amount_0_second = res2
        .attributes
        .iter()
        .find(|attr| attr.key == "withdraw_amount_0")
        .map(|attr| Uint128::from_str(&attr.value).unwrap())
        .unwrap();

    // Both withdrawals should withdraw the same amount
    assert_eq!(withdraw_amount_0_first, withdraw_amount_0_second);
    assert_eq!(withdraw_amount_0_first, Uint128::new(initial_balance / 4));
}

#[test]
fn test_withdrawal_reply_handler_partial_withdrawal() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance after withdrawal from DEX
    let contract_balance_0 = 1000000u128;
    let contract_balance_1 = 1000000u128;
    let contract_balance_lp = (contract_balance_0 + contract_balance_1) * SHARES_MULTIPLIER as u128;
    let withdrawal_amount = contract_balance_lp / 2;
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(contract_balance_0, "token0"),
            Coin::new(contract_balance_1, "token1"),
            Coin::new(withdrawal_amount, "factory/contract/lp"),
        ],
    );

    // Add LP token supply to the querier
    querier.set_supply("factory/contract/lp", contract_balance_lp);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.total_shares = Uint128::from(contract_balance_lp);

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create a mock SubMsgResult for the withdrawal reply
    let beneficiary = "user1".to_string();

    // Create a mock successful result
    let msg_result = cosmwasm_std::SubMsgResult::Ok(cosmwasm_std::SubMsgResponse {
        events: vec![],
        data: None,
        msg_responses: vec![cosmwasm_std::MsgResponse {
            type_url: "/neutron.dex.MsgWithdrawalResponse".to_string(),
            value: Binary::from(vec![]), // Empty binary since we're not using the actual response data
        }],
    });

    // Call the withdrawal reply handler
    let res = crate::execute::handle_withdrawal_reply(
        deps.as_mut(),
        env.clone(),
        msg_result,
        Uint128::from(withdrawal_amount),
        beneficiary,
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "withdrawal_reply_success");
    assert_eq!(res.attributes[1].key, "next_action");
    assert_eq!(res.attributes[1].value, "create_new_deposit");

    // Verify messages
    // First set of messages should be for withdrawal (burn LP tokens and send tokens to user)
    // Second set of messages should be for creating new deposits

    // Check that we have the right number of messages
    // We should have at least 3 messages:
    // 1. Burn LP tokens
    // 2. Send token0 to user
    // 3. Send token1 to user
    // Plus deposit messages based on fee tiers
    let expected_deposit_count: usize = config.fee_tier_config.fee_tiers.len();
    let expected_message_count = 3 + expected_deposit_count;
    assert!(res.messages.len() == expected_message_count);

    // Check first message is a burn message for LP tokens
    match &res.messages[0].msg {
        cosmwasm_std::CosmosMsg::Any(any_msg) => {
            assert_eq!(any_msg.type_url, "/osmosis.tokenfactory.v1beta1.MsgBurn");
        }
        _ => panic!("Expected Any message with MsgBurn type_url"),
    }

    // Check second message is a bank send of token0
    match &res.messages[1].msg {
        cosmwasm_std::CosmosMsg::Bank(bank_msg) => match bank_msg {
            cosmwasm_std::BankMsg::Send { to_address, amount } => {
                assert_eq!(to_address, "user1");
                assert_eq!(amount.len(), 1);
                assert_eq!(amount[0].denom, "token0");
                assert_eq!(amount[0].amount, Uint128::new(contract_balance_0 / 2));
            }
            _ => panic!("Expected BankMsg::Send"),
        },
        _ => panic!("Expected Bank message"),
    }

    // Check third message is a bank send of token1
    match &res.messages[2].msg {
        cosmwasm_std::CosmosMsg::Bank(bank_msg) => match bank_msg {
            cosmwasm_std::BankMsg::Send { to_address, amount } => {
                assert_eq!(to_address, "user1");
                assert_eq!(amount.len(), 1);
                assert_eq!(amount[0].denom, "token1");
                assert_eq!(amount[0].amount, Uint128::new(contract_balance_1 / 2));
            }
            _ => panic!("Expected BankMsg::Send"),
        },
        _ => panic!("Expected Bank message"),
    }

    // Check that the remaining messages are deposit messages
    for i in 3..res.messages.len() {
        match &res.messages[i].msg {
            cosmwasm_std::CosmosMsg::Any(any_msg) => {
                assert_eq!(any_msg.type_url, "/neutron.dex.MsgDeposit");
            }
            _ => panic!("Expected Any message with MsgDeposit type_url"),
        }
    }

    // Verify we have the right number of deposit messages based on fee tiers
    // We should have one deposit message per fee tier that has a non-zero percentage
    let expected_deposit_msgs = config
        .fee_tier_config
        .fee_tiers
        .iter()
        .filter(|tier| tier.percentage > 0)
        .count();

    assert_eq!(res.messages.len() - 3, expected_deposit_msgs);
}

#[test]
fn test_withdrawal_reply_handler_full_withdrawal() {
    // Setup
    let mut querier = setup_mock_querier();
    let env = mock_env();

    // Set contract balance after withdrawal from DEX
    let contract_balance_0 = 1000000u128;
    let contract_balance_1 = 1000000u128;
    let contract_balance_lp = (contract_balance_0 + contract_balance_1) * SHARES_MULTIPLIER as u128;
    let withdrawal_amount = contract_balance_lp;
    querier.set_contract_balance(
        env.contract.address.as_ref(),
        vec![
            Coin::new(contract_balance_0, "token0"),
            Coin::new(contract_balance_1, "token1"),
            Coin::new(withdrawal_amount, "factory/contract/lp"),
        ],
    );

    // Add LP token supply to the querier
    querier.set_supply("factory/contract/lp", contract_balance_lp);

    let mut deps = mock_dependencies_with_custom_querier(querier);
    let mut config = setup_test_config(env.clone());
    config.total_shares = Uint128::from(contract_balance_lp);

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create a mock SubMsgResult for the withdrawal reply
    let beneficiary = "user1".to_string();

    // Create a mock successful result
    let msg_result = cosmwasm_std::SubMsgResult::Ok(cosmwasm_std::SubMsgResponse {
        events: vec![],
        data: None,
        msg_responses: vec![cosmwasm_std::MsgResponse {
            type_url: "/neutron.dex.MsgWithdrawalResponse".to_string(),
            value: Binary::from(vec![]), // Empty binary since we're not using the actual response data
        }],
    });

    // Call the withdrawal reply handler
    let res = crate::execute::handle_withdrawal_reply(
        deps.as_mut(),
        env.clone(),
        msg_result,
        Uint128::from(withdrawal_amount),
        beneficiary,
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "withdrawal_reply_success");
    assert_eq!(res.attributes[1].key, "next_action");
    assert_eq!(res.attributes[1].value, "create_new_deposit");

    // Verify messages
    // We should have exactly 3 messages:
    // 1. Burn LP tokens
    // 2. Send token0 to user
    // 3. Send token1 to user
    // No deposit messages should be present
    assert_eq!(res.messages.len(), 3);

    // Check first message is a burn message for LP tokens
    match &res.messages[0].msg {
        cosmwasm_std::CosmosMsg::Any(any_msg) => {
            assert_eq!(any_msg.type_url, "/osmosis.tokenfactory.v1beta1.MsgBurn");
        }
        _ => panic!("Expected Any message with MsgBurn type_url"),
    }

    // Check second message is a bank send of token0
    match &res.messages[1].msg {
        cosmwasm_std::CosmosMsg::Bank(bank_msg) => match bank_msg {
            cosmwasm_std::BankMsg::Send { to_address, amount } => {
                assert_eq!(to_address, "user1");
                assert_eq!(amount.len(), 1);
                assert_eq!(amount[0].denom, "token0");
                assert_eq!(amount[0].amount, Uint128::new(contract_balance_0));
            }
            _ => panic!("Expected BankMsg::Send"),
        },
        _ => panic!("Expected Bank message"),
    }

    // Check third message is a bank send of token1
    match &res.messages[2].msg {
        cosmwasm_std::CosmosMsg::Bank(bank_msg) => match bank_msg {
            cosmwasm_std::BankMsg::Send { to_address, amount } => {
                assert_eq!(to_address, "user1");
                assert_eq!(amount.len(), 1);
                assert_eq!(amount[0].denom, "token1");
                assert_eq!(amount[0].amount, Uint128::new(contract_balance_1));
            }
            _ => panic!("Expected BankMsg::Send"),
        },
        _ => panic!("Expected Bank message"),
    }

    // Verify there are no deposit messages
    for msg in &res.messages {
        if let cosmwasm_std::CosmosMsg::Any(any_msg) = &msg.msg {
            assert_ne!(
                any_msg.type_url, "/neutron.dex.MsgDeposit",
                "Should not have any deposit messages"
            );
        }
    }
}

#[test]
fn test_withdrawal_reply_handler_error() {
    // Setup
    let mut deps = mock_dependencies_with_custom_querier(setup_mock_querier());
    let env = mock_env();
    let config = setup_test_config(env.clone());

    // Store config
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    // Create a mock error result
    let msg_result = cosmwasm_std::SubMsgResult::Err("Withdrawal failed".to_string());

    // Call the withdrawal reply handler
    let res = crate::execute::handle_withdrawal_reply(
        deps.as_mut(),
        env.clone(),
        msg_result,
        Uint128::new(500000),
        "user1".to_string(),
    )
    .unwrap();

    // Verify response
    assert_eq!(res.attributes.len(), 2);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "withdrawal_reply_error");
    assert_eq!(res.attributes[1].key, "error");
    assert_eq!(res.attributes[1].value, "Withdrawal failed");

    // Verify no messages were created
    assert_eq!(res.messages.len(), 0);
}
