// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::{Address, LOG_TARGET};
use sp_npos_elections::ElectionScore;
use sqliter::{async_rusqlite, rusqlite, Connection, ConnectionBuilder};
use std::num::NonZeroUsize;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database is closed")]
    Closed(async_rusqlite::AlreadyClosed),
    #[error("Failed open database connection {0}")]
    Connection(#[from] sqliter::ConnectionBuilderError),
    #[error(transparent)]
    InvalidSql(#[from] rusqlite::Error),
    #[error("Failed encode/decode JSON {0}")]
    InvalidJson(#[from] serde_json::Error),
}

impl From<async_rusqlite::AlreadyClosed> for Error {
    fn from(e: async_rusqlite::AlreadyClosed) -> Self {
        Self::Closed(e)
    }
}

#[derive(Debug, Clone, Copy)]
/// Database tables.
enum Table {
    Submissions,
    ElectionWinners,
}

impl Table {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Submissions => "submissions",
            Self::ElectionWinners => "election_winners",
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Submission {
    pub address: String,
    pub round: u32,
    pub block: u32,
    pub score: ElectionScore,
}

#[derive(Debug, Clone)]
pub struct Database(Connection);

impl Database {
    pub async fn new(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        const APP_ID: i32 = 12;

        tracing::debug!(target: LOG_TARGET, "opening db path: {:?}", path.as_ref());

        let conn = ConnectionBuilder::new()
            .app_id(APP_ID)
            .add_migration(1, |conn| {
                conn.execute_batch(
                    "
                    CREATE TABLE IF NOT EXISTS submissions (
                        id INTEGER PRIMARY KEY NOT NULL,
                        address TEXT,
                        round INTEGER,
                        block INTEGER,
                        score BLOB
                    ) STRICT;
                    CREATE TABLE IF NOT EXISTS election_winners (
                        id INTEGER PRIMARY KEY NOT NULL,
                        address TEXT,
                        round INTEGER,
                        block INTEGER,
                        score BLOB
                    ) STRICT;
                    ",
                )
                .map(|_| ())
            })
            .open(path.as_ref())
            .await?;

        Ok(Self(conn))
    }

    pub async fn insert_submission(
        &self,
        address: Option<Address>,
        round: u32,
        score: ElectionScore,
        block: u32,
    ) -> Result<(), Error> {
        self.insert(Table::Submissions, address, round, score, block)
            .await
    }

    pub async fn insert_election_winner(
        &self,
        address: Option<Address>,
        round: u32,
        score: ElectionScore,
        block: u32,
    ) -> Result<(), Error> {
        self.insert(Table::ElectionWinners, address, round, score, block)
            .await
    }

    async fn insert(
        &self,
        table: Table,
        address: Option<Address>,
        round: u32,
        score: ElectionScore,
        block: u32,
    ) -> Result<(), Error> {
        let addr = if let Some(addr) = address {
            // The debug formatter for `Address` is the hex representation of the full address.
            format!("{:?}", addr)
        } else {
            "unsigned".to_string()
        };

        let score = serde_json::to_vec(&score)?;
        self.0
            .call(move |conn| {
                conn.execute(
                    &format!(
                        "INSERT INTO {} (address, round, block, score) VALUES (?1, ?2, ?3, ?4)",
                        table.as_str(),
                    ),
                    rusqlite::params![addr, round, block, score],
                )
            })
            .await?;

        Ok(())
    }

    pub async fn get_all_submissions(&self) -> Result<Vec<Submission>, Error> {
        self.get_all(Table::Submissions).await
    }

    pub async fn get_all_election_winners(&self) -> Result<Vec<Submission>, Error> {
        self.get_all(Table::ElectionWinners).await
    }

    pub async fn get_all_unsigned_winners(&self) -> Result<Vec<Submission>, Error> {
        self.0
            .call(|conn| {
                stmt_to_submissions(
                    conn.prepare("SELECT * FROM election_winners WHERE address = 'unsigned'")?,
                )
            })
            .await
    }

    pub async fn get_most_recent_unsigned_winners(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, Error> {
        self.0.call(move |conn| {
            stmt_to_submissions(conn.prepare(&format!("SELECT * FROM election_winners WHERE address = 'unsigned' ORDER BY round DESC LIMIT {n}"))?)
        }).await
    }

    pub async fn get_most_recent_submissions(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, Error> {
        self.get_most_recent(Table::Submissions, n).await
    }

    pub async fn get_most_recent_election_winners(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, Error> {
        self.get_most_recent(Table::ElectionWinners, n).await
    }

    async fn get_all(&self, table: Table) -> Result<Vec<Submission>, Error> {
        self.0
            .call(move |conn| {
                stmt_to_submissions(conn.prepare(&format!("SELECT * FROM {}", table.as_str()))?)
            })
            .await
    }

    async fn get_most_recent(
        &self,
        table: Table,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, Error> {
        self.0
            .call(move |conn| {
                let stmt = conn.prepare(&format!(
                    "SELECT * FROM {} ORDER BY round DESC LIMIT {n}",
                    table.as_str(),
                ))?;
                stmt_to_submissions(stmt)
            })
            .await
    }
}

fn stmt_to_submissions(mut stmt: rusqlite::Statement<'_>) -> Result<Vec<Submission>, Error> {
    let rows = stmt.query_map([], |row| {
        let bytes: Vec<u8> = row.get(4)?;
        let s = serde_json::from_slice(&bytes).unwrap();

        Ok(Submission {
            address: row.get(1)?,
            round: row.get(2)?,
            block: row.get(3)?,
            score: s,
        })
    })?;

    let mut submissions = Vec::new();
    for row in rows {
        let row = row?;
        submissions.push(row);
    }

    Ok(submissions)
}

#[cfg(test)]
mod tests {
    use super::{Database, Submission};

    #[tokio::test]
    async fn it_works() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test-db1.app");

        let db = Database::new(&path).await.unwrap();
        db.insert_submission(None, 1, Default::default(), 1)
            .await
            .unwrap();

        let submissions = db.get_all_submissions().await.unwrap();

        assert_eq!(
            submissions,
            vec![Submission {
                address: "unsigned".to_string(),
                round: 1,
                block: 1,
                score: Default::default(),
            }]
        );
    }
}
