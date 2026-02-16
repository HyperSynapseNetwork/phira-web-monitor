use crate::{auth::session::PhiraLoginResponse, json_err, json_msg, AppState};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use axum_extra::extract::PrivateCookieJar;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

mod session;
use session::AuthSession;

pub async fn auth_middleware(
    State(_state): State<AppState>,
    jar: PrivateCookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let cookie = jar.get("hsn_auth").ok_or(StatusCode::UNAUTHORIZED)?;
    let session: AuthSession =
        serde_json::from_str(cookie.value()).map_err(|_| StatusCode::UNAUTHORIZED)?;

    if session.expire_at < Utc::now() + Duration::seconds(60) {
        // should refresh, but we don't know Phira's refresh api
        // suspect that they didn't implement this /oh
        return Err(StatusCode::UNAUTHORIZED);
    }
    req.extensions_mut().insert(session);
    Ok((jar, next.run(req).await).into_response())
}

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(payload): Json<LoginRequest>,
) -> (StatusCode, Response) {
    let resp = match state
        .http_client
        .post(format!("{}/login", state.args.api_base))
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_err!("failed to send request: {e}"),
            );
        }
    };
    drop(payload);
    if !resp.status().is_success() {
        return (StatusCode::UNAUTHORIZED, json_err!("invalid credentials"));
    }

    let resp = match resp.json::<PhiraLoginResponse>().await {
        Ok(resp) => resp,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_err!("failed to parse response: {e}"),
            );
        }
    };
    let Some(user_id) = resp.id else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_err!("failed to parse response"),
        );
    };
    let cookie = session::build_session_cookie(&resp, user_id);
    (
        StatusCode::OK,
        (jar.add(cookie), json_msg!("login success")).into_response(),
    )
}

#[derive(Deserialize)]
pub struct PhiraProfileResponse {
    pub id: i32,
    pub name: String,
    pub avatar: Option<String>,
    // pub badges: Vec<String>,
    // pub language: String,
    // pub bio: Option<String>,
    // pub exp: f32,
    pub rks: f32,
    pub joined: DateTime<Utc>,
    pub last_login: DateTime<Utc>,
    // pub roles: u32,
    // pub banned: bool,
    // pub login_banned: bool,
    // pub follower_count: u32,
    // pub following_count: u32,
}

pub async fn get_me_profile(
    State(state): State<AppState>,
    Extension(session): Extension<session::AuthSession>,
) -> (StatusCode, Response) {
    let token = session.token;
    let resp = match state
        .http_client
        .get(format!("{}/me", state.args.api_base))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => resp,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_err!("failed to send request: {e}"),
            );
        }
    };
    let info = match resp.json::<PhiraProfileResponse>().await {
        Ok(info) => info,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_err!("failed to parse result: {e}"),
            );
        }
    };
    (
        StatusCode::OK,
        Json(json!({
            "id": info.id,
            "last_login_time": info.last_login,
            "phira_avatar": info.avatar,
            "phira_id": info.id,
            "phira_rks": info.rks,
            "phira_username": info.name,
            "register_time": info.joined,
            "username": info.name,
        }))
        .into_response(),
    )
}
