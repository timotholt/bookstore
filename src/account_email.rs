//! Account email challenges. GET only exchanges a bearer link for a server-side challenge.
use crate::read_budget::ReadFutureExt;
use crate::{
    app::AppState,
    email::{EmailKind, EmailService},
};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::net::SocketAddr;
use tower_sessions::Session;
use uuid::Uuid;

pub fn random_secret() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}
fn digest(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn origin() -> String {
    std::env::var("PUBLIC_BASE_URL")
        .unwrap_or_else(|_| "https://www.chantelscorner.com".into())
        .trim_end_matches('/')
        .to_owned()
}

pub async fn csrf(session: &Session) -> String {
    if let Ok(Some(s)) = session.get::<String>("csrf").await {
        return s;
    }
    let s = random_secret();
    let _ = session.insert("csrf", &s).await;
    s
}
pub async fn check_csrf(session: &Session, headers: &HeaderMap, supplied: &str) -> bool {
    let expected = session.get::<String>("csrf").await.ok().flatten();
    let same_origin = headers
        .get("origin")
        .and_then(|x| x.to_str().ok())
        .map(|v| v == origin())
        .unwrap_or(false);
    same_origin && expected.as_deref() == Some(supplied) && !supplied.is_empty()
}

/// Atomic buckets are shared by all application replicas. Never trust forwarded headers.
async fn limit(db: &PgPool, key: &str, count: i32, seconds: i64) -> Result<bool, sqlx::Error> {
    let n:i32=sqlx::query_scalar("INSERT INTO account_email_rate_limits(bucket,attempts,expires_at) VALUES($1,1,now()+make_interval(secs=>$2::double precision)) ON CONFLICT(bucket) DO UPDATE SET attempts=CASE WHEN account_email_rate_limits.expires_at<=now() THEN 1 ELSE account_email_rate_limits.attempts+1 END, expires_at=CASE WHEN account_email_rate_limits.expires_at<=now() THEN EXCLUDED.expires_at ELSE account_email_rate_limits.expires_at END RETURNING attempts")
        .bind(digest(key)).bind(seconds as f64).fetch_one(db).bounded_one().await?;
    sqlx::query("DELETE FROM account_email_rate_limits WHERE expires_at < now()-interval '1 day'")
        .execute(db)
        .await?;
    Ok(n <= count)
}
async fn issuance_limits(db: &PgPool, address: &str, purpose: &str) -> bool {
    for (seconds, count) in [(60, 1), (3600, 5), (86400, 10)] {
        if !limit(
            db,
            &format!("recipient:{purpose}:{seconds}:{address}"),
            count,
            seconds,
        )
        .await
        .unwrap_or(false)
        {
            return false;
        }
    }
    limit(db, "global:email", 80, 86400).await.unwrap_or(false)
}

pub async fn signup_allowed(db: &PgPool, email: &str, peer: Option<SocketAddr>) -> bool {
    let source = peer
        .map(|p| p.ip().to_string())
        .unwrap_or_else(|| "unknown".into());
    if !limit(db, &format!("source:{source}"), 20, 3600)
        .await
        .unwrap_or(false)
    {
        return false;
    }
    issuance_limits(db, email, "signup").await
}

pub async fn issue(
    tx: &mut Transaction<'_, Postgres>,
    mail: &EmailService,
    user: &str,
    purpose: &str,
    email: &str,
    version: i64,
) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
    let id = Uuid::new_v4();
    let token = random_secret();
    let (path, kind, hours) = match purpose {
        "verify_email" => ("verify-email", EmailKind::Verification, 24),
        "change_email" => ("confirm-email-change", EmailKind::EmailChange, 24),
        _ => ("reset-password", EmailKind::PasswordReset, 0),
    };
    let now: chrono::DateTime<Utc> = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut **tx)
        .bounded_one()
        .await?;
    let expires = now
        + if hours == 0 {
            Duration::minutes(30)
        } else {
            Duration::hours(hours)
        };
    if purpose != "reset_password" {
        sqlx::query("UPDATE account_tokens SET revoked_at=now() WHERE user_id=$1 AND purpose=$2 AND consumed_at IS NULL AND revoked_at IS NULL").bind(user).bind(purpose).execute(&mut **tx).await?;
    }
    sqlx::query("INSERT INTO account_tokens(id,user_id,purpose,token_hash,target_email,auth_version,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(id).bind(user).bind(purpose).bind(digest(&token)).bind(email).bind(version).bind(expires).execute(&mut **tx).await?;
    mail.enqueue(
        tx,
        kind,
        email,
        &format!("{}/{path}?token={token}", mail.base_url()),
        Some(id),
        expires,
    )
    .await?;
    Ok(id)
}

#[derive(Serialize, Deserialize)]
struct Challenge {
    hash: String,
    purpose: String,
    expires: i64,
}
#[derive(Deserialize)]
pub struct Landing {
    token: Option<String>,
}
#[derive(Deserialize)]
pub struct Action {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    password_confirm: String,
}

#[derive(askama::Template)]
#[template(path = "account_email.html")]
struct EmailPage {
    title: String,
    message: String,
    action: String,
    recovery_path: String,
    csrf: String,
    fields: Vec<Field>,
    button: crate::ui::ButtonView,
}
struct Field {
    name: &'static str,
    label: &'static str,
    kind: &'static str,
    autocomplete: &'static str,
}
fn field(
    name: &'static str,
    label: &'static str,
    kind: &'static str,
    autocomplete: &'static str,
) -> Field {
    Field {
        name,
        label,
        kind,
        autocomplete,
    }
}
fn secure(mut r: Response) -> Response {
    r.headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    r.headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    r
}
async fn page(
    session: &Session,
    title: &str,
    message: &str,
    action: &str,
    fields: Vec<Field>,
    button: &str,
) -> Response {
    let t = EmailPage {
        title: title.into(),
        message: message.into(),
        action: action.into(),
        recovery_path: String::new(),
        csrf: csrf(session).await,
        fields,
        button: crate::ui::ButtonView::form_submit(button),
    };
    render_page(t)
}
fn render_page(t: EmailPage) -> Response {
    use askama::Template;
    match t.render() {
        Ok(s) => {
            let mut response = secure(axum::response::Html(s).into_response());
            // Native form POSTs need a non-null Origin for the CSRF check.
            // strict-origin omits the path/query, including any bearer token.
            response
                .headers_mut()
                .insert("referrer-policy", "strict-origin".parse().unwrap());
            response
        }
        Err(_) => secure(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
}
async fn invalid_link(session: &Session, purpose: &str) -> Response {
    let recovery = if purpose == "reset_password" {
        "/forgot-password"
    } else {
        "/account/verification"
    };
    render_page(EmailPage {
        title: "Link expired or unavailable".into(),
        message: INVALID.into(),
        action: String::new(),
        recovery_path: recovery.into(),
        csrf: csrf(session).await,
        fields: vec![],
        button: crate::ui::ButtonView::form_submit(""),
    })
}
async fn message(session: &Session, text: &str) -> Response {
    page(session, "Account security", text, "", vec![], "").await
}
const INVALID: &str = "This link is invalid or expired. Request a new link and try again.";
const GENERIC: &str =
    "If an eligible account exists for that address, we’ll email password reset instructions.";

async fn reset_form(session: &Session, message: &str) -> Response {
    page(
        session,
        "Reset your password",
        message,
        "/reset-password",
        vec![
            field("password", "New password", "password", "new-password"),
            field(
                "password_confirm",
                "Confirm new password",
                "password",
                "new-password",
            ),
        ],
        "Reset password",
    )
    .await
}

async fn landing(session: Session, q: Landing, purpose: &str, path: &str) -> Response {
    if let Some(token) = q.token {
        if token.len() != 43
            || URL_SAFE_NO_PAD
                .decode(&token)
                .map(|x| x.len() != 32)
                .unwrap_or(true)
        {
            return invalid_link(&session, purpose).await;
        }
        let c = Challenge {
            hash: digest(&token),
            purpose: purpose.into(),
            expires: Utc::now().timestamp() + 1800,
        };
        if session
            .insert(&format!("challenge:{purpose}"), c)
            .await
            .is_err()
        {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        return secure(Redirect::to(path).into_response());
    }
    if purpose == "reset_password" {
        reset_form(
            &session,
            "Use 15–128 characters. A few unrelated words work well.",
        )
        .await
    } else {
        page(
            &session,
            "Confirm your email",
            "Confirm this email address for your Chantel’s Corner bookstore account.",
            path,
            vec![],
            "Confirm email",
        )
        .await
    }
}
pub async fn verify_get(session: Session, Query(q): Query<Landing>) -> Response {
    landing(session, q, "verify_email", "/verify-email").await
}
pub async fn reset_get(
    State(s): State<AppState>,
    session: Session,
    Query(q): Query<Landing>,
) -> Response {
    if q.token.is_none() {
        let challenge = session
            .get::<Challenge>("challenge:reset_password")
            .await
            .ok()
            .flatten();
        let valid = if let Some(c) = challenge {
            c.purpose == "reset_password"
                && c.expires > Utc::now().timestamp()
                && matches!(token(&s.db, &c.hash, "reset_password").await, Ok(Some(_)))
        } else {
            false
        };
        if !valid {
            return invalid_link(&session, "reset_password").await;
        }
    }
    landing(session, q, "reset_password", "/reset-password").await
}
pub async fn change_get(session: Session, Query(q): Query<Landing>) -> Response {
    landing(session, q, "change_email", "/confirm-email-change").await
}
pub async fn forgot_get(session: Session) -> Response {
    page(
        &session,
        "Forgot your password?",
        "Enter the email address for your Chantel’s Corner bookstore account.",
        "/forgot-password",
        vec![field("email", "Email address", "email", "email")],
        "Send reset instructions",
    )
    .await
}
pub async fn verification_get(State(s): State<AppState>, session: Session) -> Response {
    let Ok(Some(u)) = crate::auth::get_current_user(&s.db, &session).await else {
        return Redirect::to("/login").into_response();
    };
    let verified: bool =
        sqlx::query_scalar("SELECT email_verified_at IS NOT NULL FROM users WHERE id=$1")
            .bind(&u.id)
            .fetch_one(&s.db)
            .bounded_one()
            .await
            .unwrap_or(false);
    if verified {
        return message(&session, "Your email address is verified.").await;
    }
    page(&session,"Verify your email","Check your inbox to verify your bookstore account. Resending replaces earlier verification links.","/account/verification/resend",vec![],"Resend verification email").await
}
pub async fn email_change_get(State(s): State<AppState>, session: Session) -> Response {
    if !matches!(
        crate::auth::get_current_user(&s.db, &session).await,
        Ok(Some(_))
    ) {
        return Redirect::to("/login").into_response();
    }
    page(
        &session,
        "Change email",
        "Your current sign-in address remains active until you confirm the new address.",
        "/account/email-change",
        vec![
            field("email", "New email address", "email", "email"),
            field(
                "password",
                "Current password",
                "password",
                "current-password",
            ),
        ],
        "Confirm new address",
    )
    .await
}

pub async fn forgot_post(
    State(s): State<AppState>,
    session: Session,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Form(f): Form<Action>,
) -> Response {
    if !check_csrf(&session, &headers, &f.csrf).await {
        return secure(StatusCode::FORBIDDEN.into_response());
    }
    if !s.email.available() {
        return message(
            &session,
            "Email is temporarily unavailable. Please try again later.",
        )
        .await;
    }
    let started = tokio::time::Instant::now();
    let source = peer
        .map(|p| p.0.ip().to_string())
        .unwrap_or_else(|| "unknown".into());
    if !limit(&s.db, &format!("source:{source}"), 20, 3600)
        .await
        .unwrap_or(false)
    {
        tokio::time::sleep_until(started + std::time::Duration::from_secs(1)).await;
        return message(&session, GENERIC).await;
    }
    // Commit before acknowledging. The encrypted outbox survives process termination.
    let result=async {
        let Ok(email)=crate::auth::normalize_email(&f.email) else{return Ok(());};
        if !issuance_limits(&s.db,&email,"reset_password").await{return Ok(());}
        let mut tx=s.db.begin().await?;
        let row=sqlx::query("SELECT u.id,u.auth_version FROM users u JOIN password_credentials p ON p.user_id=u.id WHERE email=$1 FOR UPDATE OF u").bind(&email).fetch_optional(&mut *tx).bounded_one().await?;
        if let Some(row)=row {
            let id:String=row.get("id");let version:i64=row.get("auth_version");
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM account_tokens WHERE user_id=$1 AND purpose='reset_password' AND revoked_at IS NULL AND consumed_at IS NULL AND expires_at>now()").bind(&id).fetch_one(&mut *tx).bounded_one().await?;
            if count<5 {issue(&mut tx,&s.email,&id,"reset_password",&email,version).await?;}
        }
        tx.commit().await?;Ok::<_,Box<dyn std::error::Error+Send+Sync>>(())
    }.await;
    if result.is_err() {
        tracing::warn!("Password recovery enqueue failed");
    }
    // Include source limiting and all database work in the same response floor.
    tokio::time::sleep_until(started + std::time::Duration::from_secs(1)).await;
    message(&session, GENERIC).await
}

pub async fn resend_post(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(f): Form<Action>,
) -> Response {
    if !check_csrf(&session, &headers, &f.csrf).await {
        return secure(StatusCode::FORBIDDEN.into_response());
    }
    let Ok(Some(u)) = crate::auth::get_current_user(&s.db, &session).await else {
        return Redirect::to("/login").into_response();
    };
    if !s.email.available() {
        return message(
            &session,
            "Email is temporarily unavailable. Please try again later.",
        )
        .await;
    }
    if !issuance_limits(&s.db, &u.email, "verify_email").await {
        return message(
            &session,
            "Please wait before requesting another verification email.",
        )
        .await;
    }
    let result=async {
        let mut tx=s.db.begin().await?;
        let r=sqlx::query("SELECT email,auth_version,email_verified_at IS NOT NULL AS verified FROM users WHERE id=$1 FOR UPDATE").bind(&u.id).fetch_one(&mut *tx).bounded_one().await?;
        let current_version: i64=r.get("auth_version");
        if session.get::<i64>("auth_version").await? != Some(current_version) { return Err(crate::auth::AuthError::InvalidCredentials.into()); }
        let current_email: String=r.get("email");
        if !r.get::<bool,_>("verified"){issue(&mut tx,&s.email,&u.id,"verify_email",&current_email,current_version).await?;}
        tx.commit().await?;Ok::<_,Box<dyn std::error::Error+Send+Sync>>(())
    }.await;
    message(
        &session,
        if result.is_ok() {
            "Verification email queued. Use the newest link."
        } else {
            "Email is temporarily unavailable. Please try again later."
        },
    )
    .await
}

async fn token(
    db: &PgPool,
    hash: &str,
    purpose: &str,
) -> Result<Option<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query("SELECT t.*,u.email FROM account_tokens t JOIN users u ON u.id=t.user_id WHERE t.token_hash=$1 AND t.purpose=$2 AND t.consumed_at IS NULL AND t.revoked_at IS NULL AND t.expires_at>now() AND t.auth_version=u.auth_version AND (t.purpose='change_email' OR t.target_email=u.email)").bind(hash).bind(purpose).fetch_optional(db).bounded_one().await
}
async fn confirm(
    s: AppState,
    session: Session,
    headers: HeaderMap,
    f: Action,
    purpose: &str,
    peer: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    if !check_csrf(&session, &headers, &f.csrf).await {
        return secure(StatusCode::FORBIDDEN.into_response());
    }
    let source = peer
        .map(|p| p.0.ip().to_string())
        .unwrap_or_else(|| "unknown".into());
    if !limit(&s.db, &format!("confirm:{source}"), 10, 900)
        .await
        .unwrap_or(false)
    {
        return message(&session, "Too many attempts. Please try again later.").await;
    }
    let Ok(Some(c)) = session
        .get::<Challenge>(&format!("challenge:{purpose}"))
        .await
    else {
        return invalid_link(&session, purpose).await;
    };
    if c.purpose != purpose || c.expires <= Utc::now().timestamp() {
        return invalid_link(&session, purpose).await;
    }
    let Ok(Some(row)) = token(&s.db, &c.hash, purpose).await else {
        return invalid_link(&session, purpose).await;
    };
    let hash = if purpose == "reset_password" {
        if f.password != f.password_confirm {
            return reset_form(
                &session,
                "Those passwords don’t match. Enter the same password in both fields.",
            )
            .await;
        }
        if let Err(crate::auth::AuthError::Validation(reason)) =
            crate::auth::validate_password(&f.password)
        {
            return reset_form(&session, &reason).await;
        }
        match crate::auth::hash_password(secrecy::Secret::new(f.password)).await {
            Ok(h) => Some(h),
            Err(_) => return message(&session, "Please try again later.").await,
        }
    } else {
        None
    };
    let id: String = row.get("user_id");
    let result=async {
        let mut tx=s.db.begin().await?;
        // All mutations lock the user first, then challenge, to serialize sibling tokens.
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE").bind(&id).fetch_one(&mut *tx).bounded_one().await?;
        let r=sqlx::query("SELECT t.*,u.email FROM account_tokens t JOIN users u ON u.id=t.user_id WHERE t.token_hash=$1 AND t.purpose=$2 AND t.consumed_at IS NULL AND t.revoked_at IS NULL AND t.expires_at>now() AND t.auth_version=u.auth_version AND (t.purpose='change_email' OR t.target_email=u.email) FOR UPDATE OF t").bind(&c.hash).bind(purpose).fetch_optional(&mut *tx).bounded_one().await?;
        let Some(r)=r else{return Ok(false);};
        let old_email:String=r.get("email");let target:String=r.get("target_email");
        if let Some(h)=hash {
            sqlx::query("UPDATE password_credentials SET password_hash=$2 WHERE user_id=$1").bind(&id).bind(h).execute(&mut *tx).await?;
            sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id=$1").bind(&id).execute(&mut *tx).await?;
            s.email.enqueue(&mut tx,EmailKind::PasswordChanged,&old_email,"",None,Utc::now()+Duration::hours(24)).await?;
        }else if purpose=="change_email"{
            sqlx::query("UPDATE users SET email=$2,email_verified_at=now(),auth_version=auth_version+1 WHERE id=$1").bind(&id).bind(target).execute(&mut *tx).await?;
            s.email.enqueue(&mut tx,EmailKind::EmailChanged,&old_email,"",None,Utc::now()+Duration::hours(24)).await?;
        }else{sqlx::query("UPDATE users SET email_verified_at=now() WHERE id=$1").bind(&id).execute(&mut *tx).await?;}
        sqlx::query("UPDATE account_tokens SET consumed_at=now() WHERE token_hash=$1").bind(&c.hash).execute(&mut *tx).await?;
        sqlx::query("UPDATE account_tokens SET revoked_at=now() WHERE user_id=$1 AND consumed_at IS NULL AND revoked_at IS NULL AND ($2 <> 'verify_email' OR purpose='verify_email')").bind(&id).bind(purpose).execute(&mut *tx).await?;
        tx.commit().await?;Ok::<_,Box<dyn std::error::Error+Send+Sync>>(true)
    }.await;
    match result {
        Ok(true) => {
            let _ = session
                .remove::<Challenge>(&format!("challenge:{purpose}"))
                .await;
            if purpose == "reset_password" {
                if session
                    .get::<String>("user_id")
                    .await
                    .ok()
                    .flatten()
                    .as_deref()
                    == Some(&id)
                {
                    let _ = session.remove::<String>("user_id").await;
                    let _ = session.remove::<i64>("auth_version").await;
                }
                let _ = session.cycle_id().await;
                let _ = session.insert("reset_completed", true).await;
                secure(Redirect::to("/login").into_response())
            } else {
                message(
                    &session,
                    "Email confirmed. You can return to the bookstore or sign in.",
                )
                .await
            }
        }
        _ => invalid_link(&session, purpose).await,
    }
}
pub async fn verify_post(
    State(s): State<AppState>,
    session: Session,
    peer: Option<ConnectInfo<SocketAddr>>,
    h: HeaderMap,
    Form(f): Form<Action>,
) -> Response {
    confirm(s, session, h, f, "verify_email", peer).await
}
pub async fn reset_post(
    State(s): State<AppState>,
    session: Session,
    peer: Option<ConnectInfo<SocketAddr>>,
    h: HeaderMap,
    Form(f): Form<Action>,
) -> Response {
    confirm(s, session, h, f, "reset_password", peer).await
}
pub async fn change_post(
    State(s): State<AppState>,
    session: Session,
    peer: Option<ConnectInfo<SocketAddr>>,
    h: HeaderMap,
    Form(f): Form<Action>,
) -> Response {
    confirm(s, session, h, f, "change_email", peer).await
}
pub async fn email_change_post(
    State(s): State<AppState>,
    session: Session,
    h: HeaderMap,
    Form(f): Form<Action>,
) -> Response {
    if !check_csrf(&session, &h, &f.csrf).await {
        return secure(StatusCode::FORBIDDEN.into_response());
    }
    let Ok(Some(u)) = crate::auth::get_current_user(&s.db, &session).await else {
        return Redirect::to("/login").into_response();
    };
    let Ok(email) = crate::auth::normalize_email(&f.email) else {
        return message(&session, "Enter a valid email address.").await;
    };
    if !s.email.available() {
        return message(&session, "Email is temporarily unavailable.").await;
    }
    if !issuance_limits(&s.db, &u.email, "change_email").await {
        return message(
            &session,
            "Please wait before requesting another email change.",
        )
        .await;
    }
    let Ok(old_hash) = sqlx::query_scalar::<_, String>(
        "SELECT password_hash FROM password_credentials WHERE user_id=$1",
    )
    .bind(&u.id)
    .fetch_one(&s.db)
    .bounded_one()
    .await
    else {
        return message(&session, "Reauthentication failed.").await;
    };
    if !crate::auth::verify_password(secrecy::Secret::new(f.password), &old_hash)
        .await
        .unwrap_or(false)
    {
        return message(&session, "Reauthentication failed.").await;
    }
    let version = session
        .get::<i64>("auth_version")
        .await
        .ok()
        .flatten()
        .unwrap_or(-1);
    let result=async{
        let mut tx=s.db.begin().await?;
        let r=sqlx::query("SELECT u.auth_version,p.password_hash FROM users u JOIN password_credentials p ON p.user_id=u.id WHERE u.id=$1 FOR UPDATE OF u").bind(&u.id).fetch_one(&mut *tx).bounded_one().await?;
        if r.get::<i64,_>("auth_version")!=version || r.get::<String,_>("password_hash")!=old_hash{return Ok(false);}
        issue(&mut tx,&s.email,&u.id,"change_email",&email,version).await?;
        s.email.enqueue(&mut tx,EmailKind::EmailChangeRequested,&u.email,"",None,Utc::now()+Duration::hours(24)).await?;
        tx.commit().await?;Ok::<_,Box<dyn std::error::Error+Send+Sync>>(true)
    }.await;
    message(
        &session,
        if matches!(result, Ok(true)) {
            "Check the new address for a confirmation link. Your current address remains active."
        } else {
            "Unable to request this change. Try again later."
        },
    )
    .await
}

pub async fn resend_webhook(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    match s.email.verify_and_record(&s.db, &headers, &body).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(crate::email::EmailError::Database(_)) => {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}
