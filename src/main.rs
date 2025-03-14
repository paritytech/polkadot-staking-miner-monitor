// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

mod db;
mod helpers;
mod legacy;
mod multi_block;
mod prometheus;
mod routes;
mod types;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use clap::Parser;
use db::Election;
use helpers::runtime_upgrade_task;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::mpsc,
};
use tracing_subscriber::{util::SubscriberInitExt, EnvFilter};
use types::{Address, Client, ElectionRound, HeaderT, ReadBlock};
use url::Url;

const LOG_TARGET: &str = "polkadot-staking-miner-monitor";

#[derive(Debug, Clone, Parser)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
struct Opt {
    /// The URL of the polkadot node to connect to.
    #[clap(long, env = "POLKADOT_URL")]
    polkadot: Url,
    /// This listen addr to listen on for a REST API to query the database.
    #[clap(long, default_value_t = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9999)), env = "LISTEN_ADDR")]
    listen_addr: SocketAddr,
    /// The URL of the PostgreSQL database to connect to.
    /// The URL should be in the form of `postgres://user:password@host:port/dbname`.
    #[clap(long, env = "POSTGRES_URL")]
    postgres: Url,
    /// Sets a custom logging filter. Syntax is `<target>=<level>`, e.g. -lpolkadot-staking-miner-monitor=debug.
    ///
    /// Log levels (least to most verbose) are error, warn, info, debug, and trace.
    /// By default, all targets log `info`. The global log level can be set with `-l<level>`.
    #[clap(long, short, default_value = "info")]
    log: String,

    /// Experimental multi-block election.
    #[clap(long, short, default_value_t = false)]
    experimental_multi_block: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Opt {
        polkadot,
        listen_addr,
        postgres,
        log,
        experimental_multi_block,
    } = Opt::parse();

    let filter = EnvFilter::from_default_env().add_directive(log.parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .finish()
        .try_init()?;

    let client = Client::new(polkadot).await?;
    let prometheus = prometheus::setup_metrics_recorder()?;

    tracing::info!(target: LOG_TARGET, "Connected to chain {}", client.chain_name());
    let db = db::Database::new(postgres).await?;
    let (stop_tx, mut stop_rx) = mpsc::channel(1);
    let stop_tx2 = stop_tx.clone();
    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    let state = (db.clone(), prometheus.clone());

    tokio::spawn(async move {
        let app = oasgen::Server::axum()
            .route_json_spec("/docs/openapi.json")
            .route_yaml_spec("/docs/openapi.yaml")
            .swagger_ui("/docs/")
            .get("/elections/", routes::all_elections)
            .get("/elections/unsigned", routes::all_unsigned_elections)
            .get("/elections/failed", routes::all_failed_elections)
            .get("/elections/signed", routes::all_signed_elections)
            .get("/elections/{n}", routes::most_recent_elections)
            .get("/slashed/", routes::all_slashed)
            .get("/slashed/{n}", routes::most_recent_slashed)
            .get("/submissions/", routes::all_submissions)
            .get("/submissions/success", routes::all_success_submissions)
            .get("/submissions/failed", routes::all_failed_submissions)
            .get("/submissions/{n}", routes::most_recent_submissions)
            .get("/metrics", routes::metrics)
            .get("/stats", routes::stats)
            .freeze()
            .into_router()
            .with_state(state);

        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                stop_tx2.closed().await;
            })
            .await
        {
            tracing::error!(target: LOG_TARGET, "Server error: {:?}", e);
        }
    });

    let mut blocks = client
        .chain_api()
        .backend()
        .stream_finalized_block_headers()
        .await?;

    let mut state = ElectionRound::new();

    tokio::spawn(runtime_upgrade_task(client.chain_api().clone(), stop_tx));

    let mut stream_int = signal(SignalKind::interrupt())?;
    let mut stream_term = signal(SignalKind::terminate())?;

    loop {
        let (block, block_ref) = tokio::select! {
            _ = stream_int.recv() => {
                return Ok(());
            }
            _ = stream_term.recv() => {
                return Ok(());
            }
            msg = stop_rx.recv() => {
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

        let block_number = block.number();

        let block_status = if experimental_multi_block {
            multi_block::run(&client, &mut state, block_ref, block, &db).await?
        } else {
            legacy::run(&client, &mut state, block_ref, block, &db).await?
        };

        let score = match block_status {
            ReadBlock::PhaseClosed | ReadBlock::Done => continue,
            ReadBlock::ElectionFinalized(score) => score,
        };

        let (election_result, round) = state.complete();

        tracing::debug!(target: LOG_TARGET, "state {:?}", state);

        prometheus::record_election(&election_result);
        db.insert_election(Election::new(election_result, round, block_number, score))
            .await?;
    }
}
