use lending_pool::{Request, ReserveEmissionMetadata};
use soroban_sdk::{testutils::Address as _, vec, Address, Symbol, Vec};

use crate::{
    pool::default_reserve_metadata,
    test_fixture::{TestFixture, TokenIndex, SCALAR_7},
};

/// Create a test fixture with a pool and a whale depositing and borrowing all assets
pub fn create_fixture_with_data<'a>(wasm: bool) -> (TestFixture<'a>, Address) {
    let mut fixture = TestFixture::create(wasm);
    fixture.env.mock_all_auths();
    fixture.env.budget().reset_unlimited();
    // create pool
    fixture.create_pool(Symbol::new(&fixture.env, "Teapot"), 0_100_000_000);

    let mut usdc_config = default_reserve_metadata();
    usdc_config.decimals = 6;
    usdc_config.c_factor = 0_900_0000;
    usdc_config.l_factor = 0_950_0000;
    usdc_config.util = 0_850_0000;
    fixture.create_pool_reserve(0, TokenIndex::USDC, usdc_config);

    let mut xlm_config = default_reserve_metadata();
    xlm_config.c_factor = 0_750_0000;
    xlm_config.l_factor = 0_750_0000;
    xlm_config.util = 0_500_0000;
    fixture.create_pool_reserve(0, TokenIndex::XLM, xlm_config);

    let mut weth_config = default_reserve_metadata();
    weth_config.decimals = 9;
    weth_config.c_factor = 0_800_0000;
    weth_config.l_factor = 0_800_0000;
    weth_config.util = 0_700_0000;
    fixture.create_pool_reserve(0, TokenIndex::WETH, weth_config);

    // enable emissions for pool
    let pool_fixture = &fixture.pools[0];

    let reserve_emissions: soroban_sdk::Vec<ReserveEmissionMetadata> = soroban_sdk::vec![
        &fixture.env,
        ReserveEmissionMetadata {
            res_index: 0, // USDC
            res_type: 0,  // d_token
            share: 0_600_0000
        },
        ReserveEmissionMetadata {
            res_index: 1, // XLM
            res_type: 1,  // b_token
            share: 0_400_0000
        },
    ];
    pool_fixture.pool.set_emissions_config(&reserve_emissions);

    // mint whale tokens
    let frodo = Address::random(&fixture.env);
    fixture.tokens[TokenIndex::USDC].mint(&frodo, &(100_000 * 10i128.pow(6)));
    fixture.tokens[TokenIndex::XLM].mint(&frodo, &(1_000_000 * SCALAR_7));
    fixture.tokens[TokenIndex::WETH].mint(&frodo, &(100 * 10i128.pow(9)));
    fixture.tokens[TokenIndex::BSTOP].mint(&frodo, &(10_000_000 * SCALAR_7));

    // deposit into backstop, add to reward zone
    fixture
        .backstop
        .deposit(&frodo, &pool_fixture.pool.address, &(2_000_000 * SCALAR_7));
    fixture
        .backstop
        .add_reward(&pool_fixture.pool.address, &Address::random(&fixture.env));
    pool_fixture.pool.update_status();

    // enable emissions
    fixture.emitter.distribute();
    fixture.backstop.update_emission_cycle();
    pool_fixture.pool.update_emissions();

    fixture.jump(60);

    // supply and borrow USDC for 80% utilization (close to target)
    let requests: Vec<Request> = vec![
        &fixture.env,
        Request {
            request_type: 2,
            address: fixture.tokens[TokenIndex::USDC].address.clone(),
            amount: 10_000 * 10i128.pow(6),
        },
        Request {
            request_type: 4,
            address: fixture.tokens[TokenIndex::USDC].address.clone(),
            amount: 8_000 * 10i128.pow(6),
        },
    ];
    pool_fixture.pool.submit(&frodo, &frodo, &frodo, &requests);

    // supply and borrow WETH for 50% utilization (below target)
    let requests: Vec<Request> = vec![
        &fixture.env,
        Request {
            request_type: 2,
            address: fixture.tokens[TokenIndex::WETH].address.clone(),
            amount: 10 * 10i128.pow(9),
        },
        Request {
            request_type: 4,
            address: fixture.tokens[TokenIndex::WETH].address.clone(),
            amount: 5 * 10i128.pow(9),
        },
    ];
    pool_fixture.pool.submit(&frodo, &frodo, &frodo, &requests);

    // supply and borrow XLM for 65% utilization (above target)
    let requests: Vec<Request> = vec![
        &fixture.env,
        Request {
            request_type: 2,
            address: fixture.tokens[TokenIndex::XLM].address.clone(),
            amount: 100_000 * SCALAR_7,
        },
        Request {
            request_type: 4,
            address: fixture.tokens[TokenIndex::XLM].address.clone(),
            amount: 65_000 * SCALAR_7,
        },
    ];
    pool_fixture.pool.submit(&frodo, &frodo, &frodo, &requests);

    fixture.jump(60 * 60); // 1 hr

    (fixture, frodo)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_create_fixture_with_data_wasm() {
        let (fixture, frodo) = create_fixture_with_data(true);
        let pool_fixture = &fixture.pools[0];

        // validate backstop deposit
        assert_eq!(
            2_000_000 * SCALAR_7,
            fixture.tokens[TokenIndex::BSTOP].balance(&fixture.backstop.address)
        );

        // validate pool actions
        assert_eq!(
            2_000 * 10i128.pow(6),
            fixture.tokens[TokenIndex::USDC].balance(&pool_fixture.pool.address)
        );
        assert_eq!(
            35_000 * SCALAR_7,
            fixture.tokens[TokenIndex::XLM].balance(&pool_fixture.pool.address)
        );
        assert_eq!(
            5 * 10i128.pow(9),
            fixture.tokens[TokenIndex::WETH].balance(&pool_fixture.pool.address)
        );

        assert_eq!(
            98_000 * 10i128.pow(6),
            fixture.tokens[TokenIndex::USDC].balance(&frodo)
        );
        assert_eq!(
            965_000 * SCALAR_7,
            fixture.tokens[TokenIndex::XLM].balance(&frodo)
        );
        assert_eq!(
            95 * 10i128.pow(9),
            fixture.tokens[TokenIndex::WETH].balance(&frodo)
        );

        // validate emissions are turned on
        assert_eq!(
            (
                0_300_0000,
                fixture.env.ledger().timestamp() - 60 * 61 + 7 * 24 * 60 * 60
            ),
            fixture.backstop.pool_eps(&pool_fixture.pool.address)
        );
        let (emis_config, emis_data) = pool_fixture
            .pool
            .get_reserve_emissions(&fixture.tokens[TokenIndex::USDC].address, &0)
            .unwrap();
        assert_eq!(
            emis_data.last_time,
            fixture.env.ledger().timestamp() - 60 * 61
        );
        assert_eq!(emis_data.index, 0);
        assert_eq!(0_180_0000, emis_config.eps);
    }

    #[test]
    fn test_create_fixture_with_data_rlib() {
        let (fixture, frodo) = create_fixture_with_data(false);
        let pool_fixture = &fixture.pools[0];

        // validate backstop deposit
        assert_eq!(
            2_000_000 * SCALAR_7,
            fixture.tokens[TokenIndex::BSTOP].balance(&fixture.backstop.address)
        );

        // validate pool actions
        assert_eq!(
            2_000 * 10i128.pow(6),
            fixture.tokens[TokenIndex::USDC].balance(&pool_fixture.pool.address)
        );
        assert_eq!(
            35_000 * SCALAR_7,
            fixture.tokens[TokenIndex::XLM].balance(&pool_fixture.pool.address)
        );
        assert_eq!(
            5 * 10i128.pow(9),
            fixture.tokens[TokenIndex::WETH].balance(&pool_fixture.pool.address)
        );

        assert_eq!(
            98_000 * 10i128.pow(6),
            fixture.tokens[TokenIndex::USDC].balance(&frodo)
        );
        assert_eq!(
            965_000 * SCALAR_7,
            fixture.tokens[TokenIndex::XLM].balance(&frodo)
        );
        assert_eq!(
            95 * 10i128.pow(9),
            fixture.tokens[TokenIndex::WETH].balance(&frodo)
        );

        // validate emissions are turned on
        assert_eq!(
            (
                0_300_0000,
                fixture.env.ledger().timestamp() - 60 * 61 + 7 * 24 * 60 * 60
            ),
            fixture.backstop.pool_eps(&pool_fixture.pool.address)
        );
        let (emis_config, emis_data) = pool_fixture
            .pool
            .get_reserve_emissions(&fixture.tokens[TokenIndex::USDC].address, &0)
            .unwrap();
        assert_eq!(
            emis_data.last_time,
            fixture.env.ledger().timestamp() - 60 * 61
        );
        assert_eq!(emis_data.index, 0);
        assert_eq!(0_180_0000, emis_config.eps);
    }
}
