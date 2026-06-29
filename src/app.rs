use axum::{
    routing::{get, post},
    Router,
};
use sqlx::SqlitePool;
use tower_http::services::{ServeDir, ServeFile};
use tower_sessions::{cookie::SameSite, MemoryStore, SessionManagerLayer};

use crate::handlers;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

pub fn build_router(state: AppState) -> Router {
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(std::env::var("APP_ENV").unwrap_or_default() == "production")
        .with_same_site(SameSite::Lax);

    Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/readyz", get(handlers::readyz))
        .route("/events", post(handlers::record_event))
        .route("/", get(handlers::home))
        .route("/catalog", get(handlers::catalog))
        .route("/books/:book_id", get(handlers::book_detail))
        .route("/cart", get(handlers::cart_page))
        .route("/cart/items", post(handlers::add_cart_item))
        .route(
            "/cart/items/:copy_id/increase",
            post(handlers::increase_cart_item),
        )
        .route(
            "/cart/items/:copy_id/decrease",
            post(handlers::decrease_cart_item),
        )
        .route(
            "/cart/items/:copy_id/remove",
            post(handlers::remove_cart_item),
        )
        .route("/checkout", post(handlers::checkout))
        .nest_service("/assets", ServeDir::new("assets"))
        .route_service("/app.js", ServeFile::new("app.js"))
        .route_service("/styles.css", ServeFile::new("styles.css"))
        .layer(session_layer)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request, StatusCode},
    };
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    async fn test_app() -> Router {
        test_app_with_db().await.0
    }

    async fn test_app_with_db() -> (Router, SqlitePool) {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect test sqlite");
        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("run test migrations");

        (build_router(AppState { db: db.clone() }), db)
    }

    async fn response_body(response: axum::response::Response) -> String {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    fn session_cookie(response: &axum::response::Response) -> String {
        response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .expect("set-cookie header")
            .to_string()
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn readyz_checks_database() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"ready");
    }

    #[tokio::test]
    async fn home_route_renders() {
        let app = test_app().await;

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn home_route_renders_product_tracking_contract() {
        let app = test_app().await;

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.contains(r#"data-track-impression="product_impression""#));
        assert!(body.contains(r#"data-track-click="product_clicked""#));
        assert!(body.contains(r#"data-track-click="add_to_cart_clicked""#));
        assert!(body.contains(r#"data-track-click="buy_now_clicked""#));
        assert!(body.contains(r#"data-source="home.best_sellers""#));
        assert!(body.contains(r#"data-target-type="book""#));
        assert!(body.contains(r#"data-target-id="b003""#));
    }

    #[tokio::test]
    async fn catalog_htmx_route_renders_results() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/catalog?q=dune")
                    .header("HX-Request", "true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn catalog_without_htmx_redirects_to_home_catalog() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/catalog")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/#catalog")
        );
    }

    #[tokio::test]
    async fn book_detail_route_renders() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/books/b003")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cart_page_route_renders() {
        let app = test_app().await;

        let response = app
            .oneshot(Request::builder().uri("/cart").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn add_cart_item_persists_anonymous_cart_in_database() {
        let (app, db) = test_app_with_db().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("copy_id=3"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let cookie = session_cookie(&response);
        let body = response_body(response).await;
        assert!(body.contains("Dune"));

        let row = sqlx::query_as::<_, (String, Option<String>, String, i64, i32)>(
            r#"
            SELECT c.session_key, c.user_id, c.status, ci.copy_id, ci.quantity
            FROM carts c
            JOIN cart_items ci ON ci.cart_id = c.id
            LIMIT 1
            "#,
        )
        .fetch_one(&db)
        .await
        .unwrap();

        assert!(!cookie.is_empty());
        assert!(!row.0.is_empty());
        assert_eq!(row.1, None);
        assert_eq!(row.2, "active");
        assert_eq!(row.3, 3);
        assert_eq!(row.4, 1);
    }

    #[tokio::test]
    async fn cart_page_reads_persisted_anonymous_cart_by_session_cookie() {
        let (app, _db) = test_app_with_db().await;
        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("copy_id=3"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = session_cookie(&add_response);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/cart")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.contains("Dune"));
        assert!(body.contains("Your Stack"));
    }

    #[tokio::test]
    async fn cart_quantity_routes_update_persisted_rows() {
        let (app, db) = test_app_with_db().await;
        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("copy_id=3"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = session_cookie(&add_response);

        let increase_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items/3/increase")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(increase_response.status(), StatusCode::OK);

        let quantity =
            sqlx::query_scalar::<_, i32>("SELECT quantity FROM cart_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(quantity, 2);

        for _ in 0..2 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/cart/items/3/decrease")
                        .header(header::COOKIE, cookie.clone())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        let remaining =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cart_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn cart_add_caps_quantity_at_available_stock() {
        let (app, db) = test_app_with_db().await;
        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("copy_id=9"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = session_cookie(&first_response);

        let second_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .header(header::COOKIE, cookie)
                    .body(Body::from("copy_id=9"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second_response.status(), StatusCode::OK);

        let quantity =
            sqlx::query_scalar::<_, i32>("SELECT quantity FROM cart_items WHERE copy_id = 9")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(quantity, 1);
    }

    #[tokio::test]
    async fn events_endpoint_persists_analytics_payload() {
        let (app, db) = test_app_with_db().await;
        let payload = json!({
            "event_name": "product_clicked",
            "source": "home.best_sellers",
            "target_type": "book",
            "target_id": "b003",
            "page_path": "/",
            "metadata": {
                "tag": "article",
                "text": "Dune"
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/events")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let row = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
            r#"
            SELECT
                session_key,
                event_name,
                source,
                target_type,
                target_id,
                page_path,
                metadata_json
            FROM analytics_events
            LIMIT 1
            "#,
        )
        .fetch_one(&db)
        .await
        .unwrap();

        assert!(!row.0.is_empty());
        assert_eq!(row.1, "product_clicked");
        assert_eq!(row.2, "home.best_sellers");
        assert_eq!(row.3, "book");
        assert_eq!(row.4, "b003");
        assert_eq!(row.5, "/");
        assert!(row.6.contains(r#""tag":"article""#));
    }

    #[tokio::test]
    async fn events_endpoint_rejects_empty_event_name() {
        let app = test_app().await;
        let payload = json!({
            "event_name": "",
            "source": "home.best_sellers"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/events")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
