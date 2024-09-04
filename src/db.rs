// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::{Address, Slashed, Submission, Winner, LOG_TARGET};
use sqliter::{async_rusqlite, rusqlite, Connection, ConnectionBuilder};
use std::{num::NonZeroUsize, str::FromStr};

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
                        score BLOB,
                        success INTEGER
                    ) STRICT;
                    CREATE TABLE IF NOT EXISTS election_winners (
                        id INTEGER PRIMARY KEY NOT NULL,
                        address TEXT,
                        round INTEGER,
                        block INTEGER,
                        score BLOB
                    ) STRICT;
                    CREATE TABLE IF NOT EXISTS slashed (
                        id INTEGER PRIMARY KEY NOT NULL,
                        address TEXT,
                        amount TEXT,
                        round INTEGER,
                        block INTEGER
                    ) STRICT;
                    ",
                )
                .map(|_| ())
            })
            .open(path.as_ref())
            .await?;

        Ok(Self(conn))
    }

    pub async fn insert_submission(&self, submission: Submission) -> Result<(), Error> {
        let Submission {
            who,
            round,
            block,
            score,
            success,
        } = submission;

        let success = if success { 1 } else { 0 };
        let who = who.to_string();
        let score = serde_json::to_vec(&score)?;
        self.0
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO submissions (address, round, block, score, success) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![who, round, block, score, success],
                )
            })
            .await?;

        Ok(())
    }

    pub async fn insert_election_winner(&self, winner: Winner) -> Result<(), Error> {
        let Winner {
            who,
            round,
            block,
            score,
        } = winner;

        let score = serde_json::to_vec(&score)?;
        let who = who.to_string();

        self.0
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO election_winners (address, round, block, score) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![who, round, block, score],
                )
            })
            .await?;

        Ok(())
    }

    pub async fn insert_slashed(&self, slashed: Slashed) -> Result<(), Error> {
        let Slashed {
            who,
            round,
            block,
            amount,
        } = slashed;

        self.0
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO slashed (address, amount, round, block) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![who.to_string(), amount, round, block],
                )
            })
            .await?;

        Ok(())
    }

    pub async fn get_all_submissions(&self) -> Result<Vec<Submission>, Error> {
        self.0
            .call(|conn| submissions(conn.prepare("SELECT * FROM submissions")?))
            .await
    }

    pub async fn get_all_election_winners(&self) -> Result<Vec<Winner>, Error> {
        self.0
            .call(|conn| winners(conn.prepare("SELECT * FROM election_winners")?))
            .await
    }

    pub async fn get_all_unsigned_winners(&self) -> Result<Vec<Winner>, Error> {
        self.0
            .call(|conn| {
                winners(conn.prepare("SELECT * FROM election_winners WHERE address = 'unsigned'")?)
            })
            .await
    }

    pub async fn get_most_recent_unsigned_winners(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Winner>, Error> {
        self.0.call(move |conn| {
            winners(conn.prepare(&format!("SELECT * FROM election_winners WHERE address = 'unsigned' ORDER BY round DESC LIMIT {n}"))?)
        }).await
    }

    pub async fn get_most_recent_submissions(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, Error> {
        self.0
            .call(move |conn| {
                let stmt = conn.prepare(&format!(
                    "SELECT * FROM submissions ORDER BY round DESC LIMIT {n}",
                ))?;
                submissions(stmt)
            })
            .await
    }

    pub async fn get_most_recent_election_winners(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Winner>, Error> {
        self.0
            .call(move |conn| {
                let stmt = conn.prepare(&format!(
                    "SELECT * FROM election_winners ORDER BY round DESC LIMIT {n}",
                ))?;
                winners(stmt)
            })
            .await
    }

    pub async fn get_all_slashed(&self) -> Result<Vec<Slashed>, Error> {
        self.0
            .call(|conn| slashed(conn.prepare("SELECT * FROM slashed")?))
            .await
    }

    pub async fn get_most_recent_slashed(&self, n: NonZeroUsize) -> Result<Vec<Slashed>, Error> {
        self.0
            .call(move |conn| {
                let stmt = conn.prepare(&format!(
                    "SELECT * FROM slashed ORDER BY round DESC LIMIT {n}",
                ))?;
                slashed(stmt)
            })
            .await
    }
}

fn submissions(mut stmt: rusqlite::Statement<'_>) -> Result<Vec<Submission>, Error> {
    let rows = stmt.query_map([], |row| {
        let who = {
            let a: String = row.get(1)?;
            Address::from_str(&a).unwrap()
        };
        let round = row.get(2)?;
        let block = row.get(3)?;
        let score = {
            let bytes: Vec<u8> = row.get(4)?;
            serde_json::from_slice(&bytes).unwrap()
        };
        let success = {
            match row.get(5)? {
                0 => false,
                1 => true,
                _ => unreachable!(),
            }
        };

        Ok(Submission {
            who,
            round,
            block,
            score,
            success,
        })
    })?;

    let mut submissions = Vec::new();
    for row in rows {
        let row = row?;
        submissions.push(row);
    }

    Ok(submissions)
}

fn winners(mut stmt: rusqlite::Statement<'_>) -> Result<Vec<Winner>, Error> {
    let rows = stmt.query_map([], |row| {
        let who = {
            let a: String = row.get(1)?;
            Address::from_str(&a).unwrap()
        };
        let round = row.get(2)?;
        let block = row.get(3)?;

        let score = {
            let bytes: Vec<u8> = row.get(4)?;
            serde_json::from_slice(&bytes).unwrap()
        };

        Ok(Winner {
            who,
            round,
            block,
            score,
        })
    })?;

    let mut winners = Vec::new();
    for row in rows {
        let row = row?;
        winners.push(row);
    }

    Ok(winners)
}

fn slashed(mut stmt: rusqlite::Statement<'_>) -> Result<Vec<Slashed>, Error> {
    let rows = stmt.query_map([], |row| {
        let who = {
            let a: String = row.get(1)?;
            Address::from_str(&a).unwrap()
        };

        Ok(Slashed {
            who,
            amount: row.get(2)?,
            round: row.get(3)?,
            block: row.get(4)?,
        })
    })?;

    let mut winners = Vec::new();
    for row in rows {
        let row = row?;
        winners.push(row);
    }

    Ok(winners)
}

#[cfg(test)]
mod tests {
    use super::{Address, Database, Slashed, Submission, Winner};

    #[tokio::test]
    async fn put_get_submission_works() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test-db1.app");

        let db = Database::new(&path).await.unwrap();
        let submission = Submission {
            who: Address::unsigned(),
            round: 1,
            block: 1,
            score: Default::default(),
            success: true,
        };

        db.insert_submission(submission.clone()).await.unwrap();
        assert_eq!(db.get_all_submissions().await.unwrap(), vec![submission]);
    }

    #[tokio::test]
    async fn put_get_winner_works() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test-db1.app");

        let db = Database::new(&path).await.unwrap();
        let winner = Winner {
            who: Address::unsigned(),
            round: 1,
            block: 1,
            score: Default::default(),
        };

        db.insert_election_winner(winner.clone()).await.unwrap();
        assert_eq!(db.get_all_election_winners().await.unwrap(), vec![winner]);
    }

    #[tokio::test]
    async fn put_get_slashed() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test-db1.app");

        let db = Database::new(&path).await.unwrap();
        let slashed = Slashed {
            who: Address::unsigned(),
            amount: "100".to_owned(),
            round: 1,
            block: 1,
        };

        db.insert_slashed(slashed.clone()).await.unwrap();
        assert_eq!(db.get_all_slashed().await.unwrap(), vec![slashed]);
    }
}
