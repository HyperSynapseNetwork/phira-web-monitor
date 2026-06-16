use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeRange {
    Hour,
    Day,
    Week,
    Month,
}

#[derive(Deserialize)]
pub struct ChartMetaResponse {
    pub id: i32,
    // ignoring other fields...
}

#[derive(Deserialize)]
pub struct ChartListResponse {
    pub count: i32,
    pub results: Vec<ChartMetaResponse>,
}

#[derive(Deserialize)]
pub struct ChartRecordResponse {
    pub count: i32,
    // ignoring other fields...
}

#[derive(Serialize)]
pub struct HotRankIncreaseInfo {
    pub chart_id: i32,
    pub increase: i32,
}

#[derive(Serialize)]
pub struct HotRankResponse {
    pub last_chart_list_update: DateTime<Utc>,
    pub last_record_update: DateTime<Utc>,
    pub page: u32,
    pub per_page: u32,
    pub time_range: TimeRange,
    pub total_results: u32,
    pub results: Vec<HotRankIncreaseInfo>,
}

#[derive(Deserialize)]
pub struct HotRankQuery {
    pub page: u32,
    pub per_page: u32,
}

#[derive(Serialize)]
pub struct ChartRankSubInfo {
    pub increase: i32,
    pub rank: u32,
    pub last_update: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct ChartRankInfo {
    pub hour: ChartRankSubInfo,
    pub day: ChartRankSubInfo,
    pub week: ChartRankSubInfo,
    pub month: ChartRankSubInfo,
}

#[derive(Serialize)]
pub struct ChartRankResponse {
    pub chart_id: i32,
    pub ranks: ChartRankInfo,
}
