use crate::read_budget::ReadFutureExt;
use crate::{db::DbPool, errors::AppError};
use axum::http::StatusCode;
use sqlx::{Postgres, Transaction};
use tower_sessions::Session;
#[derive(Clone, Debug)]
pub struct Principal {
    pub user_id: String,
    pub auth_version: i64,
}
#[derive(Clone, Copy)]
pub enum AccountLock {
    Share,
    Update,
}
pub fn denied() -> AppError {
    AppError::Http(
        StatusCode::FORBIDDEN,
        "Access denied or session expired.".into(),
    )
}
pub async fn principal(db: &DbPool, session: &Session) -> Result<Principal, AppError> {
    let user = crate::auth::get_current_user(db, session)
        .await?
        .ok_or_else(|| AppError::Http(StatusCode::UNAUTHORIZED, "Sign in to continue.".into()))?;
    Ok(Principal {
        user_id: user.id,
        auth_version: session
            .get::<i64>("auth_version")
            .await?
            .ok_or_else(denied)?,
    })
}
pub async fn require_active_tx(
    tx: &mut Transaction<'_, Postgres>,
    p: &Principal,
    lock: AccountLock,
) -> Result<(), AppError> {
    let q = match lock {
        AccountLock::Share => {
            "SELECT account_status='active' AND auth_version=$2 FROM users WHERE id=$1 FOR SHARE"
        }
        AccountLock::Update => {
            "SELECT account_status='active' AND auth_version=$2 FROM users WHERE id=$1 FOR UPDATE"
        }
    };
    if sqlx::query_scalar::<_, bool>(q)
        .bind(&p.user_id)
        .bind(p.auth_version)
        .fetch_optional(&mut **tx)
        .bounded_one()
        .await?
        != Some(true)
    {
        return Err(denied());
    }
    Ok(())
}
pub fn permission_roles(permission: &str) -> &'static [&'static str] {
    match permission {
        "reviews.moderate" => &["moderator"],
        "users.view" | "users.manage" => &["support"],
        "staff.manage" => &["administrator"],
        "orders.view" => &["fulfillment", "refund"],
        "orders.fulfill" => &["fulfillment"],
        "refunds.issue" => &["refund"],
        _ => &[],
    }
}
pub async fn has_permission(db: &DbPool, user: &str, permission: &str) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_staff_roles r JOIN users u ON u.id=r.user_id WHERE u.id=$1 AND u.account_status='active' AND r.role=ANY($2))").bind(user).bind(permission_roles(permission)).fetch_one(db).bounded_one().await?)
}
pub async fn require_permission_tx(
    tx: &mut Transaction<'_, Postgres>,
    p: &Principal,
    permission: &str,
    lock: AccountLock,
) -> Result<(), AppError> {
    require_active_tx(tx, p, lock).await?;
    if !sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM user_staff_roles WHERE user_id=$1 AND role=ANY($2))",
    )
    .bind(&p.user_id)
    .bind(permission_roles(permission))
    .fetch_one(&mut **tx)
    .bounded_one()
    .await?
    {
        return Err(denied());
    }
    Ok(())
}
#[allow(dead_code)]
pub async fn require_permission(
    db: &DbPool,
    session: &Session,
    permission: &str,
) -> Result<Principal, AppError> {
    let p = principal(db, session).await?;
    let mut tx = db.begin().await?;
    require_permission_tx(&mut tx, &p, permission, AccountLock::Share).await?;
    tx.commit().await?;
    Ok(p)
}
pub async fn administration_lock(tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock(684201014)")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
