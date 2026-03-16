use crate::{
    dtos::{LoginRequest, PhiraLoginResponse, PhiraProfileResponse, ProfileResponse},
    error::{AppErrorExt, Result},
    middlewares::auth_middleware::AuthSession,
    AppState,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use phira_mp_common::generate_secret_key;
use reqwest::header;

pub struct AuthService {
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
}

impl AuthService {
    pub fn new() -> Self {
        let secret = generate_secret_key("jwt", 64).expect("failed to generate key for cookie");
        AuthService {
            encoding_key: EncodingKey::from_secret(&secret),
            decoding_key: DecodingKey::from_secret(&secret),
        }
    }

    pub fn encode(&self, session: &AuthSession) -> Result<String> {
        encode(&Header::default(), &session, &self.encoding_key)
            .internal_server_error("failed to encode session info")
    }

    pub fn decode(&self, token: &str) -> Result<AuthSession> {
        // use default validation because we don't know Phira's refresh method
        Ok(decode(token, &self.decoding_key, &Validation::default())
            .unauthorized("failed to decode session token")?
            .claims)
    }

    pub async fn authenticate(
        &self,
        state: &AppState,
        request: LoginRequest,
    ) -> Result<AuthSession> {
        let resp = state
            .http_client
            .post(format!("{}/login", state.config.api_base))
            .json(&request)
            .send()
            .await
            .internal_server_error("failed to send login request")
            .and_then(|r| r.error_for_status().unauthorized("invalid credential"))?
            .json::<PhiraLoginResponse>()
            .await
            .internal_server_error("failed to parse response")?;
        drop(request);

        Ok(AuthSession {
            id: resp.id,
            token: resp.token,
            refresh_token: resp.refresh_token,
            exp: resp.expire_at.timestamp() as usize,
        })
    }

    pub async fn get_profile(
        &self,
        state: &AppState,
        session: &AuthSession,
    ) -> Result<ProfileResponse> {
        let resp = state
            .http_client
            .get(format!("{}/me", state.config.api_base))
            .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .internal_server_error("failed to send request")?
            .json::<PhiraProfileResponse>()
            .await
            .internal_server_error("failed to parse result")?;
        Ok(ProfileResponse {
            id: resp.id,
            username: resp.name.clone(),
            phira_avatar: resp.avatar,
            phira_id: resp.id,
            phira_rks: resp.rks,
            phira_username: resp.name,
            register_time: resp.joined,
            last_login_time: resp.last_login,
        })
    }
}
