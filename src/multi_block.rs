//! Multi-block election.
#[subxt::subxt(
    runtime_metadata_path = "artifacts/multi_block.scale",
    derive_for_all_types = "Clone, Debug, Eq, PartialEq"
)]
pub mod runtime {}

use crate::db;
use crate::types::{BlockRef, Client, ElectionRound, Header, HeaderT, ReadBlock};
use runtime::runtime_types::pallet_election_provider_multi_block::types::Phase;
use tracing::Instrument;

pub async fn run(
    client: &Client,
    state: &mut ElectionRound,
    block_ref: BlockRef,
    block: Header,
    db: &db::Database,
) -> anyhow::Result<ReadBlock> {
    let storage = client.chain_api().storage().at(block.hash());

    let phase = storage
        .fetch(&runtime::storage().multi_block().current_phase())
        .await?
        .ok_or(anyhow::anyhow!("Phase not found"))?;

    tracing::info!("Processing block {:?} phase: {:?}", block.number(), phase);

    match phase {
        Phase::Signed(_) => {
            // solutions may be submitted

            let block = client.chain_api().blocks().at(block.hash()).await?;
            let extrinsics = block.extrinsics().await?;

            for ext in extrinsics.iter() {
                let pallet_name = ext.pallet_name().unwrap();
                let variant_name = ext.variant_name().unwrap();

                match (pallet_name, variant_name) {
                    ("MultiBlockSigned", "register") => {
                        tracing::debug!(
                            target: "multi_block",
                            "register score",
                        );
                    }
                    ("MultiBlockSigned", "submit_page") => {
                        tracing::debug!(
                            target: "multi_block",
                            "submit_page",
                        );
                    }
                    _ => {}
                };
            }
        }
        Phase::SignedValidation(_) => {
            // solutions are being validated
            // report if any are invalid
        }
        Phase::Emergency => {
            todo!("emergency");
        }
        Phase::Halted => {
            // halted
            todo!("halted");
        }
        Phase::Unsigned(_) | Phase::Off | Phase::Snapshot(_) | Phase::Export(_) | Phase::Done => {
            return Ok(ReadBlock::PhaseClosed)
        }
    }

    Ok(ReadBlock::PhaseClosed)
}
