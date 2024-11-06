// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::{
    db::{Election, Slashed, Submission},
    DbAndPrometheus,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use oasgen::oasgen;
use std::num::NonZeroUsize;

type HttpError = (StatusCode, String);

#[oasgen]
pub async fn all_submissions(
    State(state): State<DbAndPrometheus>,
) -> Result<Json<Vec<Submission>>, HttpError> {
    let submissions = state
        .db
        .get_all_submissions()
        .await
        .map_err(internal_error)?;
    Ok(Json(submissions))
}

#[oasgen]
pub async fn all_success_submissions(
    State(state): State<DbAndPrometheus>,
) -> Result<Json<Vec<Submission>>, HttpError> {
    let submissions = state
        .db
        .get_all_success_submissions()
        .await
        .map_err(internal_error)?;
    Ok(Json(submissions))
}

#[oasgen]
pub async fn all_failed_submissions(
    State(state): State<DbAndPrometheus>,
) -> Result<Json<Vec<Submission>>, HttpError> {
    let submissions = state
        .db
        .get_all_failed_submissions()
        .await
        .map_err(internal_error)?;
    Ok(Json(submissions))
}

#[oasgen]
pub async fn all_unsigned_elections(
    State(state): State<DbAndPrometheus>,
) -> Result<Json<Vec<Election>>, HttpError> {
    let elections = state
        .db
        .get_all_unsigned_elections()
        .await
        .map_err(internal_error)?;
    Ok(Json(elections))
}

#[oasgen]
pub async fn all_elections(
    State(state): State<DbAndPrometheus>,
) -> Result<Json<Vec<Election>>, HttpError> {
    let winners = state.db.get_all_elections().await.map_err(internal_error)?;
    Ok(Json(winners))
}

#[oasgen]
pub async fn all_failed_elections(
    State(state): State<DbAndPrometheus>,
) -> Result<Json<Vec<Election>>, HttpError> {
    let elections = state
        .db
        .get_all_failed_elections()
        .await
        .map_err(internal_error)?;
    Ok(Json(elections))
}

#[oasgen]
pub async fn all_signed_elections(
    State(state): State<DbAndPrometheus>,
) -> Result<Json<Vec<Election>>, HttpError> {
    let elections = state
        .db
        .get_all_signed_elections()
        .await
        .map_err(internal_error)?;
    Ok(Json(elections))
}

#[oasgen]
pub async fn all_slashed(
    State(state): State<DbAndPrometheus>,
) -> Result<Json<Vec<Slashed>>, HttpError> {
    let slashed = state.db.get_all_slashed().await.map_err(internal_error)?;
    Ok(Json(slashed))
}

#[oasgen]
pub async fn most_recent_submissions(
    State(state): State<DbAndPrometheus>,
    Path(n): Path<usize>,
) -> Result<Json<Vec<Submission>>, HttpError> {
    let n = into_non_zero_usize(n)?;
    let submissions = state
        .db
        .get_most_recent_submissions(n)
        .await
        .map_err(internal_error)?;
    Ok(Json(submissions))
}

#[oasgen]
pub async fn most_recent_elections(
    State(state): State<DbAndPrometheus>,
    Path(n): Path<usize>,
) -> Result<Json<Vec<Election>>, HttpError> {
    let n = into_non_zero_usize(n)?;
    let winners = state
        .db
        .get_most_recent_elections(n)
        .await
        .map_err(internal_error)?;
    Ok(Json(winners))
}

#[oasgen]
pub async fn most_recent_slashed(
    State(state): State<DbAndPrometheus>,
    Path(n): Path<usize>,
) -> Result<Json<Vec<Slashed>>, HttpError> {
    let n = into_non_zero_usize(n)?;
    let slashed = state
        .db
        .get_most_recent_slashed(n)
        .await
        .map_err(internal_error)?;
    Ok(Json(slashed))
}

#[oasgen]
pub async fn metrics(State(state): State<DbAndPrometheus>) -> String {
    state.prometheus.render()
}

// Convert a usize into a NonZeroUsize, returning an error if the value is zero.
//
// oasgen doesn't support NonZero types yet, so we have to do this manually.
fn into_non_zero_usize(value: usize) -> Result<NonZeroUsize, HttpError> {
    NonZeroUsize::new(value).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "path param value must be non-zero".to_string(),
        )
    })
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> HttpError
where
    E: std::fmt::Display,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
