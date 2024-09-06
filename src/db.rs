// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::{Address, LOG_TARGET};
use serde::{Deserialize, Serialize};
use sp_npos_elections::ElectionScore;
use std::str::FromStr;
use std::{num::NonZeroUsize, sync::Arc};
use tokio_postgres::row::Row;
use tokio_postgres::{Client, NoTls};
use url::Url;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to decode/encode: {0}")]
    Parse(String),
    #[error("Could not find row={0} at position={1}")]
    RowNotFound(&'static str, usize),
    #[error(transparent)]
    Database(#[from] tokio_postgres::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Submission {
    pub who: Address,
    pub round: u32,
    pub block: u32,
    pub score: ElectionScore,
    pub success: bool,
}

impl TryFrom<Row> for Submission {
    type Error = Error;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        let who = {
            let val: String = row
                .try_get(1)
                .map_err(|_| Error::RowNotFound("address", 1))?;
            Address::from_str(&val).map_err(|e| Error::Parse(e.to_string()))?
        };
        let round = row.try_get(2).map_err(|_| Error::RowNotFound("round", 2))?;
        let block = row.try_get(3).map_err(|_| Error::RowNotFound("block", 3))?;
        let score = {
            let score: Vec<u8> = row.try_get(4).map_err(|_| Error::RowNotFound("score", 3))?;
            serde_json::from_slice(&score).map_err(|e| Error::Parse(e.to_string()))?
        };
        let success = row
            .try_get(5)
            .map_err(|_| Error::RowNotFound("success", 5))?;

        Ok(Self {
            who,
            round,
            block,
            score,
            success,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Winner {
    pub who: Address,
    pub round: u32,
    pub block: u32,
    pub score: ElectionScore,
}

impl TryFrom<Row> for Winner {
    type Error = Error;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        let who = {
            let val: String = row
                .try_get(1)
                .map_err(|_| Error::RowNotFound("address", 1))?;
            Address::from_str(&val).map_err(|e| Error::Parse(e.to_string()))?
        };
        let round = row.try_get(2).map_err(|_| Error::RowNotFound("round", 2))?;
        let block = row.try_get(3).map_err(|_| Error::RowNotFound("block", 3))?;
        let score = {
            let score: Vec<u8> = row.try_get(4).map_err(|_| Error::RowNotFound("score", 4))?;
            serde_json::from_slice(&score).map_err(|e| Error::Parse(e.to_string()))?
        };

        Ok(Self {
            who,
            round,
            block,
            score,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Slashed {
    pub who: Address,
    pub round: u32,
    pub block: u32,
    pub amount: String,
}

impl TryFrom<Row> for Slashed {
    type Error = Error;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        let who = {
            let val: String = row
                .try_get(1)
                .map_err(|_| Error::RowNotFound("address", 1))?;
            Address::from_str(&val).map_err(|e| Error::Parse(e.to_string()))?
        };
        let amount = row
            .try_get(2)
            .map_err(|_| Error::RowNotFound("amount", 2))?;
        let round = row.try_get(3).map_err(|_| Error::RowNotFound("round", 3))?;
        let block = row.try_get(4).map_err(|_| Error::RowNotFound("block", 4))?;

        Ok(Self {
            who,
            amount,
            round,
            block,
        })
    }
}

impl Slashed {
    pub fn new(
        who: subxt::config::substrate::AccountId32,
        round: u32,
        block: u32,
        amount: u128,
    ) -> Self {
        Self {
            who: Address::from_bytes(who.0.as_ref()),
            round,
            block,
            amount: amount.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Database(Arc<Client>);

impl Database {
    pub async fn new(url: Url) -> Result<Self, Error> {
        tracing::debug!(target: LOG_TARGET, "connecting to postgres db: {url}");
        let (db, connection) = tokio_postgres::connect(url.as_str(), NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!(target: LOG_TARGET, "connection error: {e}");
            }
        });

        db.batch_execute(
            "
            CREATE TABLE IF NOT EXISTS submissions (
                id SERIAL PRIMARY KEY,
                address TEXT,
                round OID,
                block OID,
                score BYTEA,
                success BOOLEAN
            );
            CREATE TABLE IF NOT EXISTS election_winners (
                id SERIAL PRIMARY KEY,
                address TEXT,
                round OID,
                block OID,
                score BYTEA
            );
            CREATE TABLE IF NOT EXISTS slashed (
                id SERIAL PRIMARY KEY,
                address TEXT,
                amount TEXT,
                round OID,
                block OID
            );",
        )
        .await?;

        Ok(Self(Arc::new(db)))
    }

    pub async fn insert_submission(&self, submission: Submission) -> Result<(), Error> {
        let Submission {
            who,
            round,
            block,
            score,
            success,
        } = submission;

        let who = who.to_string();
        let score = serde_json::to_vec(&score).map_err(|e| Error::Parse(e.to_string()))?;
        let stmt = self.0.prepare("INSERT INTO submissions (address, round, block, score, success) VALUES ($1, $2, $3, $4, $5)").await?;
        self.0
            .execute(&stmt, &[&who, &round, &block, &score, &success])
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

        let score = serde_json::to_vec(&score).map_err(|e| Error::Parse(e.to_string()))?;
        let who = who.to_string();

        let stmt = self
            .0
            .prepare(
                "INSERT INTO election_winners (address, round, block, score) VALUES ($1, $2, $3, $4)",
            )
            .await?;
        self.0
            .execute(&stmt, &[&who, &round, &block, &score])
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

        let who = who.to_string();

        let stmt = self
            .0
            .prepare("INSERT INTO slashed (address, amount, round, block) VALUES ($1, $2, $3, $4)")
            .await?;
        self.0
            .execute(&stmt, &[&who, &amount, &round, &block])
            .await?;

        Ok(())
    }

    pub async fn get_all_submissions(&self) -> Result<Vec<Submission>, Error> {
        collect_db_rows(self.0.query("SELECT * FROM submissions", &[]).await?)
    }

    pub async fn get_all_election_winners(&self) -> Result<Vec<Winner>, Error> {
        collect_db_rows(self.0.query("SELECT * FROM election_winners", &[]).await?)
    }

    pub async fn get_all_unsigned_winners(&self) -> Result<Vec<Winner>, Error> {
        collect_db_rows(
            self.0
                .query(
                    "SELECT * FROM election_winners WHERE address = 'unsigned'",
                    &[],
                )
                .await?,
        )
    }

    pub async fn get_most_recent_unsigned_winners(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Winner>, Error> {
        collect_db_rows(
            self.0
                .query(
                    &format!("SELECT * FROM election_winners WHERE address = 'unsigned' ORDER BY round DESC LIMIT {n}"),
                    &[],
                )
                .await?,
        )
    }

    pub async fn get_most_recent_submissions(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, Error> {
        collect_db_rows(
            self.0
                .query(
                    &format!("SELECT * FROM submissions ORDER BY round DESC LIMIT {n}"),
                    &[],
                )
                .await?,
        )
    }

    pub async fn get_most_recent_election_winners(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Winner>, Error> {
        collect_db_rows(
            self.0
                .query(
                    &format!("SELECT * FROM election_winners ORDER BY round DESC LIMIT {n}"),
                    &[],
                )
                .await?,
        )
    }

    pub async fn get_all_slashed(&self) -> Result<Vec<Slashed>, Error> {
        collect_db_rows(self.0.query("SELECT * FROM slashed", &[]).await?)
    }

    pub async fn get_most_recent_slashed(&self, n: NonZeroUsize) -> Result<Vec<Slashed>, Error> {
        collect_db_rows(
            self.0
                .query(
                    &format!("SELECT * FROM slashed ORDER BY round DESC LIMIT {n}"),
                    &[],
                )
                .await?,
        )
    }
}

fn collect_db_rows<T>(rows: Vec<tokio_postgres::Row>) -> Result<Vec<T>, Error>
where
    T: TryFrom<tokio_postgres::Row, Error = Error>,
{
    let mut items = Vec::new();

    for row in rows {
        let val = row.try_into()?;
        items.push(val);
    }

    Ok(items)
}
