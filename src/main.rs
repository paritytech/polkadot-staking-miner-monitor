mod prometheus;

#[subxt::subxt(
    runtime_metadata_path = "artifacts/metadata.scale",
    derive_for_all_types = "Clone, Debug, Eq, PartialEq",
    substitute_type(
        path = "sp_npos_elections::ElectionScore",
        with = "::subxt::utils::Static<::sp_npos_elections::ElectionScore>"
    )
)]
pub mod runtime {}

use std::collections::{BTreeSet, HashMap};

use clap::Parser;
use codec::Decode;
use pallet_election_provider_multi_phase::RawSolution;
use sp_npos_elections::ElectionScore;
use staking_miner::opt::Chain;
use subxt::config::Header as _;
use subxt::events::Phase;
use subxt::{OnlineClient, PolkadotConfig};

use runtime::election_provider_multi_phase::events::ElectionFinalized;
use runtime::runtime_types::pallet_election_provider_multi_phase::Phase as EpmPhase;

type SubxtClient = OnlineClient<PolkadotConfig>;
type Hash = subxt::ext::sp_core::H256;
type Submission = (ElectionScore, Option<Hash>, u32);
type Header =
    subxt::config::substrate::SubstrateHeader<u32, <PolkadotConfig as subxt::Config>::Hasher>;

const EPM_PALLET_NAME: &str = "ElectionProviderMultiPhase";

#[derive(Debug, Clone, Parser)]
struct Opt {
    #[clap(long)]
    url: Option<String>,
    #[clap(long, default_value_t = 9999)]
    prometheus_port: u16,
}

#[derive(Debug)]
struct SubmissionsInRound {
    submissions: Vec<Submission>,
    visited_blocks: BTreeSet<u32>,
    block_len: usize,
}

impl SubmissionsInRound {
    fn new(block_len: u32) -> Self {
        Self {
            submissions: Vec::new(),
            visited_blocks: BTreeSet::new(),
            block_len: block_len as usize,
        }
    }

    fn clear(&mut self) {
        self.submissions.clear();
        self.visited_blocks.clear();
    }

    fn add_submission(&mut self, submission: Submission) {
        self.submissions.push(submission);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Opt {
        url,
        prometheus_port,
    } = Opt::parse();

    let url = url.ok_or_else(|| anyhow::anyhow!("--url must be set"))?;
    let _prometheus_handle =
        prometheus::run(prometheus_port).map_err(|e| anyhow::anyhow!("{e}"))?;
    let _ = tracing_subscriber::fmt().try_init();

    let api = loop {
        match SubxtClient::from_url(&url).await {
            Ok(api) => break api,
            Err(e) => {
                tracing::warn!("Could not connect to {url} {}, trying to re-connect", e);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    };

    let chain: Chain = api.runtime_version().try_into()?;

    let mut blocks = api.rpc().subscribe_finalized_block_headers().await?;

    let sign_phase_len = api.constants().at(&runtime::constants()
        .election_provider_multi_phase()
        .signed_phase())?;

    let unsign_phase_len = api.constants().at(&runtime::constants()
        .election_provider_multi_phase()
        .unsigned_phase())?;

    let mut state = SubmissionsInRound::new(sign_phase_len + unsign_phase_len);

    loop {
        let block = match blocks.next().await {
            Some(Err(e)) => return Err(e.into()),
            Some(Ok(b)) => b,
            None => {
                blocks = api.rpc().subscribe_finalized_block_headers().await?;
                continue;
            }
        };

        match get_phase(&api, block.hash()).await? {
            EpmPhase::Off | EpmPhase::Emergency => {
                continue;
            }
            EpmPhase::Signed | EpmPhase::Unsigned(_) => {
                state.visited_blocks.insert(block.number());
            }
        }

        if let Some(winner) = read_block(&api, &block, &mut state, chain).await? {
            tracing::info!("state: {:?}", state);

            if state.visited_blocks.len() != state.block_len {
                let n = (state.block_len - state.visited_blocks.len()) as u32;
                let start_idx = state
                    .visited_blocks
                    .first()
                    .copied()
                    .map_or(block.number() - state.block_len as u32, |f| f - n);

                for block_num in start_idx..start_idx + n {
                    let old_block = get_header(&api, block_num).await?;
                    read_block(&api, &old_block, &mut state, chain).await?;
                }
            }

            tracing::info!("state: {:?}", state);
            assert_eq!(state.visited_blocks.len(), state.block_len);

            let (score, addr, r) = state
                .submissions
                .iter()
                .max_by(|a, b| a.0.cmp(&b.0))
                .cloned()
                .expect("A winner must exist; qed");

            assert_eq!(score, winner.score.0);
            prometheus::election_winner(r, addr, score);

            state.clear();
        }
    }
}

async fn get_phase(api: &SubxtClient, block_hash: Hash) -> anyhow::Result<EpmPhase<u32>> {
    api.storage()
        .at(block_hash)
        .fetch_or_default(
            &runtime::storage()
                .election_provider_multi_phase()
                .current_phase(),
        )
        .await
        .map_err(Into::into)
}

async fn get_round(api: &SubxtClient, block_hash: Hash) -> anyhow::Result<u32> {
    api.storage()
        .at(block_hash)
        .fetch_or_default(&runtime::storage().election_provider_multi_phase().round())
        .await
        .map_err(Into::into)
}

async fn read_block(
    api: &SubxtClient,
    block: &Header,
    state: &mut SubmissionsInRound,
    chain: Chain,
) -> anyhow::Result<Option<ElectionFinalized>> {
    let mut res = None;

    tracing::info!("fetching block: {:?}", block.number());

    match get_phase(&api, block.hash()).await? {
        EpmPhase::Off | EpmPhase::Emergency => {
            panic!("Unreachable; only called on valid blocks");
        }
        EpmPhase::Signed | EpmPhase::Unsigned(_) => {
            state.visited_blocks.insert(block.number());
        }
    }

    let round = get_round(&api, block.hash()).await?;
    let block = api.blocks().at(block.hash()).await?;
    let mut submissions = HashMap::new();

    for ext in block.body().await?.extrinsics().iter() {
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
                    let raw_solution: RawSolution<
                        staking_miner::static_types::kusama::NposSolution24,
                    > = Decode::decode(&mut bytes)?;

                    tracing::info!("score: {:?}", raw_solution.score);
                    submissions.insert(ext.index(), (raw_solution.score, addr, round));
                }
                Chain::Polkadot => {
                    let raw_solution: RawSolution<
                        staking_miner::static_types::polkadot::NposSolution16,
                    > = Decode::decode(&mut bytes)?;

                    tracing::info!("score: {:?}", raw_solution.score);
                    submissions.insert(ext.index(), (raw_solution.score, addr, round));
                }
                Chain::Westend => {
                    let raw_solution: RawSolution<
                        staking_miner::static_types::westend::NposSolution16,
                    > = Decode::decode(&mut bytes)?;

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
            if let Phase::ApplyExtrinsic(idx) = event.phase() {
                if let Some((score, addr, r)) = submissions.remove(&idx) {
                    tracing::trace!("{:?}", solution);
                    prometheus::submission(round, addr, score);

                    state.add_submission((score, addr, r));
                }
            }
        }

        if let Some(winner) =
            event.as_event::<runtime::election_provider_multi_phase::events::ElectionFinalized>()?
        {
            res = Some(winner);
        }
    }

    Ok(res)
}

async fn get_header(api: &SubxtClient, n: u32) -> anyhow::Result<Header> {
    let block_hash = api
        .rpc()
        .block_hash(Some(n.into()))
        .await?
        .expect("Known block; qed");

    let header = api
        .rpc()
        .header(Some(block_hash))
        .await
        .map_err(|e| anyhow::Error::from(e))?
        .expect("Known block; qed");

    Ok(header)
}
