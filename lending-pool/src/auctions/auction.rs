use crate::{
    errors::PoolError,
    storage,
    user_data::{UserAction, UserData},
};
use cast::i128;
use fixed_point_math::FixedPoint;
use soroban_sdk::{contracttype, Address, BytesN, Env, Map, Vec};

use super::{
    backstop_interest_auction::{
        calc_fill_interest_auction, create_interest_auction_data, fill_interest_auction,
    },
    bad_debt_auction::{
        calc_fill_bad_debt_auction, create_bad_debt_auction_data, fill_bad_debt_auction,
    },
    user_liquidation_auction::{
        calc_fill_user_liq_auction, create_user_liq_auction_data, fill_user_liq_auction,
    },
};

#[derive(Clone, PartialEq)]
#[repr(u32)]
pub enum AuctionType {
    UserLiquidation = 0,
    BadDebtAuction = 1,
    InterestAuction = 2,
}

impl AuctionType {
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => AuctionType::UserLiquidation,
            1 => AuctionType::BadDebtAuction,
            2 => AuctionType::InterestAuction,
            _ => panic!("internal error"),
        }
    }
}

#[derive(Clone)]
#[contracttype]
pub struct LiquidationMetadata {
    pub collateral: Map<BytesN<32>, i128>,
    pub liability: Map<BytesN<32>, i128>,
}

#[derive(Clone)]
#[contracttype]
pub struct AuctionQuote {
    pub bid: Vec<(BytesN<32>, i128)>,
    pub lot: Vec<(BytesN<32>, i128)>,
    pub block: u32,
}

// TODO: Rename symbol once auction code ported over
#[derive(Clone)]
#[contracttype]
pub struct AuctionData {
    pub bid: Map<u32, i128>,
    pub lot: Map<u32, i128>,
    pub block: u32,
}

/// Create an auction. Stores the resulting auction to the ledger to begin on the next block
///
/// Returns the AuctionData object created.
///
/// ### Arguments
/// * `auction_type` - The type of auction being created
///
/// ### Errors
/// If the auction is unable to be created
pub fn create(e: &Env, auction_type: u32) -> Result<AuctionData, PoolError> {
    let backstop = storage::get_backstop_address(e);
    let auction_data = match AuctionType::from_u32(auction_type) {
        AuctionType::UserLiquidation => {
            return Err(PoolError::BadRequest);
        }
        AuctionType::BadDebtAuction => create_bad_debt_auction_data(e, &backstop),
        AuctionType::InterestAuction => create_interest_auction_data(e, &backstop),
    }?;

    storage::set_auction(e, &auction_type, &backstop, &auction_data);

    return Ok(auction_data);
}

/// Create a liquidation auction. Stores the resulting auction to the ledger to begin on the next block
///
/// Returns the AuctionData object created.
///
/// ### Arguments
/// * `auction_type` - The type of auction being created
///
/// ### Errors
/// If the auction is unable to be created
pub fn create_liquidation(
    e: &Env,
    user: &Address,
    liq_data: LiquidationMetadata,
) -> Result<AuctionData, PoolError> {
    let auction_data = create_user_liq_auction_data(e, user, liq_data)?;

    storage::set_auction(
        &e,
        &(AuctionType::UserLiquidation as u32),
        &user,
        &auction_data,
    );

    return Ok(auction_data);
}

/// Delete a liquidation auction if the user being liquidated is no longer eligible for liquidation.
///
/// ### Arguments
/// * `auction_type` - The type of auction being created
///
/// ### Errors
/// If no auction exists for the user or if the user is still eligible for liquidation.
pub fn delete_liquidation(e: &Env, user: &Address) -> Result<(), PoolError> {
    if !storage::has_auction(e, &(AuctionType::UserLiquidation as u32), &user) {
        return Err(PoolError::BadRequest);
    }

    let pool_config = storage::get_pool_config(e);
    // no action is being take when calculating the users health factor
    let action = UserAction {
        asset: BytesN::from_array(e, &[0; 32]),
        b_token_delta: 0,
        d_token_delta: 0,
    };
    let user_data = UserData::load(e, &pool_config, user, &action);

    if user_data.collateral_base > user_data.liability_base {
        storage::del_auction(e, &(AuctionType::UserLiquidation as u32), &user);
        Ok(())
    } else {
        return Err(PoolError::InvalidHf);
    }
}

/// Preview the quote the auction will be filled at
///
/// ### Arguments
/// * `auction_type` - The type of auction to get a quote for
/// * `user` - The user involved in the auction
///
/// ### Errors
/// If the auction does not exist
pub fn preview_fill(e: &Env, auction_type: u32, user: &Address) -> AuctionQuote {
    let auction_data = storage::get_auction(e, &auction_type, user);
    match AuctionType::from_u32(auction_type) {
        AuctionType::UserLiquidation => calc_fill_user_liq_auction(e, &auction_data),
        AuctionType::BadDebtAuction => calc_fill_bad_debt_auction(e, &auction_data),
        AuctionType::InterestAuction => calc_fill_interest_auction(e, &auction_data),
    }
}

/// Fills the auction from the invoker. The filler is expected to maintain allowances to both
/// the pool and the backstop module.
///
/// TODO: Use auth-next to avoid required allowances
///
/// ### Arguments
/// * `auction_type` - The type of auction to fill
/// * `user` - The user involved in the auction
/// * `filler` - The Address filling the auction
///
/// ### Errors
/// If the auction does not exist, or if the pool is unable to fulfill either side
/// of the auction quote
pub fn fill(
    e: &Env,
    auction_type: u32,
    user: &Address,
    filler: &Address,
) -> Result<AuctionQuote, PoolError> {
    let auction_data = storage::get_auction(e, &auction_type, user);
    let quote = match AuctionType::from_u32(auction_type) {
        AuctionType::UserLiquidation => fill_user_liq_auction(e, &auction_data, user, filler),
        AuctionType::BadDebtAuction => fill_bad_debt_auction(e, &auction_data, filler),
        AuctionType::InterestAuction => fill_interest_auction(e, &auction_data, filler),
    };

    storage::del_auction(e, &auction_type, user);

    Ok(quote)
}

/// Get the current fill modifiers for the auction
///
/// Returns a tuple of i128's => (bid modifier, lot modifier) scaled
/// to 7 decimal places
pub(super) fn get_fill_modifiers(e: &Env, auction_data: &AuctionData) -> (i128, i128) {
    let block_dif = i128(e.ledger().sequence() - auction_data.block) * 1_0000000;
    let bid_mod: i128;
    let lot_mod: i128;
    // increment the modifier 0.5% every block
    let per_block_scalar: i128 = 0_0050000;
    if block_dif > 400_0000000 {
        bid_mod = 0;
        lot_mod = 1_0000000;
    } else if block_dif > 200_0000000 {
        bid_mod = 2_0000000
            - block_dif
                .fixed_mul_floor(per_block_scalar, 1_0000000)
                .unwrap();
        lot_mod = 1_0000000;
    } else {
        bid_mod = 1_000_0000;
        lot_mod = block_dif
            .fixed_mul_floor(per_block_scalar, 1_0000000)
            .unwrap();
    };
    (bid_mod, lot_mod)
}

#[cfg(test)]
mod tests {
    use crate::{
        dependencies::TokenClient,
        storage::PoolConfig,
        testutils::{create_mock_oracle, create_reserve, generate_contract_id, setup_reserve},
    };

    use super::*;
    use soroban_sdk::{
        map,
        testutils::{Address as AddressTestTrait, Ledger, LedgerInfo},
    };

    #[test]
    fn test_create_user_liquidation_errors() {
        let e = Env::default();
        let pool_id = generate_contract_id(&e);
        let backstop = Address::random(&e);

        e.as_contract(&pool_id, || {
            storage::set_backstop_address(&e, &backstop);

            let result = create(&e, AuctionType::UserLiquidation as u32);

            match result {
                Ok(_) => assert!(false),
                Err(err) => assert_eq!(err, PoolError::BadRequest),
            }
        });
    }

    #[test]
    fn test_delete_user_liquidation() {
        let e = Env::default();
        let pool_id = generate_contract_id(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        setup_reserve(&e, &pool_id, &bombadil, &mut reserve_0);

        let mut reserve_1 = create_reserve(&e);
        reserve_1.config.index = 1;
        setup_reserve(&e, &pool_id, &bombadil, &mut reserve_1);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &10_0000000);
        oracle_client.set_price(&reserve_1.asset, &5_0000000);

        // setup user (collateralize reserve 0 and borrow reserve 1)
        let collateral_amount = 17_8000000;
        let liability_amount = 20_0000000;

        let auction_data = AuctionData {
            bid: map![&e],
            lot: map![&e],
            block: 100,
        };
        let pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0_100_000_000,
            status: 0,
        };
        e.as_contract(&pool_id, || {
            storage::set_pool_config(&e, &pool_config);
            storage::set_user_config(&e, &samwise, &0x000000000000000A);
            TokenClient::new(&e, &reserve_0.config.b_token).mint(
                &e.current_contract_address(),
                &samwise,
                &collateral_amount,
            );
            TokenClient::new(&e, &reserve_1.config.d_token).mint(
                &e.current_contract_address(),
                &samwise,
                &liability_amount,
            );
            storage::set_auction(
                &e,
                &(AuctionType::UserLiquidation as u32),
                &samwise,
                &auction_data,
            );

            delete_liquidation(&e, &samwise).unwrap();
            assert!(!storage::has_auction(
                &e,
                &(AuctionType::UserLiquidation as u32),
                &samwise
            ));
        });
    }

    #[test]
    fn test_delete_user_liquidation_invalid_hf() {
        let e = Env::default();
        let pool_id = generate_contract_id(&e);

        let bombadil = Address::random(&e);
        let samwise = Address::random(&e);

        let mut reserve_0 = create_reserve(&e);
        setup_reserve(&e, &pool_id, &bombadil, &mut reserve_0);

        let mut reserve_1 = create_reserve(&e);
        reserve_1.config.index = 1;
        setup_reserve(&e, &pool_id, &bombadil, &mut reserve_1);

        let (oracle_id, oracle_client) = create_mock_oracle(&e);
        oracle_client.set_price(&reserve_0.asset, &10_0000000);
        oracle_client.set_price(&reserve_1.asset, &5_0000000);

        // setup user (collateralize reserve 0 and borrow reserve 1)
        let collateral_amount = 15_0000000;
        let liability_amount = 20_0000000;

        let auction_data = AuctionData {
            bid: map![&e],
            lot: map![&e],
            block: 100,
        };
        let pool_config = PoolConfig {
            oracle: oracle_id,
            bstop_rate: 0_100_000_000,
            status: 0,
        };
        e.as_contract(&pool_id, || {
            storage::set_pool_config(&e, &pool_config);
            storage::set_user_config(&e, &samwise, &0x000000000000000A);
            TokenClient::new(&e, &reserve_0.config.b_token).mint(
                &e.current_contract_address(),
                &samwise,
                &collateral_amount,
            );
            TokenClient::new(&e, &reserve_1.config.d_token).mint(
                &e.current_contract_address(),
                &samwise,
                &liability_amount,
            );
            storage::set_auction(
                &e,
                &(AuctionType::UserLiquidation as u32),
                &samwise,
                &auction_data,
            );

            let result = delete_liquidation(&e, &samwise);
            assert_eq!(result, Err(PoolError::InvalidHf));
            assert!(storage::has_auction(
                &e,
                &(AuctionType::UserLiquidation as u32),
                &samwise
            ));
        });
    }

    #[test]
    fn test_get_fill_modifiers() {
        let e = Env::default();

        let auction_data = AuctionData {
            bid: map![&e],
            lot: map![&e],
            block: 1000,
        };

        let mut bid_modifier: i128;
        let mut receive_from_modifier: i128;

        e.ledger().set(LedgerInfo {
            timestamp: 12345,
            protocol_version: 1,
            sequence_number: 1000,
            network_id: Default::default(),
            base_reserve: 10,
        });
        (bid_modifier, receive_from_modifier) = get_fill_modifiers(&e, &auction_data);
        assert_eq!(bid_modifier, 1_0000000);
        assert_eq!(receive_from_modifier, 0);

        e.ledger().set(LedgerInfo {
            timestamp: 12345,
            protocol_version: 1,
            sequence_number: 1100,
            network_id: Default::default(),
            base_reserve: 10,
        });
        (bid_modifier, receive_from_modifier) = get_fill_modifiers(&e, &auction_data);
        assert_eq!(bid_modifier, 1_0000000);
        assert_eq!(receive_from_modifier, 0_5000000);

        e.ledger().set(LedgerInfo {
            timestamp: 12345,
            protocol_version: 1,
            sequence_number: 1200,
            network_id: Default::default(),
            base_reserve: 10,
        });
        (bid_modifier, receive_from_modifier) = get_fill_modifiers(&e, &auction_data);
        assert_eq!(bid_modifier, 1_0000000);
        assert_eq!(receive_from_modifier, 1_0000000);

        e.ledger().set(LedgerInfo {
            timestamp: 12345,
            protocol_version: 1,
            sequence_number: 1201,
            network_id: Default::default(),
            base_reserve: 10,
        });
        (bid_modifier, receive_from_modifier) = get_fill_modifiers(&e, &auction_data);
        assert_eq!(bid_modifier, 0_9950000);
        assert_eq!(receive_from_modifier, 1_0000000);

        e.ledger().set(LedgerInfo {
            timestamp: 12345,
            protocol_version: 1,
            sequence_number: 1300,
            network_id: Default::default(),
            base_reserve: 10,
        });
        (bid_modifier, receive_from_modifier) = get_fill_modifiers(&e, &auction_data);
        assert_eq!(bid_modifier, 0_5000000);
        assert_eq!(receive_from_modifier, 1_0000000);

        e.ledger().set(LedgerInfo {
            timestamp: 12345,
            protocol_version: 1,
            sequence_number: 1400,
            network_id: Default::default(),
            base_reserve: 10,
        });
        (bid_modifier, receive_from_modifier) = get_fill_modifiers(&e, &auction_data);
        assert_eq!(bid_modifier, 0);
        assert_eq!(receive_from_modifier, 1_0000000);
    }
}
