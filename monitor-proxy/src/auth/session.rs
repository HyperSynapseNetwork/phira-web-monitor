use axum_extra::extract::cookie::{self, Cookie};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

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
    pub expire_at: DateTime<Utc>,
}

pub fn build_session_cookie(data: &PhiraLoginResponse, user_id: i32) -> Cookie<'static> {
    let session = AuthSession {
        id: user_id,
        token: data.token.clone(),
        refresh_token: data.refresh_token.clone(),
        expire_at: data.expire_at.clone(),
    };
    let value = serde_json::to_string(&session).unwrap();
    let expiration = {
        let nanos = data.expire_at.timestamp_nanos_opt().unwrap_or(0) as i128;
        OffsetDateTime::from_unix_timestamp_nanos(nanos).expect("timestamp out of range")
    };
    Cookie::build(("hsn_auth", value))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(cookie::SameSite::Lax)
        .expires(expiration)
        .build()
}
