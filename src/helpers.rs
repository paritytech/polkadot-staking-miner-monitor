// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use std::collections::HashMap;

use crate::db::{self, Slashed, Submission};
use crate::types::runtime;
use crate::types::runtime::election_provider_multi_phase::events::ElectionFinalized;
use crate::types::{
    Address, ChainClient, Client, ElectionRound, EpmPhase, ExtrinsicDetails, Hash, Header, HeaderT,
    EPM_PALLET_NAME,
};
use crate::LOG_TARGET;

use codec::Decode;
use scale_info::PortableRegistry;
use scale_info::TypeInfo;
use sp_npos_elections::ElectionScore;
use subxt::dynamic::At;
use subxt::ext::scale_encode::EncodeAsType;
use tokio::sync::mpsc;

pub async fn get_phase(client: &Client, block_hash: Hash) -> anyhow::Result<EpmPhase> {
    client
        .chain_api()
        .storage()
        .at(block_hash)
        .fetch_or_default(
            &runtime::storage()
                .election_provider_multi_phase()
                .current_phase(),
        )
        .await
        .map_err(Into::into)
}

pub async fn get_round(client: &Client, block_hash: Hash) -> anyhow::Result<u32> {
    client
        .chain_api()
        .storage()
        .at(block_hash)
        .fetch_or_default(&runtime::storage().election_provider_multi_phase().round())
        .await
        .map_err(Into::into)
}

/// Represent the result of reading a block.
pub enum ReadBlock {
    ElectionFinalized(ElectionFinalized),
    PhaseClosed,
    Done,
}

pub async fn read_block(
    client: &Client,
    block: &Header,
    state: &mut ElectionRound,
    db: &db::Database,
) -> anyhow::Result<ReadBlock> {
    let mut res = ReadBlock::Done;
    let phase = get_phase(client, block.hash()).await?.0;
    let round = get_round(client, block.hash()).await?;

    tracing::trace!(
        target: LOG_TARGET,
        "fetch block={}, phase={:?}, round={round}",
        block.number(),
        phase,
    );

    if !phase.is_signed() && !phase.is_unsigned_open() && !state.waiting_for_election_finalized() {
        return Ok(ReadBlock::PhaseClosed);
    }

    let block = client.chain_api().blocks().at(block.hash()).await?;
    let mut submissions = HashMap::new();

    let extrinsics = block.extrinsics().await?;

    for ext in extrinsics.iter() {
        let ext = ext?;

        let pallet_name = ext.pallet_name()?;
        let call = ext.variant_name()?;

        if pallet_name != EPM_PALLET_NAME {
            continue;
        }

        tracing::debug!(target: LOG_TARGET, "extrinsic={}_{}, idx={}", pallet_name, call, ext.index());

        if call == "submit" {
            // TODO: use multiaddress here instead of asserting the address is 33 bytes
            let address = ext
                .address_bytes()
                .map(|b| Address::from_bytes(&b[1..]))
                .ok_or_else(|| anyhow::anyhow!("EPM::submit must have an address"))?;

            let score = get_solution_score(&ext)?;
            submissions.insert(ext.index(), (score, address, round));
        }

        if call == "submit_unsigned" {
            let score = get_solution_score(&ext)?;
            submissions.insert(ext.index(), (score, Address::unsigned(), round));
        }
    }

    for event in block.events().await?.iter() {
        let event = event?;

        if event.pallet_name() != EPM_PALLET_NAME {
            continue;
        }

        tracing::debug!(target: LOG_TARGET, "event={}_{}", event.pallet_name(), event.variant_name());

        if (event.as_event::<runtime::election_provider_multi_phase::events::SolutionStored>()?)
            .is_some()
        {
            if let subxt::events::Phase::ApplyExtrinsic(idx) = event.phase() {
                if let Some((score, who, r)) = submissions.remove(&idx) {
                    tracing::trace!(target: LOG_TARGET, "Solution submitted who={who},score={:?}", score);
                    db.insert_submission(Submission::new(who, r, block.number(), score, true))
                        .await?;
                }
            }
        }

        if let Some(winner) =
            event.as_event::<runtime::election_provider_multi_phase::events::ElectionFinalized>()?
        {
            res = ReadBlock::ElectionFinalized(winner);
        }

        if let Some(rewarded) =
            event.as_event::<runtime::election_provider_multi_phase::events::Rewarded>()?
        {
            state.set_winner(Address::from_bytes(rewarded.account.0.as_slice()));
        }

        if let Some(slashed) =
            event.as_event::<runtime::election_provider_multi_phase::events::Slashed>()?
        {
            db.insert_slashed(Slashed::new(
                slashed.account,
                round,
                block.number(),
                slashed.value,
            ))
            .await?;
        }
    }

    for (_, missed) in submissions.into_iter() {
        let (score, who, r) = missed;
        db.insert_submission(Submission::new(who, r, block.number(), score, false))
            .await?;
    }

    Ok(res)
}

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

// Read the previous blocks in the current round.
pub async fn read_remaining_blocks_in_round(
    client: &Client,
    state: &mut ElectionRound,
    block_num: u64,
    db: &db::Database,
) -> anyhow::Result<()> {
    let first_block = std::cmp::min(
        block_num,
        state
            .first_block()
            .expect("At least one block processed; qed"),
    );

    let mut prev_block = first_block.checked_sub(1);
    while let Some(b) = prev_block {
        let old_block = get_block(client, b).await?;
        match read_block(client, &old_block, state, db).await? {
            ReadBlock::PhaseClosed | ReadBlock::ElectionFinalized(_) => break,
            ReadBlock::Done => {}
        }
        prev_block = b.checked_sub(1);
    }

    Ok(())
}

fn get_solution_score(ext: &ExtrinsicDetails) -> Result<ElectionScore, anyhow::Error> {
    let scale_val = ext.field_values()?;

    let score = {
        let val = scale_val
            .at("raw_solution")
            .ok_or_else(|| anyhow::anyhow!("RawSolution not found"))?
            .at("score")
            .ok_or_else(|| anyhow::anyhow!("RawSolution::score not found"))?;
        decode_scale_val(val)?
    };

    Ok(score)
}

fn make_type<T: TypeInfo + 'static>() -> (u32, PortableRegistry) {
    let m = scale_info::MetaType::new::<T>();
    let mut types = scale_info::Registry::new();
    let id = types.register_type(&m);
    let portable_registry: PortableRegistry = types.into();

    (id.id, portable_registry)
}

fn decode_scale_val<T, Ctx>(val: &subxt::ext::scale_value::Value<Ctx>) -> Result<T, anyhow::Error>
where
    T: Decode + TypeInfo + 'static,
{
    let (ty_id, types) = make_type::<T>();

    let bytes = val.encode_as_type(ty_id, &types)?;
    Decode::decode(&mut bytes.as_ref()).map_err(Into::into)
}
