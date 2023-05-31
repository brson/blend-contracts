use crate::{
    admin, allowance, balance,
    errors::TokenError,
    events,
    interface::{BlendPoolToken, CAP4606},
    pool::require_noncollateralized,
    storage::{self, Asset},
};
use soroban_sdk::{contractimpl, panic_with_error, Address, Bytes, Env};

pub struct Token;

#[contractimpl]
impl CAP4606 for Token {
    fn initialize(e: Env, admin: Address, decimal: u32, name: Bytes, symbol: Bytes) {
        if storage::has_pool(&e) {
            panic_with_error!(&e, TokenError::AlreadyInitializedError)
        }
        storage::write_pool(&e, &admin);

        storage::write_decimals(&e, &decimal);
        storage::write_name(&e, &name);
        storage::write_symbol(&e, &symbol);
    }

    // --------------------------------------------------------------------------------
    // Admin interface – privileged functions.
    // --------------------------------------------------------------------------------

    fn clawback(e: Env, from: Address, amount: i128) {
        let admin = storage::read_pool(&e);
        admin.require_auth();

        require_nonnegative(&e, amount);
        balance::spend_balance_no_auth(&e, &from, &amount).unwrap();

        events::clawback(&e, admin, from, amount);
    }

    fn mint(e: Env, to: Address, amount: i128) {
        let admin = storage::read_pool(&e);
        admin.require_auth();

        require_nonnegative(&e, amount);
        balance::receive_balance(&e, &to, &amount).unwrap();

        events::mint(&e, admin, to, amount);
    }

    fn set_admin(e: Env, _new_admin: Address) {
        panic_with_error!(&e, TokenError::NotImplemented)
    }

    fn set_authorized(e: Env, _id: Address, _authorize: bool) {
        panic_with_error!(&e, TokenError::NotImplemented)
    }

    // --------------------------------------------------------------------------------
    // Token interface
    // --------------------------------------------------------------------------------

    fn increase_allowance(e: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth();

        require_nonnegative(&e, amount);
        allowance::increase_allowance(&e, &from, &spender, &amount).unwrap();

        events::increase_allowance(&e, from, spender, amount);
    }

    fn decrease_allowance(e: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth();

        require_nonnegative(&e, amount);
        allowance::decrease_allowance(&e, &from, &spender, &amount).unwrap();

        events::decrease_allowance(&e, from, spender, amount);
    }

    fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        require_nonnegative(&e, amount);
        require_noncollateralized(&e, &from);
        balance::spend_balance(&e, &from, &amount).unwrap();
        balance::receive_balance(&e, &to, &amount).unwrap();

        events::transfer(&e, from, to, amount);
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        require_nonnegative(&e, amount);
        require_noncollateralized(&e, &from);
        allowance::spend_allowance(&e, &from, &spender, &amount).unwrap();
        balance::spend_balance(&e, &from, &amount).unwrap();
        balance::receive_balance(&e, &to, &amount).unwrap();

        events::transfer(&e, from, to, amount);
    }

    fn burn(e: Env, _from: Address, _amount: i128) {
        panic_with_error!(&e, TokenError::NotImplemented)
    }

    fn burn_from(e: Env, _spender: Address, _from: Address, _amount: i128) {
        panic_with_error!(&e, TokenError::NotImplemented)
    }

    // --------------------------------------------------------------------------------
    // Read-only Token interface
    // --------------------------------------------------------------------------------

    fn balance(e: Env, id: Address) -> i128 {
        storage::read_balance(&e, &id)
    }

    fn spendable(e: Env, id: Address) -> i128 {
        storage::read_balance(&e, &id)
    }

    fn authorized(e: Env, _id: Address) -> bool {
        panic_with_error!(&e, TokenError::NotImplemented)
    }

    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        storage::read_allowance(&e, &from, &spender)
    }

    // --------------------------------------------------------------------------------
    // Descriptive Interface
    // --------------------------------------------------------------------------------

    fn decimals(e: Env) -> u32 {
        storage::read_decimals(&e)
    }

    fn name(e: Env) -> Bytes {
        storage::read_name(&e)
    }

    fn symbol(e: Env) -> Bytes {
        storage::read_symbol(&e)
    }
}

pub struct BToken;

#[contractimpl]
impl BlendPoolToken for BToken {
    fn pool(e: Env) -> Address {
        storage::read_pool(&e)
    }

    fn asset(e: Env) -> Asset {
        storage::read_asset(&e)
    }

    fn initialize_asset(e: Env, admin: Address, asset: Address, index: u32) {
        admin::require_is_pool(&e, &admin);
        admin.require_auth();

        if storage::has_asset(&e) {
            panic_with_error!(&e, TokenError::AlreadyInitializedError)
        }
        storage::write_asset(
            &e,
            &Asset {
                id: asset,
                res_index: index,
            },
        );
        storage::write_pool(&e, &admin);
    }
}

fn require_nonnegative(e: &Env, amount: i128) {
    if amount.is_negative() {
        panic_with_error!(&e, TokenError::NegativeAmountError)
    }
}
