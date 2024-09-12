// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

mod db;
mod helpers;
mod routes;
mod types;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use clap::Parser;
use db::Election;
use helpers::{
    get_phase, get_round, read_block, read_remaining_blocks_in_round, runtime_upgrade_task,
    ReadBlock,
};
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::mpsc,
};
use tracing_subscriber::{util::SubscriberInitExt, EnvFilter};
use types::{Address, Client, ElectionRound, HeaderT};
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
    pub log: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Opt {
        polkadot,
        listen_addr,
        postgres,
        log,
    } = Opt::parse();

    let filter = EnvFilter::from_default_env().add_directive(log.parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .finish()
        .try_init()?;

    let client = Client::new(polkadot).await?;

    tracing::info!(target: LOG_TARGET, "Connected to chain {}", client.chain_name());
    let db = db::Database::new(postgres).await?;

    let (stop_tx, mut stop_rx) = mpsc::channel(1);

    let db2 = db.clone();
    let stop_tx2 = stop_tx.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                _ = stop_tx2.blocking_send(format!("Failed to create HTTP server threadpool: {e}"));
                return;
            }
        };
        let rp = rt.block_on(async {
            use actix_web::{web, App, HttpServer};

            let server = oasgen::Server::actix()
                .route_json_spec("/docs/openapi.json")
                .route_yaml_spec("/docs/openapi.yaml")
                .swagger_ui("/docs/")
                .get("/submissions", routes::all_submissions)
                .get("/elections", routes::all_elections)
                .get("/slashed", routes::all_slashed)
                .get("/submissions/{n}", routes::most_recent_submissions)
                .get("/elections/{n}", routes::most_recent_elections)
                .get("/slashed/{n}", routes::most_recent_slashed)
                .freeze();

            HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(db2.clone()))
                    .service(server.clone().into_service())
            })
            .bind(listen_addr)?
            .run()
            .await
        });

        let close = match rp {
            Ok(()) => "HTTP Server closed".to_string(),
            Err(e) => e.to_string(),
        };
        _ = stop_tx2.blocking_send(close);
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

        let election_finalized = match read_block(&client, &block, &mut state, &db).await? {
            ReadBlock::PhaseClosed => unreachable!("Phase already checked; qed"),
            ReadBlock::ElectionFinalized(winner) => {
                read_remaining_blocks_in_round(&client, &mut state, block.number() as u64, &db)
                    .await?;
                winner
            }
            ReadBlock::Done => continue,
        };

        tracing::debug!(target: LOG_TARGET, "state {:?}", state);

        let election_result = state.complete();

        db.insert_election(Election::new(
            election_result,
            round,
            block.number(),
            election_finalized.score.0,
        ))
        .await?;
    }
}
