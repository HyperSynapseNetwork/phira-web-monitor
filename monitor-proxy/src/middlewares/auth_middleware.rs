use crate::{
    error::{AppErrorExt, Result},
    AppState,
};
use anyhow::anyhow;
use axum::{
    extract::{Query, Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub id: i32,
    pub token: String,
    pub refresh_token: String,
    pub exp: usize,
}

#[derive(Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

pub async fn require_auth(
    State(state): State<AppState>,
    params: Query<TokenQuery>,
    mut req: Request,
    next: Next,
) -> Result<Response> {
    let token = if let Some(auth_header) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if auth_header.starts_with("Bearer ") {
            &auth_header[7..]
        } else {
            return Err(anyhow!("invalid Authorization header"))
                .unauthorized("failed to authorize");
        }
    } else {
        &params
            .token
            .as_ref()
            .ok_or_else(|| anyhow!("missing Authorization header"))
            .unauthorized("missing Authorization header")?
    };
    let session = state.auth_service.decode(&token)?;
    req.extensions_mut().insert(session);

    Ok(next.run(req).await)
}
