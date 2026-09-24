//! Read-only Neon monitoring. Polls are independent of visitor requests.
use crate::app::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

pub static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
pub static CACHE_FILLS: AtomicU64 = AtomicU64::new(0);
pub static CACHE_ERRORS: AtomicU64 = AtomicU64::new(0);
pub static CACHE_FILL_BYTES: AtomicU64 = AtomicU64::new(0);
pub static DB_ROWS: AtomicU64 = AtomicU64::new(0);
pub static DB_QUERIES: AtomicU64 = AtomicU64::new(0);
pub static BUDGET_REJECTIONS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub configured: bool,
    pub provider_transfer_bytes: Option<u64>,
    pub estimated_month_usd: Option<f64>,
    pub estimated_day_usd: Option<f64>,
    pub month_budget_usd: f64,
    pub day_budget_usd: f64,
    pub usage_through: Option<DateTime<Utc>>,
    pub day_basis: &'static str,
    pub last_success: Option<DateTime<Utc>>,
    pub last_attempt: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Notice {
    pub title: String,
    pub message: String,
    pub level: &'static str,
}
impl Snapshot {
    pub fn notice(&self, now: DateTime<Utc>) -> Option<Notice> {
        if !self.configured
            || self.last_error.is_some()
            || self
                .last_success
                .is_none_or(|t| (now - t).num_seconds() > 900)
        {
            return Some(Notice {title:"Usage monitoring unavailable".into(), message:"The latest service-usage check is unavailable. Browsing remains available while monitoring recovers.".into(), level:"warning"});
        }
        let (Some(month), Some(day)) = (self.estimated_month_usd, self.estimated_day_usd) else {
            return Some(Notice {
                title: "Usage monitoring unavailable".into(),
                message: "Neon cost estimates are not available yet.".into(),
                level: "warning",
            });
        };
        let exceeded = month >= self.month_budget_usd || day > self.day_budget_usd;
        if !exceeded && month < self.month_budget_usd * 0.8 {
            return None;
        }
        Some(Notice {
            title: if exceeded { "Neon spending alert" } else { "Neon budget warning" }.into(),
            message: format!("Estimated Neon usage: ${day:.2} today (UTC; alert above ${:.2}), ${month:.2} this month (budget ${:.2}). Metering can be delayed. Estimates exclude taxes, credits, and non-database services. Browsing remains available.", self.day_budget_usd, self.month_budget_usd),
            level: if exceeded { "error" } else { "warning" },
        })
    }
}
pub struct UsageMonitor {
    snapshot: RwLock<Snapshot>,
    api_key: Option<String>,
    org_id: Option<String>,
    metrics_token: Option<String>,
}
impl UsageMonitor {
    pub fn from_env() -> Arc<Self> {
        let api_key = std::env::var("NEON_API_KEY").ok().filter(|v| !v.is_empty());
        let org_id = std::env::var("NEON_ORG_ID")
            .ok()
            .filter(|v| !v.is_empty() && v.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'));
        Arc::new(Self {
            snapshot: RwLock::new(Snapshot {
                configured: api_key.is_some() && org_id.is_some(),
                provider_transfer_bytes: None,
                estimated_month_usd: None,
                estimated_day_usd: None,
                month_budget_usd: 10.0,
                day_budget_usd: 0.50,
                usage_through: None,
                day_basis: "UTC calendar day",
                last_success: None,
                last_attempt: None,
                last_error: None,
            }),
            api_key,
            org_id,
            metrics_token: std::env::var("OPERATIONS_METRICS_TOKEN")
                .ok()
                .filter(|v| v.len() >= 32),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn notice(&self) -> Option<Notice> {
        self.snapshot().notice(Utc::now())
    }
    pub fn spawn(self: Arc<Self>) {
        tokio::spawn(async move {
            let client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            {
                Ok(c) => c,
                Err(_) => return,
            };
            let mut timer = tokio::time::interval(Duration::from_secs(300));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                timer.tick().await;
                self.refresh(&client).await;
            }
        });
    }
    async fn refresh(&self, client: &reqwest::Client) {
        let now = Utc::now();
        let result = match (&self.api_key, &self.org_id) {
            (Some(key), Some(org)) => {
                async {
                    let start = Utc
                        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                        .single()
                        .ok_or("Invalid calendar month")?;
                    // The API rounds daily bounds down; tomorrow includes today's partial bucket.
                    let end = now
                        .date_naive()
                        .succ_opt()
                        .ok_or("Invalid calendar day")?
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc();
                    let response = client
                        .get("https://console.neon.tech/api/v2/consumption_history/v2/projects")
                        .query(&[
                            ("org_id", org.as_str()),
                            ("from", &start.to_rfc3339()),
                            ("to", &end.to_rfc3339()),
                            ("granularity", "daily"),
                            ("limit", "100"),
                            ("metrics", METRICS),
                        ])
                        .bearer_auth(key)
                        .send()
                        .await
                        .map_err(|_| "Neon API connection failed")?;
                    if !response.status().is_success() {
                        return Err("Neon API rejected usage request");
                    }
                    let data: ConsumptionResponse = response
                        .json()
                        .await
                        .map_err(|_| "Neon usage response invalid")?;
                    estimate(&data, now)
                }
                .await
            }
            _ => Err("Neon usage monitoring is not configured"),
        };
        let mut snapshot = self.snapshot.write().unwrap_or_else(|e| e.into_inner());
        snapshot.last_attempt = Some(now);
        match result {
            Ok(cost) => {
                snapshot.provider_transfer_bytes = Some(cost.transfer_bytes as u64);
                snapshot.estimated_month_usd = Some(cost.month);
                snapshot.estimated_day_usd = Some(cost.day);
                snapshot.usage_through = Some(cost.through);
                snapshot.last_success = Some(now);
                snapshot.last_error = None;
            }
            Err(message) => {
                snapshot.last_error = Some(message.into());
            }
        }
        tracing::info!(provider_transfer_bytes=?snapshot.provider_transfer_bytes,estimated_month_usd=?snapshot.estimated_month_usd,estimated_day_usd=?snapshot.estimated_day_usd,last_success=?snapshot.last_success,monitor_error=?snapshot.last_error,cache_hits=CACHE_HITS.load(Ordering::Relaxed),cache_fills=CACHE_FILLS.load(Ordering::Relaxed),cache_fill_bytes=CACHE_FILL_BYTES.load(Ordering::Relaxed),cache_errors=CACHE_ERRORS.load(Ordering::Relaxed),database_rows=DB_ROWS.load(Ordering::Relaxed),database_queries=DB_QUERIES.load(Ordering::Relaxed),read_budget_rejections=BUDGET_REJECTIONS.load(Ordering::Relaxed),"server usage statistics");
    }
    #[cfg(test)]
    pub fn set_cost_for_test(&self, month: f64, day: f64) {
        let mut snapshot = self.snapshot.write().unwrap();
        snapshot.estimated_month_usd = Some(month);
        snapshot.estimated_day_usd = Some(day);
    }
    #[cfg(test)]
    pub fn healthy_for_test() -> Arc<Self> {
        Arc::new(Self {
            snapshot: RwLock::new(Snapshot {
                configured: true,
                provider_transfer_bytes: Some(0),
                estimated_month_usd: Some(0.0),
                estimated_day_usd: Some(0.0),
                month_budget_usd: 10.0,
                day_budget_usd: 0.50,
                usage_through: Some(Utc::now()),
                day_basis: "UTC calendar day",
                last_success: Some(Utc::now()),
                last_attempt: Some(Utc::now()),
                last_error: None,
            }),
            api_key: None,
            org_id: None,
            metrics_token: Some("test-operations-token-at-least-32-characters".into()),
        })
    }
}
const METRICS: &str = "compute_unit_seconds,root_branch_bytes_month,child_branch_bytes_month,instant_restore_bytes_month,public_network_transfer_bytes,private_network_transfer_bytes,extra_branches_month,snapshot_storage_bytes_month";
#[derive(Deserialize)]
struct ConsumptionResponse {
    projects: Vec<ProjectUsage>,
    pagination: Option<Pagination>,
}
#[derive(Deserialize)]
struct Pagination {
    cursor: Option<String>,
}
#[derive(Deserialize)]
struct ProjectUsage {
    periods: Vec<Period>,
}
#[derive(Deserialize)]
struct Period {
    period_plan: String,
    period_start: DateTime<Utc>,
    consumption: Vec<Bucket>,
}
#[derive(Deserialize)]
struct Bucket {
    timeframe_start: DateTime<Utc>,
    timeframe_end: DateTime<Utc>,
    metrics: Vec<Metric>,
}
#[derive(Deserialize)]
struct Metric {
    metric_name: String,
    value: f64,
}
struct Estimate {
    month: f64,
    day: f64,
    transfer_bytes: f64,
    through: DateTime<Utc>,
}

// Official v2 conversion: storage is byte-months (already divided by 744), not byte-hours.
// https://neon.com/docs/introduction/usage-calculations
fn estimate(data: &ConsumptionResponse, now: DateTime<Utc>) -> Result<Estimate, &'static str> {
    if data.projects.is_empty()
        || data
            .pagination
            .as_ref()
            .and_then(|p| p.cursor.as_ref())
            .is_some_and(|s| !s.is_empty())
    {
        return Err("Neon organization usage is incomplete");
    }
    let mut result = Estimate {
        month: 0.,
        day: 0.,
        transfer_bytes: 0.,
        through: now,
    };
    let mut buckets = 0;
    let mut today_seen = false;
    for project in &data.projects {
        let mut network = 0.0_f64;
        let mut ordered = Vec::new();
        for period in &project.periods {
            let (compute_rate, children) = match period.period_plan.as_str() {
                "launch" => (0.106, 9.),
                "scale" => (0.222, 24.),
                _ => return Err("Neon plan requires verified pricing"),
            };
            for bucket in &period.consumption {
                ordered.push((bucket, period.period_start, compute_rate, children));
            }
        }
        ordered.sort_by_key(|(b, _, _, _)| b.timeframe_start);
        for (bucket, period_start, compute_rate, children) in ordered {
            if bucket.timeframe_start.year() != now.year()
                || bucket.timeframe_start.month() != now.month()
                || bucket.timeframe_start > now
                || bucket.timeframe_end <= bucket.timeframe_start
            {
                return Err("Neon returned an unexpected usage period");
            }
            buckets += 1;
            today_seen |= bucket.timeframe_start.date_naive() == now.date_naive();
            let hours = (bucket.timeframe_end.min(now) - bucket.timeframe_start.max(period_start))
                .num_seconds()
                .max(0) as f64
                / 3600.;
            let mut cost = 0.;
            let mut seen = std::collections::HashSet::new();
            for metric in &bucket.metrics {
                let v = metric.value;
                if !v.is_finite() || v < 0. || !seen.insert(&metric.metric_name) {
                    return Err("Neon returned invalid usage values");
                }
                cost += match metric.metric_name.as_str() {
                    "compute_unit_seconds" => v / 3600. * compute_rate,
                    "root_branch_bytes_month" | "child_branch_bytes_month" => v / 1e9 * 0.35,
                    "instant_restore_bytes_month" => v / 1e9 * 0.20,
                    "snapshot_storage_bytes_month" => v / 1e9 * 0.09,
                    "private_network_transfer_bytes" => v / 1e9 * 0.01,
                    "extra_branches_month" => (v - children * hours).max(0.) / 744. * 1.50,
                    "public_network_transfer_bytes" => {
                        let before = (network / 1e9 - 500.).max(0.);
                        network += v;
                        result.transfer_bytes += v;
                        ((network / 1e9 - 500.).max(0.) - before) * 0.10
                    }
                    _ => return Err("Neon returned an unsupported usage metric"),
                };
            }
            result.month += cost;
            if bucket.timeframe_start.date_naive() == now.date_naive() {
                result.day += cost;
            }
        }
    }
    // Zero-valued metrics may be omitted, but absent time buckets are not evidence of zero usage.
    if buckets == 0 || !today_seen {
        return Err("Neon usage has not arrived yet");
    }
    Ok(result)
}

pub async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(expected) = state.usage.metrics_token.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let supplied = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    // Constant-time MAC comparison, reusing the existing HMAC dependency.
    use hmac::{Hmac, Mac};
    let mut check = Hmac::<sha2::Sha256>::new_from_slice(expected.as_bytes())
        .expect("HMAC accepts any key size");
    check.update(supplied.as_bytes());
    let actual = check.finalize().into_bytes();
    let mut check = Hmac::<sha2::Sha256>::new_from_slice(expected.as_bytes())
        .expect("HMAC accepts any key size");
    check.update(expected.as_bytes());
    if check.verify_slice(&actual).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ([(axum::http::header::CACHE_CONTROL,"no-store")],Json(serde_json::json!({
        "neon":state.usage.snapshot(),"notice":state.usage.notice(),
        "process":{"cache_hits":CACHE_HITS.load(Ordering::Relaxed),"cache_fills":CACHE_FILLS.load(Ordering::Relaxed),"estimated_cache_fill_bytes":CACHE_FILL_BYTES.load(Ordering::Relaxed),"cache_errors":CACHE_ERRORS.load(Ordering::Relaxed),"database_rows":DB_ROWS.load(Ordering::Relaxed),"database_queries":DB_QUERIES.load(Ordering::Relaxed),"read_budget_rejections":BUDGET_REJECTIONS.load(Ordering::Relaxed)}
    }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn estimates_invoice_units_and_incremental_daily_transfer() {
        let now = Utc.with_ymd_and_hms(2026, 9, 24, 23, 0, 0).unwrap();
        let data: ConsumptionResponse = serde_json::from_value(serde_json::json!({"projects":[{"periods":[{
            "period_plan":"launch", "period_start":"2026-09-01T00:00:00Z", "consumption":[
                {"timeframe_start":"2026-09-23T00:00:00Z", "timeframe_end":"2026-09-24T00:00:00Z", "metrics":[{"metric_name":"public_network_transfer_bytes","value":499000000000_u64}]},
                {"timeframe_start":"2026-09-24T00:00:00Z", "timeframe_end":"2026-09-25T00:00:00Z", "metrics":[
                    {"metric_name":"public_network_transfer_bytes","value":2000000000_u64},
                    {"metric_name":"compute_unit_seconds","value":3600},
                    {"metric_name":"root_branch_bytes_month","value":1000000000},
                    {"metric_name":"snapshot_storage_bytes_month","value":1000000000},
                    {"metric_name":"instant_restore_bytes_month","value":1000000000}
                ]}
            ]
        }]}]})).unwrap();
        let cost = estimate(&data, now).unwrap();
        assert!((cost.month - 0.846).abs() < 1e-8);
        assert!((cost.day - 0.846).abs() < 1e-8);
        assert_eq!(cost.transfer_bytes, 501000000000.);
        assert!(estimate(&data, now + chrono::Duration::days(1)).is_err());
    }
    #[test]
    fn incomplete_or_unknown_provider_data_is_not_zero_cost() {
        let now = Utc::now();
        for value in [
            serde_json::json!({"projects":[]}),
            serde_json::json!({"projects":[{"periods":[]}],"pagination":{"cursor":"next"}}),
            serde_json::json!({"projects":[{"periods":[{"period_plan":"enterprise","period_start":now,"consumption":[]}]}]}),
        ] {
            let data = serde_json::from_value(value).unwrap();
            assert!(estimate(&data, now).is_err());
        }
    }
    #[test]
    fn thresholds_missing_and_stale_metrics_are_distinct() {
        let mut s = UsageMonitor::healthy_for_test().snapshot();
        let now = Utc::now();
        assert!(s.notice(now).is_none());
        s.estimated_month_usd = Some(8.);
        assert_eq!(s.notice(now).unwrap().level, "warning");
        s.estimated_month_usd = Some(10.);
        assert_eq!(s.notice(now).unwrap().level, "error");
        s.last_error = Some("failed".into());
        assert_eq!(s.notice(now).unwrap().title, "Usage monitoring unavailable");
        s.last_error = None;
        s.last_success = Some(now - chrono::Duration::minutes(16));
        assert_eq!(s.notice(now).unwrap().title, "Usage monitoring unavailable");
        s.last_success = Some(now);
        s.estimated_month_usd = Some(0.);
        s.estimated_day_usd = Some(0.50);
        assert!(s.notice(now).is_none());
        s.estimated_day_usd = Some(0.501);
        assert_eq!(s.notice(now).unwrap().level, "error");
        s.estimated_day_usd = Some(0.);
        assert!(s.notice(now).is_none());
    }
}
