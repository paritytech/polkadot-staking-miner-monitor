// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::db::Database;
use actix_web::{http::StatusCode, web, Error, HttpResponse, Result};
use std::num::NonZeroUsize;

type BoxError = Box<dyn std::error::Error>;

/// Returns the home page of the site.
pub async fn home() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(String::from(
            "query the /submissions, /winners or /unsigned-winners endpoints",
        )))
}

pub async fn all_submissions(db: web::Data<Database>) -> Result<HttpResponse, Error> {
    let submissions = db.get_all_submissions().await.map_err(BoxError::from)?;
    let body = serde_json::to_string(&submissions).map_err(BoxError::from)?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("application/json; charset=utf-8")
        .body(body))
}

pub async fn all_election_winners(db: web::Data<Database>) -> Result<HttpResponse, Error> {
    let submissions = db
        .get_all_election_winners()
        .await
        .map_err(BoxError::from)?;
    let body = serde_json::to_string(&submissions).map_err(BoxError::from)?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("application/json; charset=utf-8")
        .body(body))
}

pub async fn all_unsigned_winners(db: web::Data<Database>) -> Result<HttpResponse, Error> {
    let submissions = db
        .get_all_unsigned_winners()
        .await
        .map_err(BoxError::from)?;
    let body = serde_json::to_string(&submissions).map_err(BoxError::from)?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("application/json; charset=utf-8")
        .body(body))
}

pub async fn all_slashed(db: web::Data<Database>) -> Result<HttpResponse, Error> {
    let slashed = db.get_all_slashed().await.map_err(BoxError::from)?;

    let body = serde_json::to_string(&slashed).map_err(BoxError::from)?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("application/json; charset=utf-8")
        .body(body))
}

pub async fn most_recent_submissions(
    db: web::Data<Database>,
    info: web::Path<NonZeroUsize>,
) -> Result<HttpResponse, Error> {
    let submissions = db
        .get_most_recent_submissions(info.into_inner())
        .await
        .map_err(BoxError::from)?;
    let body = serde_json::to_string(&submissions).map_err(BoxError::from)?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("application/json; charset=utf-8")
        .body(body))
}

pub async fn most_recent_election_winners(
    db: web::Data<Database>,
    info: web::Path<NonZeroUsize>,
) -> Result<HttpResponse, Error> {
    let submissions = db
        .get_most_recent_election_winners(info.into_inner())
        .await
        .map_err(BoxError::from)?;
    let body = serde_json::to_string(&submissions).map_err(BoxError::from)?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("application/json; charset=utf-8")
        .body(body))
}

pub async fn most_recent_unsigned_winners(
    db: web::Data<Database>,
    info: web::Path<NonZeroUsize>,
) -> Result<HttpResponse, Error> {
    let submissions = db
        .get_most_recent_unsigned_winners(info.into_inner())
        .await
        .map_err(BoxError::from)?;
    let body = serde_json::to_string(&submissions).map_err(BoxError::from)?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("application/json; charset=utf-8")
        .body(body))
}

pub async fn most_recent_slashed(
    db: web::Data<Database>,
    info: web::Path<NonZeroUsize>,
) -> Result<HttpResponse, Error> {
    let slashed = db
        .get_most_recent_slashed(info.into_inner())
        .await
        .map_err(BoxError::from)?;
    let body = serde_json::to_string(&slashed).map_err(BoxError::from)?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("application/json; charset=utf-8")
        .body(body))
}
