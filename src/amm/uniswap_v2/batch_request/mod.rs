use ethers::{
    abi::{ParamType, Token},
    providers::Middleware,
    types::{Bytes, H160, U256},
};
use std::sync::Arc;

use crate::{
    amm::{AutomatedMarketMaker, AMM},
    errors::AMMError,
};

use ethers::prelude::abigen;

use super::UniswapV2Pool;

abigen!(

    IGetUniswapV2PairsBatchRequest,
        "src/amm/uniswap_v2/batch_request/GetUniswapV2PairsBatchRequestABI.json";

    IGetUniswapV2PoolDataBatchRequest,
        "src/amm/uniswap_v2/batch_request/GetUniswapV2PoolDataBatchRequestABI.json";
);

fn populate_pool_data_from_tokens(
    mut pool: UniswapV2Pool,
    tokens: Vec<Token>,
) -> Option<UniswapV2Pool> {
    pool.token_a = tokens[0].to_owned().into_address()?;
    pool.token_a_decimals = tokens[1].to_owned().into_uint()?.as_u32() as u8;
    pool.token_b = tokens[2].to_owned().into_address()?;
    pool.token_b_decimals = tokens[3].to_owned().into_uint()?.as_u32() as u8;
    pool.reserve_0 = tokens[4].to_owned().into_uint()?.as_u128();
    pool.reserve_1 = tokens[5].to_owned().into_uint()?.as_u128();

    Some(pool)
}

pub async fn get_pairs_batch_request<M: Middleware>(
    factory: H160,
    from: U256,
    step: U256,
    middleware: Arc<M>,
) -> Result<Vec<H160>, AMMError<M>> {
    let constructor_args = Token::Tuple(vec![
        Token::Uint(from),
        Token::Uint(step),
        Token::Address(factory),
    ]);

    let deployer = IGetUniswapV2PairsBatchRequest::deploy(middleware, constructor_args)
        .map_err(|e| AMMError::ContractError("get_pairs_batch_request", factory, e))?;
    let return_data: Bytes = deployer
        .call_raw()
        .await
        .map_err(|e| AMMError::ProviderError("get_pairs_batch_request", factory, e))?;

    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Address))],
        &return_data,
    )?;

    let mut pairs = vec![];
    for token_array in return_data_tokens {
        if let Some(arr) = token_array.into_array() {
            for token in arr {
                if let Some(addr) = token.into_address() {
                    if !addr.is_zero() {
                        pairs.push(addr);
                    }
                }
            }
        }
    }

    Ok(pairs)
}

pub async fn get_amm_data_batch_request<M: Middleware>(
    amms: &mut [AMM],
    middleware: Arc<M>,
) -> Result<(), AMMError<M>> {
    let batch_start = amms.first().map(|a| a.address()).unwrap_or_default();

    let target_addresses = amms
        .iter()
        .map(|a| Token::Address(a.address()))
        .collect::<Vec<Token>>();

    let constructor_args = Token::Tuple(vec![Token::Array(target_addresses)]);

    let deployer = IGetUniswapV2PoolDataBatchRequest::deploy(middleware.clone(), constructor_args)
        .map_err(|e| AMMError::ContractError("get_amm_data_batch_request", batch_start, e))?;

    let return_data: Bytes = deployer
        .call_raw()
        .await
        .map_err(|e| AMMError::ProviderError("get_amm_data_batch_request", batch_start, e))?;
    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Tuple(vec![
            ParamType::Address,   // token a
            ParamType::Uint(8),   // token a decimals
            ParamType::Address,   // token b
            ParamType::Uint(8),   // token b decimals
            ParamType::Uint(112), // reserve 0
            ParamType::Uint(112), // reserve 1
        ])))],
        &return_data,
    )?;

    let mut pool_idx = 0;

    for tokens in return_data_tokens {
        if let Some(tokens_arr) = tokens.into_array() {
            for tup in tokens_arr {
                if let Some(pool_data) = tup.into_tuple() {
                    //If the pool token A is not zero, signaling that the pool data was populated
                    if let Some(address) = pool_data[0].to_owned().into_address() {
                        if !address.is_zero() {
                            //Update the pool data
                            if let AMM::UniswapV2Pool(uniswap_v2_pool) = amms
                                .get_mut(pool_idx)
                                .expect("Pool idx should be in bounds")
                            {
                                if let Some(pool) = populate_pool_data_from_tokens(
                                    uniswap_v2_pool.to_owned(),
                                    pool_data,
                                ) {
                                    *uniswap_v2_pool = pool;
                                }
                            }
                        }
                    }

                    pool_idx += 1;
                }
            }
        }
    }

    Ok(())
}

pub async fn get_v2_pool_data_batch_request<M: Middleware>(
    pool: &mut UniswapV2Pool,
    middleware: Arc<M>,
) -> Result<(), AMMError<M>> {
    let constructor_args = Token::Tuple(vec![Token::Array(vec![Token::Address(pool.address)])]);

    let deployer = IGetUniswapV2PoolDataBatchRequest::deploy(middleware.clone(), constructor_args)
        .map_err(|e| AMMError::ContractError("get_v2_pool_data_batch_request", pool.address, e))?;

    let return_data: Bytes = deployer
        .call_raw()
        .await
        .map_err(|e| AMMError::ProviderError("get_v2_pool_data_batch_request", pool.address, e))?;
    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Tuple(vec![
            ParamType::Address,   // token a
            ParamType::Uint(8),   // token a decimals
            ParamType::Address,   // token b
            ParamType::Uint(8),   // token b decimals
            ParamType::Uint(112), // reserve 0
            ParamType::Uint(112), // reserve 1
        ])))],
        &return_data,
    )?;

    for tokens in return_data_tokens {
        if let Some(tokens_arr) = tokens.into_array() {
            for tup in tokens_arr {
                let pool_data = tup
                    .into_tuple()
                    .ok_or(AMMError::BatchRequestError(pool.address))?;

                *pool = populate_pool_data_from_tokens(pool.to_owned(), pool_data)
                    .ok_or(AMMError::BatchRequestError(pool.address))?;
            }
        }
    }

    Ok(())
}
