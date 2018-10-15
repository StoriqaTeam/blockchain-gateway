use super::super::error::*;
use super::super::utils::response_with_model;
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
    unimplemented!()
}
