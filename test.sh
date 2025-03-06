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
SHOW_TX_HASH=false

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

update_config() {
    local max_blocks_old_token_a=$1
    local max_blocks_old_token_b=$2
    local base_fee=$3
    local base_deposit_percentage=$4
    local ambient_fee=$5
    local deposit_ambient=$6
    local deposit_cap=$7

    local msg='{"update_config":{'

    # Add optional parameters only if they are provided
    if [ ! -z "$max_blocks_old_token_a" ]; then
        msg+='"max_blocks_old_token_a":'$max_blocks_old_token_a','
    fi
    if [ ! -z "$max_blocks_old_token_b" ]; then
        msg+='"max_blocks_old_token_b":'$max_blocks_old_token_b','
    fi
    if [ ! -z "$base_fee" ]; then
        msg+='"base_fee":'$base_fee','
    fi
    if [ ! -z "$base_deposit_percentage" ]; then
        msg+='"base_deposit_percentage":'$base_deposit_percentage','
    fi
    if [ ! -z "$ambient_fee" ]; then
        msg+='"ambient_fee":'$ambient_fee','
    fi
    if [ ! -z "$deposit_ambient" ]; then
        msg+='"deposit_ambient":'$deposit_ambient','
    fi
    if [ ! -z "$deposit_cap" ]; then
        msg+='"deposit_cap":"'$deposit_cap'"'
    fi

    # Remove trailing comma if it exists
    msg=${msg%,}
    msg+='}}'

    execute_contract $contract_address "$msg"
}

execute_contract() {
    local contract_addr="$1"
    local msg="$2"
    local amount="$3"                   # Make sure we capture the amount parameter
    local from_account="${4:-$account}" # Use provided account or default to $account
    
    # print_info "Executing contract with account: $from_account"
    # print_info "Contract address: $contract_addr"
    # print_info "Message: $msg"
    # print_info "Amount: ${amount:-'no amount'}"

    # Execute the transaction and capture the response
    local resp=$(neutrond tx wasm execute "$contract_addr" "$msg" ${amount:+--amount "$amount"} \
        --from "$from_account" \
        --chain-id "$chain_id" \
        --gas-prices "$gas_price" \
        --gas-adjustment "$gas_adjustment" \
        --gas auto \
        --yes \
        --output json)

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
        "timestamp_stale": 300,
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

# Generic config update helper
test_config_change() {
    local param_name=$1
    local param_value=$2
        
    # Build the update message based on the parameter
    local msg="{\"update_config\":{"
    case "$param_name" in
        "max_blocks_old_token_a")
            msg+="\"max_blocks_old_token_a\":$param_value"
            ;;
        "max_blocks_old_token_b")
            msg+="\"max_blocks_old_token_b\":$param_value"
            ;;
        "deposit_cap")
            msg+="\"deposit_cap\":\"$param_value\""
            ;;
        "base_fee")
            msg+="\"fee_tier_config\":{\"fee_tiers\":[{\"fee\":$param_value,\"percentage\":30},{\"fee\":150,\"percentage\":70}]}"
            ;;
        *)
            print_error "Unhandled parameter: $param_name"
            return 1
            ;;
    esac
    msg+="}}"

    print_info "Sending message: $msg"

    # Execute config update
    local response
    response=$(execute_contract $contract_address "$msg")
    local exec_status=$?
    
    print_info "Execute response: $response"
    print_info "Execute status: $exec_status"

    if [ $exec_status -ne 0 ]; then
        print_error "Failed to update $param_name"
        return 1
    fi

    # Wait for transaction to be processed
    wait_for_tx

    # Verify config change
    local config=$(query_contract $contract_address '{"get_config":{}}')
    if [ $? -ne 0 ]; then
        print_error "Failed to query config"
        return 1
    fi
    
    print_info "Current config: $config"
    
    # Handle nested structure based on parameter name
    local actual_value
    case "$param_name" in
        "max_blocks_old_token_a")
            actual_value=$(echo "$config" | jq -r '.data.pair_data.token_0.max_blocks_old')
            ;;
        "max_blocks_old_token_b")
            actual_value=$(echo "$config" | jq -r '.data.pair_data.token_1.max_blocks_old')
            ;;
        "deposit_cap")
            actual_value=$(echo "$config" | jq -r '.data.deposit_cap')
            ;;
        "base_fee")
            actual_value=$(echo "$config" | jq -r '.data.fee_tier_config.fee_tiers[0].fee')
            ;;
    esac

    print_info "Actual value: $actual_value"
    print_info "Expected value: $param_value"

    if [ $? -ne 0 ] || [ -z "$actual_value" ] || [ "$actual_value" = "null" ]; then
        print_error "Failed to get $param_name from config response: $config"
        return 1
    fi

    if ! assert_equals "$param_value" "$actual_value" "$param_name update mismatch"; then
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

##########################
#### TEST SCENARIOS #####
##########################

# Basic deposit flow
test_basic_deposit_flow() {
    print_section "Basic Deposit Flow Test"
    local failed=0

    # Try deposits directly
    if ! run_subtest "NTRN Deposit" test_deposit "untrn" "20000000" "20000000"; then
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

# Example test scenario: Config update flow
test_config_update_flow() {
    print_section "Config Update Flow Test"
    local failed=0

    # Test token A max blocks update
    if ! run_subtest "Update Token A Max Blocks" test_config_change "max_blocks_old_token_a" "5"; then
        failed=1
    fi

    # Test token B max blocks update
    if ! run_subtest "Update Token B Max Blocks" test_config_change "max_blocks_old_token_b" "5"; then
        failed=1
    fi

    # Test deposit cap update
    if ! run_subtest "Update Deposit Cap" test_config_change "deposit_cap" "50000"; then
        failed=1
    fi

    # Test fee tier update
    if ! run_subtest "Update Fee Tiers" test_config_change "base_fee" "30"; then
        failed=1
    fi

    return $failed
}

# Example test scenario: Multiple deposits
test_multiple_deposits() {
    print_section "Multiple Deposits Test"
    local failed=0

    # Reset contract state first
    if ! setup_suite; then
        print_error "Failed to reset contract state"
        return 1
    fi

    if ! run_subtest "First NTRN Deposit" test_deposit "untrn" "10000000" "10000000"; then
        failed=1
    fi
    
    if [ $failed -eq 0 ]; then
        if ! run_subtest "Second NTRN Deposit" test_deposit "untrn" "20000000" "30000000"; then
            failed=1
        fi
    fi

    # Only verify LP tokens if deposits succeeded
    if [ $failed -eq 0 ]; then
        if ! run_subtest "LP Token Check" verify_lp_tokens "$account" "1"; then
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
    # local query_msg='{
    #     "get_prices": {
    #         "token_a": {
    #             "base": "NTRN",
    #             "decimals": 6
    #         },
    #         "token_b": {
    #             "base": "USDC",
    #             "decimals": 6
    #         },
  
    #         "max_blocks_old": 2
    #     }
    # }'
    # query_contract $oracle_address "$query_msg"
    query_contract $contract_address '{"get_prices":{}}'
}

main() {
    print_section "$TEST_SUITE_NAME"

    # Setup
    setup_suite


    # # Run test scenarios
    # run_test "Query slinky prices" query_slinky_prices
    run_test "Basic Deposit Flow" test_basic_deposit_flow
    # run_test "Config Update Flow" test_config_update_flow
    # run_test "Multiple Deposits" test_multiple_deposits
    run_test "Dex Deposit" test_dex_deposit
    # Add more test scenarios here as needed

    # Teardown
    teardown_suite

    # Print test summary
    print_section "TEST SUMMARY"
    echo "Total tests: $TOTAL_TESTS"
    echo "Passed: $PASSED_TESTS"
    echo "Failed: $FAILED_TESTS"

    [ $FAILED_TESTS -eq 0 ]
}

# Run the test suite
main "$@"
