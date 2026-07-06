use crate::{
    dtos::{HotRankQuery, TimeRange},
    error::Result,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    Json,
};

pub async fn get_hot_rank(
    State(state): State<AppState>,
    Path(time_range): Path<TimeRange>,
    Query(query): Query<HotRankQuery>,
) -> Result<Response> {
    state
        .topchart_service
        .get_hot_rank(state.clone(), time_range, query.page, query.per_page)
        .await
        .map(|r| Json(r).into_response())
}

pub async fn get_chart_rank(
    State(state): State<AppState>,
    Path(chart_id): Path<i32>,
) -> Result<Response> {
    state
        .topchart_service
        .get_chart_rank(state.clone(), chart_id)
        .await
        .map(|r| Json(r).into_response())
}
