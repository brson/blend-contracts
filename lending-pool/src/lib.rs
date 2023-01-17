#![no_std]

#[cfg(any(test, feature = "testutils"))]
extern crate std;

mod auction;
mod errors;
mod interest;
mod pool;
mod reserve;
mod storage;
mod user_config;
mod user_data;
mod user_validator;

mod dependencies;

pub mod testutils;
pub use crate::pool::{Pool, PoolClient};
