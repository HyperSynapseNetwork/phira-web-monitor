use crate::{
    dtos::auth_dto::{LoginRequest, LoginResponse},
    error::Result,
    middlewares::auth_middleware::AuthSession,
    AppState,
};
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Response> {
    let session = state.auth_service.authenticate(&state, payload).await?;
    let token = state.auth_service.encode(&session)?;
    Ok(Json(LoginResponse { token }).into_response())
}

pub async fn get_me_profile(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Response> {
    state
        .auth_service
        .get_profile(&state, &session)
        .await
        .map(|s| Json(s).into_response())
}
