//! Bounded secondary-list navigation; pagination is shared by full pages and fragments.
use std::collections::HashMap;
tokio::task_local! { static PAGES: HashMap<String,String>; }
pub fn number(key: &str) -> u32 {
    PAGES
        .try_with(|q| {
            q.get(key)
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1)
        })
        .unwrap_or(1)
}
pub fn offset(key: &str, size: i64) -> i64 {
    (i64::from(number(key)) - 1) * size
}
pub async fn scope<T>(query: &str, future: impl std::future::Future<Output = T>) -> T {
    PAGES
        .scope(
            serde_urlencoded::from_str(query).unwrap_or_default(),
            future,
        )
        .await
}

pub fn url(base: &str, key: &str, page: u32) -> String {
    let mut query = PAGES.try_with(Clone::clone).unwrap_or_default();
    query.insert(key.to_owned(), page.to_string());
    format!(
        "{base}?{}",
        serde_urlencoded::to_string(query).unwrap_or_default()
    )
}
