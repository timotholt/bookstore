//! Disposable, process-local public catalog cache. Never caches personal data.
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use moka::future::Cache;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

type Entries = Cache<String, Arc<Vec<u8>>>;
#[derive(Clone, Copy)]
pub enum Lifetime {
    Offers,
    Counts,
    Metadata,
}
pub struct CatalogCache {
    offers: Entries,
    counts: Entries,
    metadata: Entries,
    concurrency: Semaphore,
    rate: std::sync::Mutex<(Instant, u32)>,
    generation: AtomicU64,
}
tokio::task_local! { static CURRENT: Arc<CatalogCache>; }
impl CatalogCache {
    pub fn new() -> Self {
        fn cache(mib: u64, seconds: u64) -> Entries {
            Cache::builder()
                .max_capacity(mib * 1024 * 1024)
                .weigher(|k: &String, v: &Arc<Vec<u8>>| {
                    (k.len() + v.len() + 1024).min(u32::MAX as usize) as u32
                })
                .time_to_live(Duration::from_secs(seconds))
                .build()
        }
        Self {
            offers: cache(32, 30),
            counts: cache(8, 300),
            metadata: cache(24, 3600),
            concurrency: Semaphore::new(2),
            rate: std::sync::Mutex::new((Instant::now(), 0)),
            generation: AtomicU64::new(0),
        }
    }
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.offers.invalidate_all();
        self.counts.invalidate_all();
        self.metadata.invalidate_all();
    }
    async fn load<T: Serialize + DeserializeOwned>(
        &self,
        kind: &'static str,
        key: String,
        lifetime: Lifetime,
        future: impl Future<Output = Result<T, sqlx::Error>>,
    ) -> Result<T, sqlx::Error> {
        let cache = match lifetime {
            Lifetime::Offers => &self.offers,
            Lifetime::Counts => &self.counts,
            Lifetime::Metadata => &self.metadata,
        };
        let key = format!("{}:{kind}:{key}", self.generation.load(Ordering::SeqCst));
        if key.len() > 4096 {
            return Err(sqlx::Error::Protocol("catalog filter is too long".into()));
        }
        if let Some(bytes) = cache.get(&key).await {
            crate::usage::CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            tracing::debug!(query = kind, cache = "hit", "catalog cache");
            return serde_json::from_slice(&bytes).map_err(|e| sqlx::Error::Decode(Box::new(e)));
        }
        let bytes = cache
            .try_get_with(key, async {
                let _permit =
                    tokio::time::timeout(Duration::from_secs(2), self.concurrency.acquire())
                        .await
                        .map_err(|_| sqlx::Error::PoolTimedOut)?
                        .map_err(|_| sqlx::Error::PoolClosed)?;
                {
                    let mut rate = self.rate.lock().unwrap_or_else(|e| e.into_inner());
                    if rate.0.elapsed() >= Duration::from_secs(60) {
                        *rate = (Instant::now(), 0);
                    }
                    if rate.1 >= 120 {
                        return Err(sqlx::Error::Protocol(
                            "catalog refresh rate exceeded".into(),
                        ));
                    }
                    rate.1 += 1;
                }
                let value = future.await.inspect_err(|_| {
                    crate::usage::CACHE_ERRORS.fetch_add(1, Ordering::Relaxed);
                })?;
                let bytes =
                    serde_json::to_vec(&value).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
                tracing::info!(
                    query = kind,
                    cache = "fill",
                    payload_bytes = bytes.len(),
                    "catalog cache"
                );
                crate::usage::CACHE_FILLS.fetch_add(1, Ordering::Relaxed);
                crate::usage::CACHE_FILL_BYTES.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                Ok::<_, sqlx::Error>(Arc::new(bytes))
            })
            .await
            .map_err(|e| match Arc::try_unwrap(e) {
                Ok(e) => e,
                Err(e) => match e.as_ref() {
                    sqlx::Error::RowNotFound => sqlx::Error::RowNotFound,
                    _ => sqlx::Error::Protocol(format!("coalesced catalog load: {e}")),
                },
            })?;
        serde_json::from_slice(&bytes).map_err(|e| sqlx::Error::Decode(Box::new(e)))
    }
}
pub async fn cached<T: Serialize + DeserializeOwned>(
    kind: &'static str,
    key: String,
    lifetime: Lifetime,
    future: impl Future<Output = Result<T, sqlx::Error>>,
) -> Result<T, sqlx::Error> {
    match CURRENT.try_with(Arc::clone) {
        Ok(cache) => cache.load(kind, key, lifetime, future).await,
        Err(_) => future.await,
    }
}
pub async fn middleware(
    State(cache): State<Arc<CatalogCache>>,
    request: Request,
    next: Next,
) -> Response {
    let mutates = request.method() == axum::http::Method::POST && request.uri().path() != "/events";
    let response = CURRENT.scope(cache.clone(), next.run(request)).await;
    if mutates && (response.status().is_success() || response.status().is_redirection()) {
        cache.invalidate();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn coalesces_misses_and_invalidation_changes_generation() {
        let cache = CatalogCache::new();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let load = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Ok::<_, sqlx::Error>(vec!["book".to_owned()])
        };
        let (a, b) = tokio::join!(
            cache.load("test", "one".into(), Lifetime::Offers, load()),
            cache.load("test", "one".into(), Lifetime::Offers, load())
        );
        assert_eq!(a.unwrap(), b.unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        cache.invalidate();
        cache
            .load("test", "one".into(), Lifetime::Offers, load())
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn failures_are_not_cached() {
        let cache = CatalogCache::new();
        assert!(cache
            .load::<Vec<String>>("test", "one".into(), Lifetime::Offers, async {
                Err(sqlx::Error::RowNotFound)
            })
            .await
            .is_err());
        assert_eq!(
            cache
                .load("test", "one".into(), Lifetime::Offers, async {
                    Ok(vec!["ok".to_owned()])
                })
                .await
                .unwrap(),
            vec!["ok"]
        );
    }
    #[tokio::test]
    async fn late_fill_cannot_restore_an_invalidated_generation() {
        let cache = CatalogCache::new();
        let began = tokio::sync::Notify::new();
        let finish = tokio::sync::Notify::new();
        let old = cache.load("race", "book".into(), Lifetime::Offers, async {
            began.notify_one();
            finish.notified().await;
            Ok::<_, sqlx::Error>("old".to_owned())
        });
        let update = async {
            began.notified().await;
            cache.invalidate();
            finish.notify_one();
        };
        let (old, _) = tokio::join!(old, update);
        assert_eq!(old.unwrap(), "old");
        let fresh = cache
            .load("race", "book".into(), Lifetime::Offers, async {
                Ok::<_, sqlx::Error>("new".to_owned())
            })
            .await
            .unwrap();
        assert_eq!(fresh, "new");
        assert_eq!(cache.offers.policy().max_capacity(), Some(32 * 1024 * 1024));
        assert_eq!(
            cache.offers.policy().time_to_live(),
            Some(Duration::from_secs(30))
        );
    }
}
