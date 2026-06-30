use axum::{
    routing::{get, post},
    Router,
};
use tower_http::services::{ServeDir, ServeFile};
use tower_sessions::{cookie::SameSite, MemoryStore, SessionManagerLayer};

use crate::db::DbPool;
use crate::handlers;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
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
        .route(
            "/cart/items/:copy_id/restore",
            post(handlers::restore_cart_item),
        )
        .route(
            "/cart/items/:copy_id/save-for-later",
            post(handlers::save_cart_item_for_later),
        )
        .route(
            "/saved-items/:copy_id/move-to-cart",
            post(handlers::move_saved_item_to_cart),
        )
        .route(
            "/saved-items/:copy_id/remove",
            post(handlers::remove_saved_item),
        )
        .route(
            "/checkout",
            get(handlers::checkout).post(handlers::checkout),
        )
        .nest_service("/assets", ServeDir::new("assets"))
        .route_service("/app.js", ServeFile::new("app.js"))
        .route_service("/styles.css", ServeFile::new("styles.css"))
        .layer(session_layer)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations_postgres");
    static TEST_DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    struct PostgresTestDb {
        pool: DbPool,
        _guard: tokio::sync::MutexGuard<'static, ()>,
    }

    impl PostgresTestDb {
        fn pool(&self) -> DbPool {
            self.pool.clone()
        }
    }

    async fn postgres_test_db() -> PostgresTestDb {
        dotenvy::dotenv().ok();
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for Postgres tests");
        crate::db::require_postgres_url(&database_url)
            .expect("DATABASE_URL must be postgres:// or postgresql:// for Postgres tests");
        let database_url = direct_postgres_url(&database_url);

        let guard = TEST_DB_LOCK.lock().await;
        let schema = format!("test_runtime_{}", std::process::id());
        let quoted_schema = quote_ident(&schema);
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("connect admin Postgres test database");

        sqlx::query(&format!("DROP SCHEMA IF EXISTS {quoted_schema} CASCADE"))
            .execute(&admin)
            .await
            .expect("drop Postgres test schema");
        sqlx::query(&format!("CREATE SCHEMA {quoted_schema}"))
            .execute(&admin)
            .await
            .expect("create Postgres test schema");
        admin.close().await;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url_with_search_path(&database_url, &schema))
            .await
            .expect("connect schema-scoped Postgres test database");

        MIGRATOR
            .run(&pool)
            .await
            .expect("run Postgres test migrations");

        PostgresTestDb {
            pool,
            _guard: guard,
        }
    }

    fn database_url_with_search_path(database_url: &str, schema: &str) -> String {
        let separator = if database_url.contains('?') { '&' } else { '?' };
        format!("{database_url}{separator}options=-csearch_path%3D{schema}")
    }

    fn direct_postgres_url(database_url: &str) -> String {
        database_url.replace("-pooler.", ".")
    }

    fn quote_ident(ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn test_app(db: DbPool) -> Router {
        build_router(AppState { db })
    }

    fn test_app_with_db(db: DbPool) -> (Router, DbPool) {
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

    fn named_cookie(response: &axum::response::Response, name: &str) -> String {
        response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .filter_map(|value| value.split(';').next())
            .find(|value| value.starts_with(&format!("{}=", name)))
            .expect("named set-cookie header")
            .to_string()
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

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
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

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
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn home_route_renders_product_tracking_contract() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

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
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

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
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

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
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

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
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

        let response = app
            .oneshot(Request::builder().uri("/cart").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn add_cart_item_persists_anonymous_cart_in_database() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);

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
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, _db) = test_app_with_db(db);
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
        assert!(body.contains("Items in your cart"));
        assert!(body.contains(r##"hx-target="#cartPageMain""##));
        assert!(body.contains(r#"hx-swap="outerHTML show:none""#));
    }

    #[tokio::test]
    async fn cart_page_reads_persisted_cart_by_browser_cart_cookie_after_session_store_loss() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
        let add_response = app
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
        let cart_cookie = named_cookie(&add_response, cart::BROWSER_CART_KEY_COOKIE);
        let restarted_app = build_router(AppState { db });

        let response = restarted_app
            .oneshot(
                Request::builder()
                    .uri("/cart")
                    .header(header::COOKIE, cart_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.contains("Dune"));
        assert!(body.contains("Items in your cart"));
    }

    #[tokio::test]
    async fn cart_quantity_routes_update_persisted_rows() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
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
    async fn cart_page_remove_returns_page_fragment_instead_of_redirecting() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
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
                    .method("POST")
                    .uri("/cart/items/3/remove")
                    .header(header::COOKIE, cookie)
                    .header("X-Cart-View", "page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("HX-Redirect"), None);
        let body = response_body(response).await;
        assert!(body.contains(r#"id="cartPageMain""#));
        assert!(body.contains("Your stack is empty."));
        assert!(body.contains("was removed from your cart."));

        let remaining =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cart_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn cart_remove_shows_undo_notice_and_restore_recovers_quantity() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
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

        let remove_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items/3/remove")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(remove_response.status(), StatusCode::OK);
        let remove_body = response_body(remove_response).await;
        assert!(remove_body.contains("was removed from your cart."));
        assert!(remove_body.contains("Undo"));
        assert!(remove_body.contains(r#"data-track-click="cart_item_remove_undone""#));

        let removed_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cart_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(removed_count, 0);

        let restore_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items/3/restore")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(restore_response.status(), StatusCode::OK);
        let restore_body = response_body(restore_response).await;
        assert!(restore_body.contains("Dune"), "{}", restore_body);
        assert!(!restore_body.contains("was removed from your cart."));

        let quantity =
            sqlx::query_scalar::<_, i32>("SELECT quantity FROM cart_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(quantity, 2);
    }

    #[tokio::test]
    async fn cart_drawer_css_keeps_replacement_visible_while_body_is_cart_open() {
        let css = std::fs::read_to_string("styles.css").expect("read styles");
        assert!(css.contains("body.cart-open .cart-drawer"));
        assert!(css.contains("body.cart-open .cart-drawer:not(.is-open) .cart-panel"));
        assert!(css.contains("body.cart-drawer-updating .cart-panel"));
    }

    #[tokio::test]
    async fn cart_save_for_later_moves_item_out_of_cart_and_into_saved_items() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
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
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items/3/save-for-later")
                    .header(header::COOKIE, cookie.clone())
                    .header("X-Cart-View", "page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("HX-Redirect"), None);
        let fragment_body = response_body(response).await;
        assert!(fragment_body.contains(r#"id="cartPageMain""#));
        assert!(fragment_body.contains("Saved for later"));

        let active_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cart_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(active_count, 0);

        let saved_quantity =
            sqlx::query_scalar::<_, i32>("SELECT quantity FROM saved_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(saved_quantity, 1);

        let cart_response = app
            .oneshot(
                Request::builder()
                    .uri("/cart")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cart_response.status(), StatusCode::OK);
        let body = response_body(cart_response).await;
        assert!(body.contains("Saved for later"));
        assert!(body.contains("Move to cart"));
        assert!(body.contains("Dune"));
        assert!(body.contains(r#"data-track-click="saved_item_moved_to_cart""#));
    }

    #[tokio::test]
    async fn saved_item_move_to_cart_restores_cart_and_removes_saved_row() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
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

        let save_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items/3/save-for-later")
                    .header(header::COOKIE, cookie.clone())
                    .header("X-Cart-View", "page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(save_response.status(), StatusCode::OK);

        let move_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/saved-items/3/move-to-cart")
                    .header(header::COOKIE, cookie)
                    .header("X-Cart-View", "page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(move_response.status(), StatusCode::OK);
        assert_eq!(move_response.headers().get("HX-Redirect"), None);
        let move_body = response_body(move_response).await;
        assert!(move_body.contains(r#"id="cartPageMain""#));

        let cart_quantity =
            sqlx::query_scalar::<_, i32>("SELECT quantity FROM cart_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(cart_quantity, 1);

        let saved_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM saved_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(saved_count, 0);
    }

    #[tokio::test]
    async fn saved_item_move_to_cart_works_after_session_store_loss() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
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
        let cart_cookie = named_cookie(&add_response, cart::BROWSER_CART_KEY_COOKIE);
        let session_cookie = session_cookie(&add_response);

        let save_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cart/items/3/save-for-later")
                    .header(header::COOKIE, session_cookie)
                    .header("X-Cart-View", "page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(save_response.status(), StatusCode::OK);

        let restarted_app = build_router(AppState { db: db.clone() });
        let move_response = restarted_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/saved-items/3/move-to-cart")
                    .header(header::COOKIE, cart_cookie)
                    .header("X-Cart-View", "page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(move_response.status(), StatusCode::OK);
        let cart_quantity =
            sqlx::query_scalar::<_, i32>("SELECT quantity FROM cart_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(cart_quantity, 1);
        let saved_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM saved_items WHERE copy_id = 3")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(saved_count, 0);
    }

    #[tokio::test]
    async fn cart_add_caps_quantity_at_available_stock() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
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
        let second_body = response_body(second_response).await;
        assert!(second_body.contains("Only 1 available"));
        assert!(second_body.contains(r#"disabled aria-disabled="true">+</button>"#));

        let quantity =
            sqlx::query_scalar::<_, i32>("SELECT quantity FROM cart_items WHERE copy_id = 9")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(quantity, 1);
    }

    #[tokio::test]
    async fn cart_page_marks_stock_capped_lines() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, _db) = test_app_with_db(db);
        let add_response = app
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
        assert!(body.contains("Only 1 available"));
        assert!(body.contains(r#"disabled aria-disabled="true">+</button>"#));
    }

    #[tokio::test]
    async fn checkout_route_renders_review_page_from_cart() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, _db) = test_app_with_db(db);
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
                    .uri("/checkout")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.contains("Secure Checkout"));
        assert!(body.contains("Place your order"));
        assert!(body.contains("Items in this order"));
        assert!(body.contains("Arriving for pickup in 1-2 days"));
        assert!(body.contains("Dune"));
        assert!(body.contains("Encrypted checkout"));
        assert!(body.contains(
            r#"<a class="brand checkout-brand" href="/" aria-label="Davis's Books home">"#
        ));
        assert!(!body.contains("All Genres"));
        assert!(!body.contains("Best Sellers"));
        assert!(!body.contains("checkout-cart-link"));
    }

    #[tokio::test]
    async fn review_foundation_tables_migrate() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (_app, db) = test_app_with_db(db);

        sqlx::query(
            r#"
            INSERT INTO reviews (
                book_id,
                user_id,
                rating,
                title,
                body,
                status,
                verified_purchase
            )
            VALUES ('b003', 'user-1', 5, 'Loved it', 'A classic for a reason.', 'published', true)
            "#,
        )
        .execute(&db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO review_aggregates (
                book_id,
                published_count,
                rating_sum,
                average_rating,
                star_5_count,
                verified_count
            )
            VALUES ('b003', 1, 5, 5.0, 1, 1)
            "#,
        )
        .execute(&db)
        .await
        .unwrap();

        let aggregate = sqlx::query_as::<_, (i64, i64, f64, i64)>(
            r#"
            SELECT
                published_count::int8,
                rating_sum::int8,
                average_rating::float8,
                verified_count::int8
            FROM review_aggregates
            WHERE book_id = 'b003'
            "#,
        )
        .fetch_one(&db)
        .await
        .unwrap();

        assert_eq!(aggregate, (1, 5, 5.0, 1));
    }

    #[tokio::test]
    async fn events_endpoint_persists_analytics_payload() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
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
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);
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
