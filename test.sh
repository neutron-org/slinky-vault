#!/bin/bash

# Colors for terminal output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test suite configuration
TEST_SUITE_NAME="Slinky Vault Integration Tests"
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Configuration
contract="./artifacts/mmvault.wasm"
contract_oracle="./artifacts/slinky_oracle.wasm"
account=testnet2
account2=testnet
chain_id=test-1
node=http://localhost:26657
gas_price="0.025untrn"
gas_adjustment="2.5"
token_a="uibcusdc"
token_b="untrn"
pair_id=$token_a"<>"$token_b

# Add this near the top with other configuration variables
SHOW_TX_HASH=true

# Parse command line arguments
while getopts "v" opt; do
  case $opt in
    v)
      SHOW_TX_HASH=true
      ;;
    \?)
      echo "Invalid option: -$OPTARG" >&2
      exit 1
      ;;
  esac
done

# Reset OPTIND to parse other arguments if needed later
shift $((OPTIND-1))

##########################
######### HELPERS ########
##########################

wait_for_tx() {
    sleep 1
}

query_contract() {
    neutrond q wasm contract-state smart $1 "$2" --node $node --output json
}

place_limit_order_gtc() {
    local base_denom=$1
    local quote_denom=$2
    local amount=$3
    local price=$4
    
    neutrond tx dex place-limit-order neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kf \
        "$base_denom" "$quote_denom" "[0]" "$amount" "GOOD_TIL_CANCELLED" \
        --price "$price" --from testnet2 --chain-id test-1 --node $node --fees 2000untrn -y > /dev/null
    sleep 2
}

print_vault_info() {
    print_section "VAULT STATUS"

    # Print contract balances
    print_info "Contract Balances:"
    neutrond q bank balances $contract_address --node $node

    # Print pool reserves
    print_info "Pool Reserves:"

    # Get and store token_a reserves
    token_a_reserves_1=$(neutrond q dex list-pool-reserves $pair_id $token_a --node $node --output json |
        jq -r '.pool_reserves[0].reserves_maker_denom // "0"')
    token_a_reserves_2=$(neutrond q dex list-pool-reserves $pair_id $token_a --node $node --output json |
        jq -r '.pool_reserves[1].reserves_maker_denom // "0"')
    echo "Token A ($token_a) reserves:"
    echo "  Fee tier 1: $token_a_reserves_1"
    echo "  Fee tier 2: $token_a_reserves_2"

    # Get and store token_b reserves
    token_b_reserves_1=$(neutrond q dex list-pool-reserves $pair_id $token_b --node $node --output json |
        jq -r '.pool_reserves[0].reserves_maker_denom // "0"')
    token_b_reserves_2=$(neutrond q dex list-pool-reserves $pair_id $token_b --node $node --output json |
        jq -r '.pool_reserves[1].reserves_maker_denom // "0"')
    echo "Token B ($token_b) reserves:"
    echo "  Fee tier 1: $token_b_reserves_1"
    echo "  Fee tier 2: $token_b_reserves_2"

    # Calculate and show totals
    total_token_a=$(echo "$token_a_reserves_1 + $token_a_reserves_2" | bc)
    total_token_b=$(echo "$token_b_reserves_1 + $token_b_reserves_2" | bc)
    echo "Total reserves:"
    echo "  Total $token_a: $total_token_a"
    echo "  Total $token_b: $total_token_b"

    # Print deposits status
    print_info "Deposits Status:"
    neutrond q wasm contract-state smart $contract_address '{"get_deposits":{}}' --node $node --trace
}

get_lp_balance() {
    local contract_addr=$1
    local lp_denom=$2

    # Query balances and use more explicit jq parsing
    local balance=$(neutrond q bank balances $contract_addr --node $node --output json |
        jq --arg denom "$lp_denom" '.balances[] | select(.denom == $denom) | .amount' | tr -d '"')

    # If balance is null or empty, return 0
    if [ -z "$balance" ] || [ "$balance" = "null" ]; then
        echo "0"
    else
        echo "$balance"
    fi
}

execute_contract() {
    local contract_addr="$1"
    local msg="$2"
    local amount="$3"                   # Make sure we capture the amount parameter
    local from_account="${4:-$account}" # Use provided account or default to $account
    
    # Add debug output
    print_info "Debug: execute_contract called with:"
    print_info "  contract_addr: $contract_addr"
    print_info "  msg: $msg"
    print_info "  amount: ${amount:-'<no amount>'}"
    print_info "  from_account: $from_account"

    # Execute the transaction and capture the response
    local resp
    if [ -n "$amount" ]; then
        print_info "Debug: Executing with amount"
        resp=$(neutrond tx wasm execute "$contract_addr" "$msg" --amount "$amount" \
            --from "$from_account" \
            --chain-id "$chain_id" \
            --gas-prices "$gas_price" \
            --gas-adjustment "$gas_adjustment" \
            --gas auto \
            --yes \
            --output json)
    else
        print_info "Debug: Executing without amount"
        resp=$(neutrond tx wasm execute "$contract_addr" "$msg" \
            --from "$from_account" \
            --chain-id "$chain_id" \
            --gas-prices "$gas_price" \
            --gas-adjustment "$gas_adjustment" \
            --gas auto \
            --yes \
            --output json)
    fi

    # Check if the transaction was successful
    local tx_hash=$(echo "$resp" | jq -r ".txhash")
    if [ -n "$tx_hash" ]; then
        if [ "$SHOW_TX_HASH" = true ]; then
            print_success "Transaction hash: $tx_hash"
        fi
        wait_for_tx
        # Query the transaction result
        local tx_result=$(neutrond q tx "$tx_hash" --node "$node" --output json)
        if [ "$(echo "$tx_result" | jq -r .code)" != "0" ]; then
            print_error "Transaction failed: $(echo "$tx_result" | jq -r .raw_log)"
        fi
    else
        print_error "Failed to execute transaction: $resp"
    fi
}

place_limit_order() {
    local base_denom=$1
    local quote_denom=$2
    local amount=$3
    local price=$4

    neutrond tx dex place-limit-order neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kf \
        "$base_denom" "$quote_denom" "[0]" "$amount" "IMMEDIATE_OR_CANCEL" \
        --price "$price" --from testnet2 --chain-id test-1 --node $node --fees 2000untrn -y >/dev/null
}
place_limit_order_gtc() {
    local base_denom=$1
    local quote_denom=$2
    local amount=$3
    local price=$4

    neutrond tx dex place-limit-order neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kf \
        "$base_denom" "$quote_denom" "[0]" "$amount" "GOOD_TIL_CANCELLED" \
        --price "$price" --from testnet2 --chain-id test-1 --node $node --fees 2000untrn -y >/dev/null
}
print_section() {
    echo -e "\n=== $1 ==="
}

print_test() {
    echo "▶ $1"
}

print_result() {
    if [ $? -eq 0 ]; then
        echo "✓ PASS"
    else
        echo "✗ FAIL"
    fi
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}➜ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

store_and_instantiate_contract() {
    local contract_path="${1:-$contract}"  # Use provided path or default to $contract
    local instantiate_msg="${2:-'{}'}"     # Default to empty JSON object if no message provided
    
    print_section "STORING AND INSTANTIATING CONTRACT"
    print_info "Using contract: $contract_path"
    
    # Store contract
    local store_resp=$(neutrond tx wasm store $contract_path --from $account --chain-id $chain_id \
        --gas-prices $gas_price --gas-adjustment $gas_adjustment --gas auto --output json -y)
    
    # Check if store was successful
    if [ $? -ne 0 ]; then
        print_error "Failed to store contract"
        return 1
    fi
    
    local tx_hash=$(echo $store_resp | jq -r ".txhash")
    print_info "Store transaction hash: $tx_hash"
    wait_for_tx
    
    # Get code ID and verify it exists
    code_id=$(neutrond q tx $tx_hash --output json --node $node | \
        jq -r '.events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value')
    
    if [ -z "$code_id" ]; then
        print_error "Failed to get code ID"
        return 1
    fi
    print_success "Code ID: $code_id"
    
    # Instantiate contract
    print_section "CONTRACT INSTANTIATION"
    local inst_resp=$(neutrond tx wasm instantiate $code_id "$instantiate_msg" \
        --label "contract-$(date +%s)" \
        --admin neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kf \
        --gas auto \
        --chain-id $chain_id \
        --from $account \
        --gas-prices $gas_price \
        --gas-adjustment $gas_adjustment \
        -y --output json)
    
    if [ $? -ne 0 ]; then
        print_error "Failed to instantiate contract"
        return 1
    fi
    
    tx_hash=$(echo $inst_resp | jq -r ".txhash")
    wait_for_tx
    
    # Get and verify contract address
    contract_address=$(neutrond q tx $tx_hash --output json --node $node | \
        jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
    
    if [ -z "$contract_address" ]; then
        print_error "Failed to get contract address"
        return 1
    fi
    
    print_success "Contract Address: $contract_address"
    
    # Export contract address for other functions to use
    export contract_address
    return 0
}

setup_suite() {
    print_section "SETTING UP TEST SUITE"
    
    # Setup node configuration
    neutrond config node $node
    if [ $? -ne 0 ]; then
        print_error "Failed to configure node"
        return 1
    fi
    
    # Store and instantiate oracle contract first (with empty init message)
    if ! store_and_instantiate_contract "$contract_oracle" "{}"; then
        print_error "Failed to setup oracle contract"
        return 1
    fi
    export oracle_address="$contract_address"  # Export the oracle address
    
    # Store and instantiate vault contract with vault-specific init message
    local init_msg='{
        "whitelist": ["neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kf","neutron1m9l358xunhhwds0568za49mzhvuxx9ux8xafx2"],
        "token_a": {"denom": "'$token_a'", "decimals": 6, "pair": {"base": "USDC", "quote": "USD"}, "max_blocks_old": 2}, 
        "token_b": {"denom": "'$token_b'", "decimals": 6, "pair": {"base": "NTRN", "quote": "USD"}, "max_blocks_old": 2}, 
        "fee_tier_config": {
            "fee_tiers": [
                {"fee": 10, "percentage": 30},
                {"fee": 150, "percentage": 70}
            ]
        },
        "deposit_cap": "10000",
        "timestamp_stale": 3000,
        "paused": false,
        "oracle_contract": "'$oracle_address'"
    }'
    
    if ! store_and_instantiate_contract "$contract" "$init_msg"; then
        print_error "Failed to setup vault contract"
        return 1
    fi

    # Create token
    print_info "Creating token..."
    if ! execute_contract $contract_address '{"create_token": {}}'; then
        print_error "Failed to create token"
        return 1
    fi
    print_success "Token created successfully"
    
    # Verify contract is queryable
    if ! query_contract $contract_address '{"get_config":{}}' > /dev/null 2>&1; then
        print_error "Contract is not queryable after deployment"
        return 1
    fi
    
    print_success "Test suite setup completed"
    return 0
}

teardown_suite() {
    print_section "CLEANING UP TEST SUITE"
    # TODO cleanup logic
    print_success "Test suite cleanup completed"
}

assert_equals() {
    local expected="$1"
    local actual="$2"
    local message="$3"

    if [ "$expected" = "$actual" ]; then
        return 0
    else
        echo "Assert failed: $message"
        echo "Expected: $expected"
        echo "Actual: $actual"
        return 1
    fi
}

run_subtest() {
    local test_name="$1"
    local test_function="$2"
    shift 2  
    
    print_test "$test_name"
    if $test_function "$@"; then
        print_success "PASS"
        return 0
    else
        print_error "FAIL"
        return 1
    fi
}

##########################
### TEST CASE HELPERS ###
##########################

# Generic deposit helper that verifies balance before deposit
test_deposit() {
    local token_denom=$1
    local amount=$2
    local expected_balance=$3

    # Verify the test account has enough funds
    local current_balance=$(neutrond q bank balances $account --node $node --output json | \
        jq --arg denom "$token_denom" '.balances[] | select(.denom == $denom) | .amount' | tr -d '"')
    
    print_info "Current balance of ${token_denom}: ${current_balance:-0}"
    
    if [ -z "$current_balance" ] || [ "$current_balance" -lt "$amount" ]; then
        print_error "Insufficient funds in test account. Has: ${current_balance:-0} ${token_denom}, Needs: ${amount} ${token_denom}"
        return 1
    fi

    # Execute deposit with funds
    if ! execute_contract $contract_address '{"deposit":{}}' "${amount}${token_denom}"; then
        print_error "Failed to deposit ${amount}${token_denom}"
        return 1
    fi

    # Wait for transaction to be processed
    wait_for_tx

    # Verify contract balance
    local balance=$(neutrond q bank balances $contract_address --node $node --output json | \
        jq --arg denom "$token_denom" '.balances[] | select(.denom == $denom) | .amount' | tr -d '"')
    
    if ! assert_equals "$expected_balance" "$balance" "${token_denom} deposit amount mismatch"; then
        return 1
    fi
    return 0
}

# Generic LP token verification helper
verify_lp_tokens() {
    local expected_holder=$1
    local expected_min_balance=${2:-0}  # Default to 0 if not provided

    # Query config to get LP token denom
    local config_response=$(query_contract $contract_address '{"get_config":{}}')
    if [ $? -ne 0 ]; then
        print_error "Failed to query config"
        return 1
    fi
    
    local lp_denom=$(echo "$config_response" | jq -r '.data.lp_denom')
    if [ -z "$lp_denom" ] || [ "$lp_denom" = "null" ]; then
        print_error "LP token denom not found"
        return 1
    fi

    local lp_balance=$(get_lp_balance "$expected_holder" "$lp_denom")
    if [ -z "$lp_balance" ] || [ "$lp_balance" = "null" ]; then
        lp_balance=0
    fi
    
    if [ "$lp_balance" -lt "$expected_min_balance" ]; then
        print_error "LP balance ($lp_balance) less than expected ($expected_min_balance)"
        return 1
    fi

    return 0
}

test_migration() {
    local new_contract_path=$1
    local migration_msg="${2:-'{}'}"  # Default to empty message if none provided
    
    print_info "Starting contract migration"
    
    # Store new contract code
    local store_resp=$(neutrond tx wasm store $new_contract_path --from $account --chain-id $chain_id \
        --gas-prices $gas_price --gas-adjustment $gas_adjustment --gas auto --output json -y)
    
    if [ $? -ne 0 ]; then
        print_error "Failed to store new contract"
        return 1
    fi
    
    local tx_hash=$(echo $store_resp | jq -r ".txhash")
    wait_for_tx
    
    # Get new code ID
    local new_code_id=$(neutrond q tx $tx_hash --output json --node $node | \
        jq -r '.events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value')
    
    if [ -z "$new_code_id" ]; then
        print_error "Failed to get new code ID"
        return 1
    fi
    print_success "New Code ID: $new_code_id"
    
    # Execute migration
    local migrate_resp=$(neutrond tx wasm migrate $contract_address $new_code_id "$migration_msg" \
        --from $account \
        --chain-id $chain_id \
        --gas-prices $gas_price \
        --gas-adjustment $gas_adjustment \
        --gas auto \
        -y --output json)
    
    if [ $? -ne 0 ]; then
        print_error "Failed to migrate contract"
        return 1
    fi
    
    tx_hash=$(echo $migrate_resp | jq -r ".txhash")
    wait_for_tx
    
    # Verify contract is still queryable
    if ! query_contract $contract_address '{"get_config":{}}' > /dev/null 2>&1; then
        print_error "Contract is not queryable after migration"
        return 1
    fi
    
    print_success "Migration completed successfully"
    return 0
}

##########################
#### TEST SCENARIOS #####
##########################

# Basic deposit flow
test_basic_deposit_flow() {
    print_section "Basic Deposit Flow Test"
    local failed=0

    # Try deposits directly
    if ! run_subtest "NTRN Deposit" test_deposit "untrn" "200000000" "200000000"; then
        failed=1
    fi

    if ! run_subtest "USDC Deposit" test_deposit "uibcusdc" "20000000" "20000000"; then
        failed=1
    fi
    
    # Only verify LP tokens if deposits succeeded
    if [ $failed -eq 0 ]; then
        if ! run_subtest "LP Token Verification" verify_lp_tokens "$account" "1"; then
            failed=1
        fi
    fi

    return $failed
}

test_dex_deposit() {
    print_section "DEX Deposit Test"
    local failed=0

    # Execute deposit to DEX
    local deposit_msg='{"dex_deposit":{}}'
    if ! execute_contract $contract_address "$deposit_msg"; then
        print_error "Failed to execute deposit to DEX"
        return 1
    fi

    # Wait for transaction to be processed
    wait_for_tx

    # Print vault info to see the results
    print_vault_info

    return $failed
}
test_dex_withdrawal() {
    print_section "DEX Withdrawal Test"
    local failed=0

    # Get initial contract balances and pool reserves
    print_info "Getting initial state..."
    local initial_balances=$(neutrond q bank balances $contract_address --node $node --output json)
    local initial_token_a=$(echo "$initial_balances" | jq -r '.balances[] | select(.denom == "'$token_a'") | .amount // "0"')
    local initial_token_b=$(echo "$initial_balances" | jq -r '.balances[] | select(.denom == "'$token_b'") | .amount // "0"')
    
    print_info "Initial contract balances:"
    print_info "Token A ($token_a): $initial_token_a"
    print_info "Token B ($token_b): $initial_token_b"
    # Execute withdrawal from DEX
    local withdrawal_msg='{"dex_withdrawal":{}}'
    if ! execute_contract $contract_address "$withdrawal_msg"; then
        print_error "Failed to execute withdrawal from DEX"
        return 1
    fi

    # Wait for transaction to be processed
    wait_for_tx

    # Get final contract balances
    print_info "Getting final state..."
    local final_balances=$(neutrond q bank balances $contract_address --node $node --output json)
    local final_token_a=$(echo "$final_balances" | jq -r '.balances[] | select(.denom == "'$token_a'") | .amount // "0"')
    local final_token_b=$(echo "$final_balances" | jq -r '.balances[] | select(.denom == "'$token_b'") | .amount // "0"')
    
    print_info "Final contract balances:"
    print_info "Token A ($token_a): $final_token_a"
    print_info "Token B ($token_b): $final_token_b"

    # Verify that balances have increased (indicating successful withdrawal)
    if [ "$final_token_a" -le "$initial_token_a" ] && [ "$final_token_b" -le "$initial_token_b" ]; then
        print_error "No balance increase detected after withdrawal"
        print_error "Token A: $initial_token_a -> $final_token_a"
        print_error "Token B: $initial_token_b -> $final_token_b"
        failed=1
    else
        print_success "Balance increase detected after withdrawal"
        print_success "Token A: $initial_token_a -> $final_token_a"
        print_success "Token B: $initial_token_b -> $final_token_b"
    fi

    # Print final vault info
    print_vault_info

    return $failed
}

test_contract_migration() {
    print_section "Contract Migration Test"
    local failed=0
    
    # Get initial contract balances before any test deposits
    local initial_contract_balances=$(neutrond q bank balances $contract_address --node $node --output json)
    local initial_untrn=$(echo "$initial_contract_balances" | jq -r '.balances[] | select(.denom == "untrn") | .amount // "0"')
    print_info "Initial contract untrn balance: $initial_untrn"
    
    # basic deposit to ensure the contract is working
    local deposit_amount="10000000"
    local expected_balance=$(echo "$initial_untrn + $deposit_amount" | bc)
    if ! run_subtest "Initial Deposit" test_deposit "untrn" "$deposit_amount" "$expected_balance"; then
        failed=1
        return $failed
    fi

    # Get current config and balances before migration
    local pre_migration_config=$(query_contract $contract_address '{"get_config":{}}')
    local pre_migration_balances=$(neutrond q bank balances $contract_address --node $node --output json)
    local pre_migration_untrn=$(echo "$pre_migration_balances" | jq -r '.balances[] | select(.denom == "untrn") | .amount // "0"')
    print_info "Pre-migration untrn balance: $pre_migration_untrn"

    # Extract current values from config
    local current_lp_denom=$(echo "$pre_migration_config" | jq -r '.data.lp_denom')
    local current_token_0_amount=$(echo "$pre_migration_config" | jq -r '.data.balances.token_0.amount')
    local current_token_1_amount=$(echo "$pre_migration_config" | jq -r '.data.balances.token_1.amount')
    local current_value_deposited=$(echo "$pre_migration_config" | jq -r '.data.value_deposited')
    local current_total_shares=$(echo "$pre_migration_config" | jq -r '.data.total_shares')

    print_info "Current state before migration:"
    print_info "LP Denom: $current_lp_denom"
    print_info "Token 0 Amount: $current_token_0_amount"
    print_info "Token 1 Amount: $current_token_1_amount"
    print_info "Total Shares: $current_total_shares"
    print_info "Value Deposited: $current_value_deposited"

    # Migration message that preserves current state
    local migration_msg='{
        "config": {
            "pair_data": {
                "token_0": {
                    "denom": "'$token_a'",
                    "decimals": 6,
                    "pair": {"base": "USDC", "quote": "USD"},
                    "max_blocks_old": 2
                },
                "token_1": {
                    "denom": "'$token_b'",
                    "decimals": 6,
                    "pair": {"base": "NTRN", "quote": "USD"},
                    "max_blocks_old": 2
                },
                "pair_id": "'$pair_id'"
            },
            "balances": {
                "token_0": {"denom": "'$token_a'", "amount": "'$current_token_0_amount'"},
                "token_1": {"denom": "'$token_b'", "amount": "'$current_token_1_amount'"}
            },
            "fee_tier_config": {
                "fee_tiers": [
                    {"fee": 10, "percentage": 30},
                    {"fee": 150, "percentage": 70}
                ]
            },
            "lp_denom": "'$current_lp_denom'",
            "total_shares": "'$current_total_shares'",
            "whitelist": ["neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kf","neutron1m9l358xunhhwds0568za49mzhvuxx9ux8xafx2"],
            "deposit_cap": "10000",
            "timestamp_stale": 300,
            "paused": false,
            "oracle_contract": "'$oracle_address'",
            "value_deposited": "'$current_value_deposited'",
            "skew": false,
            "imbalance": 0,
            "migration_successful": true
        }
    }'
    
    print_info "Migration message: $migration_msg"
    
    # Perform migration
    if ! run_subtest "Contract Migration" test_migration "./upgrade-test-artifacts/mmvault.wasm" "$migration_msg"; then
        print_error "Migration failed"
        failed=1
        return $failed
    fi
    
    # Wait a bit longer after migration
    sleep 2
    
    # Verify state after migration
    local post_config=$(query_contract $contract_address '{"get_config":{}}')
    print_info "Post-migration config: $post_config"
    
    # Verify migration_successful field is true
    local migration_successful=$(echo "$post_config" | jq -r '.data.migration_successful')
    if [ "$migration_successful" != "true" ]; then
        print_error "migration_successful field not set correctly after migration. Got: $migration_successful"
        failed=1
        return $failed
    fi
    print_success "migration_successful field verified"

    # Verify deposits were preserved
    if ! run_subtest "Verify Deposits Preserved" verify_lp_tokens "$account" "1"; then
        failed=1
        return $failed
    fi

    # Get post-migration balances
    local post_migration_balances=$(neutrond q bank balances $contract_address --node $node --output json)
    local post_migration_untrn=$(echo "$post_migration_balances" | jq -r '.balances[] | select(.denom == "untrn") | .amount // "0"')
    print_info "Post-migration untrn balance: $post_migration_untrn"

    # Test that contract is still functional by making a new deposit
    local post_migration_deposit_amount="5000000"
    local expected_final_balance=$(echo "$post_migration_untrn + $post_migration_deposit_amount" | bc)
    
    if ! run_subtest "Post-Migration Deposit" test_deposit "untrn" "$post_migration_deposit_amount" "$expected_final_balance"; then
        failed=1
    fi
    
    return $failed
}

test_basic_withdrawal_flow() {
    print_section "Basic Withdrawal Flow Test"
    local failed=0

    # Get initial contract and user balances
    local initial_contract_balances=$(neutrond q bank balances $contract_address --node $node --output json)
    local initial_user_balances=$(neutrond q bank balances $account --node $node --output json)
    
    local initial_contract_token_a=$(echo "$initial_contract_balances" | jq -r '.balances[] | select(.denom == "'$token_a'") | .amount // "0"')
    local initial_contract_token_b=$(echo "$initial_contract_balances" | jq -r '.balances[] | select(.denom == "'$token_b'") | .amount // "0"')
    local initial_user_token_a=$(echo "$initial_user_balances" | jq -r '.balances[] | select(.denom == "'$token_a'") | .amount // "0"')
    local initial_user_token_b=$(echo "$initial_user_balances" | jq -r '.balances[] | select(.denom == "'$token_b'") | .amount // "0"')
    
    print_info "Initial contract balances:"
    print_info "Token A ($token_a): $initial_contract_token_a"
    print_info "Token B ($token_b): $initial_contract_token_b"
    print_info "Initial user balances:"
    print_info "Token A ($token_a): $initial_user_token_a"
    print_info "Token B ($token_b): $initial_user_token_b"

    # Get initial LP token balance
    local config_response=$(query_contract $contract_address '{"get_config":{}}')
    local lp_denom=$(echo "$config_response" | jq -r '.data.lp_denom')
    local initial_lp_balance=$(get_lp_balance "$account" "$lp_denom")
    print_info "Initial LP balance: $initial_lp_balance"

    if [ "$initial_lp_balance" = "0" ]; then
        print_error "No LP tokens to withdraw"
        return 1
    fi

    # Execute withdrawal with the LP tokens sent along
    local withdraw_msg=$(printf '{"withdraw":{"amount":"%s"}}' "$initial_lp_balance")
    if ! execute_contract $contract_address "$withdraw_msg" "${initial_lp_balance}${lp_denom}"; then
        print_error "Failed to execute withdrawal"
        return 1
    fi

    # Wait for transaction to be processed
    wait_for_tx

    # Get final LP token balance
    local final_lp_balance=$(get_lp_balance "$account" "$lp_denom")
    print_info "Final LP balance: $final_lp_balance"

    # Verify LP tokens were burned
    if [ "$final_lp_balance" -ge "$initial_lp_balance" ]; then
        print_error "LP tokens were not burned"
        print_error "Initial: $initial_lp_balance, Final: $final_lp_balance"
        failed=1
    else
        print_success "LP tokens were burned successfully"
        print_success "Initial: $initial_lp_balance, Final: $final_lp_balance"
    fi

    # Get final contract and user balances
    local final_contract_balances=$(neutrond q bank balances $contract_address --node $node --output json)
    local final_user_balances=$(neutrond q bank balances $account --node $node --output json)
    
    local final_contract_token_a=$(echo "$final_contract_balances" | jq -r '.balances[] | select(.denom == "'$token_a'") | .amount // "0"')
    local final_contract_token_b=$(echo "$final_contract_balances" | jq -r '.balances[] | select(.denom == "'$token_b'") | .amount // "0"')
    local final_user_token_a=$(echo "$final_user_balances" | jq -r '.balances[] | select(.denom == "'$token_a'") | .amount // "0"')
    local final_user_token_b=$(echo "$final_user_balances" | jq -r '.balances[] | select(.denom == "'$token_b'") | .amount // "0"')
    
    print_info "Final contract balances:"
    print_info "Token A ($token_a): $final_contract_token_a"
    print_info "Token B ($token_b): $final_contract_token_b"
    print_info "Final user balances:"
    print_info "Token A ($token_a): $final_user_token_a"
    print_info "Token B ($token_b): $final_user_token_b"

    # Verify contract balances decreased
    if [ "$final_contract_token_a" -ge "$initial_contract_token_a" ] || [ "$final_contract_token_b" -ge "$initial_contract_token_b" ]; then
        print_error "Contract balances did not decrease as expected"
        print_error "Token A: $initial_contract_token_a -> $final_contract_token_a"
        print_error "Token B: $initial_contract_token_b -> $final_contract_token_b"
        failed=1
    else
        print_success "Contract balances decreased as expected"
        print_success "Token A: $initial_contract_token_a -> $final_contract_token_a"
        print_success "Token B: $initial_contract_token_b -> $final_contract_token_b"
    fi

    # Verify user balances increased
    if [ "$final_user_token_a" -le "$initial_user_token_a" ] || [ "$final_user_token_b" -le "$initial_user_token_b" ]; then
        print_error "User balances did not increase as expected"
        print_error "Token A: $initial_user_token_a -> $final_user_token_a"
        print_error "Token B: $initial_user_token_b -> $final_user_token_b"
        failed=1
    else
        print_success "User balances increased as expected"
        print_success "Token A: $initial_user_token_a -> $final_user_token_a"
        print_success "Token B: $initial_user_token_b -> $final_user_token_b"
    fi

    # Print final vault info
    print_vault_info

    return $failed
}

##########################
### MAIN TEST RUNNER ####
##########################

# Add this function before the main() function
run_test() {
    local test_name="$1"
    local test_function="$2"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    print_section "Running Test: $test_name"
    if $test_function; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        print_success "Test passed: $test_name"
        return 0
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        print_error "Test failed: $test_name"
        return 1
    fi
}

query_slinky_prices() {
    query_contract $contract_address '{"get_prices":{}}'
}

main() {
    print_section "$TEST_SUITE_NAME"

    # Setup
    setup_suite

    query_contract $contract_address '{"get_config":{}}' --output json
    neutrond q bank balances $account --node $node
    execute_contract $contract_address '{"deposit":{}}' "940000000uibcusdc"
    neutrond q bank balances $account --node $node
    neutrond q bank balances $contract_address --node $node
    execute_contract $contract_address '{"deposit":{}}' "940000000uibcusdc"
    neutrond q bank balances $account --node $node
    neutrond q bank balances $contract_address --node $node
    query_contract $contract_address '{"get_config":{}}' --output json
    # # # Run test scenarios
    run_test "Query slinky prices" query_slinky_prices
    run_test "Basic Deposit Flow" test_basic_deposit_flow
    
    run_test "Dex Deposit -- real" test_dex_deposit

    # run_test "Contract Migration" test_contract_migration
    run_test "Dex Withdrawal" test_dex_withdrawal

    run_test "Basic Withdrawal Flow" test_basic_withdrawal_flow
    # # Teardown
    # teardown_suite

    # # Print test summary
    # print_section "TEST SUMMARY"
    # echo "Total tests: $TOTAL_TESTS"
    # echo "Passed: $PASSED_TESTS"
    # echo "Failed: $FAILED_TESTS"

    [ $FAILED_TESTS -eq 0 ]
}

# Run the test suite
main "$@"
