// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

mod db;
mod helpers;
mod routes;
mod types;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use clap::Parser;
use db::Winner;
use helpers::*;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::mpsc,
};
use tracing_subscriber::util::SubscriberInitExt;
use types::*;
use url::Url;

const LOG_TARGET: &str = "polkadot-staking-miner-monitor";

#[derive(Debug, Clone, Parser)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
struct Opt {
    /// The URL of the polkadot node to connect to.
    #[clap(long)]
    polkadot: Url,
    /// This listen addr to listen on for a REST API to query the database.
    #[clap(long, default_value_t = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9999)))]
    listen_addr: SocketAddr,
    /// The URL of the PostgreSQL database to connect to.
    /// The URL should be in the form of `postgres://user:password@host:port/dbname`.
    #[clap(long)]
    postgres: Url,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Opt {
        polkadot,
        listen_addr,
        postgres,
    } = Opt::parse();

    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env()?)
        .finish()
        .try_init()?;

    let client = Client::new(polkadot).await?;

    tracing::info!(target: LOG_TARGET, "Connected to chain {}", client.chain_name());

    let db = db::Database::new(postgres).await?;

    let db2 = db.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            use actix_web::{web, App, HttpServer};

            HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(db2.clone()))
                    .route("/", web::get().to(routes::home))
                    .route("/submissions", web::get().to(routes::all_submissions))
                    .route("/winners", web::get().to(routes::all_election_winners))
                    .route(
                        "/unsigned-winners",
                        web::get().to(routes::all_unsigned_winners),
                    )
                    .route("/slashed", web::get().to(routes::all_slashed))
                    .route(
                        "/submissions/{n}",
                        web::get().to(routes::most_recent_submissions),
                    )
                    .route(
                        "/winners/{n}",
                        web::get().to(routes::most_recent_election_winners),
                    )
                    .route(
                        "/unsigned-winners/{n}",
                        web::get().to(routes::most_recent_unsigned_winners),
                    )
                    .route("/slashed/{n}", web::get().to(routes::most_recent_slashed))
            })
            .bind(listen_addr)?
            .run()
            .await
        })
        .unwrap();
    });

    let mut blocks = client
        .chain_api()
        .backend()
        .stream_finalized_block_headers()
        .await?;

    let mut state = SubmissionsInRound::new();

    let (upgrade_tx, mut upgrade_rx) = mpsc::channel(1);
    tokio::spawn(runtime_upgrade_task(client.chain_api().clone(), upgrade_tx));

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

        // If the winner is rewarded it's signed otherwise it's unsigned.
        let who = match state.complete() {
            Some(who) => who,
            None => Address::unsigned(),
        };

        db.insert_election_winner(Winner {
            who,
            round,
            block: block.number(),
            score: election_finalized.score.0,
        })
        .await?;
        state.clear();
    }
}
