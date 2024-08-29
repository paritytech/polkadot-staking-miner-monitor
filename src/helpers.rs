// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use std::collections::HashMap;

use crate::prometheus;
use crate::runtime;
use crate::runtime::election_provider_multi_phase::events::ElectionFinalized;
use crate::types::*;
use codec::Decode;
use pallet_election_provider_multi_phase::RawSolution;
use subxt::config::Header as _;

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
    state: &mut SubmissionsInRound,
    chain: Chain,
) -> anyhow::Result<ReadBlock> {
    let mut res = ReadBlock::Done;
    let phase = get_phase(client, block.hash()).await?.0;

    tracing::info!("fetching block={}, phase={:?}", block.number(), phase);

    if !phase.is_signed() && !phase.is_unsigned_open() {
        return Ok(ReadBlock::PhaseClosed);
    }

    let round = get_round(client, block.hash()).await?;
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

        tracing::info!("extrinsic={}_{}, idx={}", pallet_name, call, ext.index());

        if call == "submit" || call == "submit_unsigned" {
            // TODO: use multiaddress here.
            let addr = ext.address_bytes().map(|b| Hash::from_slice(&b[1..]));

            let mut bytes = ext.field_bytes();

            match chain {
                Chain::Kusama => {
                    let raw_solution: RawSolution<kusama::NposSolution24> =
                        Decode::decode(&mut bytes)?;

                    tracing::info!("score: {:?}", raw_solution.score);
                    submissions.insert(ext.index(), (raw_solution.score, addr, round));
                }
                Chain::Polkadot => {
                    let raw_solution: RawSolution<polkadot::NposSolution16> =
                        Decode::decode(&mut bytes)?;

                    tracing::info!("score: {:?}", raw_solution.score);
                    submissions.insert(ext.index(), (raw_solution.score, addr, round));
                }
                Chain::Westend => {
                    let raw_solution: RawSolution<westend::NposSolution16> =
                        Decode::decode(&mut bytes)?;

                    tracing::info!("score: {:?}", raw_solution.score);
                    submissions.insert(ext.index(), (raw_solution.score, addr, round));
                }
            }
        }
    }

    for event in block.events().await?.iter() {
        let event = event?;

        if event.pallet_name() == "ElectionProviderMultiPhase" {
            tracing::info!("event={}_{}", event.pallet_name(), event.variant_name());
        }

        if let Some(phase) =
            event.as_event::<runtime::election_provider_multi_phase::events::PhaseTransitioned>()?
        {
            tracing::info!("{:?}", phase);
        }

        if let Some(solution) =
            event.as_event::<runtime::election_provider_multi_phase::events::SolutionStored>()?
        {
            if let subxt::events::Phase::ApplyExtrinsic(idx) = event.phase() {
                if let Some((score, addr, r)) = submissions.remove(&idx) {
                    tracing::trace!("{:?}", solution);
                    prometheus::submission(&chain.to_string(), round, addr, score);

                    state.add_submission((score, addr, r));
                }
            }
        }

        if let Some(winner) =
            event.as_event::<runtime::election_provider_multi_phase::events::ElectionFinalized>()?
        {
            res = ReadBlock::ElectionFinalized(winner);
        }
    }

    Ok(res)
}

pub async fn get_block(client: &Client, n: u32) -> anyhow::Result<Header> {
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
        .map_err(|e| anyhow::Error::from(e))?
        .expect("Known block; qed");

    Ok(header)
}

/// Runs until the RPC connection fails or updating the metadata failed.
pub async fn runtime_upgrade_task(client: ChainClient) {
    let updater = client.updater();

    let mut update_stream = match updater.runtime_updates().await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("Failed to get runtime upgrade subscription: {:?}", e);
            return;
        }
    };

    loop {
        // if the runtime upgrade subscription fails then try establish a new one and if it fails quit.
        let update = match update_stream.next().await {
            Some(Ok(update)) => update,
            _ => {
                tracing::warn!("Runtime upgrade subscription failed");
                update_stream = match updater.runtime_updates().await {
                    Ok(u) => u,
                    Err(e) => {
                        tracing::error!("Failed to get runtime upgrade subscription: {:?}", e);
                        return;
                    }
                };
                continue;
            }
        };

        let version = update.runtime_version().spec_version;
        match updater.apply_update(update) {
            Ok(()) => {
                tracing::info!("upgrade to version: {} successful", version);
            }
            Err(e) => {
                tracing::debug!("upgrade to version: {} failed: {:?}", version, e);
            }
        }
    }
}
