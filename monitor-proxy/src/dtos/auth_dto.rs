use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub id: i32,
    pub username: String,
    pub phira_avatar: Option<String>,
    pub phira_id: i32,
    pub phira_rks: f32,
    pub phira_username: String,
    pub register_time: DateTime<Utc>,
    pub last_login_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhiraLoginResponse {
    pub id: i32,
    pub token: String,
    pub refresh_token: String,
    pub expire_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct PhiraProfileResponse {
    pub id: i32,
    pub name: String,
    pub avatar: Option<String>,
    pub badges: Vec<String>,
    pub language: String,
    pub bio: Option<String>,
    pub exp: f32,
    pub rks: f32,
    pub joined: DateTime<Utc>,
    pub last_login: DateTime<Utc>,
    pub roles: u32,
    pub banned: bool,
    pub login_banned: bool,
    pub follower_count: u32,
    pub following_count: u32,
}
