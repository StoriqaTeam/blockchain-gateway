use super::super::requests::*;
use super::super::responses::*;
use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use models::*;
use prelude::*;

pub fn get_nonce(ctx: &Context, address: EthereumAddress) -> ControllerFuture {
    let address_clone = address.clone();
    Box::new(
        ctx.ethereum_service
            .get_nonce(address)
            .map_err(ectx!(convert => address_clone))
            .and_then(|nonce| {
                let resp = NonceResponse { nonce };
                response_with_model(&resp)
            }),
    )
}

pub fn post_ethereum_transactions(ctx: &Context) -> ControllerFuture {
    let ethereum_service = ctx.ethereum_service.clone();
    let body = ctx.body.clone();
    Box::new(
        parse_body::<PostEthereumTransactionRequest>(ctx.body.clone())
            .and_then(move |input| ethereum_service.send_raw_tx(input.raw).map_err(ectx!(convert => body)))
            .and_then(|hash| {
                let resp = TxHashResponse { tx_hash: hash };
                response_with_model(&resp)
            }),
    )
}
