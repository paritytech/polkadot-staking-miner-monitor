//! Multi-block election.

use crate::db;
use crate::types::{BlockRef, Client, ElectionRound, Header, ReadBlock};

#[subxt::subxt(
    runtime_metadata_path = "artifacts/multi_block.scale",
    derive_for_all_types = "Clone, Debug, Eq, PartialEq"
)]
pub mod runtime {}

pub async fn run(
    _client: Client,
    _state: &mut ElectionRound,
    _block_ref: BlockRef,
    _block: Header,
    _db: &db::Database,
) -> anyhow::Result<ReadBlock> {
    todo!();
}
