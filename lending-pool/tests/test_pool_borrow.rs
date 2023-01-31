#![cfg(test)]
use cast::i128;
use soroban_auth::{Identifier, Signature};
use soroban_sdk::{
    testutils::{Accounts, Ledger, LedgerInfo},
    Env, Status,
};

mod common;
use crate::common::{
    create_mock_oracle, create_wasm_lending_pool, generate_contract_id, pool_helper, PoolError,
    TokenClient,
};

#[test]
fn test_pool_borrow_no_collateral_panics() {
    let e = Env::default();

    let bombadil = e.accounts().generate_and_create();
    let bombadil_id = Identifier::Account(bombadil.clone());

    let sauron = e.accounts().generate_and_create();
    let sauron_id = Identifier::Account(sauron.clone());

    let (mock_oracle, mock_oracle_client) = create_mock_oracle(&e);

    let backstop_address = generate_contract_id(&e);
    let (pool, pool_client) = create_wasm_lending_pool(&e);
    let pool_id = Identifier::Contract(pool.clone());
    pool_client.initialize(
        &bombadil_id,
        &mock_oracle,
        &backstop_address,
        &0_200_000_000,
    );
    pool_client.with_source_account(&bombadil).set_status(&0);

    let (asset1_id, _, _) = pool_helper::setup_reserve(&e, &pool_id, &pool_client, &bombadil);

    mock_oracle_client.set_price(&asset1_id, &2_0000000);

    // borrow
    let borrow_amount = 0_0000001;
    let result =
        pool_client
            .with_source_account(&sauron)
            .try_borrow(&asset1_id, &borrow_amount, &sauron_id);
    // TODO: The try_borrow is returning a different error than it should
    match result {
        Ok(_) => assert!(false),
        Err(error) => match error {
            Ok(p_error) => assert_eq!(p_error, PoolError::InvalidHf),
            Err(s_error) => assert_eq!(s_error, Status::from_contract_error(3)),
        },
    }
}

#[test]
fn test_pool_borrow_bad_hf_panics() {
    let e = Env::default();

    let bombadil = e.accounts().generate_and_create();
    let bombadil_id = Identifier::Account(bombadil.clone());

    let sauron = e.accounts().generate_and_create();
    let sauron_id = Identifier::Account(sauron.clone());

    let (mock_oracle, mock_oracle_client) = create_mock_oracle(&e);

    let backstop_address = generate_contract_id(&e);
    let (pool, pool_client) = create_wasm_lending_pool(&e);
    let pool_id = Identifier::Contract(pool.clone());
    pool_client.initialize(
        &bombadil_id,
        &mock_oracle,
        &backstop_address,
        &0_200_000_000,
    );
    pool_client.with_source_account(&bombadil).set_status(&0);

    let (asset1_id, b_token1_id, _) =
        pool_helper::setup_reserve(&e, &pool_id, &pool_client, &bombadil);

    mock_oracle_client.set_price(&asset1_id, &2_0000000);

    let asset1_client = TokenClient::new(&e, asset1_id.clone());
    let b_token1_client = TokenClient::new(&e, b_token1_id.clone());
    asset1_client.with_source_account(&bombadil).mint(
        &Signature::Invoker,
        &0,
        &sauron_id,
        &10_0000000,
    );
    asset1_client.with_source_account(&sauron).incr_allow(
        &Signature::Invoker,
        &0,
        &pool_id,
        &(u64::MAX as i128),
    );

    let minted_btokens = pool_client
        .with_source_account(&sauron)
        .supply(&asset1_id, &1_0000000);
    assert_eq!(b_token1_client.balance(&sauron_id), minted_btokens as i128);

    // borrow
    let borrow_amount = 0_5358000; // 0.75 cf * 0.75 lf => 0.5625 / 1.05 hf min => 0.5357 max
    let result =
        pool_client
            .with_source_account(&sauron)
            .try_borrow(&asset1_id, &borrow_amount, &sauron_id);
    match result {
        Ok(_) => {
            assert!(false);
        }
        Err(error) => match error {
            Ok(p_error) => assert_eq!(p_error, PoolError::InvalidHf),
            Err(s_error) => assert_eq!(s_error, Status::from_contract_error(3)),
        },
    }
}

#[test]
fn test_pool_borrow_good_hf_borrows() {
    let e = Env::default();

    let bombadil = e.accounts().generate_and_create();
    let bombadil_id = Identifier::Account(bombadil.clone());

    let samwise = e.accounts().generate_and_create();
    let samwise_id = Identifier::Account(samwise.clone());

    let (mock_oracle, mock_oracle_client) = create_mock_oracle(&e);

    let backstop_address = generate_contract_id(&e);
    let (pool, pool_client) = create_wasm_lending_pool(&e);
    let pool_id = Identifier::Contract(pool.clone());
    pool_client.initialize(
        &bombadil_id,
        &mock_oracle,
        &backstop_address,
        &0_200_000_000,
    );
    pool_client.with_source_account(&bombadil).set_status(&0);

    let (asset1_id, b_token1_id, d_token1_id) =
        pool_helper::setup_reserve(&e, &pool_id, &pool_client, &bombadil);

    mock_oracle_client.set_price(&asset1_id, &2_0000000);

    let asset1_client = TokenClient::new(&e, asset1_id.clone());
    let b_token1_client = TokenClient::new(&e, b_token1_id.clone());
    let d_token1_client = TokenClient::new(&e, d_token1_id.clone());
    asset1_client.with_source_account(&bombadil).mint(
        &Signature::Invoker,
        &0,
        &samwise_id,
        &10_0000000,
    );
    asset1_client.with_source_account(&samwise).incr_allow(
        &Signature::Invoker,
        &0,
        &pool_id,
        &(u64::MAX as i128),
    );

    let minted_btokens = pool_client
        .with_source_account(&samwise)
        .supply(&asset1_id, &1_0000000);
    assert_eq!(b_token1_client.balance(&samwise_id), minted_btokens as i128);

    // borrow
    let borrow_amount = 0_5357000; // 0.75 cf * 0.75 lf => 0.5625 / 1.05 hf min => 0.5357 max
    let minted_dtokens =
        pool_client
            .with_source_account(&samwise)
            .borrow(&asset1_id, &borrow_amount, &samwise_id);
    assert_eq!(
        asset1_client.balance(&samwise_id),
        10_0000000 - 1_0000000 + 0_5357000
    );
    assert_eq!(asset1_client.balance(&pool_id), 1_0000000 - 0_5357000);
    assert_eq!(b_token1_client.balance(&samwise_id), minted_btokens as i128);
    assert_eq!(d_token1_client.balance(&samwise_id), minted_dtokens as i128);
}

#[test]
fn test_pool_borrow_on_ice_panics() {
    let e = Env::default();

    let bombadil = e.accounts().generate_and_create();
    let bombadil_id = Identifier::Account(bombadil.clone());

    let sauron = e.accounts().generate_and_create();
    let sauron_id = Identifier::Account(sauron.clone());

    let (mock_oracle, mock_oracle_client) = create_mock_oracle(&e);

    let backstop_address = generate_contract_id(&e);
    let (pool, pool_client) = create_wasm_lending_pool(&e);
    let pool_id = Identifier::Contract(pool.clone());
    pool_client.initialize(
        &bombadil_id,
        &mock_oracle,
        &backstop_address,
        &0_200_000_000,
    );
    pool_client.with_source_account(&bombadil).set_status(&1);

    let (asset1_id, b_token1_id, _) =
        pool_helper::setup_reserve(&e, &pool_id, &pool_client, &bombadil);

    mock_oracle_client.set_price(&asset1_id, &2_0000000);

    let asset1_client = TokenClient::new(&e, asset1_id.clone());
    let b_token1_client = TokenClient::new(&e, b_token1_id.clone());
    asset1_client.with_source_account(&bombadil).mint(
        &Signature::Invoker,
        &0,
        &sauron_id,
        &10_0000000,
    );
    asset1_client.with_source_account(&sauron).incr_allow(
        &Signature::Invoker,
        &0,
        &pool_id,
        &(u64::MAX as i128),
    );

    let minted_btokens = pool_client
        .with_source_account(&sauron)
        .supply(&asset1_id, &1_0000000);
    assert_eq!(b_token1_client.balance(&sauron_id), minted_btokens as i128);

    // borrow
    let borrow_amount = 0_5358000; // 0.75 cf * 0.75 lf => 0.5625 / 1.05 hf min => 0.5357 max
    let result =
        pool_client
            .with_source_account(&sauron)
            .try_borrow(&asset1_id, &borrow_amount, &sauron_id);
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

#[test]
fn test_pool_borrow_frozen_panics() {
    let e = Env::default();

    let bombadil = e.accounts().generate_and_create();
    let bombadil_id = Identifier::Account(bombadil.clone());

    let sauron = e.accounts().generate_and_create();
    let sauron_id = Identifier::Account(sauron.clone());

    let (mock_oracle, mock_oracle_client) = create_mock_oracle(&e);

    let backstop_address = generate_contract_id(&e);
    let (pool, pool_client) = create_wasm_lending_pool(&e);
    let pool_id = Identifier::Contract(pool.clone());
    pool_client.initialize(
        &bombadil_id,
        &mock_oracle,
        &backstop_address,
        &0_200_000_000,
    );
    pool_client.with_source_account(&bombadil).set_status(&1);

    let (asset1_id, b_token1_id, _) =
        pool_helper::setup_reserve(&e, &pool_id, &pool_client, &bombadil);

    mock_oracle_client.set_price(&asset1_id, &2_0000000);

    let asset1_client = TokenClient::new(&e, asset1_id.clone());
    let b_token1_client = TokenClient::new(&e, b_token1_id.clone());
    asset1_client.with_source_account(&bombadil).mint(
        &Signature::Invoker,
        &0,
        &sauron_id,
        &10_0000000,
    );
    asset1_client.with_source_account(&sauron).incr_allow(
        &Signature::Invoker,
        &0,
        &pool_id,
        &(u64::MAX as i128),
    );

    let minted_btokens = pool_client
        .with_source_account(&sauron)
        .supply(&asset1_id, &1_0000000);
    assert_eq!(b_token1_client.balance(&sauron_id), minted_btokens as i128);

    pool_client.with_source_account(&bombadil).set_status(&2);
    // borrow
    let borrow_amount = 0_5358000; // 0.75 cf * 0.75 lf => 0.5625 / 1.05 hf min => 0.5357 max
    let result =
        pool_client
            .with_source_account(&sauron)
            .try_borrow(&asset1_id, &borrow_amount, &sauron_id);
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

// TODO: Unit test for issues/2
// -> d_rate > 1, try and borrow one stroop
#[test]
fn test_pool_borrow_one_stroop() {
    let e = Env::default();

    let bombadil = e.accounts().generate_and_create();
    let bombadil_id = Identifier::Account(bombadil.clone());

    let samwise = e.accounts().generate_and_create();
    let samwise_id = Identifier::Account(samwise.clone());

    let (mock_oracle, mock_oracle_client) = create_mock_oracle(&e);

    let backstop_address = generate_contract_id(&e);
    let (pool, pool_client) = create_wasm_lending_pool(&e);
    let pool_id = Identifier::Contract(pool.clone());
    pool_client.initialize(
        &bombadil_id,
        &mock_oracle,
        &backstop_address,
        &0_200_000_000,
    );
    pool_client.with_source_account(&bombadil).set_status(&0);

    let (asset1_id, b_token1_id, d_token1_id) =
        pool_helper::setup_reserve(&e, &pool_id, &pool_client, &bombadil);

    mock_oracle_client.set_price(&asset1_id, &2_0000000);

    let asset1_client = TokenClient::new(&e, asset1_id.clone());
    let b_token1_client = TokenClient::new(&e, b_token1_id.clone());
    let d_token1_client = TokenClient::new(&e, d_token1_id.clone());
    asset1_client.with_source_account(&bombadil).mint(
        &Signature::Invoker,
        &0,
        &samwise_id,
        &10_0000000,
    );
    asset1_client.with_source_account(&samwise).incr_allow(
        &Signature::Invoker,
        &0,
        &pool_id,
        &i128(u64::MAX),
    );

    // supply
    let minted_btokens = pool_client
        .with_source_account(&samwise)
        .supply(&asset1_id, &1_0000000);
    assert_eq!(b_token1_client.balance(&samwise_id), minted_btokens);

    // borrow
    let minted_dtokens =
        pool_client
            .with_source_account(&samwise)
            .borrow(&asset1_id, &0_5355000, &samwise_id);
    assert_eq!(d_token1_client.balance(&samwise_id), minted_dtokens);

    // allow interest to accumulate
    // IR -> 6%
    e.ledger().set(LedgerInfo {
        timestamp: 12345,
        protocol_version: 1,
        sequence_number: 6307200, // 1 year
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    // borrow 1 stroop
    let borrow_amount = 0_0000001;
    let minted_dtokens_2 =
        pool_client
            .with_source_account(&samwise)
            .borrow(&asset1_id, &borrow_amount, &samwise_id);
    assert_eq!(
        asset1_client.balance(&samwise_id),
        10_0000000 - 1_0000000 + 0_5355000 + 0_0000001
    );
    assert_eq!(
        asset1_client.balance(&pool_id),
        1_0000000 - 0_5355000 - 0_0000001
    );
    assert_eq!(
        d_token1_client.balance(&samwise_id),
        (minted_dtokens + minted_dtokens_2)
    );
    assert_eq!(minted_dtokens_2, 1);
}
