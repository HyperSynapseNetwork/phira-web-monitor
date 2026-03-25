use crate::{
    error::{AppErrorExt, Result},
    AppState,
};
use axum::{
    body::Body,
    extract::{Path, State},
    response::Response,
};
use log::info;
use reqwest::header;

pub async fn get_chart(State(state): State<AppState>, Path(id): Path<String>) -> Result<Response> {
    info!("Processing chart request for ID: {id}");
    let stream = state
        .chart_service
        .handle_chart_request(&state, &id)
        .await?;
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from_stream(stream))
        .internal_server_error("failed to build response")
}
