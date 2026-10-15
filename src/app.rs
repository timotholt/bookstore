use axum::{
    routing::{get, post},
    Router,
};
use tower_http::services::{ServeDir, ServeFile};
use tower_sessions::{cookie::SameSite, SessionManagerLayer};

use crate::db::DbPool;
use crate::handlers;

#[derive(Clone)]
pub struct AppState {
    pub usage: std::sync::Arc<crate::usage::UsageMonitor>,
    pub db: DbPool,
    pub email: std::sync::Arc<crate::email::EmailService>,
}

pub fn build_router(state: AppState) -> Router {
    let session_store = tower_sessions_sqlx_store::PostgresStore::new(state.db.clone());
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(std::env::var("APP_ENV").unwrap_or_default() == "production")
        .with_same_site(SameSite::Lax)
        .with_expiry(tower_sessions::Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::days(7),
        ));

    let catalog_cache = std::sync::Arc::new(crate::catalog_cache::CatalogCache::new());
    Router::new()
        .route(
            "/webhooks/resend",
            post(crate::account_email::resend_webhook)
                .layer(axum::extract::DefaultBodyLimit::max(65536)),
        )
        .route(
            "/forgot-password",
            get(crate::account_email::forgot_get).post(crate::account_email::forgot_post),
        )
        .route(
            "/reset-password",
            get(crate::account_email::reset_get).post(crate::account_email::reset_post),
        )
        .route(
            "/verify-email",
            get(crate::account_email::verify_get).post(crate::account_email::verify_post),
        )
        .route(
            "/confirm-email-change",
            get(crate::account_email::change_get).post(crate::account_email::change_post),
        )
        .route(
            "/account/verification",
            get(crate::account_email::verification_get),
        )
        .route(
            "/account/verification/resend",
            post(crate::account_email::resend_post),
        )
        .route(
            "/account/email-change",
            get(crate::account_email::email_change_get)
                .post(crate::account_email::email_change_post),
        )
        .route("/ops/usage", get(crate::usage::metrics))
        .route("/healthz", get(handlers::healthz))
        .route("/readyz", get(handlers::readyz))
        .route("/version", get(handlers::version))
        .route("/events", post(handlers::record_event))
        .route("/", get(handlers::home))
        .route(
            "/signup",
            get(handlers::signup_page).post(handlers::signup_action),
        )
        .route(
            "/login",
            get(handlers::login_page).post(handlers::login_action),
        )
        .route("/logout", post(handlers::logout_action))
        .route("/account", get(handlers::account_home_page))
        .route("/account/security", get(handlers::security_page))
        .route("/account/orders", get(handlers::orders_page))
        .route(
            "/account/preferences",
            get(handlers::preferences_page).post(handlers::preferences_action),
        )
        .route(
            "/account/profile",
            get(handlers::profile_page).post(handlers::profile_action),
        )
        .route("/catalog", get(handlers::catalog))
        .route("/search", get(handlers::search_page))
        .route("/books/:book_id", get(handlers::book_detail))
        .route("/account/reviews", get(crate::review_web::my_reviews))
        .route(
            "/account/reviews/activity",
            get(crate::review_web::activity),
        )
        .route("/books/:book_id/reviews/edit", get(crate::review_web::edit))
        .route("/books/:book_id/reviews", post(crate::review_web::submit))
        .route("/reviews/:id/delete", post(crate::review_web::delete))
        .route("/reviews/:id/vote", post(crate::review_web::vote))
        .route("/reviews/:id/report", post(crate::review_web::report))
        .route("/staff/users", get(crate::admin::users))
        .route(
            "/staff/users/:id",
            get(crate::admin::detail).post(crate::admin::action),
        )
        .route("/staff/reviews", get(crate::review_web::staff))
        .route(
            "/staff/reviews/:id",
            get(crate::review_web::staff_detail).post(crate::review_web::moderate),
        )
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
            get(handlers::checkout).post(handlers::checkout_paused),
        )
        .nest_service("/assets", ServeDir::new("assets"))
        .route_service("/app.js", ServeFile::new("app.js"))
        .route_service("/styles.css", ServeFile::new("styles.css"))
        .layer(axum::extract::DefaultBodyLimit::max(16384))
        .layer(session_layer)
        .layer(axum::middleware::from_fn_with_state(
            catalog_cache,
            crate::catalog_cache::middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.email.clone(),
            crate::email::wake_after_mutation,
        ))
        .layer(axum::middleware::from_fn(crate::read_budget::middleware))
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

    // Tests create schemas and wake the compute; never let them reach a hosted database.
    fn is_local_postgres_url(database_url: &str) -> bool {
        use std::str::FromStr;
        sqlx::postgres::PgConnectOptions::from_str(database_url)
            .map(|o| {
                matches!(o.get_host(), "localhost" | "127.0.0.1" | "::1")
                    || o.get_host().starts_with('/')
            })
            .unwrap_or(false)
    }

    #[test]
    fn postgres_tests_refuse_hosted_databases() {
        assert!(is_local_postgres_url(
            "postgres://postgres:postgres@localhost:5432/postgres?sslmode=disable"
        ));
        assert!(is_local_postgres_url(
            "postgres://me@127.0.0.1/bookstore_test"
        ));
        assert!(!is_local_postgres_url(
            "postgresql://u:p@ep-x-pooler.c-2.us-west-2.aws.neon.tech/neondb?sslmode=require"
        ));
    }

    fn test_principal(id: &str) -> crate::access::Principal {
        crate::access::Principal {
            user_id: id.into(),
            auth_version: 0,
        }
    }

    async fn postgres_test_db() -> PostgresTestDb {
        crate::db::load_runtime_env();
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for Postgres tests");
        crate::db::require_postgres_url(&database_url)
            .expect("DATABASE_URL must be postgres:// or postgresql:// for Postgres tests");
        assert!(
            is_local_postgres_url(&database_url),
            "Postgres tests need a local DATABASE_URL (localhost); .env points at the hosted \
             database. Run with DATABASE_URL=postgres://localhost/<test db> cargo test"
        );
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

        tower_sessions_sqlx_store::PostgresStore::new(pool.clone())
            .migrate()
            .await
            .expect("run session store test migrations");

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

    async fn csrf_form(app: &Router, path: &str, cookie: Option<&str>) -> (String, String) {
        let mut req = Request::builder().uri(path);
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        let response = app
            .clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let new_cookie = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("id="))
            .map(|v| v.split(';').next().unwrap().to_owned())
            .unwrap_or_else(|| cookie.unwrap_or("").to_owned());
        let body = response_body(response).await;
        let token = body
            .split("name=\"csrf\" value=\"")
            .nth(1)
            .expect("CSRF input")
            .split('"')
            .next()
            .unwrap()
            .to_owned();
        (new_cookie, token)
    }

    fn test_app(db: DbPool) -> Router {
        build_router(AppState {
            usage: crate::usage::UsageMonitor::healthy_for_test(),
            db,
            email: std::sync::Arc::new(crate::email::EmailService::test_capture()),
        })
    }

    fn test_app_with_db(db: DbPool) -> (Router, DbPool) {
        (
            build_router(AppState {
                usage: crate::usage::UsageMonitor::healthy_for_test(),
                db: db.clone(),
                email: std::sync::Arc::new(crate::email::EmailService::test_capture()),
            }),
            db,
        )
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
    async fn catalog_seed_uses_chantels_corner_brand() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();

        let title: String = sqlx::query_scalar("SELECT title FROM books WHERE id = 'm001'")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(title, "Chantel's Corner Brass Bookmark");

        let legacy_quotes: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM book_copies WHERE staff_quote LIKE '%Davis Team'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(legacy_quotes, 0);

        let description: String = sqlx::query_scalar(
            "SELECT description FROM book_collections WHERE slug = 'staff-picks'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(description, "Books highlighted by Chantel's Corner staff.");
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
    async fn search_htmx_route_renders_only_results_fragment() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/search?per_page=48&page=1")
                    .header("HX-Request", "true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.starts_with(r#"<div id="catalogResults">"#));
        assert!(body.contains("Items per page"));
        assert!(!body.contains("<html"));
        assert!(!body.contains("Search all items"));
    }

    #[tokio::test]
    async fn catalog_htmx_route_accepts_listing_filters() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

        for uri in [
            "/catalog?listing=new",
            "/catalog?listing=used",
            "/catalog?q=atomic&listing=new",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .header("HX-Request", "true")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK, "{uri}");
        }
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
            Some("/search")
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
    async fn profile_page_requires_login() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/account/profile")
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
            Some("/login")
        );
    }

    #[tokio::test]
    async fn signup_signs_in_and_profile_update_persists() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);

        let (signup_cookie, signup_csrf) = csrf_form(&app, "/signup", None).await;
        let signup_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/signup")
                    .header(header::COOKIE,signup_cookie)
                    .header(header::ORIGIN,std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "https://www.chantelscorner.com".into()))
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(
                        format!("first_name=Taylor&last_name=Reader&email=reader%40example.com&password=UniqueHorseBookstore27%21&password_confirm=UniqueHorseBookstore27%21&csrf={signup_csrf}"),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(signup_response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            signup_response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/account/profile")
        );
        let cookie = session_cookie(&signup_response);

        let profile_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/account/profile")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(profile_response.status(), StatusCode::OK);
        let profile_body = response_body(profile_response).await;
        assert!(profile_body.contains("reader@example.com"));
        assert!(profile_body.contains("Profile"));

        let (_, profile_csrf) = csrf_form(&app, "/account/profile", Some(&cookie)).await;
        let update_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/account/profile")
                    .header(header::ORIGIN,std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "https://www.chantelscorner.com".into()))
                    .header(header::COOKIE, cookie)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(
                        format!("first_name=Taylor&last_name=Reader&email=reader%40example.com&phone_number=555-0101&address_line1=123%20Book%20St&address_line2=Apt%204&address_city=La%20Habra&address_state=CA&address_postal_code=90631&marketing_opt_in=yes&csrf={profile_csrf}"),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(update_response.status(), StatusCode::OK);
        let update_body = response_body(update_response).await;
        assert!(update_body.contains("Profile saved."));

        let row = sqlx::query_as::<
            _,
            (
                Option<String>,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                bool,
            ),
        >(
            r#"
            SELECT
                first_name,
                last_name,
                email,
                phone_number,
                address_line1,
                address_line2,
                address_city,
                address_state,
                address_postal_code,
                marketing_opt_in
            FROM users
            WHERE email = 'reader@example.com'
            "#,
        )
        .fetch_one(&db)
        .await
        .unwrap();

        assert_eq!(row.0.as_deref(), Some("Taylor"));
        assert_eq!(row.1.as_deref(), Some("Reader"));
        assert_eq!(row.2, "reader@example.com");
        assert_eq!(row.3.as_deref(), Some("555-0101"));
        assert_eq!(row.4.as_deref(), Some("123 Book St"));
        assert_eq!(row.5.as_deref(), Some("Apt 4"));
        assert_eq!(row.6.as_deref(), Some("La Habra"));
        assert_eq!(row.7.as_deref(), Some("CA"));
        assert_eq!(row.8.as_deref(), Some("90631"));
        assert!(row.9);
    }

    #[tokio::test]
    async fn account_flyout_and_pages_render_for_signed_in_user() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db);

        let (signup_cookie, signup_csrf) = csrf_form(&app, "/signup", None).await;
        let signup_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/signup")
                    .header(header::COOKIE,signup_cookie)
                    .header(header::ORIGIN,std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "https://www.chantelscorner.com".into()))
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(
                        format!("first_name=Account&last_name=Reader&email=account%40example.com&password=UniqueHorseBookstore27%21&password_confirm=UniqueHorseBookstore27%21&csrf={signup_csrf}"),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(signup_response.status(), StatusCode::SEE_OTHER);
        let cookie = session_cookie(&signup_response);

        let home_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(home_response.status(), StatusCode::OK);
        let home_body = response_body(home_response).await;
        assert!(home_body.contains("account-menu-panel"));
        assert!(home_body.contains(r#"href="/account/security""#));
        assert!(home_body.contains(r#"href="/account/orders""#));
        assert!(home_body.contains(r#"href="/account/preferences""#));

        for (uri, expected) in [
            ("/account", "Your Account"),
            ("/account/security", "Login & Security"),
            ("/account/orders", "Your Orders"),
            ("/account/preferences", "Shopping Preferences"),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .header(header::COOKIE, cookie.clone())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = response_body(response).await;
            assert!(body.contains(expected), "{uri}: {body}");
        }
    }

    async fn post_form(
        app: &Router,
        path: &str,
        cookie: &str,
        body: String,
    ) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::COOKIE, cookie)
                    .header(
                        header::ORIGIN,
                        std::env::var("PUBLIC_BASE_URL")
                            .unwrap_or_else(|_| "https://www.chantelscorner.com".into()),
                    )
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
    async fn challenge_cookie(app: &Router, path: &str, token: &str) -> (String, String) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("{path}?token={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()["location"], path);
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        let cookie = session_cookie(&response);
        csrf_form(app, path, Some(&cookie)).await
    }
    async fn test_token(db: &DbPool, user: &str, email: &str, purpose: &str) -> String {
        use sha2::{Digest, Sha256};
        let token = crate::account_email::random_secret();
        sqlx::query("INSERT INTO account_tokens(id,user_id,purpose,token_hash,target_email,auth_version,expires_at) SELECT $1,id,$3,$4,$5,auth_version,now()+interval '1 hour' FROM users WHERE id=$2")
            .bind(uuid::Uuid::new_v4()).bind(user).bind(purpose).bind(format!("{:x}",Sha256::digest(token.as_bytes()))).bind(email).execute(db).await.unwrap();
        token
    }
    #[tokio::test]
    async fn account_email_security_lifecycle() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db.clone());
        let svc = crate::email::EmailService::test_capture();
        let user = crate::auth::register_user(
            &db,
            &svc,
            "Email",
            "Reader",
            "secure@example.com",
            secrecy::Secret::new("Bookstore-unique-old-password1!".into()),
        )
        .await
        .unwrap();
        let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM email_outbox")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(queued, 1);
        let reset = test_token(&db, &user.id, &user.email, "reset_password").await;
        let verify = test_token(&db, &user.id, &user.email, "verify_email").await;
        let (verify_cookie, verify_csrf) = challenge_cookie(&app, "/verify-email", &verify).await;
        let verified: bool =
            sqlx::query_scalar("SELECT email_verified_at IS NOT NULL FROM users WHERE id=$1")
                .bind(&user.id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(!verified, "scanner GET must not verify");
        let rejected = post_form(&app, "/verify-email", &verify_cookie, "csrf=wrong".into()).await;
        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
        let (second_cookie, second_csrf) = challenge_cookie(&app, "/verify-email", &verify).await;
        let (first, second) = tokio::join!(
            post_form(
                &app,
                "/verify-email",
                &verify_cookie,
                format!("csrf={verify_csrf}")
            ),
            post_form(
                &app,
                "/verify-email",
                &second_cookie,
                format!("csrf={second_csrf}")
            )
        );
        let replies = [response_body(first).await, response_body(second).await];
        assert_eq!(
            replies
                .iter()
                .filter(|r| r.contains("Email confirmed"))
                .count(),
            1
        );
        assert_eq!(
            replies
                .iter()
                .filter(|r| r.contains("invalid or expired"))
                .count(),
            1
        );
        let replay = post_form(
            &app,
            "/verify-email",
            &verify_cookie,
            format!("csrf={verify_csrf}"),
        )
        .await;
        assert!(response_body(replay).await.contains("invalid or expired"));
        let (login_cookie, login_csrf) = csrf_form(&app, "/login", None).await;
        let login=post_form(&app,"/login",&login_cookie,format!("email=secure%40example.com&password=Bookstore-unique-old-password1%21&csrf={login_csrf}")).await;
        assert_eq!(login.status(), StatusCode::SEE_OTHER);
        let authenticated = session_cookie(&login);
        let (c1, t1) = challenge_cookie(&app, "/reset-password", &reset).await;
        let (c2, t2) = challenge_cookie(&app, "/reset-password", &reset).await;
        let invalid = post_form(
            &app,
            "/reset-password",
            &c1,
            format!("csrf={t1}&password=short&password_confirm=short"),
        )
        .await;
        let html = response_body(invalid).await;
        assert!(html.contains("Use at least 6 characters."));
        assert!(html.contains("action=\"/reset-password\""));
        assert!(!html.contains("Validation error:"));
        assert!(!html.contains("href=\"/forgot-password\""));
        assert!(!html.contains("href=\"/account/verification\""));
        let form1=format!("csrf={t1}&password=Bookstore-unique-new-password1%21&password_confirm=Bookstore-unique-new-password1%21");
        let form2=format!("csrf={t2}&password=Bookstore-unique-new-password1%21&password_confirm=Bookstore-unique-new-password1%21");
        let (a, b) = tokio::join!(
            post_form(&app, "/reset-password", &c1, form1),
            post_form(&app, "/reset-password", &c2, form2)
        );
        assert_eq!(
            [a.status(), b.status()]
                .iter()
                .filter(|s| **s == StatusCode::SEE_OTHER)
                .count(),
            1
        );
        let bodies = [response_body(a).await, response_body(b).await];
        assert_eq!(
            bodies
                .iter()
                .filter(|b| b.contains("invalid or expired"))
                .count(),
            1
        );
        let stale = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/account/security")
                    .header(header::COOKIE, &authenticated)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stale.status(), StatusCode::SEE_OTHER);
        assert_eq!(stale.headers()["location"], "/login");
        let row:(String,i64)=sqlx::query_as("SELECT p.password_hash,u.auth_version FROM password_credentials p JOIN users u ON u.id=p.user_id WHERE u.id=$1").bind(&user.id).fetch_one(&db).await.unwrap();
        assert_eq!(row.1, 1);
        assert!(!crate::auth::verify_password(
            secrecy::Secret::new("Bookstore-unique-old-password1!".into()),
            &row.0
        )
        .await
        .unwrap());
        assert!(crate::auth::verify_password(
            secrecy::Secret::new("Bookstore-unique-new-password1!".into()),
            &row.0
        )
        .await
        .unwrap());
        let no_login = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/account/security")
                    .header(header::COOKIE, &c1)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(no_login.headers()["location"], "/login");
    }
    #[tokio::test]
    async fn recovery_is_generic_and_expired_wrong_purpose_tokens_fail() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db.clone());
        let user = crate::auth::register_user(
            &db,
            &crate::email::EmailService::test_capture(),
            "Email",
            "Reader",
            "known@example.com",
            secrecy::Secret::new("Bookstore-unique-old-password1!".into()),
        )
        .await
        .unwrap();
        let (c, t) = csrf_form(&app, "/forgot-password", None).await;
        let known = post_form(
            &app,
            "/forgot-password",
            &c,
            format!("csrf={t}&email=known%40example.com"),
        )
        .await;
        let unknown = post_form(
            &app,
            "/forgot-password",
            &c,
            format!("csrf={t}&email=unknown%40example.com"),
        )
        .await;
        assert_eq!(known.status(), unknown.status());
        assert_eq!(response_body(known).await, response_body(unknown).await);
        let wrong = test_token(&db, &user.id, &user.email, "verify_email").await;
        let landing = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/reset-password?token={wrong}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = session_cookie(&landing);
        let rejected = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/reset-password")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let html = response_body(rejected).await;
        assert!(html.contains("invalid or expired"));
        assert!(!html.contains("name=\"password\""));
        let (c, t) = csrf_form(&app, "/forgot-password", Some(&cookie)).await;
        let r=post_form(&app,"/reset-password",&c,format!("csrf={t}&password=Bookstore-unique-new-password1%21&password_confirm=Bookstore-unique-new-password1%21")).await;
        assert!(response_body(r).await.contains("invalid or expired"));
        sqlx::query(
            "UPDATE account_tokens SET expires_at=now()-interval '1 second' WHERE user_id=$1",
        )
        .bind(&user.id)
        .execute(&db)
        .await
        .unwrap();
        let (c, t) = challenge_cookie(&app, "/verify-email", &wrong).await;
        let r = post_form(&app, "/verify-email", &c, format!("csrf={t}")).await;
        assert!(response_body(r).await.contains("invalid or expired"));
        // Direct profile writes cannot bypass ownership confirmation.
        let r = crate::auth::update_user_profile(
            &db,
            &user.id,
            crate::auth::ProfileUpdate {
                first_name: "Email",
                last_name: "Reader",
                email: "attacker@example.com",
                phone_number: "",
                address_line1: "",
                address_line2: "",
                address_city: "",
                address_state: "",
                address_postal_code: "",
                marketing_opt_in: false,
            },
            0,
            0,
        )
        .await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn email_change_requires_reauthentication_and_confirmed_ownership() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db.clone());
        let user = crate::auth::register_user(
            &db,
            &crate::email::EmailService::test_capture(),
            "Email",
            "Reader",
            "old@example.com",
            secrecy::Secret::new("Bookstore-unique-old-password1!".into()),
        )
        .await
        .unwrap();
        let (c, t) = csrf_form(&app, "/login", None).await;
        let r = post_form(
            &app,
            "/login",
            &c,
            format!("csrf={t}&email=old%40example.com&password=Bookstore-unique-old-password1%21"),
        )
        .await;
        let signed = session_cookie(&r);
        let (c, t) = csrf_form(&app, "/account/email-change", Some(&signed)).await;
        let failed = post_form(
            &app,
            "/account/email-change",
            &c,
            format!("csrf={t}&email=new%40example.com&password=incorrect-password"),
        )
        .await;
        assert!(response_body(failed)
            .await
            .contains("Reauthentication failed"));
        sqlx::query("DELETE FROM account_email_rate_limits")
            .execute(&db)
            .await
            .unwrap();
        let sent = post_form(
            &app,
            "/account/email-change",
            &c,
            format!("csrf={t}&email=new%40example.com&password=Bookstore-unique-old-password1%21"),
        )
        .await;
        assert!(response_body(sent).await.contains("Check the new address"));
        let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id=$1")
            .bind(&user.id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(email, "old@example.com");
        let token = test_token(&db, &user.id, "new@example.com", "change_email").await;
        let (c, t) = challenge_cookie(&app, "/confirm-email-change", &token).await;
        let confirmed = post_form(&app, "/confirm-email-change", &c, format!("csrf={t}")).await;
        assert!(response_body(confirmed).await.contains("Email confirmed"));
        let row: (String, bool, i64) = sqlx::query_as(
            "SELECT email,email_verified_at IS NOT NULL,auth_version FROM users WHERE id=$1",
        )
        .bind(&user.id)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(row, ("new@example.com".into(), true, 1));
        let stale = post_form(&app, "/confirm-email-change", &c, format!("csrf={t}")).await;
        assert!(response_body(stale).await.contains("invalid or expired"));
        crate::auth::register_user(
            &db,
            &crate::email::EmailService::test_capture(),
            "Other",
            "Reader",
            "occupied@example.com",
            secrecy::Secret::new("Bookstore-unique-old-password1!".into()),
        )
        .await
        .unwrap();
        let collision = test_token(&db, &user.id, "occupied@example.com", "change_email").await;
        let (c, t) = challenge_cookie(&app, "/confirm-email-change", &collision).await;
        let denied = post_form(&app, "/confirm-email-change", &c, format!("csrf={t}")).await;
        assert!(response_body(denied).await.contains("invalid or expired"));
        let unchanged: String = sqlx::query_scalar("SELECT email FROM users WHERE id=$1")
            .bind(&user.id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(unchanged, "new@example.com");
    }

    #[tokio::test]
    async fn account_forms_preserve_origin_without_referring_token_urls() {
        let test_db = postgres_test_db().await;
        let app = test_app(test_db.pool.clone());
        let missing = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/reset-password")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let missing = response_body(missing).await;
        assert!(missing.contains("Link expired or unavailable"));
        assert!(missing.contains("href=\"/forgot-password\""));
        assert!(!missing.contains("name=\"password\""));
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/forgot-password")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()["referrer-policy"], "strict-origin");
        let html = response_body(response).await;
        assert!(html.contains("name=\"referrer\" content=\"strict-origin\""));
        let (cookie, csrf) = csrf_form(&app, "/forgot-password", None).await;
        let rejected = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/forgot-password")
                    .header(header::COOKIE, cookie)
                    .header(header::ORIGIN, "null")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "csrf={csrf}&email=unknown%40example.com"
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/reset-password?token={}",
                        crate::account_email::random_secret()
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        assert_eq!(response.headers()["location"], "/reset-password");
    }

    #[tokio::test]
    async fn account_mail_rollback_and_database_rate_limits() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let app = test_app(db.clone());
        sqlx::query("CREATE FUNCTION fail_mail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'intentional test failure'; END $$").execute(&db).await.unwrap();
        sqlx::query("CREATE TRIGGER fail_mail BEFORE INSERT ON email_outbox FOR EACH ROW EXECUTE FUNCTION fail_mail()").execute(&db).await.unwrap();
        let failed = crate::auth::register_user(
            &db,
            &crate::email::EmailService::test_capture(),
            "Rollback",
            "Reader",
            "rollback@example.com",
            secrecy::Secret::new("Bookstore-unique-old-password1!".into()),
        )
        .await;
        assert!(failed.is_err());
        let users: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE email='rollback@example.com'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(users, 0);
        let tokens: i64 = sqlx::query_scalar("SELECT count(*) FROM account_tokens")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(tokens, 0);
        sqlx::query("DROP TRIGGER fail_mail ON email_outbox")
            .execute(&db)
            .await
            .unwrap();
        let user = crate::auth::register_user(
            &db,
            &crate::email::EmailService::test_capture(),
            "Rate",
            "Reader",
            "rate@example.com",
            secrecy::Secret::new("Bookstore-unique-old-password1!".into()),
        )
        .await
        .unwrap();
        let (c, t) = csrf_form(&app, "/forgot-password", None).await;
        let start = std::time::Instant::now();
        let known = post_form(
            &app,
            "/forgot-password",
            &c,
            format!("csrf={t}&email=rate%40example.com"),
        )
        .await;
        let known_time = start.elapsed();
        let start = std::time::Instant::now();
        let unknown = post_form(
            &app,
            "/forgot-password",
            &c,
            format!("csrf={t}&email=absent%40example.com"),
        )
        .await;
        let unknown_time = start.elapsed();
        assert_eq!(response_body(known).await, response_body(unknown).await);
        assert!(known_time >= std::time::Duration::from_secs(1));
        assert!(unknown_time >= std::time::Duration::from_secs(1));
        // The generic response and one-second floor are the demo contract. Exact
        // equality is not deterministic with live PostgreSQL and email work.
        let restarted = test_app(db.clone());
        let _ = post_form(
            &restarted,
            "/forgot-password",
            &c,
            format!("csrf={t}&email=rate%40example.com"),
        )
        .await;
        let resets: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM account_tokens WHERE user_id=$1 AND purpose='reset_password'",
        )
        .bind(&user.id)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(resets, 1, "recipient limiter survives router restart");
        let raw:i64=sqlx::query_scalar("SELECT count(*) FROM account_email_rate_limits WHERE bucket LIKE '%example%' OR bucket LIKE '%127.%'").fetch_one(&db).await.unwrap();
        assert_eq!(raw, 0);
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
        let restarted_app = build_router(AppState {
            usage: crate::usage::UsageMonitor::healthy_for_test(),
            db,
            email: std::sync::Arc::new(crate::email::EmailService::test_capture()),
        });

        assert!(cart_cookie.starts_with("chantels_cart_key="));
        let legacy_cookie = cart_cookie.replacen("chantels_cart_key=", "davis_cart_key=", 1);
        let legacy_response = restarted_app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/cart")
                    .header(header::COOKIE, legacy_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(legacy_response.status(), StatusCode::OK);
        assert!(!response_body(legacy_response).await.contains("Dune"));

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

        let restarted_app = build_router(AppState {
            usage: crate::usage::UsageMonitor::healthy_for_test(),
            db: db.clone(),
            email: std::sync::Arc::new(crate::email::EmailService::test_capture()),
        });
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
        assert!(body.contains("New orders are temporarily paused"));
        assert!(body.contains("transitioning ownership"));
        assert!(body.contains("Items in your cart"));
        assert!(body.contains("Items in your cart are not reserved"));
        assert!(body.contains("Dune"));
        assert!(body.contains("No payment is collected or order placed"));
        assert!(!body.contains("name=\"payment\""));
        assert!(body.contains(
            r#"<a class="brand checkout-brand" href="/" aria-label="Chantel&#x27;s Corner home">"#
        ));
        assert!(!body.contains("All Genres"));
        assert!(!body.contains("Best Sellers"));
        assert!(!body.contains("checkout-cart-link"));
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
    #[tokio::test]
    async fn committed_signup_wakes_idle_email_worker() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let email = std::sync::Arc::new(crate::email::EmailService::test_capture());
        let app = build_router(AppState {
            db: db.clone(),
            email: email.clone(),
            usage: crate::usage::UsageMonitor::healthy_for_test(),
        });
        let worker = email.spawn_worker(db.clone());
        // Let the empty startup sweep enter its long idle wait.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let (cookie, csrf) = csrf_form(&app, "/signup", None).await;
        let response = app.oneshot(Request::builder().method("POST").uri("/signup")
            .header(header::COOKIE,cookie)
            .header(header::ORIGIN,std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "https://www.chantelscorner.com".into()))
            .header(header::CONTENT_TYPE,"application/x-www-form-urlencoded")
            .body(Body::from(format!("first_name=Idle&last_name=Worker&email=idle%40example.com&password=UniqueHorseBookstore27%21&password_confirm=UniqueHorseBookstore27%21&csrf={csrf}"))).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let sent: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM email_outbox WHERE recipient='idle@example.com' AND status='accepted')").fetch_one(&db).await.unwrap();
                if sent { break; }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await;
        worker.abort();
        assert!(
            result.is_ok(),
            "committed mail was not delivered promptly after idle"
        );
    }

    #[tokio::test]
    async fn spending_alert_refresh_and_private_metrics() {
        let test_db = postgres_test_db().await;
        let usage = crate::usage::UsageMonitor::healthy_for_test();
        let app = build_router(AppState {
            db: test_db.pool(),
            usage: usage.clone(),
            email: std::sync::Arc::new(crate::email::EmailService::test_capture()),
        });
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(!response_body(response)
            .await
            .contains("Neon spending alert"));
        usage.set_cost_for_test(2., 0.51);
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(
            body.contains("Neon spending alert")
                && body.contains("$0.51 today")
                && body.contains("role=\"alert\"")
        );
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/ops/usage")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ops/usage")
                    .header(
                        header::AUTHORIZATION,
                        "Bearer test-operations-token-at-least-32-characters",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_body(response).await;
        assert!(body.contains("estimated_day_usd") && body.contains("cache_hits"));
        assert!(!body.contains("test-operations-token"));
    }

    #[tokio::test]
    async fn large_catalog_reads_are_bounded_and_warm_requests_reuse_cache() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        sqlx::query("INSERT INTO books(id,slug,title,description,search_text,primary_genre_id,is_new_arrival) SELECT 'budget-'||n,'budget-'||n,'Budget book '||lpad(n::text,5,'0'),repeat('long description ',500),'budget book', (SELECT id FROM genres ORDER BY id LIMIT 1),true FROM generate_series(1,10001) n")
            .execute(&db).await.unwrap();
        sqlx::query("INSERT INTO book_copies(book_id,condition,price,format,stock) SELECT id,'Good',5,'Paperback',1 FROM books WHERE id LIKE 'budget-%'")
            .execute(&db).await.unwrap();
        let filters = crate::models::CatalogFilters::default();
        let all_count = crate::store::count_books(&db, &filters).await.unwrap();
        assert!(all_count > 10000);
        let page = crate::store::list_books(&db, &filters).await.unwrap();
        assert_eq!(page.len(), 24);
        assert!(page.iter().all(|b| b.description.is_empty()));
        let bad = crate::models::CatalogFilters {
            per_page: Some(10000),
            ..Default::default()
        };
        assert!(crate::store::list_books(&db, &bad).await.is_err());
        assert!(crate::store::collection_books(&db, "best-sellers", -1)
            .await
            .is_err());
        assert!(
            crate::store::books_by_copy_ids(&db, &(1..=101).collect::<Vec<_>>())
                .await
                .is_err()
        );
        for path in [
            "/",
            "/search?per_page=96",
            "/books/budget-1",
            "/cart",
            "/login",
        ] {
            let app = test_app(db.clone());
            let mut rows = Vec::new();
            for _ in 0..2 {
                let response = app
                    .clone()
                    .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK, "{path}");
                rows.push(
                    response.headers()["x-test-read-rows"]
                        .to_str()
                        .unwrap()
                        .parse::<usize>()
                        .unwrap(),
                );
                let body = response_body(response).await;
                if path.starts_with("/search") {
                    assert!(body.contains(&all_count.to_string()), "total missing");
                }
            }
            assert!(rows[0] <= 200, "{path}: {rows:?}");
            assert_eq!(
                rows[1], 2,
                "warm {path} queried PostgreSQL beyond the reserved session allowance"
            );
            println!(
                "cache evidence: {path}, cold rows={}, warm rows={}",
                rows[0], rows[1]
            );
        }
    }

    #[tokio::test]
    async fn large_baskets_and_variants_are_paged_without_losing_totals() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        sqlx::query("INSERT INTO books(id,slug,title) SELECT 'page-'||n,'page-'||n,'Page book '||lpad(n::text,3,'0') FROM generate_series(1,45)n").execute(&db).await.unwrap();
        sqlx::query("INSERT INTO book_copies(book_id,price,stock,format) SELECT id,5,1,'Paperback' FROM books WHERE id LIKE 'page-%'").execute(&db).await.unwrap();
        let key = uuid::Uuid::new_v4().to_string();
        let cart_id: i64 = sqlx::query_scalar(
            "INSERT INTO carts(session_key,status) VALUES($1,'active') RETURNING id",
        )
        .bind(&key)
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO cart_items(cart_id,copy_id,quantity) SELECT $1,id,1 FROM book_copies WHERE book_id LIKE 'page-%'").bind(cart_id).execute(&db).await.unwrap();
        sqlx::query("INSERT INTO saved_items(session_key,copy_id,quantity) SELECT $1,id,1 FROM book_copies WHERE book_id LIKE 'page-%'").bind(&key).execute(&db).await.unwrap();
        sqlx::query("INSERT INTO book_copies(book_id,price,stock,format) SELECT 'page-1',100+n,1,'Option '||n FROM generate_series(1,25)n").execute(&db).await.unwrap();
        sqlx::query("INSERT INTO variant_attributes(variant_id,name,value) SELECT c.id,'attribute-'||n,'value-'||n FROM book_copies c CROSS JOIN generate_series(1,7)n WHERE c.book_id='page-1'").execute(&db).await.unwrap();
        let app = test_app(db.clone());
        for page in 1..=3 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/cart?cart_page={page}&saved_page={page}"))
                        .header(header::COOKIE, format!("chantels_cart_key={key}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let rows: usize = response.headers()["x-test-read-rows"]
                .to_str()
                .unwrap()
                .parse()
                .unwrap();
            assert!(rows <= 200);
            let body = response_body(response).await;
            assert!(
                body.contains("$225.00"),
                "full basket subtotal missing on page {page}"
            );
            assert!(body.contains(&format!("Page {page} of 3")));
            assert!(body.contains("45 items"));
        }
        for path in [
            "/books/page-1",
            "/books/page-1?copy_page=2",
            "/books/page-1?attribute_page=2",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            let body = response_body(response).await;
            assert!(body.contains("Page "), "continuation missing {path}");
        }
        // Cart calculations use live offers, even after a detail snapshot is cached.
        sqlx::query(
            "UPDATE book_copies SET price=6 WHERE book_id LIKE 'page-%' AND format='Paperback'",
        )
        .execute(&db)
        .await
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/cart")
                    .header(header::COOKIE, format!("chantels_cart_key={key}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response_body(response).await.contains("$270.00"));
    }

    #[test]
    fn runtime_queries_require_explicit_read_guards() {
        for (name, source) in [
            ("store", include_str!("store.rs")),
            ("cart", include_str!("cart.rs")),
            ("auth", include_str!("auth.rs")),
            ("access", include_str!("access.rs")),
            ("admin", include_str!("admin.rs")),
            ("handlers", include_str!("handlers.rs")),
            ("reviews", include_str!("reviews.rs")),
            ("review_web", include_str!("review_web.rs")),
            ("account_email", include_str!("account_email.rs")),
            ("email", include_str!("email/mod.rs")),
        ] {
            let source = source.split("#[cfg(test)]\nmod ").next().unwrap();
            for (needle, guard) in [
                (".fetch_all(", ".bounded_rows("),
                (".fetch_one(", ".bounded_one()"),
                (".fetch_optional(", ".bounded_one()"),
            ] {
                for tail in source.split(needle).skip(1) {
                    assert!(
                        tail.split(".await").next().unwrap().contains(guard),
                        "unguarded {needle} in {name}"
                    );
                }
            }
            assert!(
                !source.contains(".fetch("),
                "streamed reads require explicit review: {name}"
            );
            assert!(
                !source.contains("sqlx::raw_sql"),
                "raw SQL requires explicit review: {name}"
            );
        }
    }
    async fn review_account(app: &Router, email: &str) -> (String, String) {
        let (cookie, token) = csrf_form(app, "/signup", None).await;
        let response=post_form(app,"/signup",&cookie,format!("first_name=Reader&last_name=Test&email={email}&password=UniqueHorseBookstore27%21&password_confirm=UniqueHorseBookstore27%21&csrf={token}")).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let cookie = session_cookie(&response);
        csrf_form(app, "/books/b003/reviews/edit", Some(&cookie)).await
    }
    async fn review_get(app: &Router, path: &str, cookie: &str) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }
    async fn catalog_fingerprint(db: &DbPool) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for table in [
            "books",
            "authors",
            "book_authors",
            "genres",
            "book_genres",
            "book_copies",
            "variant_attributes",
            "book_collections",
            "book_collection_items",
            "book_subjects",
        ] {
            let digest:String=sqlx::query_scalar(&format!("SELECT md5(COALESCE(string_agg(row_to_json(t)::text,'' ORDER BY row_to_json(t)::text),'')) FROM {table} t")).fetch_one(db).await.unwrap();
            result.push((table.into(), digest));
        }
        result
    }
    #[tokio::test]
    async fn review_expired_session_preserves_draft_in_html_without_saving() {
        let test_db = postgres_test_db().await;
        let (app, db) = test_app_with_db(test_db.pool());
        let (cookie, csrf) = review_account(&app, "expired-review%40example.com").await;
        sqlx::query(
            "UPDATE users SET auth_version=auth_version+1 WHERE email='expired-review@example.com'",
        )
        .execute(&db)
        .await
        .unwrap();
        let response = post_form(
            &app,
            "/books/b009/reviews",
            &cookie,
            format!("rating=4&title=Keep+my+title&body=Keep+my+review+text&revision=0&csrf={csrf}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = response_body(response).await;
        assert!(body.contains("<!doctype html>"));
        assert!(body.contains("Keep my title"));
        assert!(body.contains("Keep my review text"));
        assert!(body.contains("Sign in"));
        assert!(body.contains("Your review has not been submitted"));
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM reviews")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let response = post_form(
            &app,
            "/books/b009/reviews",
            &cookie,
            "rating=4&title=Still+here&body=Retained+after+csrf+expiry&revision=0&csrf=expired"
                .into(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response_body(response)
            .await
            .contains("Retained after csrf expiry"));
    }

    #[tokio::test]
    async fn review_http_lifecycle_preserves_catalog_and_enforces_ownership() {
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        let (app, db) = test_app_with_db(db);
        let before = catalog_fingerprint(&db).await;
        let (a, csrf_a) = review_account(&app, "review-a%40example.com").await;
        let (b, csrf_b) = review_account(&app, "review-b%40example.com").await;
        let (staff, csrf_staff) = review_account(&app, "review-staff%40example.com").await;
        sqlx::query("INSERT INTO user_staff_roles(user_id,role) SELECT id,'moderator' FROM users WHERE email='review-staff@example.com'").execute(&db).await.unwrap();
        let bad = post_form(
            &app,
            "/books/b003/reviews",
            &a,
            format!("rating=7&title=Retain+this&body=Retain+body&revision=0&csrf={csrf_a}"),
        )
        .await;
        assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
        assert!(response_body(bad).await.contains("Retain body"));
        let no_csrf = post_form(
            &app,
            "/books/b003/reviews",
            &a,
            "rating=5&title=x&body=x&revision=0&csrf=bad".into(),
        )
        .await;
        assert_eq!(no_csrf.status(), StatusCode::FORBIDDEN);
        let submitted=post_form(&app,"/books/b003/reviews",&a,format!("rating=5&title=Great+book&body=%3Cscript%3Ealert%281%29%3C%2Fscript%3E&revision=0&user_id=forged-author&csrf={csrf_a}")).await;
        assert_eq!(submitted.status(), StatusCode::SEE_OTHER);
        let (id, v1): (i64, i64) = sqlx::query_as("SELECT id,current_revision FROM reviews")
            .fetch_one(&db)
            .await
            .unwrap();
        let pending = response_body(review_get(&app, "/account/reviews", &a).await).await;
        assert!(pending.contains("Awaiting moderation"));
        assert!(pending.contains("&lt;script&gt;"));
        let other = response_body(review_get(&app, "/account/reviews", &b).await).await;
        assert!(!other.contains("Great book"));
        let public = response_body(review_get(&app, "/books/b003", "").await).await;
        assert!(!public.contains("Great book"));
        assert!(public.contains("No reviews yet"));
        assert_eq!(
            review_get(&app, "/staff/reviews", &b).await.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post_form(
                &app,
                &format!("/reviews/{id}/delete"),
                &b,
                format!("revision={v1}&csrf={csrf_b}")
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post_form(
                &app,
                &format!("/staff/reviews/{id}"),
                &b,
                format!("revision={v1}&action=approve&csrf={csrf_b}")
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post_form(
                &app,
                &format!("/staff/reviews/{id}"),
                &staff,
                format!("revision={v1}&action=approve&csrf={csrf_staff}")
            )
            .await
            .status(),
            StatusCode::SEE_OTHER
        );
        let public = response_body(review_get(&app, "/books/b003", "").await).await;
        assert!(public.contains("Great book"));
        assert!(public.contains("5.0 out of 5"));
        assert!(!public.contains("<script>alert(1)</script>"));
        // Repeated votes do not inflate totals; authors cannot vote on themselves.
        for _ in 0..2 {
            assert_eq!(
                post_form(
                    &app,
                    &format!("/reviews/{id}/vote"),
                    &b,
                    format!("revision={v1}&vote=1&csrf={csrf_b}")
                )
                .await
                .status(),
                StatusCode::SEE_OTHER
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT helpful_count FROM reviews WHERE id=$1")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            post_form(
                &app,
                &format!("/reviews/{id}/vote"),
                &a,
                format!("revision={v1}&vote=1&csrf={csrf_a}")
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            post_form(
                &app,
                &format!("/reviews/{id}/report"),
                &b,
                format!("revision={v1}&reason=Contains+unsafe+content&csrf={csrf_b}")
            )
            .await
            .status(),
            StatusCode::SEE_OTHER
        );
        let reports =
            response_body(review_get(&app, "/staff/reviews?filter=reported", &staff).await).await;
        assert!(reports.contains("Great book"));
        let detail =
            response_body(review_get(&app, &format!("/staff/reviews/{id}"), &staff).await).await;
        assert!(detail.contains("Contains unsafe content"));
        assert_eq!(post_form(&app,"/books/b003/reviews",&a,format!("rating=2&title=Changed+opinion&body=Updated+review&revision={v1}&csrf={csrf_a}")).await.status(),StatusCode::SEE_OTHER);
        let v2: i64 = sqlx::query_scalar("SELECT current_revision FROM reviews WHERE id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_ne!(v1, v2);
        let still_public = response_body(review_get(&app, "/books/b003", "").await).await;
        assert!(still_public.contains("Great book"));
        assert!(!still_public.contains("Changed opinion"));
        assert_eq!(
            post_form(
                &app,
                &format!("/staff/reviews/{id}"),
                &staff,
                format!("revision={v1}&action=approve&csrf={csrf_staff}")
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            post_form(
                &app,
                &format!("/staff/reviews/{id}"),
                &staff,
                format!("revision={v2}&action=reject&reason=Unrelated+content&csrf={csrf_staff}")
            )
            .await
            .status(),
            StatusCode::SEE_OTHER
        );
        let mine = response_body(review_get(&app, "/account/reviews", &a).await).await;
        assert!(mine.contains("Unrelated content"));
        assert!(mine.contains("Changes rejected"));
        assert_eq!(
            post_form(
                &app,
                "/books/b003/reviews",
                &a,
                format!(
                    "rating=2&title=Changed+opinion&body=Now+relevant&revision={v2}&csrf={csrf_a}"
                )
            )
            .await
            .status(),
            StatusCode::SEE_OTHER
        );
        let v3: i64 = sqlx::query_scalar("SELECT current_revision FROM reviews WHERE id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(
            post_form(
                &app,
                &format!("/staff/reviews/{id}"),
                &staff,
                format!("revision={v3}&action=approve&csrf={csrf_staff}")
            )
            .await
            .status(),
            StatusCode::SEE_OTHER
        );
        let public = response_body(review_get(&app, "/books/b003", "").await).await;
        assert!(public.contains("2.0 out of 5"));
        assert!(public.contains("Changed opinion"));
        assert!(!public.contains("Great book"));
        assert_eq!(
            post_form(
                &app,
                &format!("/staff/reviews/{id}"),
                &staff,
                format!("revision={v3}&action=remove&reason=Policy+violation&csrf={csrf_staff}")
            )
            .await
            .status(),
            StatusCode::SEE_OTHER
        );
        let public = response_body(review_get(&app, "/books/b003", "").await).await;
        assert!(public.contains("No reviews yet"));
        assert!(!public.contains("Changed opinion"));
        assert_eq!(
            post_form(
                &app,
                &format!("/reviews/{id}/delete"),
                &a,
                format!("revision={v3}&csrf={csrf_a}")
            )
            .await
            .status(),
            StatusCode::SEE_OTHER
        );
        let mine = response_body(review_get(&app, "/account/reviews", &a).await).await;
        assert!(!mine.contains("Changed opinion"));
        assert_eq!(
            post_form(
                &app,
                &format!("/staff/reviews/{id}"),
                &staff,
                format!("revision={v3}&action=approve&csrf={csrf_staff}")
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        let unauth = review_get(&app, "/books/b003/reviews/edit", "").await;
        assert_eq!(unauth.status(), StatusCode::SEE_OTHER);
        let anon_post = post_form(
            &app,
            "/books/b003/reviews",
            "",
            format!("rating=5&title=x&body=x&revision=0&csrf={csrf_a}"),
        )
        .await;
        assert_eq!(anon_post.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            review_get(&app, "/checkout", "").await.status(),
            StatusCode::OK
        );
        assert_eq!(
            post_form(&app, "/checkout", &a, "".into()).await.status(),
            StatusCode::CONFLICT
        );
        let (anon_cookie, anon_csrf) = csrf_form(&app, "/signup", None).await;
        assert_eq!(
            post_form(
                &app,
                "/books/b003/reviews",
                &anon_cookie,
                format!("rating=5&title=x&body=x&revision=0&csrf={anon_csrf}")
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        let wiped: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM review_revisions WHERE review_id=$1 AND (title<>'' OR body<>'')",
        )
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(wiped, 0, "deleted author text retained");
        let author: String = sqlx::query_scalar("SELECT user_id FROM reviews WHERE id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO user_staff_roles(user_id,role) VALUES($1,'moderator')")
            .bind(&author)
            .execute(&db)
            .await
            .unwrap();
        let key = crate::reviews::work(&db, "b003").await.unwrap().work_key;
        let input = crate::reviews::Input {
            rating: 3,
            title: "New submission".into(),
            body: String::new(),
            revision: v3,
        };
        assert_eq!(
            crate::reviews::submit(&db, &test_principal(&author), "b003", &input)
                .await
                .unwrap(),
            id
        );
        let v4 = crate::reviews::own(&db, &key, &author)
            .await
            .unwrap()
            .unwrap()
            .current_revision
            .unwrap();
        assert!(
            crate::reviews::moderate(&db, &test_principal(&author), id, v4, "approve", "")
                .await
                .is_err(),
            "moderator approved own review"
        );
        assert!(crate::reviews::moderate(
            &db,
            &test_principal("forged-author"),
            id,
            v4,
            "approve",
            ""
        )
        .await
        .is_err());
        let input = crate::reviews::Input {
            revision: v4,
            ..input
        };
        crate::reviews::submit(&db, &test_principal(&author), "b003", &input)
            .await
            .unwrap();
        let current = crate::reviews::own(&db, &key, &author)
            .await
            .unwrap()
            .unwrap()
            .current_revision
            .unwrap();
        assert!(matches!(
            crate::reviews::submit(
                &db,
                &test_principal(&author),
                "b003",
                &crate::reviews::Input {
                    revision: current,
                    ..input
                }
            )
            .await,
            Err(crate::errors::AppError::Http(
                StatusCode::TOO_MANY_REQUESTS,
                _
            ))
        ));
        sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id=$1")
            .bind(&author)
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(
            post_form(
                &app,
                "/books/b003/reviews",
                &a,
                format!("rating=5&title=x&body=x&revision={current}&csrf={csrf_a}")
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            before,
            catalog_fingerprint(&db).await,
            "review/checkout flows changed catalog records"
        );
    }
    #[tokio::test]
    async fn review_editions_concurrency_pagination_and_database_constraints() {
        use crate::reviews;
        let test_db = postgres_test_db().await;
        let db = test_db.pool();
        // Only synthetic fixtures in this isolated local schema; no deployed catalog changes.
        sqlx::query("INSERT INTO books(id,slug,title,primary_author_id,isbn) SELECT 'review-edition','review-edition',upper(title)||'  ',primary_author_id,'TEST-EDITION' FROM books WHERE id='b003'").execute(&db).await.unwrap();
        let work = reviews::work(&db, "b003").await.unwrap();
        assert_eq!(
            work.work_key,
            reviews::work(&db, "review-edition").await.unwrap().work_key
        );
        sqlx::query("INSERT INTO users(id,email) VALUES('moderator','mod@example.com')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO user_staff_roles(user_id,role) VALUES('moderator','moderator')")
            .execute(&db)
            .await
            .unwrap();
        let mut handles = vec![];
        for n in 0..22 {
            let user = format!("reviewer-{n}");
            sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
                .bind(&user)
                .bind(format!("{user}@example.com"))
                .execute(&db)
                .await
                .unwrap();
            let input = reviews::Input {
                rating: if n % 2 == 0 { 5 } else { 1 },
                title: format!("Review {n}"),
                body: String::new(),
                revision: 0,
            };
            let id = reviews::submit(&db, &test_principal(&user), "b003", &input)
                .await
                .unwrap();
            let r = reviews::own(&db, &work.work_key, &user)
                .await
                .unwrap()
                .unwrap();
            let rev = r.current_revision.unwrap();
            let db = db.clone();
            handles.push(tokio::spawn(async move {
                reviews::moderate(&db, &test_principal("moderator"), id, rev, "approve", "")
                    .await
                    .unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let summary = reviews::summary(&db, &work.work_key).await.unwrap();
        assert_eq!(summary.published_count, 22);
        assert_eq!(summary.average_rating, 3.);
        assert_eq!(summary.star_1_count, 11);
        assert_eq!(summary.star_5_count, 11);
        assert_eq!(
            reviews::public_list(&db, &work.work_key, 1, "recent")
                .await
                .unwrap()
                .len(),
            20
        );
        assert_eq!(
            reviews::public_list(&db, &work.work_key, 2, "recent")
                .await
                .unwrap()
                .len(),
            2
        );
        let same = reviews::own(&db, &work.work_key, "reviewer-0")
            .await
            .unwrap()
            .unwrap();
        let input = reviews::Input {
            rating: 4,
            title: "Other edition edit".into(),
            body: String::new(),
            revision: same.current_revision.unwrap(),
        };
        let reviewer_zero = test_principal("reviewer-0");
        let (first, second) = tokio::join!(
            reviews::submit(&db, &reviewer_zero, "review-edition", &input),
            reviews::submit(&db, &reviewer_zero, "b003", &input)
        );
        assert_eq!(
            usize::from(first.is_ok()) + usize::from(second.is_ok()),
            1,
            "stale concurrent edit accepted"
        );
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM reviews WHERE user_id='reviewer-0'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(count, 1);
        assert!(sqlx::query("INSERT INTO reviews(book_id,user_id,work_key,rating) VALUES('b003','nonexistent',$1,5)").bind(&work.work_key).execute(&db).await.is_err());
        assert!(sqlx::query(
            "INSERT INTO review_revisions(review_id,rating,title,body) VALUES($1,6,'','')"
        )
        .bind(same.id)
        .execute(&db)
        .await
        .is_err());
        let initial = reviews::own(&db, &work.work_key, "reviewer-1")
            .await
            .unwrap()
            .unwrap();
        assert!(reviews::moderate(
            &db,
            &test_principal("reviewer-1"),
            initial.id,
            initial.current_revision.unwrap(),
            "approve",
            ""
        )
        .await
        .is_err());
        // Unsupported physical deletion is blocked; disabling preserves public reviews.
        assert!(sqlx::query("DELETE FROM users WHERE id='reviewer-1'")
            .execute(&db)
            .await
            .is_err());
        sqlx::query("UPDATE users SET account_status='disabled',auth_version=auth_version+1 WHERE id='reviewer-1'").execute(&db).await.unwrap();
        assert_eq!(
            reviews::summary(&db, &work.work_key)
                .await
                .unwrap()
                .published_count,
            22
        );
        crate::reviews::vote(
            &db,
            &test_principal("reviewer-2"),
            same.id,
            same.published_revision.unwrap(),
            1,
        )
        .await
        .unwrap();
        crate::reviews::vote(
            &db,
            &test_principal("reviewer-2"),
            same.id,
            same.published_revision.unwrap(),
            0,
        )
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT helpful_count FROM reviews WHERE id=$1")
                .bind(same.id)
                .fetch_one(&db)
                .await
                .unwrap(),
            0,
            "voter deletion left stale helpful count"
        );
        let min = crate::models::CatalogFilters {
            min_rating: Some("4".into()),
            ..Default::default()
        };
        let matches = crate::store::list_books(&db, &min).await.unwrap();
        assert!(
            matches.is_empty(),
            "unrated books or 3-star titles matched 4+ stars"
        );
    }
    fn admin_action(body: &str) -> crate::admin::Action {
        serde_urlencoded::from_str(&format!("csrf=test&{body}")).unwrap()
    }
    async fn admin_fixture(db: &DbPool) {
        sqlx::query("INSERT INTO users(id,email) VALUES('owner','owner@example.com'),('owner2','owner2@example.com'),('support','support@example.com'),('customer','customer@example.com'),('staff','staff@example.com')").execute(db).await.unwrap();
        sqlx::query("INSERT INTO user_staff_roles(user_id,role) VALUES('owner','administrator'),('owner','support'),('owner2','administrator'),('owner2','support'),('support','support'),('staff','moderator')").execute(db).await.unwrap();
    }
    #[tokio::test]
    async fn admin_permissions_profiles_audit_and_last_owner_are_atomic() {
        let test = postgres_test_db().await;
        let db = test.pool();
        admin_fixture(&db).await;
        let before = catalog_fingerprint(&db).await;
        let owner = test_principal("owner");
        let support = test_principal("support");
        let disable = admin_action("action=status&status=disabled&reason=Support+request");
        assert!(crate::admin::mutate(&db, &support, "staff", &disable)
            .await
            .is_err());
        assert!(crate::admin::mutate(&db, &support, "customer", &disable)
            .await
            .is_ok());
        assert!(
            !crate::access::has_permission(&db, "customer", "reviews.moderate")
                .await
                .unwrap()
        );
        assert!(crate::admin::mutate(
            &db,
            &owner,
            "owner",
            &admin_action("action=grant&role=moderator&reason=Self+grant")
        )
        .await
        .is_err());
        let edit=admin_action("action=profile&first_name=Corrected&last_name=Reader&address_city=Pasadena&version=0&reason=Customer+correction");
        crate::admin::mutate(&db, &support, "customer", &edit)
            .await
            .unwrap();
        assert!(crate::admin::mutate(&db, &support, "customer", &edit)
            .await
            .is_err());
        crate::auth::update_preferences(&db, "customer", true, 1)
            .await
            .unwrap_err(); // disabled account
        let enable = admin_action("action=status&status=active&reason=Customer+restored");
        crate::admin::mutate(&db, &support, "customer", &enable)
            .await
            .unwrap();
        crate::auth::update_preferences(&db, "customer", true, 2)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT first_name FROM users WHERE id='customer'")
                .fetch_one(&db)
                .await
                .unwrap(),
            "Corrected"
        );
        crate::admin::mutate(&db, &owner, "owner2", &disable)
            .await
            .unwrap();
        assert!(crate::admin::mutate(&db, &owner, "owner", &disable)
            .await
            .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT account_status FROM users WHERE id='owner'")
                .fetch_one(&db)
                .await
                .unwrap(),
            "active"
        );
        let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_audit")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(audit, 4);
        assert!(sqlx::query("DELETE FROM users WHERE id='customer'")
            .execute(&db)
            .await
            .is_err());
        assert_eq!(before, catalog_fingerprint(&db).await);
    }
    #[tokio::test]
    async fn admin_concurrent_owner_removal_preserves_combined_capability() {
        let test = postgres_test_db().await;
        let db = test.pool();
        admin_fixture(&db).await;
        let a = test_principal("owner");
        let b = test_principal("owner2");
        let f = admin_action("action=revoke&role=support&reason=Role+removal");
        let (x, y) = tokio::join!(
            crate::admin::mutate(&db, &a, "owner2", &f),
            crate::admin::mutate(&db, &b, "owner", &f)
        );
        assert_eq!(usize::from(x.is_ok()) + usize::from(y.is_ok()), 1);
        let capable:i64=sqlx::query_scalar("SELECT count(*) FROM users u WHERE account_status='active' AND EXISTS(SELECT 1 FROM user_staff_roles WHERE user_id=u.id AND role='administrator') AND EXISTS(SELECT 1 FROM user_staff_roles WHERE user_id=u.id AND role='support')").fetch_one(&db).await.unwrap();
        assert_eq!(capable, 1);
        // Independent administrator stays authorized across both removals;
        // this proves the invariant itself, not incidental actor-session revocation.
        sqlx::query("INSERT INTO user_staff_roles(user_id,role) VALUES('owner','support'),('owner2','support'),('staff','administrator') ON CONFLICT DO NOTHING").execute(&db).await.unwrap();
        let independent = test_principal("staff");
        let (x, y) = tokio::join!(
            crate::admin::mutate(&db, &independent, "owner", &f),
            crate::admin::mutate(&db, &independent, "owner2", &f)
        );
        assert_eq!(usize::from(x.is_ok()) + usize::from(y.is_ok()), 1);
        let failure = if let Err(e) = x { e } else { y.unwrap_err() };
        assert!(failure
            .to_string()
            .contains("Keep at least one active administrator"));
    }
    #[tokio::test]
    async fn admin_operator_idempotency_and_security_epoch() {
        let test = postgres_test_db().await;
        let db = test.pool();
        admin_fixture(&db).await;
        crate::admin::operator(
            &db,
            "customer@example.com",
            "--bootstrap-admin",
            "local-test",
            "Recovery setup",
        )
        .await
        .unwrap();
        crate::admin::operator(
            &db,
            "customer@example.com",
            "--bootstrap-admin",
            "local-test",
            "Recovery setup",
        )
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT auth_version FROM users WHERE id='customer'")
                .fetch_one(&db)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM admin_audit WHERE target_id='customer'"
            )
            .fetch_one(&db)
            .await
            .unwrap(),
            1
        );
        assert!(crate::auth::update_preferences(&db, "customer", true, 0)
            .await
            .is_err());
        let mail = crate::email::EmailService::test_capture();
        let mut tx = db.begin().await.unwrap();
        assert!(crate::account_email::issue(
            &mut tx,
            &mail,
            "customer",
            "reset_password",
            "customer@example.com",
            0
        )
        .await
        .is_err());
        tx.rollback().await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM account_tokens WHERE user_id='customer'"
            )
            .fetch_one(&db)
            .await
            .unwrap(),
            0
        );
    }

    async fn wait_account_lock(db: &DbPool, needle: &str) {
        tokio::time::timeout(std::time::Duration::from_secs(5),async{
            loop{let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock' AND query LIKE $1)").bind(format!("%{needle}%")).fetch_one(db).await.unwrap();if blocked{break}tokio::time::sleep(std::time::Duration::from_millis(10)).await;}
        }).await.expect("account mutation lock barrier");
    }
    #[tokio::test]
    async fn account_revocation_defeats_inflight_profile_preferences_and_email() {
        let test = postgres_test_db().await;
        let db = test.pool();
        admin_fixture(&db).await;
        for path in ["preferences", "profile", "issue"] {
            let epoch: i64 =
                sqlx::query_scalar("SELECT auth_version FROM users WHERE id='customer'")
                    .fetch_one(&db)
                    .await
                    .unwrap();
            let mut revoke = db.begin().await.unwrap();
            sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id='customer'")
                .execute(&mut *revoke)
                .await
                .unwrap();
            let worker_db = db.clone();
            let writer = tokio::spawn(async move {
                match path {
                    "preferences" => {
                        crate::auth::update_preferences(&worker_db, "customer", true, epoch)
                            .await
                            .is_ok()
                    }
                    "profile" => crate::auth::update_user_profile(
                        &worker_db,
                        "customer",
                        crate::auth::ProfileUpdate {
                            first_name: "Stale",
                            last_name: "User",
                            email: "customer@example.com",
                            phone_number: "",
                            address_line1: "",
                            address_line2: "",
                            address_city: "",
                            address_state: "",
                            address_postal_code: "",
                            marketing_opt_in: false,
                        },
                        epoch,
                        0,
                    )
                    .await
                    .is_ok(),
                    _ => {
                        let mail = crate::email::EmailService::test_capture();
                        let mut tx = worker_db.begin().await.unwrap();
                        let result = crate::account_email::issue(
                            &mut tx,
                            &mail,
                            "customer",
                            "reset_password",
                            "customer@example.com",
                            epoch,
                        )
                        .await;
                        let ok = result.is_ok();
                        if ok {
                            tx.commit().await.unwrap()
                        } else {
                            tx.rollback().await.unwrap()
                        }
                        ok
                    }
                }
            });
            wait_account_lock(&db, "FROM users WHERE id=").await;
            revoke.commit().await.unwrap();
            assert!(!writer.await.unwrap(), "stale mutation accepted: {path}");
        }
        let app = test_app(db.clone());
        let token = test_token(&db, "customer", "customer@example.com", "reset_password").await;
        let (cookie, csrf) = challenge_cookie(&app, "/reset-password", &token).await;
        let mut revoke = db.begin().await.unwrap();
        sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id='customer'")
            .execute(&mut *revoke)
            .await
            .unwrap();
        let writer = tokio::spawn(async move {
            post_form(&app,"/reset-password",&cookie,format!("csrf={csrf}&password=NewUniqueBookstorePassword27%21&password_confirm=NewUniqueBookstorePassword27%21")).await
        });
        wait_account_lock(&db, "SELECT id FROM users WHERE id=").await;
        revoke.commit().await.unwrap();
        assert!(response_body(writer.await.unwrap())
            .await
            .contains("invalid or expired"));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM account_tokens WHERE consumed_at IS NOT NULL"
            )
            .fetch_one(&db)
            .await
            .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn account_write_commits_before_waiting_revocation() {
        let test = postgres_test_db().await;
        let db = test.pool();
        admin_fixture(&db).await;
        // Local-only trigger creates a deterministic barrier after the account row lock.
        sqlx::query("CREATE FUNCTION test_profile_barrier() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(684201099); RETURN NEW; END $$").execute(&db).await.unwrap();
        sqlx::query("CREATE TRIGGER test_profile_barrier BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION test_profile_barrier()").execute(&db).await.unwrap();
        let mut gate = db.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(684201099)")
            .execute(&mut *gate)
            .await
            .unwrap();
        let writer_db = db.clone();
        let writer = tokio::spawn(async move {
            crate::auth::update_preferences(&writer_db, "customer", true, 0).await
        });
        wait_account_lock(&db, "UPDATE users SET marketing_opt_in=").await;
        let revoke_db = db.clone();
        let revoker = tokio::spawn(async move {
            sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id='customer'")
                .execute(&revoke_db)
                .await
        });
        wait_account_lock(&db, "UPDATE users SET auth_version=").await;
        gate.commit().await.unwrap();
        writer.await.unwrap().unwrap();
        revoker.await.unwrap().unwrap();
        assert_eq!(
            sqlx::query_as::<_, (bool, i64)>(
                "SELECT marketing_opt_in,auth_version FROM users WHERE id='customer'"
            )
            .fetch_one(&db)
            .await
            .unwrap(),
            (true, 1)
        );
    }
    #[tokio::test]
    async fn admin_migration_preserves_moderators_and_catalog() {
        let test = postgres_test_db().await;
        let db = test.pool();
        let schema = format!("admin_migration_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&db)
            .await
            .unwrap();
        let url = database_url_with_search_path(&std::env::var("DATABASE_URL").unwrap(), &schema);
        let old = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        for migration in MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < 20261014002000)
        {
            sqlx::raw_sql(&migration.sql).execute(&old).await.unwrap();
        }
        sqlx::query("INSERT INTO users(id,email) VALUES('legacy-mod','legacy@example.com')")
            .execute(&old)
            .await
            .unwrap();
        sqlx::query("INSERT INTO review_moderators(user_id) VALUES('legacy-mod')")
            .execute(&old)
            .await
            .unwrap();
        let before = catalog_fingerprint(&old).await;
        sqlx::raw_sql(include_str!(
            "../migrations_postgres/20261014002000_admin_access.sql"
        ))
        .execute(&old)
        .await
        .unwrap();
        assert!(
            crate::access::has_permission(&old, "legacy-mod", "reviews.moderate")
                .await
                .unwrap()
        );
        assert!(
            !crate::access::has_permission(&old, "legacy-mod", "users.manage")
                .await
                .unwrap()
        );
        assert_eq!(
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT to_regclass('review_moderators')::text"
            )
            .fetch_one(&old)
            .await
            .unwrap(),
            None
        );
        assert_eq!(before, catalog_fingerprint(&old).await);
        old.close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&db)
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn admin_audit_failure_rolls_back_account_change() {
        let test = postgres_test_db().await;
        let db = test.pool();
        admin_fixture(&db).await;
        sqlx::query("CREATE FUNCTION reject_test_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test audit failure'; END $$").execute(&db).await.unwrap();
        sqlx::query("CREATE TRIGGER reject_test_audit BEFORE INSERT ON admin_audit FOR EACH ROW EXECUTE FUNCTION reject_test_audit()").execute(&db).await.unwrap();
        assert!(crate::admin::mutate(
            &db,
            &test_principal("support"),
            "customer",
            &admin_action("action=sessions&reason=Security+revocation")
        )
        .await
        .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT auth_version FROM users WHERE id='customer'")
                .fetch_one(&db)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn admin_support_cannot_read_historical_staff_audit() {
        let test = postgres_test_db().await;
        let db = test.pool();
        admin_fixture(&db).await;
        let app = test_app(db.clone());
        let (cookie, _) = review_account(&app, "audit-support%40example.com").await;
        sqlx::query("INSERT INTO user_staff_roles(user_id,role) SELECT id,'support' FROM users WHERE email='audit-support@example.com'").execute(&db).await.unwrap();
        sqlx::query("INSERT INTO admin_audit(actor_id,target_id,action,reason) VALUES('owner','customer','revoke','CLASSIFIED_STAFF_HISTORY'),('owner','customer','profile','VISIBLE_ACCOUNT_HISTORY')").execute(&db).await.unwrap();
        let response = review_get(&app, "/staff/users/customer", &cookie).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.contains("VISIBLE_ACCOUNT_HISTORY"));
        assert!(!body.contains("CLASSIFIED_STAFF_HISTORY"));
    }
    #[tokio::test]
    async fn stale_customer_profile_cannot_overwrite_staff_correction() {
        let test = postgres_test_db().await;
        let db = test.pool();
        admin_fixture(&db).await;
        let app = test_app(db.clone());
        let (cookie, csrf) = review_account(&app, "profile-conflict%40example.com").await;
        let id: String =
            sqlx::query_scalar("SELECT id FROM users WHERE email='profile-conflict@example.com'")
                .fetch_one(&db)
                .await
                .unwrap();
        crate::admin::mutate(
            &db,
            &test_principal("owner"),
            &id,
            &admin_action(
                "action=profile&version=0&first_name=StaffCorrection&reason=Customer+correction",
            ),
        )
        .await
        .unwrap();
        let form=format!("csrf={csrf}&profile_version=0&first_name=OldCustomerForm&last_name=Reader&email=profile-conflict%40example.com&phone_number=&address_line1=&address_line2=&address_city=&address_state=&address_postal_code=");
        let response = post_form(&app, "/account/profile", &cookie, form).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response_body(response).await;
        assert!(body.contains("OldCustomerForm"));
        assert!(body.contains("Reload profile"));
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT first_name FROM users WHERE id=$1")
                .bind(&id)
                .fetch_one(&db)
                .await
                .unwrap(),
            "StaffCorrection"
        );
        let form=format!("csrf={csrf}&profile_version=1&first_name=NewCustomerForm&last_name=Reader&email=profile-conflict%40example.com&phone_number=&address_line1=&address_line2=&address_city=&address_state=&address_postal_code=");
        assert_eq!(
            post_form(&app, "/account/profile", &cookie, form)
                .await
                .status(),
            StatusCode::OK
        );
    }
}
