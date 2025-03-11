use crate::error::ContractError;
use crate::msg::{CombinedPriceResponse, ConfigUpdateMsg, WithdrawPayload};
use crate::state::{
    Config, CONFIG, CREATE_TOKEN_REPLY_ID, DEX_WITHDRAW_REPLY_ID, WITHDRAW_REPLY_ID,
};
use crate::utils::*;
use cosmwasm_std::{
    Addr, Binary, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, SubMsg,
    SubMsgResult, Uint128,
};
use neutron_std::types::neutron::dex::{DexQuerier, MsgWithdrawal, QueryAllUserDepositsResponse};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::osmosis::tokenfactory::v1beta1::{MsgCreateDenom, MsgMint};
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

    // Iterate through the sent funds
    for coin in sent_funds.iter() {
        if coin.denom == config.pair_data.token_0.denom {
            if coin.amount == Uint128::zero() {
                return Err(ContractError::InvalidTokenAmount);
            }
            token0_deposited += coin.amount;
        } else if coin.denom == config.pair_data.token_1.denom {
            if coin.amount == Uint128::zero() {
                return Err(ContractError::InvalidTokenAmount);
            }
            token1_deposited += coin.amount;
        } else {
            // Return an error if an unsupported token is sent
            return Err(ContractError::InvalidToken);
        }
    }

    // Check if deposit would exceed the deposit cap
    let prices: CombinedPriceResponse = get_prices(deps.as_ref(), env.clone())?;
    let (deposit_value_0, deposit_value_1) = get_token_value(prices.clone(), token0_deposited, token1_deposited)?;
    let deposit_value = deposit_value_0.checked_add(deposit_value_1)?;
    let exceeds_cap = config.value_deposited.checked_add(deposit_value_0)? > PrecDec::from_atomics(config.deposit_cap, 0).unwrap();
    
    // Only enforce deposit cap for non-whitelisted addresses
    if exceeds_cap && !config.whitelist.contains(&info.sender) {
        return Err(ContractError::ExceedsDepositCap {});
    }

    // update the deposit value
    config.value_deposited += deposit_value;

    let amount_to_mint = get_mint_amount(
        env.clone(),
        &deps,
        config.clone(),
        prices,
        deposit_value_0,
        deposit_value_1,
    )?;

    if amount_to_mint.is_zero() {
        return Err(ContractError::InvalidTokenAmount);
    }

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
        .add_attribute("minted_amount", amount_to_mint.to_string()))
}

pub fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    config: Config,
) -> Result<Response, ContractError> {
    // Load the contract configuration
    // Query the caller's LP token balance - using bank query for full balance

    let payload = WithdrawPayload {
        sender: info.sender.to_string(),
        amount: amount.to_string(),
    };

    let dex_querier = DexQuerier::new(&deps.querier);
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(env.contract.address.to_string(), None, true)?;

    let mut messages: Vec<CosmosMsg> = vec![];

    // Add all withdrawals from existing deposits
    for deposit in res.deposits.iter() {
        let msg_withdrawal = MsgWithdrawal {
            creator: env.contract.address.to_string(),
            receiver: env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            shares_to_remove: vec![deposit.shares_owned.parse().expect("Failed to parse")],
            tick_indexes_a_to_b: vec![deposit.center_tick_index],
            fees: vec![deposit.fee],
        };
        messages.push(msg_withdrawal.into());
    }

    // If no existing deposits, handle direct withdrawal
    if messages.is_empty() {
        let (withdrawal_messages, withdraw_amount_0, withdraw_amount_1) = get_withdrawal_messages(
            &env,
            &deps,
            &config,
            amount,
            info.sender.to_string()
        )?;
        messages.extend(withdrawal_messages);

        return Ok(Response::new()
            .add_messages(messages)
            .add_attribute("action", "withdrawal")
            .add_attribute("withdraw_amount_0", withdraw_amount_0.to_string())
            .add_attribute("withdraw_amount_1", withdraw_amount_1.to_string()));
    }

    // Handle withdrawal from existing deposits
    Ok(Response::new()
        .add_submessages(flatten_msgs_always_reply(
            &[messages],
            WITHDRAW_REPLY_ID,
            Some(Binary::from(payload.encode_to_vec())),
        )?)
        .add_attribute("action", "withdrawal"))
}

pub fn dex_deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration
    let mut config = CONFIG.load(deps.storage)?;

    // get the last executed timestamp.
    let last_executed = env.block.time.seconds() - config.timestamp_stale;
    // update the timestamp_stale to the current timestamp.
    config.timestamp_stale = env.block.time.seconds();
    CONFIG.save(deps.storage, &config)?;

    if config.paused {
        return Err(ContractError::Paused {});
    }

    // if the last executed timestamp is greater than the timestamp_stale, return an error.
    if last_executed > config.timestamp_stale {
        return Err(ContractError::StaleTimestamp {});
    }

    if !config.whitelist.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    // get the current slinky price and tick index
    let prices: crate::msg::CombinedPriceResponse = get_prices(deps.as_ref(), env.clone())?;
    let tick_index = price_to_tick_index(prices.price_0_to_1)?;

    let balances = query_contract_balance(&deps, env.clone(), config.pair_data.clone())?;

    let messages = get_deposit_messages(
        &env,
        config.clone(),
        tick_index,
        prices,
        balances[0].amount,
        balances[1].amount,
    )?;

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
    if !config.whitelist.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    // Check if there are any active deposits
    let dex_querier = DexQuerier::new(&deps.querier);
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(env.contract.address.to_string(), None, true)?;

    // Create withdrawal messages
    let messages = res
        .deposits
        .iter()
        .map(|deposit| {
            Into::<CosmosMsg>::into(MsgWithdrawal {
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
            })
        })
        .collect::<Vec<CosmosMsg>>();

    // Add the message to the response and return
    Ok(Response::new()
        .add_submessages(flatten_msgs_always_reply(
            &[messages],
            DEX_WITHDRAW_REPLY_ID,
            None,
        )?)
        .add_attribute("action", "dex_withdrawal"))
}

pub fn handle_dex_withdrawal_reply(
    deps: DepsMut,
    _env: Env,
    msg_result: SubMsgResult,
) -> Result<Response, ContractError> {
    match msg_result {
        SubMsgResult::Ok(result) => {
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
    match msg_result {
        SubMsgResult::Ok(_result) => {
            let mut messages: Vec<CosmosMsg> = vec![];
            let mut config = CONFIG.load(deps.storage)?;
            let (withdrawal_messages, withdraw_amount_0, withdraw_amount_1) =
                get_withdrawal_messages(&env, &deps, &config.clone(), burn_amount, beneficiary)?;
            messages.extend(withdrawal_messages);

            // update the deposited value
            let prices: CombinedPriceResponse = get_prices(deps.as_ref(), env.clone())?;
            let (value_withdrawn_0, value_withdrawn_1) = get_token_value(prices.clone(), withdraw_amount_0, withdraw_amount_1)?;
            let value_withdrawn = value_withdrawn_0.checked_add(value_withdrawn_1)?;

            config.value_deposited = config.value_deposited.checked_sub(value_withdrawn)?;
            CONFIG.save(deps.storage, &config)?;

            let tick_index = price_to_tick_index(prices.price_0_to_1)?;

            // Create deposit messages
            let deposit_msgs = get_deposit_messages(
                &env,
                config,
                tick_index,
                prices,
                withdraw_amount_0,
                withdraw_amount_1,
            )?;
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

    if !config.whitelist.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    if !config.lp_denom.is_empty() {
        return Err(ContractError::TokenAlreadyCreated {});
    }

    // Create subdenom
    let subdenom = format!(
        "{}-{}",
        config.pair_data.token_0.pair.base, config.pair_data.token_1.pair.base
    );

    // Create the full denom string that will be used later
    let full_denom = format!("factory/{}/{}", env.contract.address, subdenom);

    let msg = SubMsg::reply_on_success(
        MsgCreateDenom {
            sender: env.contract.address.to_string(),
            subdenom: subdenom.clone(),
        },
        CREATE_TOKEN_REPLY_ID,
    );

    Ok(Response::new()
        .add_submessage(msg)
        .add_attribute("action", "create_token")
        .add_attribute("denom", full_denom))
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

pub fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    update: ConfigUpdateMsg,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if !config.whitelist.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    // Update owner if provided
    if let Some(whitelist) = update.whitelist {
        let whitelist = whitelist
            .iter()
            .map(|addr| deps.api.addr_validate(addr).map_err(ContractError::Std))
            .collect::<Result<Vec<Addr>, ContractError>>()?;
        config.whitelist = whitelist;
    }

    // Update max_blocks_old if provided
    if let Some(max_blocks_old_token_a) = update.max_blocks_old_token_a {
        config.pair_data.token_0.max_blocks_old = max_blocks_old_token_a;
    }

    if let Some(max_blocks_old_token_b) = update.max_blocks_old_token_b {
        config.pair_data.token_1.max_blocks_old = max_blocks_old_token_b;
    }

    // Update deposit_cap if provided
    if let Some(deposit_cap) = update.deposit_cap {
        config.deposit_cap = deposit_cap;
    }

    // Update timestamp_stale if provided
    if let Some(timestamp_stale) = update.timestamp_stale {
        if timestamp_stale == 0 {
            return Err(ContractError::InvalidConfig {
                reason: "timestamp_stale must be greater than 0".to_string(),
            });
        }
        config.timestamp_stale = timestamp_stale;
    }

    // Update fee_tier_config if provided
    if let Some(fee_tier_config) = update.fee_tier_config {
        // Validate fee tiers
        let mut total_percentage = 0u64;
        for tier in &fee_tier_config.fee_tiers {
            total_percentage += tier.percentage;
        }
        if total_percentage > 100 {
            return Err(ContractError::InvalidFeeTier {
                reason: "Total fee tier percentages must be <= 100%".to_string(),
            });
        }
        config.fee_tier_config = fee_tier_config;
    }

    if let Some(paused) = update.paused {
        config.paused = paused;
    }

    if let Some(skew) = update.skew {
        config.skew = skew;
    }

    // Save updated config
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("owner", format!("{:?}", config.whitelist))
        .add_attribute("max_blocks_old_token_a", config.pair_data.token_0.max_blocks_old.to_string())
        .add_attribute("max_blocks_old_token_b", config.pair_data.token_1.max_blocks_old.to_string())
        .add_attribute("deposit_cap", config.deposit_cap.to_string())
        .add_attribute("timestamp_stale", config.timestamp_stale.to_string())
        .add_attribute("total_shares", config.total_shares.to_string())
        .add_attribute("lp_denom", config.lp_denom))
}
