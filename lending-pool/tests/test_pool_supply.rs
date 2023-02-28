#![cfg(test)]
use cast::i128;
use soroban_sdk::{testutils::Address as AddressTestTrait, Address, Env, Status};

mod common;
use crate::common::{
    create_mock_oracle, create_wasm_lending_pool, generate_contract_id, pool_helper, PoolError,
    TokenClient,
};

#[test]
fn test_pool_supply_on_ice() {
    let e = Env::default();

    let bombadil = Address::random(&e);

    let sauron = Address::random(&e);

    let (mock_oracle, mock_oracle_client) = create_mock_oracle(&e);

    let backstop_id = generate_contract_id(&e);
    let backstop = Address::from_contract_id(&e, &backstop_id);
    let (pool_id, pool_client) = create_wasm_lending_pool(&e);
    let pool = Address::from_contract_id(&e, &pool_id);
    pool_client.initialize(
        &bombadil,
        &mock_oracle,
        &backstop_id,
        &backstop,
        &0_200_000_000,
    );
    pool_client.set_status(&bombadil, &1);

    let (asset1_id, _b_token1_id, _) =
        pool_helper::setup_reserve(&e, &pool, &pool_client, &bombadil);

    mock_oracle_client.set_price(&asset1_id, &2_0000000);

    let asset1_client = TokenClient::new(&e, &asset1_id);

    asset1_client.mint(&bombadil, &sauron, &10_0000000);
    asset1_client.incr_allow(&sauron, &pool, &i128(u64::MAX));

    let result = pool_client.try_supply(&sauron, &asset1_id, &1_0000000);

    match result {
        Ok(_) => assert!(true),
        Err(_) => assert!(false),
    }
}

#[test]
fn test_pool_supply_frozen_panics() {
    let e = Env::default();

    let bombadil = Address::random(&e);

    let sauron = Address::random(&e);

    let (mock_oracle, mock_oracle_client) = create_mock_oracle(&e);

    let backstop_id = generate_contract_id(&e);
    let backstop = Address::from_contract_id(&e, &backstop_id);
    let (pool_id, pool_client) = create_wasm_lending_pool(&e);
    let pool = Address::from_contract_id(&e, &pool_id);
    pool_client.initialize(
        &bombadil,
        &mock_oracle,
        &backstop_id,
        &backstop,
        &0_200_000_000,
    );
    pool_client.set_status(&bombadil, &2);

    let (asset1_id, _b_token1_id, _) =
        pool_helper::setup_reserve(&e, &pool, &pool_client, &bombadil);

    mock_oracle_client.set_price(&asset1_id, &2_0000000);

    let asset1_client = TokenClient::new(&e, &asset1_id);

    asset1_client.mint(&bombadil, &sauron, &10_0000000);
    asset1_client.incr_allow(&sauron, &pool, &i128(u64::MAX));

    let result = pool_client.try_supply(&sauron, &asset1_id, &1_0000000);

    match result {
        Ok(_) => {
            assert!(false);
        }
        Err(error) => match error {
            Ok(p_error) => assert_eq!(p_error, PoolError::InvalidPoolStatus),
            Err(s_error) => assert_eq!(s_error, Status::from_contract_error(4)),
        },
    }
}
