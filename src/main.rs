// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

mod helpers;
mod prometheus;
mod types;

use std::str::FromStr;

use clap::Parser;
use helpers::*;
use types::*;

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
    tracing_subscriber::fmt()
        .try_init()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let client = Client::new(&url).await?;
    let runtime_version = client.rpc().state_get_runtime_version(None).await?;
    let spec_name = match runtime_version.other.get("specName") {
        Some(serde_json::Value::String(n)) => n.clone(),
        Some(_) => return Err(anyhow::anyhow!("specName is not a string")),
        None => return Err(anyhow::anyhow!("specName not found")),
    };

    let chain_name = Chain::from_str(&spec_name).unwrap();

    let mut blocks = client
        .chain_api()
        .backend()
        .stream_finalized_block_headers()
        .await?;

    let mut state = SubmissionsInRound::new();

    let upgrade_task = tokio::spawn(runtime_upgrade_task(client.chain_api().clone()));

    loop {
        if upgrade_task.is_finished() {
            panic!("Upgrade task failed; terminate app");
        }

        let (block, block_ref) = match blocks.next().await {
            Some(Err(e)) => return Err(e.into()),
            Some(Ok(b)) => b,
            None => {
                blocks = client
                    .chain_api()
                    .backend()
                    .stream_finalized_block_headers()
                    .await?;
                continue;
            }
        };

        let curr_phase = get_phase(&client, block_ref.hash()).await?.0;

        tracing::info!("block={}, phase={:?}", block.number(), curr_phase);

        if !curr_phase.is_signed() && !curr_phase.is_unsigned_open() {
            state.clear();
            continue;
        }

        let winner = match read_block(&client, &block, &mut state, chain_name).await? {
            ReadBlock::PhaseClosed => unreachable!("Phase already checked; qed"),
            ReadBlock::ElectionFinalized(winner) => winner,
            ReadBlock::Done => continue,
        };

        // Read the previous blocks in the round.
        let mut prev_block = block.number().checked_sub(1);
        while let Some(b) = prev_block {
            let old_block = get_block(&client, b).await?;
            match read_block(&client, &old_block, &mut state, chain_name).await? {
                ReadBlock::PhaseClosed | ReadBlock::ElectionFinalized(_) => break,
                ReadBlock::Done => {}
            }
            prev_block = b.checked_sub(1);
        }

        tracing::info!("state: {:?}", state);

        let (score, addr, r) = state
            .submissions
            .iter()
            .max_by(|a, b| a.0.cmp(&b.0))
            .cloned()
            .expect("A winner must exist; qed");

        assert_eq!(score, winner.score.0);
        prometheus::election_winner(&chain_name.to_string(), r, addr, score);

        state.clear();
    }
}
