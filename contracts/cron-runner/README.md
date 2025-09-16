# Cron Runner Contract

This contract is designed to be called by the Neutron Cron module to perform deposit/withdrawal cycles

## Overview

The contract performs the following operations:
1. Takes a list of vault addresses
2. Calls `dex_withdrawal` on each vault to withdraw all active DEX positions
3. Calls `dex_deposit` on each vault to redeploy liquidity

## Authorization

The contract supports two types of authorized addresses:
- **Cron Module**: Can execute the main rebalancing function
- **Admin**: Can update configuration and manage the contract

## Functions

### Execute Messages

#### `RunRebalancing {}`
- **Authorization**: Cron address or admin address
- **Purpose**: Main function called to rebalance all vaults
- **Behavior**: Calls `dex_withdrawal` followed by `dex_deposit` on each configured vault

#### `UpdateConfig { new_config }`
- **Authorization**: Admin address only
- **Purpose**: Update the contract configuration
- **Parameters**:
  - `vault_addresses`: Optional new list of vault addresses (replaces current list)
  - `cron_address`: Optional new cron address
  - `admin_address`: Optional new admin address

### Query Messages

#### `GetConfig {}`
Returns the current configuration including vault addresses, cron address, and admin address.

#### `GetVaultList {}`
Returns just the list of vault addresses.

## Usage

1. **Instantiate** the contract with:
   - List of vault contract addresses to manage
   - Cron module address (typically the Neutron cron module)
   - Admin address (for configuration management and migrations)

2. **Automated Operation**: The Cron module will automatically call `RunRebalancing{}` at the beginning of every block

3. **Management**: Admins can update the vault list and authorization settings as needed

## Example Instantiation

```json
{
  "vault_addresses": [
    "neutron1vault1...",
    "neutron1vault2...",
    "neutron1vault3..."
  ],
  "cron_address": "neutron1cron...",
  "admin_address": "neutron1admin..."
}
```

## Message Flow

For each vault in the list, the contract creates two messages:
1. `{"dex_withdrawal": {}}` - Withdraws all active DEX positions
2. `{"dex_deposit": {}}` - Redeploys liquidity with fresh parameters

These messages are executed in order, ensuring that each vault first withdraws its positions before redepositing.
