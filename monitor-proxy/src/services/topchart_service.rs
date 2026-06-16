use crate::{
    dtos::{
        ChartListResponse, ChartRankInfo, ChartRankResponse, ChartRankSubInfo, ChartRecordResponse,
        HotRankIncreaseInfo, HotRankResponse, TimeRange,
    },
    entity::{chart_record, chart_statistic},
    error::{AppErrorExt, Result},
    utils::{batch_insert, DEFAULT_BATCH_SIZE},
    AppState,
};
use anyhow::Context;
use chrono::{DateTime, Utc};
use futures::{stream, StreamExt, TryFutureExt, TryStreamExt};
use log::{error, info, warn};
use sea_orm::{
    sea_query::OnConflict, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use std::{collections::HashMap, time::Duration};
use tokio::{
    sync::{OnceCell, RwLock},
    task::JoinHandle,
    time::{self, Instant},
};

const MAX_PARALLEL_REQUESTS: usize = 4;
const REQUEST_RETRY_COUNT: usize = 3;
const QUERY_BUFFER: i64 = 7_200_000; // 2 hours in milliseconds

macro_rules! with_retry {
    (@inner $body:block) => {{
        let mut attemps = 0;
        let mut timeout = 1;
        loop {
            let f = async || -> anyhow::Result<_> { $body };
            match f().await {
                Ok(r) => break Ok(r),
                Err(e) if attemps < REQUEST_RETRY_COUNT => {
                    warn!("request failed, retrying in {} seconds: {:#?}", timeout, e);
                    time::sleep(time::Duration::from_secs(timeout)).await;
                    attemps += 1;
                    timeout *= 2;
                }
                Err(e) => break Err(e),
            }
        }
    }};
    (|$($arg:ident $(: $ty:ty)?),*| $body:block) => {{
        async |$($arg $(: $ty)?),*| {
            with_retry!(@inner $body)
        }
    }};
    ($($body:tt)*) => {{
        with_retry!(@inner { $($body)* })
    }};
}

pub struct TopChartService {
    update_task: OnceCell<JoinHandle<()>>,
    last_chart_list_update: RwLock<DateTime<Utc>>,
    last_record_update: RwLock<DateTime<Utc>>,
}

impl TopChartService {
    pub fn new() -> Self {
        TopChartService {
            update_task: OnceCell::new(),
            last_chart_list_update: RwLock::new(Utc::now()),
            last_record_update: RwLock::new(Utc::now()),
        }
    }

    /// update chart list and statistics
    pub async fn update_all(state: AppState) -> anyhow::Result<()> {
        info!("updating chart statistics...");

        // get total chart count
        let count = state
            .http_client
            .get(format!("{}/chart", state.config.api_base))
            .query(&[("pageNum", 0), ("page", 1)])
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .context("failed to get total number of charts")?
            .json::<ChartListResponse>()
            .await
            .context("failed to parse response json")?
            .count;
        info!("total chart count: {}", count);

        // get all chart ids
        let chart_ids = stream::iter(1..=(count + 29) / 30)
            .map(with_retry!(|i| {
                let r = state
                    .http_client
                    .get(format!("{}/chart", state.config.api_base))
                    .query(&[("pageNum", 30), ("page", i)])
                    .send()
                    .and_then(async |r| r.error_for_status())
                    .and_then(|r| r.json::<ChartListResponse>())
                    .await
                    .with_context(|| format!("failed to get chart list of page {}", i))?;
                Ok(r.results.iter().map(|r| r.id).collect::<Vec<_>>())
            }))
            .buffer_unordered(MAX_PARALLEL_REQUESTS)
            .try_concat()
            .await?;
        *state.topchart_service.last_chart_list_update.write().await = Utc::now();

        // update counts
        info!("updating chart records...");

        let chart_records: Vec<_> = stream::iter(chart_ids)
            .map(with_retry!(|id| {
                let r = state
                    .http_client
                    .get(format!("{}/record/query/{}", state.config.api_base, id))
                    .query(&[("pageNum", 0)])
                    .send()
                    .await
                    .with_context(|| format!("failed to fetch record for {}", id))?;
                if r.status() == reqwest::StatusCode::NOT_FOUND {
                    info!("chart {} not found, will be deleted", id);
                    return Ok(None);
                }
                let json = r
                    .error_for_status()
                    .with_context(|| format!("failed to query record for {}", id))?
                    .json::<ChartRecordResponse>()
                    .await
                    .with_context(|| format!("failed to parse json for {}", id))?;
                Ok(Some((id, json.count)))
            }))
            .buffer_unordered(MAX_PARALLEL_REQUESTS)
            .try_filter_map(|res| async move { Ok(res) })
            .try_collect()
            .await?;

        let now = Utc::now();
        let cur_timestamp = now.timestamp_millis();
        *state.topchart_service.last_record_update.write().await = now;

        // insert into db
        info!("chart records fetched, updating database...");
        batch_insert(
            &state.db,
            chart_records
                .iter()
                .copied()
                .map(|(id, count)| chart_record::ActiveModel {
                    chart_id: Set(id),
                    timestamp: Set(cur_timestamp),
                    count: Set(count),
                }),
            DEFAULT_BATCH_SIZE,
            |q| q,
        )
        .await
        .context("failed to update database")?;

        info!("cleaning up stale charts & expired records from database...");
        let expire_time = cur_timestamp - 3_024_000_000; // 35 days
        let subquery = sea_orm::sea_query::Query::select()
            .column(chart_record::Column::ChartId)
            .from(chart_record::Entity)
            .and_where(chart_record::Column::Timestamp.eq(cur_timestamp))
            .to_owned();
        tokio::try_join!(
            chart_statistic::Entity::delete_many()
                .filter(chart_statistic::Column::ChartId.not_in_subquery(subquery.clone()))
                .exec(&state.db),
            chart_record::Entity::delete_many()
                .filter(chart_record::Column::ChartId.not_in_subquery(subquery))
                .exec(&state.db),
            chart_record::Entity::delete_many()
                .filter(chart_record::Column::Timestamp.lt(expire_time))
                .exec(&state.db)
        )
        .context("failed to execute database cleanup tasks")?;

        let t_hour = cur_timestamp - 3_600_000 + 600_000;
        let t_day = cur_timestamp - 86_400_000 + 600_000;
        let t_week = cur_timestamp - 604_800_000 + 600_000;
        let t_month = cur_timestamp - 2_592_000_000 + 600_000;

        info!("querying baselines...");
        let (baselines_h, baselines_d, baselines_w, baselines_m) = tokio::try_join!(
            Self::query_baselines(&state.db, t_hour),
            Self::query_baselines(&state.db, t_day),
            Self::query_baselines(&state.db, t_week),
            Self::query_baselines(&state.db, t_month),
        )?;

        // update statistics
        info!("updating statistics...");
        batch_insert(
            &state.db,
            chart_records
                .iter()
                .copied()
                .map(|(id, current)| chart_statistic::ActiveModel {
                    chart_id: Set(id),
                    count_hour: Set(current - baselines_h.get(&id).unwrap_or(&current)),
                    count_day: Set(current - baselines_d.get(&id).unwrap_or(&current)),
                    count_week: Set(current - baselines_w.get(&id).unwrap_or(&current)),
                    count_month: Set(current - baselines_m.get(&id).unwrap_or(&current)),
                }),
            DEFAULT_BATCH_SIZE,
            |q| {
                q.on_conflict(
                    OnConflict::column(chart_statistic::Column::ChartId)
                        .update_columns([
                            chart_statistic::Column::CountHour,
                            chart_statistic::Column::CountDay,
                            chart_statistic::Column::CountWeek,
                            chart_statistic::Column::CountMonth,
                        ])
                        .to_owned(),
                )
            },
        )
        .await
        .context("failed to update statistics")?;
        info!("chart statistics updated");

        Ok(())
    }

    /// Query the baseline counts for each chart at a given cutoff time.
    /// Looks for records in [cutoff - QUERY_BUFFER, cutoff] and takes MAX(count)
    /// per chart (equivalent to the latest record's count due to monotonicity).
    async fn query_baselines(
        db: &DatabaseConnection,
        cutoff: i64,
    ) -> anyhow::Result<HashMap<i32, i32>> {
        Ok(chart_record::Entity::find()
            .select_only()
            .column(chart_record::Column::ChartId)
            .column_as(chart_record::Column::Count.max(), "max_count")
            .filter(chart_record::Column::Timestamp.gte(cutoff - QUERY_BUFFER))
            .filter(chart_record::Column::Timestamp.lte(cutoff))
            .group_by(chart_record::Column::ChartId)
            .into_tuple()
            .all(db)
            .await
            .context("failed to query baselines")?
            .into_iter()
            .filter_map(|(id, count): (i32, Option<i32>)| Some((id, count?)))
            .collect())
    }

    pub fn launch_update_task(&self, state: AppState) -> anyhow::Result<()> {
        self.update_task
            .set(tokio::spawn(async move {
                loop {
                    let mut next_update = Instant::now() + Duration::from_hours(1);
                    if let Err(e) = Self::update_all(state.clone()).await {
                        error!("failed to update chart rank info: {e:#?}, will retry after 5min");
                        next_update = Instant::now() + Duration::from_mins(5);
                    }
                    time::sleep_until(next_update).await;
                }
            }))
            .context("failed to launch periodically update task")
    }

    pub async fn get_chart_rank(
        &self,
        state: AppState,
        chart_id: i32,
    ) -> Result<ChartRankResponse> {
        let query_info = async |column: chart_statistic::Column| -> Result<_> {
            let cnt = chart_statistic::Entity::find_by_id(chart_id)
                .column(column)
                .into_tuple::<i32>()
                .one(&state.db)
                .await
                .internal_server_error("failed to query database")?
                .not_found("chart info not recorded")?;
            let rank = chart_statistic::Entity::find()
                .filter(column.gt(cnt))
                .count(&state.db)
                .await
                .map(|x| x as u32 + 1)
                .internal_server_error("failed to query database")?;
            Ok(ChartRankSubInfo {
                increase: cnt,
                rank,
                last_update: *self.last_record_update.read().await,
            })
        };
        Ok(ChartRankResponse {
            chart_id,
            ranks: ChartRankInfo {
                hour: query_info(chart_statistic::Column::CountHour).await?,
                day: query_info(chart_statistic::Column::CountDay).await?,
                week: query_info(chart_statistic::Column::CountWeek).await?,
                month: query_info(chart_statistic::Column::CountMonth).await?,
            },
        })
    }

    pub async fn get_hot_rank(
        &self,
        state: AppState,
        range: TimeRange,
        page: u32,
        per_page: u32,
    ) -> Result<HotRankResponse> {
        let column = match &range {
            TimeRange::Hour => chart_statistic::Column::CountHour,
            TimeRange::Day => chart_statistic::Column::CountDay,
            TimeRange::Week => chart_statistic::Column::CountWeek,
            TimeRange::Month => chart_statistic::Column::CountMonth,
        };

        let paginator = chart_statistic::Entity::find()
            .select_only()
            .column(chart_statistic::Column::ChartId)
            .column(column)
            .order_by_desc(column)
            .into_tuple::<(i32, i32)>()
            .paginate(&state.db, per_page as u64);

        let total = paginator
            .num_items()
            .await
            .internal_server_error("failed to query total items")? as u32;

        let results = paginator
            .fetch_page(page.saturating_sub(1) as u64)
            .await
            .internal_server_error("failed to fetch page")?
            .into_iter()
            .map(|(chart_id, increase)| HotRankIncreaseInfo { chart_id, increase })
            .collect();

        Ok(HotRankResponse {
            last_chart_list_update: *self.last_chart_list_update.read().await,
            last_record_update: *self.last_record_update.read().await,
            page,
            per_page,
            time_range: range,
            total_results: total,
            results,
        })
    }
}

impl Default for TopChartService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TopChartService {
    fn drop(&mut self) {
        if let Some(task) = self.update_task.get() {
            task.abort();
        }
    }
}
