// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::types::{ChainClient, Client, Header};
use crate::LOG_TARGET;

use codec::Decode;
use scale_info::PortableRegistry;
use scale_info::TypeInfo;
use subxt::ext::scale_encode::EncodeAsType;
use tokio::sync::mpsc;

pub async fn get_block(client: &Client, n: u64) -> anyhow::Result<Header> {
    let block_hash = client
        .rpc()
        .chain_get_block_hash(Some(n.into()))
        .await?
        .expect("Known block; qed");

    let header = client
        .chain_api()
        .backend()
        .block_header(block_hash)
        .await
        .map_err(anyhow::Error::from)?
        .expect("Known block; qed");

    Ok(header)
}

/// Runs until the RPC connection fails or updating the metadata failed.
pub async fn runtime_upgrade_task(client: ChainClient, tx: mpsc::Sender<String>) {
    let updater = client.updater();

    let mut update_stream = match updater.runtime_updates().await {
        Ok(u) => u,
        Err(e) => {
            _ = tx.send(e.to_string()).await;
            return;
        }
    };

    loop {
        // if the runtime upgrade subscription fails then try establish a new one and if it fails quit.
        let update = match update_stream.next().await {
            Some(Ok(update)) => update,
            _ => {
                update_stream = match updater.runtime_updates().await {
                    Ok(u) => u,
                    Err(e) => {
                        _ = tx.send(e.to_string()).await;
                        return;
                    }
                };
                continue;
            }
        };

        let version = update.runtime_version().spec_version;

        match updater.apply_update(update) {
            Ok(()) => {
                tracing::info!(target: LOG_TARGET, "upgrade to version: {} successful", version);
            }
            Err(e) => {
                tracing::debug!(target: LOG_TARGET, "upgrade to version: {} failed: {:?}", version, e);
            }
        }
    }
}

pub fn make_type<T: TypeInfo + 'static>() -> (u32, PortableRegistry) {
    let m = scale_info::MetaType::new::<T>();
    let mut types = scale_info::Registry::new();
    let id = types.register_type(&m);
    let portable_registry: PortableRegistry = types.into();

    (id.id, portable_registry)
}

pub fn decode_scale_val<T, Ctx>(
    val: &subxt::ext::scale_value::Value<Ctx>,
) -> Result<T, anyhow::Error>
where
    T: Decode + TypeInfo + 'static,
{
    let (ty_id, types) = make_type::<T>();

    let bytes = val.encode_as_type(ty_id, &types)?;
    Decode::decode(&mut bytes.as_ref()).map_err(Into::into)
}
