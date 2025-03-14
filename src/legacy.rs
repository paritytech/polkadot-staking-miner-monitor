//! Single block election (legacy).

use crate::db;
use crate::helpers::{decode_scale_val, get_block};
use crate::types::{
    Address, BlockRef, Client, ElectionRound, ExtrinsicDetails, Hash, Header, HeaderT, ReadBlock,
};
use crate::LOG_TARGET;
use sp_npos_elections::ElectionScore;
use std::collections::HashMap;
use subxt::dynamic::At;

const EPM_PALLET_NAME: &str = "ElectionProviderMultiPhase";

pub type EpmPhase = subxt::utils::Static<pallet_election_provider_multi_phase::Phase<u32>>;

#[subxt::subxt(
    runtime_metadata_path = "artifacts/metadata.scale",
    derive_for_all_types = "Clone, Debug, Eq, PartialEq",
    substitute_type(
        path = "sp_npos_elections::ElectionScore",
        with = "::subxt::utils::Static<sp_npos_elections::ElectionScore>"
    ),
    substitute_type(
        path = "pallet_election_provider_multi_phase::Phase",
        with = "::subxt::utils::Static<pallet_election_provider_multi_phase::Phase<u32>>"
    )
)]
pub mod runtime {}

pub async fn run(
    client: &Client,
    state: &mut ElectionRound,
    block_ref: BlockRef,
    block: Header,
    db: &db::Database,
) -> anyhow::Result<ReadBlock> {
    let curr_phase = get_phase(client, block_ref.hash()).await?.0;
    let round = get_round(client, block_ref.hash()).await?;

    tracing::info!(
        target: LOG_TARGET,
        "block={}, phase={:?}, round={:?}",
        block.number(),
        curr_phase,
        round
    );

    if !curr_phase.is_signed()
        && !curr_phase.is_unsigned_open()
        && !state.waiting_for_election_finalized()
    {
        state.clear();
        return Ok(ReadBlock::PhaseClosed);
    }

    state.new_block(block.number() as u64, round);

    match read_block(&client, &block, state, db).await? {
        ReadBlock::PhaseClosed => unreachable!("Phase already checked; qed"),
        ReadBlock::ElectionFinalized(winner) => {
            read_remaining_blocks_in_round(&client, state, block.number() as u64, db).await?;
            Ok(ReadBlock::ElectionFinalized(winner))
        }
        ReadBlock::Done => Ok(ReadBlock::Done),
    }
}

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
                    db.insert_submission(db::Submission::new(who, r, block.number(), score, true))
                        .await?;
                }
            }
        }

        if let Some(winner) =
            event.as_event::<runtime::election_provider_multi_phase::events::ElectionFinalized>()?
        {
            res = ReadBlock::ElectionFinalized(winner.score.0);
        }

        if let Some(rewarded) =
            event.as_event::<runtime::election_provider_multi_phase::events::Rewarded>()?
        {
            state.set_winner(Address::from_bytes(rewarded.account.0.as_slice()));
        }

        if let Some(slashed) =
            event.as_event::<runtime::election_provider_multi_phase::events::Slashed>()?
        {
            db.insert_slashed(db::Slashed::new(
                slashed.account,
                round,
                block.number(),
                slashed.value,
            ))
            .await?;
        }

        if event
            .as_event::<runtime::election_provider_multi_phase::events::ElectionFailed>()?
            .is_some()
        {
            state.election_failed();
        }
    }

    for (_, missed) in submissions.into_iter() {
        let (score, who, r) = missed;
        db.insert_submission(db::Submission::new(who, r, block.number(), score, false))
            .await?;
    }

    Ok(res)
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
