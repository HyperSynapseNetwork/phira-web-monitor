use axum::{
    extract::{FromRequestParts, Query},
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    response::Response,
};
use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, Validation};
use serde::{Deserialize, Serialize};

use crate::{json_err, AppState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhiraLoginResponse {
    pub id: Option<i32>,
    pub token: String,
    pub refresh_token: String,
    pub expire_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub id: i32,
    pub token: String,
    pub refresh_token: String,
    pub exp: usize,
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

impl FromRequestParts<AppState> for AuthSession {
    type Rejection = (StatusCode, Response);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Try Authorization header first, then fall back to ?token= query parameter
        // (needed for WebSocket upgrade requests where browsers can't set custom headers)
        let token_string;
        let token = if let Some(auth_header) = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
        {
            if auth_header.starts_with("Bearer ") {
                &auth_header[7..]
            } else {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    json_err!("Invalid Authorization header"),
                ));
            }
        } else {
            // Fallback: extract token from query parameter
            token_string = Query::<TokenQuery>::from_request_parts(parts, state)
                .await
                .ok()
                .and_then(|q| q.0.token)
                .ok_or_else(|| {
                    (
                        StatusCode::UNAUTHORIZED,
                        json_err!("Missing Authorization header or token query parameter"),
                    )
                })?;
            &token_string
        };
        // use default validation because we don't know Phira's refresh method
        Ok(decode(token, &state.decoding_key, &Validation::default())
            .map_err(|_| (StatusCode::UNAUTHORIZED, json_err!("Invalid bearer token")))?
            .claims)
    }
}
