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
        .route("/healthz", get(handlers::healthz))
        .route("/readyz", get(handlers::readyz))
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
        .layer(axum::extract::DefaultBodyLimit::max(16384))
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
        crate::db::load_runtime_env();
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
            db,
            email: std::sync::Arc::new(crate::email::EmailService::test_capture()),
        })
    }

    fn test_app_with_db(db: DbPool) -> (Router, DbPool) {
        (
            build_router(AppState {
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
                        format!("first_name=Taylor&last_name=Reader&email=reader%40example.com&password=uniquehorsebookstore27&password_confirm=uniquehorsebookstore27&csrf={signup_csrf}"),
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
                        format!("first_name=Account&last_name=Reader&email=account%40example.com&password=uniquehorsebookstore27&password_confirm=uniquehorsebookstore27&csrf={signup_csrf}"),
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
            secrecy::Secret::new("bookstore-unique-old-password".into()),
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
        let login=post_form(&app,"/login",&login_cookie,format!("email=secure%40example.com&password=bookstore-unique-old-password&csrf={login_csrf}")).await;
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
        assert!(html.contains("Use at least 15 characters."));
        assert!(html.contains("action=\"/reset-password\""));
        assert!(!html.contains("Validation error:"));
        assert!(!html.contains("href=\"/forgot-password\""));
        assert!(!html.contains("href=\"/account/verification\""));
        let form1=format!("csrf={t1}&password=bookstore-unique-new-password&password_confirm=bookstore-unique-new-password");
        let form2=format!("csrf={t2}&password=bookstore-unique-new-password&password_confirm=bookstore-unique-new-password");
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
            secrecy::Secret::new("bookstore-unique-old-password".into()),
            &row.0
        )
        .await
        .unwrap());
        assert!(crate::auth::verify_password(
            secrecy::Secret::new("bookstore-unique-new-password".into()),
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
            secrecy::Secret::new("bookstore-unique-old-password".into()),
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
        let r=post_form(&app,"/reset-password",&c,format!("csrf={t}&password=bookstore-unique-new-password&password_confirm=bookstore-unique-new-password")).await;
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
            secrecy::Secret::new("bookstore-unique-old-password".into()),
        )
        .await
        .unwrap();
        let (c, t) = csrf_form(&app, "/login", None).await;
        let r = post_form(
            &app,
            "/login",
            &c,
            format!("csrf={t}&email=old%40example.com&password=bookstore-unique-old-password"),
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
            format!("csrf={t}&email=new%40example.com&password=bookstore-unique-old-password"),
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
            secrecy::Secret::new("bookstore-unique-old-password".into()),
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
            secrecy::Secret::new("bookstore-unique-old-password".into()),
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
            secrecy::Secret::new("bookstore-unique-old-password".into()),
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
        assert!(
            known_time.abs_diff(unknown_time) < std::time::Duration::from_millis(150),
            "timing: {known_time:?} vs {unknown_time:?}"
        );
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
        assert!(body.contains("Checkout Preview"));
        assert!(body.contains("Order placement unavailable"));
        assert!(body.contains("Items in your cart"));
        assert!(body.contains("Availability and pickup have not been confirmed"));
        assert!(body.contains("Dune"));
        assert!(body.contains("No order or payment is processed"));
        assert!(body.contains("disabled aria-disabled=\"true\""));
        assert!(body.contains(
            r#"<a class="brand checkout-brand" href="/" aria-label="Chantel&#x27;s Corner home">"#
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
