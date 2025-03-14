// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::types::ElectionResult as InnerElectionResult;
use crate::{Address, LOG_TARGET};
use oasgen::OaSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sp_npos_elections::ElectionScore;
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::Arc;
use tokio_postgres::row::Row;
use tokio_postgres::{Client, NoTls};
use url::Url;

refinery::embed_migrations!("migrations");

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to decode/encode: {0}")]
    Parse(String),
    #[error("Could not find row={0} at position={1}")]
    RowNotFound(&'static str, usize),
    #[error(transparent)]
    Database(#[from] tokio_postgres::Error),
    #[error(transparent)]
    Migration(#[from] refinery::Error),
}

#[derive(Debug, Clone)]
pub struct Database(Arc<Client>);

impl Database {
    pub async fn new(url: Url) -> Result<Self, Error> {
        tracing::debug!(target: LOG_TARGET, "connecting to postgres db: {url}");
        let (mut db, connection) = tokio_postgres::connect(url.as_str(), NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!(target: LOG_TARGET, "connection error: {e}");
            }
        });

        migrations::runner().run_async(&mut db).await?;
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
        let stmt = self.0.prepare("INSERT INTO submissions (address, round, block, score, success) VALUES ($1, $2, $3, $4, $5)").await?;
        self.0
            .execute(&stmt, &[&who, &round, &block, &score, &success])
            .await?;

        Ok(())
    }

    pub async fn insert_election(&self, election: Election) -> Result<(), Error> {
        let Election {
            result,
            winner,
            round,
            block,
            score,
            ..
        } = election;

        let stmt = self
            .0
            .prepare(
                "INSERT INTO elections (result, address, round, block, score) VALUES ($1, $2, $3, $4, $5)",
            )
            .await?;
        self.0
            .execute(&stmt, &[&result, &winner, &round, &block, &score])
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

    pub async fn get_all_success_submissions(&self) -> Result<Vec<Submission>, Error> {
        collect_db_rows(
            self.0
                .query("SELECT * FROM submissions where success = true", &[])
                .await?,
        )
    }

    pub async fn get_all_failed_submissions(&self) -> Result<Vec<Submission>, Error> {
        collect_db_rows(
            self.0
                .query("SELECT * FROM submissions where success = false", &[])
                .await?,
        )
    }

    pub async fn get_all_unsigned_elections(&self) -> Result<Vec<Election>, Error> {
        collect_db_rows(
            self.0
                .query("SELECT * FROM elections where result = 'unsigned'", &[])
                .await?,
        )
    }

    pub async fn get_all_signed_elections(&self) -> Result<Vec<Election>, Error> {
        collect_db_rows(
            self.0
                .query("SELECT * FROM elections where result = 'signed'", &[])
                .await?,
        )
    }

    pub async fn get_all_failed_elections(&self) -> Result<Vec<Election>, Error> {
        collect_db_rows(
            self.0
                .query(
                    "SELECT * FROM elections where result = 'election failed'",
                    &[],
                )
                .await?,
        )
    }

    pub async fn get_all_elections(&self) -> Result<Vec<Election>, Error> {
        collect_db_rows(self.0.query("SELECT * FROM elections", &[]).await?)
    }

    pub async fn get_all_slashed(&self) -> Result<Vec<Slashed>, Error> {
        collect_db_rows(self.0.query("SELECT * FROM slashed", &[]).await?)
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

    pub async fn get_most_recent_elections(&self, n: NonZeroUsize) -> Result<Vec<Election>, Error> {
        collect_db_rows(
            self.0
                .query(
                    &format!("SELECT * FROM elections ORDER BY round DESC LIMIT {n}"),
                    &[],
                )
                .await?,
        )
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

    pub async fn get_stats(&self) -> Result<Stats, Error> {
        let submissions = self
            .collect_count("SELECT COUNT(*) FROM submissions")
            .await?;

        let submissions_failed = self
            .collect_count("SELECT COUNT(*) FROM submissions WHERE success = false")
            .await?;

        let submissions_success = self
            .collect_count("SELECT COUNT(*) FROM submissions WHERE success = true")
            .await?;

        let elections = self.collect_count("SELECT COUNT(*) FROM elections").await?;

        let elections_failed = self
            .collect_count("SELECT COUNT(*) FROM elections WHERE result = 'election failed'")
            .await?;

        let elections_signed = self
            .collect_count("SELECT COUNT(*) FROM elections WHERE result = 'signed'")
            .await?;

        let elections_unsigned = self
            .collect_count("SELECT COUNT(*) FROM elections WHERE result = 'unsigned'")
            .await?;

        let slashed = self.collect_count("SELECT COUNT(*) FROM slashed").await?;

        Ok(Stats {
            submissions: Submissions {
                total: submissions,
                failed: submissions_failed,
                success: submissions_success,
            },
            elections: Elections {
                total: elections,
                failed: elections_failed,
                signed: elections_signed,
                unsigned: elections_unsigned,
            },
            slashed,
        })
    }

    async fn collect_count(&self, statement: &str) -> Result<u64, Error> {
        let row = self.0.query_one(statement, &[]).await?;
        Ok(row.get::<_, i64>(0) as u64)
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, OaSchema)]
pub struct Submission {
    who: Address,
    round: u32,
    block: u32,
    score: serde_json::Value,
    success: bool,
}

impl Submission {
    pub fn new(who: Address, round: u32, block: u32, score: ElectionScore, success: bool) -> Self {
        Self {
            who,
            round,
            block,
            score: serde_json::to_value(score).expect("ElectionScore serialize infallible; qed"),
            success,
        }
    }
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
        let score = row.try_get(4).map_err(|_| Error::RowNotFound("score", 4))?;
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, OaSchema)]
pub struct Election {
    result: String,
    winner: serde_json::Value,
    round: u32,
    block: u32,
    score: serde_json::Value,
}

impl Election {
    pub fn new(
        election: InnerElectionResult,
        round: u32,
        block: u32,
        score: ElectionScore,
    ) -> Self {
        let (result, winner) = match election {
            InnerElectionResult::Signed(addr) => (
                "signed".to_string(),
                serde_json::to_value(&addr).expect("AccountId serialize infallible; qed"),
            ),
            InnerElectionResult::Unsigned => ("unsigned".to_string(), json!(null)),
            InnerElectionResult::Failed => ("election failed".to_string(), json!(null)),
        };

        Self {
            result,
            winner,
            round,
            block,
            score: serde_json::to_value(score).expect("ElectionScore serialize infallible; qed"),
        }
    }
}

impl TryFrom<Row> for Election {
    type Error = Error;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        let result = row
            .try_get(1)
            .map_err(|_| Error::RowNotFound("result", 1))?;
        let winner: serde_json::Value = row
            .try_get(2)
            .map_err(|_| Error::RowNotFound("address", 2))?;
        let round = row.try_get(3).map_err(|_| Error::RowNotFound("round", 3))?;
        let block = row.try_get(4).map_err(|_| Error::RowNotFound("block", 4))?;
        let score = row.try_get(5).map_err(|_| Error::RowNotFound("score", 5))?;

        Ok(Self {
            result,
            winner,
            round,
            block,
            score,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, OaSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct Stats {
    submissions: Submissions,
    elections: Elections,
    slashed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct Submissions {
    total: u64,
    failed: u64,
    success: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct Elections {
    total: u64,
    failed: u64,
    signed: u64,
    unsigned: u64,
}
