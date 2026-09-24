//! Runtime read limits. Reservations happen before SQL futures are polled.
use axum::{extract::Request, middleware::Next, response::Response};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

pub const MAX_QUERY_ROWS: usize = 100;
pub const MAX_REQUEST_ROWS: usize = 200;
tokio::task_local! { static BUDGET: Arc<AtomicUsize>; }

pub fn limit(value: usize) -> Result<usize, sqlx::Error> {
    if (1..=MAX_QUERY_ROWS).contains(&value) {
        Ok(value)
    } else {
        Err(sqlx::Error::Protocol(
            "read limit must be between 1 and 100".into(),
        ))
    }
}

struct Reservation {
    budget: Option<Arc<AtomicUsize>>,
    max: usize,
}
impl Reservation {
    fn new(max: usize) -> Result<Self, sqlx::Error> {
        limit(max)?;
        let budget = BUDGET.try_with(Arc::clone).ok();
        if let Some(ref counter) = budget {
            counter
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    n.checked_add(max).filter(|v| *v <= MAX_REQUEST_ROWS)
                })
                .map_err(|_| {
                    crate::usage::BUDGET_REJECTIONS.fetch_add(1, Ordering::Relaxed);
                    sqlx::Error::Protocol("request database read budget exceeded".into())
                })?;
        }
        Ok(Self { budget, max })
    }
    fn finish(self, rows: usize) -> Result<(), sqlx::Error> {
        if rows > self.max {
            return Err(sqlx::Error::Protocol(
                "SQL returned more than its declared row limit".into(),
            ));
        }
        if let Some(counter) = self.budget {
            counter.fetch_sub(self.max - rows, Ordering::SeqCst);
        }
        crate::usage::DB_ROWS.fetch_add(rows as u64, Ordering::Relaxed);
        crate::usage::DB_QUERIES.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(rows, maximum = self.max, "database read");
        Ok(())
    }
}

pub trait ReadFutureExt<T>: Future<Output = Result<T, sqlx::Error>> + Sized {
    async fn bounded_one(self) -> Result<T, sqlx::Error> {
        let reservation = Reservation::new(1)?;
        let value = self.await?;
        reservation.finish(1)?;
        Ok(value)
    }
}
impl<T, F: Future<Output = Result<T, sqlx::Error>>> ReadFutureExt<T> for F {}
pub trait RowsFutureExt<T>: Future<Output = Result<Vec<T>, sqlx::Error>> + Sized {
    async fn bounded_rows(self, max: usize) -> Result<Vec<T>, sqlx::Error> {
        let reservation = Reservation::new(max)?;
        let value = self.await?;
        reservation.finish(value.len())?;
        Ok(value)
    }
}
impl<T, F: Future<Output = Result<Vec<T>, sqlx::Error>>> RowsFutureExt<T> for F {}

pub async fn scope<T>(future: impl Future<Output = T>) -> T {
    // tower-sessions uses singleton-by-primary-key reads outside application SQL.
    BUDGET.scope(Arc::new(AtomicUsize::new(2)), future).await
}
pub async fn middleware(request: Request, next: Next) -> Response {
    let query = request.uri().query().unwrap_or("").to_owned();
    crate::pages::scope(
        &query,
        scope(async move {
            let response = next.run(request).await;
            let rows = BUDGET.with(|b| b.load(Ordering::SeqCst));
            tracing::info!(
                reserved_session_rows = 2,
                database_rows = rows,
                "request read budget"
            );
            #[cfg(test)]
            let response = {
                let mut response = response;
                response
                    .headers_mut()
                    .insert("x-test-read-rows", rows.to_string().parse().unwrap());
                response
            };
            response
        }),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn rejects_before_polling_and_refunds_unused_rows() {
        scope(async {
            async { Ok::<_, sqlx::Error>(vec![0; 98]) }
                .bounded_rows(100)
                .await
                .unwrap();
            async { Ok::<_, sqlx::Error>(vec![0; 100]) }
                .bounded_rows(100)
                .await
                .unwrap();
            assert!(async {
                panic!("must not execute");
                #[allow(unreachable_code)]
                Ok::<_, sqlx::Error>(1)
            }
            .bounded_one()
            .await
            .is_err());
        })
        .await;
        assert!(limit(0).is_err());
        assert!(limit(101).is_err());
    }
}
