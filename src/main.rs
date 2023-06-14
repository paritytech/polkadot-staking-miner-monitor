mod helpers;
mod prometheus;
mod types;

#[subxt::subxt(
    runtime_metadata_path = "artifacts/metadata.scale",
    derive_for_all_types = "Clone, Debug, Eq, PartialEq",
    substitute_type(
        path = "sp_npos_elections::ElectionScore",
        with = "::subxt::utils::Static<::sp_npos_elections::ElectionScore>"
    )
)]
pub mod runtime {}

use clap::Parser;
use helpers::*;
use subxt::config::Header as _;
use types::*;

use runtime::runtime_types::pallet_election_provider_multi_phase::Phase as EpmPhase;

#[derive(Debug, Clone, Parser)]
struct Opt {
    #[clap(long)]
    url: Option<String>,
    #[clap(long, default_value_t = 9999)]
    prometheus_port: u16,
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

    let chain_name = get_chain_name(&api);
    let mut blocks = api.rpc().subscribe_finalized_block_headers().await?;

    let sign_phase_len = ThreadSafeCounter::new(
        api.constants().at(&runtime::constants()
            .election_provider_multi_phase()
            .signed_phase())?,
    );

    let unsign_phase_len = ThreadSafeCounter::new(
        api.constants().at(&runtime::constants()
            .election_provider_multi_phase()
            .unsigned_phase())?,
    );

    let mut state = SubmissionsInRound::new(sign_phase_len.clone(), unsign_phase_len.clone());

    let upgrade_task = tokio::spawn(runtime_upgrade_task(
        api.clone(),
        sign_phase_len,
        unsign_phase_len,
    ));

    loop {
        if upgrade_task.is_finished() {
            panic!("Upgrade task failed; terminate app");
        }

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

        if let Some(winner) = read_block(&api, &block, &mut state, &chain_name).await? {
            tracing::info!("state: {:?}", state);

            let num_blocks = (state.unsign_phase_len.read() + state.sign_phase_len.read()) as usize;

            if state.visited_blocks.len() != num_blocks {
                let n = (num_blocks - state.visited_blocks.len()) as u32;
                let start_idx = state
                    .visited_blocks
                    .first()
                    .copied()
                    .map_or(block.number() - num_blocks as u32, |f| f - n);

                for block_num in start_idx..start_idx + n {
                    let old_block = get_header(&api, block_num).await?;
                    read_block(&api, &old_block, &mut state, &chain_name).await?;
                }
            }

            tracing::info!("state: {:?}", state);
            assert_eq!(state.visited_blocks.len(), num_blocks);

            let (score, addr, r) = state
                .submissions
                .iter()
                .max_by(|a, b| a.0.cmp(&b.0))
                .cloned()
                .expect("A winner must exist; qed");

            assert_eq!(score, winner.score.0);
            prometheus::election_winner(&chain_name, r, addr, score);

            state.clear();
        }
    }
}
