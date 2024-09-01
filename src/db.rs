// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use std::num::NonZeroUsize;

use crate::{Address, LOG_TARGET};
use rusqlite::params;
use sp_npos_elections::ElectionScore;

type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
type ConnectionManager = r2d2_sqlite::SqliteConnectionManager;

const SUBMISSIONS_SQL: &str = "BEGIN;
CREATE TABLE IF NOT EXISTS submissions(address TEXT, round INTEGER, block INTEGER, score BLOB);
END;";

const WINNERS_SQL: &str = "BEGIN;
CREATE TABLE IF NOT EXISTS election_winners(address TEXT, round INTEGER, block INTEGER, score BLOB);
END;";

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
pub struct Submission {
    pub address: String,
    pub round: u32,
    pub block: u32,
    pub score: ElectionScore,
}

#[derive(Debug, Clone)]
pub struct Database(Pool);

impl Database {
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, anyhow::Error> {
        tracing::debug!(target: LOG_TARGET, "opening db path: {:?}", path.as_ref());
        let manager = ConnectionManager::file(path);
        let pool = r2d2::Pool::new(manager)?;
        let conn = pool.get()?;
        conn.execute_batch(SUBMISSIONS_SQL)?;
        conn.execute_batch(WINNERS_SQL)?;

        Ok(Self(pool))
    }

    pub async fn insert_submission(
        &self,
        address: Option<Address>,
        round: u32,
        score: ElectionScore,
        block: u32,
    ) -> Result<(), anyhow::Error> {
        self.insert(Table::Submissions, address, round, score, block)
    }

    pub async fn insert_election_winner(
        &self,
        address: Option<Address>,
        round: u32,
        score: ElectionScore,
        block: u32,
    ) -> Result<(), anyhow::Error> {
        self.insert(Table::ElectionWinners, address, round, score, block)
    }

    fn insert(
        &self,
        table: Table,
        address: Option<Address>,
        round: u32,
        score: ElectionScore,
        block: u32,
    ) -> Result<(), anyhow::Error> {
        let addr = if let Some(addr) = address {
            // The debug formatter for `Address` is the hex representation of the full address.
            format!("{:?}", addr)
        } else {
            "unsigned".to_string()
        };

        let score = serde_json::to_vec(&score)?;
        let db = self.0.get()?;
        db.execute(
            &format!(
                "INSERT INTO {} (address, round, block, score) VALUES (?1, ?2, ?3, ?4)",
                table.as_str(),
            ),
            params![addr, round, block, score],
        )?;

        Ok(())
    }

    pub async fn get_all_submissions(&self) -> Result<Vec<Submission>, anyhow::Error> {
        self.get_all(Table::Submissions).await
    }

    pub async fn get_all_election_winners(&self) -> Result<Vec<Submission>, anyhow::Error> {
        self.get_all(Table::ElectionWinners).await
    }

    pub async fn get_all_unsigned_winners(&self) -> Result<Vec<Submission>, anyhow::Error> {
        let db = self.0.get()?;
        let stmt = db.prepare("SELECT * FROM election_winners WHERE address = 'unsigned'")?;
        Self::stmt_to_submissions(stmt).await
    }

    pub async fn get_most_recent_unsigned_winners(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, anyhow::Error> {
        let db = self.0.get()?;
        let stmt = db.prepare(&format!("SELECT * FROM election_winners WHERE address = 'unsigned' ORDER BY round DESC LIMIT {n}"))?;
        Self::stmt_to_submissions(stmt).await
    }

    pub async fn get_most_recent_submissions(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, anyhow::Error> {
        self.get_most_recent(Table::Submissions, n).await
    }

    pub async fn get_most_recent_election_winners(
        &self,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, anyhow::Error> {
        self.get_most_recent(Table::ElectionWinners, n).await
    }

    async fn get_all(&self, table: Table) -> Result<Vec<Submission>, anyhow::Error> {
        let db = self.0.get()?;
        let stmt = db.prepare(&format!("SELECT * FROM {}", table.as_str()))?;
        Self::stmt_to_submissions(stmt).await
    }

    async fn get_most_recent(
        &self,
        table: Table,
        n: NonZeroUsize,
    ) -> Result<Vec<Submission>, anyhow::Error> {
        let db = self.0.get()?;
        let stmt = db.prepare(&format!(
            "SELECT * FROM {} ORDER BY round DESC LIMIT {n}",
            table.as_str(),
        ))?;
        Self::stmt_to_submissions(stmt).await
    }

    async fn stmt_to_submissions(
        mut stmt: rusqlite::Statement<'_>,
    ) -> Result<Vec<Submission>, anyhow::Error> {
        let rows = stmt.query_map([], |row| {
            let bytes: Vec<u8> = row.get(3)?;
            let s = serde_json::from_slice(&bytes).unwrap();

            Ok(Submission {
                address: row.get(0)?,
                round: row.get(1)?,
                block: row.get(2)?,
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
}
