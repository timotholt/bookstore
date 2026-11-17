//! Process usage counters: catalog cache and database read budget.
//! Logged every five minutes and exposed to operators at `/ops/usage`.
use crate::app::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
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

pub struct UsageMonitor {
    metrics_token: Option<String>,
}
impl UsageMonitor {
    pub fn from_env() -> Arc<Self> {
        Arc::new(Self {
            metrics_token: std::env::var("OPERATIONS_METRICS_TOKEN")
                .ok()
                .filter(|v| v.len() >= 32),
        })
    }
    pub fn spawn(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(300));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                timer.tick().await;
                tracing::info!(
                    cache_hits = CACHE_HITS.load(Ordering::Relaxed),
                    cache_fills = CACHE_FILLS.load(Ordering::Relaxed),
                    cache_fill_bytes = CACHE_FILL_BYTES.load(Ordering::Relaxed),
                    cache_errors = CACHE_ERRORS.load(Ordering::Relaxed),
                    database_rows_accounted = DB_ROWS.load(Ordering::Relaxed),
                    database_read_queries = DB_QUERIES.load(Ordering::Relaxed),
                    read_budget_rejections = BUDGET_REJECTIONS.load(Ordering::Relaxed),
                    "server usage statistics"
                );
            }
        });
    }
    #[cfg(test)]
    pub fn for_test() -> Arc<Self> {
        Arc::new(Self {
            metrics_token: Some("test-operations-token-at-least-32-characters".into()),
        })
    }
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
        "process":{"cache_hits":CACHE_HITS.load(Ordering::Relaxed),"cache_fills":CACHE_FILLS.load(Ordering::Relaxed),"estimated_cache_fill_bytes":CACHE_FILL_BYTES.load(Ordering::Relaxed),"cache_errors":CACHE_ERRORS.load(Ordering::Relaxed),"database_rows_accounted":DB_ROWS.load(Ordering::Relaxed),"database_read_queries":DB_QUERIES.load(Ordering::Relaxed),"read_budget_rejections":BUDGET_REJECTIONS.load(Ordering::Relaxed)}
    }))).into_response()
}
