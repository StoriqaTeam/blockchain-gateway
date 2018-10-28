use super::super::requests::*;
use super::super::responses::TxHashResponse;
use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use models::*;
use prelude::*;

pub fn get_utxos(ctx: &Context, address: BitcoinAddress) -> ControllerFuture {
    let address_clone = address.clone();
    Box::new(
        ctx.bitcoin_service
            .get_utxos(address)
            .map_err(ectx!(convert => address_clone))
            .and_then(|utxos| response_with_model(&utxos)),
    )
}

pub fn post_bitcoin_transactions(ctx: &Context) -> ControllerFuture {
    let bitcoin_service = ctx.bitcoin_service.clone();
    let body = ctx.body.clone();
    Box::new(
        parse_body::<PostBitcoinTransactionRequest>(ctx.body.clone())
            .and_then(move |input| bitcoin_service.send_raw_tx(input.raw).map_err(ectx!(convert => body)))
            .and_then(|hash| {
                let resp = TxHashResponse { tx_hash: hash };
                response_with_model(&resp)
            }),
    )
}
