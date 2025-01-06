use crate::error::ContractError;
use crate::msg::{CombinedPriceResponse, WithdrawPayload};
use crate::state::{CONFIG, CREATE_TOKEN_REPLY_ID, DEX_WITHDRAW_REPLY_ID, WITHDRAW_REPLY_ID};
use crate::utils::*;
use cosmwasm_std::{
    BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, SubMsg, SubMsgResult, Uint128,
};
use neutron_std::types::neutron::dex::{DexQuerier, MsgWithdrawal, QueryAllUserDepositsResponse};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::osmosis::tokenfactory::v1beta1::{MsgBurn, MsgCreateDenom, MsgMint};
use prost::Message;

pub fn deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let mut messages: Vec<CosmosMsg> = vec![];
    // Load the contract configuration from storage
    let mut config = CONFIG.load(deps.storage)?;

    // Extract the sent funds from the transaction info
    let sent_funds = info.funds;

    // If no funds are sent, return an error
    if sent_funds.is_empty() {
        return Err(ContractError::NoFundsSent {});
    }
    let mut token0_deposited = Uint128::zero();
    let mut token1_deposited = Uint128::zero();
    // Iterate through the sent funds if the denoms match the expected vault denom, and are greater than zero we add them to the config balances.
    for coin in sent_funds.iter() {
        if coin.denom == config.balances.token_0.denom {
            if coin.amount == Uint128::zero() {
                return Err(ContractError::InvalidTokenAmount);
            }
            token0_deposited += coin.amount;
            config.balances.token_0.amount += coin.amount;
        } else if coin.denom == config.balances.token_1.denom {
            if coin.amount == Uint128::zero() {
                return Err(ContractError::InvalidTokenAmount);
            }
            token1_deposited += coin.amount;
            config.balances.token_1.amount += coin.amount;
        } else {
            // Return an error if an unsupported token is sent
            return Err(ContractError::InvalidToken);
        }
    }

    let amount_to_mint = get_mint_amount(
        env.clone(),
        &deps,
        config.clone(),
        token0_deposited,
        token1_deposited,
    )?;
    // Mint LP tokens
    let mint_msg = MsgMint {
        sender: env.contract.address.to_string(),
        amount: Some(
            Coin {
                denom: config.lp_denom.clone(),
                amount: amount_to_mint,
            }
            .into(),
        ),
        mint_to_address: info.sender.to_string(),
    };
    messages.push(mint_msg.into());
    // Save the updated configuration with new balances back to the contract's storage
    CONFIG.save(deps.storage, &config)?;

    // Return a success response with updated balances
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "deposit")
        .add_attribute("from", info.sender.to_string())
        .add_attribute("token_0_amount", config.balances.token_0.amount.to_string())
        .add_attribute("token_1_amount", config.balances.token_1.amount.to_string()))
}

pub fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // Load the contract configuration to access the owner address and balances
    let config = CONFIG.load(deps.storage)?;

    // Query the caller's LP token balance
    let lp_balance = deps
        .querier
        .query_balance(info.sender.clone(), config.lp_denom.clone())?;

    // Check if the user has enough LP tokens
    if lp_balance.amount < amount {
        return Err(ContractError::InsufficientFundsForWithdrawal {});
    }

    let payload = WithdrawPayload {
        sender: info.sender.to_string(),
        amount: amount.to_string(),
    };

    let dex_querier = DexQuerier::new(&deps.querier);
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(env.contract.address.to_string(), None, true)?;

    // Add check for empty deposits
    if res.deposits.is_empty() {
        // perform withdrawal
    }

    let mut messages: Vec<SubMsg> = vec![];

    // Add all withdrawals except the last one without reply
    for deposit in res.deposits.iter().take(res.deposits.len() - 1) {
        let withdraw_msg = Into::<CosmosMsg>::into(MsgWithdrawal {
            creator: env.contract.address.to_string(),
            receiver: env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            shares_to_remove: vec![deposit.shares_owned.parse().expect("Failed to parse")],
            tick_indexes_a_to_b: vec![deposit.center_tick_index],
            fees: vec![deposit.fee],
        });
        messages.push(SubMsg::new(withdraw_msg));
    }

    // Add the last withdrawal with reply
    if let Some(last_deposit) = res.deposits.last() {
        let withdraw_msg = Into::<CosmosMsg>::into(MsgWithdrawal {
            creator: env.contract.address.to_string(),
            receiver: env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            shares_to_remove: vec![last_deposit.shares_owned.parse().expect("Failed to parse")],
            tick_indexes_a_to_b: vec![last_deposit.center_tick_index],
            fees: vec![last_deposit.fee],
        });
        messages.push(
            SubMsg::reply_on_success(withdraw_msg, WITHDRAW_REPLY_ID)
                .with_payload(payload.encode_to_vec()),
        );
    }

    // Add the message to the response and return
    Ok(Response::new()
        .add_submessages(messages)
        .add_attribute("action", "withdrawal"))
}

pub fn dex_deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration
    let config = CONFIG.load(deps.storage)?;

    // Verify that the sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // get the current slinky price and tick index
    let prices: crate::msg::CombinedPriceResponse = get_prices(deps.as_ref(), env.clone())?;
    let tick_index = price_to_tick_index(prices.price_0_to_1)?;

    // Save the updated config directly to storage
    CONFIG.save(deps.storage, &config)?;

    let messages = get_deposit_messages(&env, config.clone(), tick_index, prices)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "dex_deposit"))
}

pub fn dex_withdrawal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // Load the contract configuration to access the owner address and balances
    let config = CONFIG.load(deps.storage)?;

    // Verify that the sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Prepare a vector to hold withdrawals
    let mut messages: Vec<SubMsg> = vec![];
    // Check if there are any active deposits
    let dex_querier = DexQuerier::new(&deps.querier);
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(env.contract.address.to_string(), None, true)?;

    // If there are any active deposits, withdraw all of them
    for deposit in res.deposits.iter() {
        let withdraw_msg = Into::<CosmosMsg>::into(MsgWithdrawal {
            creator: env.contract.address.to_string(),
            receiver: env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            shares_to_remove: vec![deposit
                .shares_owned
                .parse()
                .expect("Failed to parse the string as an integer")],
            tick_indexes_a_to_b: vec![deposit.center_tick_index],
            fees: vec![deposit.fee],
        });

        // Wrap the DexMsg into a SubMsg with reply
        messages.push(SubMsg::reply_always(withdraw_msg, DEX_WITHDRAW_REPLY_ID));
    }

    // Add the message to the response and return
    Ok(Response::new()
        .add_submessages(messages)
        .add_attribute("action", "dex_withdrawal"))
}

pub fn handle_dex_withdrawal_reply(
    deps: DepsMut,
    env: Env,
    msg_result: SubMsgResult,
) -> Result<Response, ContractError> {
    match msg_result {
        SubMsgResult::Ok(result) => {
            let mut config = CONFIG.load(deps.storage)?;
            let (amount0, amount1) = extract_withdrawal_amounts(&result)?;

            config.balances.token_0.amount += amount0;
            config.balances.token_1.amount += amount1;

            CONFIG.save(deps.storage, &config)?;

            Ok(Response::new().add_attribute("action", "withdrawal_reply_success"))
        }
        SubMsgResult::Err(err) => Ok(Response::new()
            .add_attribute("action", "withdrawal_reply_error")
            .add_attribute("error", err)),
    }
}

pub fn handle_withdrawal_reply(
    deps: DepsMut,
    env: Env,
    msg_result: SubMsgResult,
    burn_amount: Uint128,
    beneficiary: String,
) -> Result<Response, ContractError> {
    let mut messages: Vec<CosmosMsg> = vec![];

    match msg_result {
        SubMsgResult::Ok(_result) => {
            // get contract balances
            let mut config = CONFIG.load(deps.storage)?;
            let balances = query_contract_balance(&deps, env.clone(), config.pair_data.clone())?;

            let withdrawal_ratio = PrecDec::from_ratio(burn_amount, config.total_shares);

            // Calculate total withdrawal amounts for each token
            let withdraw_amount_0 = precdec_to_uint128(
                PrecDec::from_ratio(balances[0].amount, 1u128) * withdrawal_ratio,
            )?;
            let withdraw_amount_1 = precdec_to_uint128(
                PrecDec::from_ratio(balances[1].amount, 1u128) * withdrawal_ratio,
            )?;

            CONFIG.save(deps.storage, &config)?;

            // burn the LP tokens

            let burn_msg = MsgBurn {
                sender: env.contract.address.to_string(),
                amount: Some(
                    Coin {
                        denom: config.lp_denom.clone(),
                        amount: burn_amount,
                    }
                    .into(),
                ),
                burn_from_address: beneficiary.clone(),
            };

            messages.push(burn_msg.into());

            if !(config.balances.token_0.amount.is_zero() && withdraw_amount_0.is_zero()) {
                messages.push(
                    BankMsg::Send {
                        to_address: beneficiary.clone(),
                        amount: vec![Coin {
                            denom: config.balances.token_0.denom.clone(),
                            amount: withdraw_amount_0,
                        }],
                    }
                    .into(),
                );
            }

            if !(config.balances.token_1.amount.is_zero() && withdraw_amount_1.is_zero()) {
                messages.push(
                    BankMsg::Send {
                        to_address: beneficiary.clone(),
                        amount: vec![Coin {
                            denom: config.balances.token_1.denom.clone(),
                            amount: withdraw_amount_1,
                        }],
                    }
                    .into(),
                );
            }

            // Get current prices and tick index for new deposit
            let prices: CombinedPriceResponse = get_prices(deps.as_ref(), env.clone())?;
            let tick_index = price_to_tick_index(prices.price_0_to_1)?;

            config.balances.token_0.amount = balances[0].amount - withdraw_amount_0;
            config.balances.token_1.amount = balances[1].amount - withdraw_amount_1;
            config.total_shares -= burn_amount;
            CONFIG.save(deps.storage, &config)?;

            // Create deposit messages
            let deposit_msgs = get_deposit_messages(&env, config, tick_index, prices)?;
            messages.extend(deposit_msgs);
            Ok(Response::new()
                .add_messages(messages)
                .add_attribute("action", "withdrawal_reply_success")
                .add_attribute("next_action", "create_new_deposit"))
        }
        SubMsgResult::Err(err) => Ok(Response::new()
            .add_attribute("action", "withdrawal_reply_error")
            .add_attribute("error", err)),
    }
}

pub fn execute_create_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if !config.lp_denom.is_empty() {
        return Err(ContractError::TokenAlreadyCreated {});
    }

    let subdenom = format!(
        "{}-{}",
        config.pair_data.token_0.pair.base, config.pair_data.token_1.pair.base
    );
    let msg = SubMsg::reply_on_success(
        MsgCreateDenom {
            sender: env.contract.address.to_string(),
            subdenom: subdenom.clone(),
        },
        CREATE_TOKEN_REPLY_ID,
    );

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_submessage(msg)
        .add_attribute("action", "create_token")
        .add_attribute("denom", subdenom.clone()))
}

pub fn handle_create_token_reply(
    deps: DepsMut,
    msg_result: SubMsgResult,
) -> Result<Response, ContractError> {
    match msg_result {
        SubMsgResult::Ok(result) => {
            let denom = extract_denom(&result)?;

            let mut config = CONFIG.load(deps.storage)?;
            config.lp_denom = denom.clone();
            CONFIG.save(deps.storage, &config)?;

            Ok(Response::new()
                .add_attribute("action", "create_token_reply_success")
                .add_attribute("new_token_denom", denom.clone()))
        }
        SubMsgResult::Err(err) => Ok(Response::new()
            .add_attribute("action", "create_token_reply_error")
            .add_attribute("error", err)),
    }
}
