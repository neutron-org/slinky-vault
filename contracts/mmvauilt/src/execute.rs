use crate::error::ContractError;
use crate::msg::InstantiateMsg;
use crate::state::{CONFIG, CRON_MODULE_ADDRESS, DEX_WITHDRAW_REPLY_ID};
use crate::utils::*;
use cosmwasm_std::{
    Addr, CosmosMsg, DepsMut, Env, MessageInfo, Response, SubMsg, SubMsgResult, Uint128, Coin, BankMsg,
};
use neutron_std::types::neutron::dex::{DexQuerier, MsgWithdrawal, QueryAllUserDepositsResponse};

pub fn deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let messages: Vec<CosmosMsg> = vec![];
    // Load the contract configuration from storage
    let mut config = CONFIG.load(deps.storage)?;
    //if calles is not the owner error
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
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

pub fn withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration to access the owner address and balances
    let mut config = CONFIG.load(deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Query current contract balances
    let balances = query_contract_balance(&deps, env.clone(), config.pair_data.clone())?;
    
    // Create bank send messages for both tokens
    if balances[0].amount > Uint128::zero() {
        messages.push(
            BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: balances[0].denom.clone(),
                    amount: balances[0].amount,
                }],
            }
            .into(),
        );
    }
    if balances[1].amount > Uint128::zero() {
        messages.push(
            BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: balances[1].denom.clone(),
                    amount: balances[1].amount,
                }],
            }
            .into(),
        );
    }

    // Update config balances to zero
    config.balances.token_0.amount = Uint128::zero();
    config.balances.token_1.amount = Uint128::zero();
    CONFIG.save(deps.storage, &config)?;

    // Add the message to the response and return
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "withdrawal")
        .add_attribute("token_0_amount", balances[0].amount.to_string())
        .add_attribute("token_1_amount", balances[1].amount.to_string()))
}

// depends on up-to-date config
pub fn dex_deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration
    let mut config = CONFIG.load(deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    let cron_address = Addr::unchecked(CRON_MODULE_ADDRESS);
    // if the caller is not the owner or the cron module, return an error
    if info.sender != config.owner && info.sender != cron_address {
        return Err(ContractError::Unauthorized {});
    }
    let balances = query_contract_balance(&deps, env.clone(), config.pair_data.clone())?;

    config.balances.token_0.amount = balances[0].amount;
    config.balances.token_1.amount = balances[1].amount; 
    
    CONFIG.save(deps.storage, &config)?;

    // get the current slinky price and tick index
    let prices: crate::msg::CombinedPriceResponse = get_prices(deps.as_ref(), env.clone())?;
    let tick_index = price_to_tick_index(prices.price_0_to_1)?;

    let (lo_messages, token_0_usable, token_1_usable) =
        prepare_state(&deps, &env, &config, tick_index)?;
    messages.extend(lo_messages);
    let deposit_messages = get_deposit_messages(
        &env,
        config.clone(),
        tick_index,
        prices,
        token_0_usable,
        token_1_usable,
    )?;
    messages.extend(deposit_messages);

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
    let cron_address = Addr::unchecked(CRON_MODULE_ADDRESS);

    // if the caller is not the owner or the cron module, return an error
    if info.sender != config.owner && info.sender != cron_address {
        return Err(ContractError::Unauthorized {});
    }

    // Prepare a vector to hold withdrawals
    let mut messages: Vec<CosmosMsg> = vec![];
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
        messages.push(withdraw_msg);
    }

    // Add the message to the response and return
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "dex_withdrawal"))
}


pub fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    max_blocks_old: Option<u64>,
    base_fee: Option<u64>,
    base_deposit_percentage: Option<u64>,
    ambient_fee: Option<u64>,
    deposit_ambient: Option<bool>,
    deposit_cap: Option<Uint128>,
) -> Result<Response, ContractError> {
    // Load and verify owner
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Update max_blocks_old if provided
    if let Some(blocks) = max_blocks_old {
        if blocks > 2 {
            return Err(ContractError::MalformedInput {
                input: "max_block_old".to_string(),
                reason: "must be <=2".to_string(),
            });
        }
        config.max_blocks_old = blocks;
    }

    // Update base_fee if provided
    if let Some(fee) = base_fee {
        InstantiateMsg::validate_base_fee(fee)?;
        config.base_fee = fee;
    }

    // Update base_deposit_percentage if provided
    if let Some(percentage) = base_deposit_percentage {
        InstantiateMsg::validate_base_deposit_percentage(percentage)?;
        config.base_deposit_percentage = percentage;
    }

    // Update ambient_fee if provided
    if let Some(fee) = ambient_fee {
        config.ambient_fee = fee;
    }

    // Update deposit_ambient if provided
    if let Some(deposit) = deposit_ambient {
        config.deposit_ambient = deposit;
    }

    // Update deposit_cap if provided
    if let Some(cap) = deposit_cap {
        config.deposit_cap = cap;
    }

    // Save updated config
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("max_blocks_old", config.max_blocks_old.to_string())
        .add_attribute("base_fee", config.base_fee.to_string())
        .add_attribute(
            "base_deposit_percentage",
            config.base_deposit_percentage.to_string(),
        )
        .add_attribute("ambient_fee", config.ambient_fee.to_string())
        .add_attribute("deposit_ambient", config.deposit_ambient.to_string())
        .add_attribute("deposit_cap", config.deposit_cap.to_string()))
}
