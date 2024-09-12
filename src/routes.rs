// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::db::{Database, Election, Slashed, Submission};
use actix_web::{web, web::Json, Error, Result};
use oasgen::{oasgen, OaSchema};
use std::num::NonZeroUsize;

type BoxError = Box<dyn std::error::Error>;

#[oasgen]
pub async fn all_submissions(db: web::Data<Database>) -> Result<Json<Vec<Submission>>, Error> {
    let submissions = db.get_all_submissions().await.map_err(BoxError::from)?;
    Ok(Json(submissions))
}

#[oasgen]
pub async fn all_elections(db: web::Data<Database>) -> Result<Json<Vec<Election>>, Error> {
    let winners = db.get_all_elections().await.map_err(BoxError::from)?;
    Ok(Json(winners))
}

#[oasgen]
pub async fn all_slashed(db: web::Data<Database>) -> Result<Json<Vec<Slashed>>, Error> {
    let slashed = db.get_all_slashed().await.map_err(BoxError::from)?;
    Ok(Json(slashed))
}

#[oasgen]
pub async fn most_recent_submissions(
    db: web::Data<Database>,
    info: web::Path<usize>,
) -> Result<Json<Vec<Submission>>, Error> {
    let n = into_non_zero_usize(info.into_inner())?;
    let submissions = db
        .get_most_recent_submissions(n)
        .await
        .map_err(BoxError::from)?;
    Ok(Json(submissions))
}

#[oasgen]
pub async fn most_recent_elections(
    db: web::Data<Database>,
    info: web::Path<usize>,
) -> Result<Json<Vec<Election>>, Error> {
    let n = into_non_zero_usize(info.into_inner())?;
    let winners = db
        .get_most_recent_elections(n)
        .await
        .map_err(BoxError::from)?;
    Ok(Json(winners))
}

#[oasgen]
pub async fn most_recent_slashed(
    db: web::Data<Database>,
    info: web::Path<usize>,
) -> Result<Json<Vec<Slashed>>, Error> {
    let n = into_non_zero_usize(info.into_inner())?;
    let slashed = db
        .get_most_recent_slashed(n)
        .await
        .map_err(BoxError::from)?;
    Ok(Json(slashed))
}

// Convert a usize into a NonZeroUsize, returning an error if the value is zero.
//
// oasgen doesn't support NonZero types yet, so we have to do this manually.
fn into_non_zero_usize(value: usize) -> Result<NonZeroUsize, Error> {
    NonZeroUsize::new(value).ok_or_else(|| {
        actix_web::error::ErrorBadRequest("/path/n must be positive integer".to_string())
    })
}
