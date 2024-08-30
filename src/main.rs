// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

mod helpers;
mod prometheus;
mod types;

use clap::Parser;
use helpers::*;
use tokio::sync::mpsc;
use tracing_subscriber::util::SubscriberInitExt;
use types::*;

const LOG_TARGET: &str = "staking-miner-monitor";

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
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()?;

    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(filter)
        .finish()
        .try_init()?;

    let client = Client::new(&url).await?;

    let mut blocks = client
        .chain_api()
        .backend()
        .stream_finalized_block_headers()
        .await?;

    let mut state = SubmissionsInRound::new();

    let (upgrade_tx, mut upgrade_rx) = mpsc::channel(1);
    tokio::spawn(runtime_upgrade_task(client.chain_api().clone(), upgrade_tx));

    loop {
        let (block, block_ref) = tokio::select! {
            msg = upgrade_rx.recv() => {
                let msg = msg.unwrap_or_else(|| "Unknown".to_string());
                return Err(anyhow::anyhow!("Upgrade task failed: {msg}"));
            }
            block = blocks.next() => {
                match block {
                    Some(Ok(block)) => {
                        block
                    }
                    Some(Err(e)) => {
                        if e.is_disconnected_will_reconnect() {
                            continue;
                        }
                        return Err(e.into());
                    }
                    None => {
                        blocks = client
                            .chain_api()
                            .backend()
                            .stream_finalized_block_headers()
                            .await?;
                        continue;
                    }
                }
            }
        };

        let curr_phase = get_phase(&client, block_ref.hash()).await?.0;
        let round = get_round(&client, block_ref.hash()).await?;

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
            continue;
        }

        state.new_block(block.number() as u64, round);

        let winner = match read_block(&client, &block, &mut state).await? {
            ReadBlock::PhaseClosed => unreachable!("Phase already checked; qed"),
            ReadBlock::ElectionFinalized(winner) => {
                read_remaining_blocks_in_round(&client, &mut state, block.number() as u64).await?;
                winner
            }
            ReadBlock::Done => continue,
        };

        tracing::debug!(target: LOG_TARGET, "submissions: {:?}", state);

        let (score, addr, r) = state
            .submissions
            .iter()
            .max_by(|a, b| a.0.cmp(&b.0))
            .cloned()
            .expect("A winner must exist; qed");

        assert_eq!(score, winner.score.0);
        prometheus::election_winner(client.chain_name().as_str(), r, addr, score);
        state.clear();
    }
}
