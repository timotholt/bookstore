//! Title/author review identities; catalog records are never mutated here.
use crate::{
    db::DbPool,
    errors::AppError,
    read_budget::{ReadFutureExt, RowsFutureExt},
};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

pub fn conflict(message: &str) -> AppError {
    AppError::Http(StatusCode::CONFLICT, message.into())
}
pub fn forbidden() -> AppError {
    AppError::Http(
        StatusCode::FORBIDDEN,
        "You do not have permission to do that.".into(),
    )
}

#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct Work {
    pub work_key: String,
    pub title: String,
    pub author: String,
}
async fn load_work(db: &DbPool, book: &str) -> Result<Work, AppError> {
    sqlx::query_as("SELECT review_title_key(b.title,COALESCE(a.name,''),b.id) AS work_key,b.title,COALESCE(a.name,'') AS author FROM books b LEFT JOIN authors a ON a.id=b.primary_author_id WHERE b.id=$1")
        .bind(book).fetch_optional(db).bounded_one().await?.ok_or(AppError::NotFound)
}
pub async fn is_moderator(db: &DbPool, user: &str) -> Result<bool, AppError> {
    crate::access::has_permission(db, user, "reviews.moderate").await
}
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct Review {
    pub id: i64,
    pub book_id: String,
    pub user_id: String,
    pub status: String,
    pub current_revision: Option<i64>,
    pub published_revision: Option<i64>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct Revision {
    pub id: i64,
    pub rating: i32,
    pub title: String,
    pub body: String,
    pub status: String,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub book_id: String,
    pub user_id: String,
    pub book_title: String,
    pub author: String,
    pub display_name: String,
    pub rating: i32,
    pub title: String,
    pub body: String,
    pub status: String,
    pub reason: String,
    pub review_status: String,
    pub revision: i64,
    pub published_revision: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub helpful_count: i32,
    pub report_count: i64,
}
const ENTRY: &str = "SELECT r.id,r.book_id,r.user_id,w.title AS book_title,w.author, COALESCE(NULLIF(u.first_name,''),'Reader') AS display_name,v.rating,v.title,v.body,v.status,v.reason,r.status AS review_status,v.id AS revision,r.published_revision,v.created_at,r.helpful_count,(SELECT count(*) FROM review_reports rp WHERE rp.review_id=r.id AND rp.status='open') AS report_count FROM reviews r JOIN review_works w USING(work_key) JOIN users u ON u.id=r.user_id JOIN review_revisions v ON v.id=";
#[derive(Clone, Debug, Default, sqlx::FromRow, Serialize, Deserialize)]
pub struct Summary {
    pub published_count: i64,
    pub average_rating: f64,
    pub star_1_count: i64,
    pub star_2_count: i64,
    pub star_3_count: i64,
    pub star_4_count: i64,
    pub star_5_count: i64,
}
async fn load_summary(db: &DbPool, key: &str) -> Result<Summary, AppError> {
    Ok(sqlx::query_as("SELECT published_count,average_rating,star_1_count,star_2_count,star_3_count,star_4_count,star_5_count FROM review_work_aggregates WHERE work_key=$1").bind(key).fetch_optional(db).bounded_one().await?.unwrap_or_default())
}
pub async fn own(db: &DbPool, key: &str, user: &str) -> Result<Option<Review>, AppError> {
    Ok(
        sqlx::query_as("SELECT * FROM reviews WHERE work_key=$1 AND user_id=$2")
            .bind(key)
            .bind(user)
            .fetch_optional(db)
            .bounded_one()
            .await?,
    )
}
pub async fn revision(db: &DbPool, id: i64) -> Result<Revision, AppError> {
    Ok(sqlx::query_as("SELECT * FROM review_revisions WHERE id=$1")
        .bind(id)
        .fetch_one(db)
        .bounded_one()
        .await?)
}
async fn load_public_list(
    db: &DbPool,
    key: &str,
    page: u32,
    sort: &str,
) -> Result<Vec<Entry>, AppError> {
    let order = match sort {
        "highest" => "r.rating DESC,r.published_at DESC,r.id DESC",
        "lowest" => "r.rating ASC,r.published_at DESC,r.id DESC",
        "helpful" => "r.helpful_count DESC,r.published_at DESC,r.id DESC",
        _ => "r.published_at DESC,r.id DESC",
    };
    Ok(sqlx::query_as(&format!("{ENTRY}r.published_revision WHERE r.work_key=$1 AND r.status='published' AND r.deleted_at IS NULL ORDER BY {order} LIMIT 20 OFFSET $2"))
       .bind(key).bind(i64::from(page.max(1)-1)*20).fetch_all(db).bounded_rows(20).await?)
}
pub async fn mine(db: &DbPool, user: &str, page: u32) -> Result<(Vec<Entry>, i64), AppError> {
    let count =
        sqlx::query_scalar("SELECT count(*) FROM reviews WHERE user_id=$1 AND deleted_at IS NULL")
            .bind(user)
            .fetch_one(db)
            .bounded_one()
            .await?;
    let rows = sqlx::query_as(&format!("{ENTRY}r.current_revision WHERE r.user_id=$1 AND r.deleted_at IS NULL ORDER BY v.created_at DESC,r.id DESC LIMIT 20 OFFSET $2"))
       .bind(user).bind(i64::from(page.max(1)-1)*20).fetch_all(db).bounded_rows(20).await?;
    Ok((rows, count))
}
pub async fn queue(db: &DbPool, filter: &str, page: u32) -> Result<(Vec<Entry>, i64), AppError> {
    let condition = match filter {
        "published" => "r.status='published'",
        "rejected" => "v.status='rejected'",
        "removed" => "r.status='removed'",
        "reported" => {
            "EXISTS(SELECT 1 FROM review_reports rp WHERE rp.review_id=r.id AND rp.status='open')"
        }
        _ => "v.status='pending'",
    };
    let count=sqlx::query_scalar(&format!("SELECT count(*) FROM reviews r JOIN review_revisions v ON v.id=r.current_revision WHERE r.deleted_at IS NULL AND {condition}"))
      .fetch_one(db).bounded_one().await?;
    let rows=sqlx::query_as(&format!("{ENTRY}r.current_revision WHERE r.deleted_at IS NULL AND {condition} ORDER BY v.created_at,r.id LIMIT 20 OFFSET $1"))
      .bind(i64::from(page.max(1)-1)*20).fetch_all(db).bounded_rows(20).await?;
    Ok((rows, count))
}
#[derive(Clone, Debug, Deserialize)]
pub struct Input {
    pub rating: i32,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub revision: i64,
}
pub fn validate(input: &Input) -> Result<(), AppError> {
    if !(1..=5).contains(&input.rating) {
        return Err(AppError::Validation(
            "Choose a rating from 1 to 5 stars.".into(),
        ));
    }
    if input.title.chars().count() > 120 || input.body.chars().count() > 5000 {
        return Err(AppError::Validation(
            "Use at most 120 characters for the title and 5,000 for the review.".into(),
        ));
    }
    Ok(())
}
async fn lock_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    key: &str,
) -> Result<(), AppError> {
    sqlx::query("SELECT work_key FROM review_works WHERE work_key=$1 FOR UPDATE")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn submit(
    db: &DbPool,
    principal: &crate::access::Principal,
    book: &str,
    input: &Input,
) -> Result<i64, AppError> {
    validate(input)?;
    let work = work(db, book).await?;
    let mut tx = db.begin().await?;
    crate::access::require_active_tx(&mut tx, principal, crate::access::AccountLock::Share).await?;
    let user = principal.user_id.as_str();
    // Serialize this account's quota check with its submissions.
    sqlx::query("SELECT pg_advisory_xact_lock(7141,hashtext($1))")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let recent:i64=sqlx::query_scalar("SELECT count(*) FROM review_revisions v JOIN reviews r ON r.id=v.review_id WHERE r.user_id=$1 AND v.created_at>now()-interval '1 minute'").bind(user).fetch_one(&mut *tx).bounded_one().await?;
    if recent >= 5 {
        return Err(AppError::Http(
            StatusCode::TOO_MANY_REQUESTS,
            "Please wait a minute before submitting more reviews.".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO review_works(work_key,title,author) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
    )
    .bind(&work.work_key)
    .bind(&work.title)
    .bind(&work.author)
    .execute(&mut *tx)
    .await?;
    lock_work(&mut tx, &work.work_key).await?;
    let existing: Option<Review> =
        sqlx::query_as("SELECT * FROM reviews WHERE work_key=$1 AND user_id=$2 FOR UPDATE")
            .bind(&work.work_key)
            .bind(user)
            .fetch_optional(&mut *tx)
            .bounded_one()
            .await?;
    let id = if let Some(r) = existing {
        if r.current_revision.unwrap_or(0) != input.revision {
            return Err(conflict("Your review changed. Reload before saving."));
        }
        if let Some(old) = r.current_revision {
            sqlx::query(
                "UPDATE review_revisions SET status='superseded' WHERE id=$1 AND status='pending'",
            )
            .bind(old)
            .execute(&mut *tx)
            .await?;
        }
        r.id
    } else {
        if input.revision != 0 {
            return Err(conflict(
                "This review no longer exists. Reload before saving.",
            ));
        }
        sqlx::query_scalar("INSERT INTO reviews(book_id,user_id,work_key,rating,title,body) VALUES($1,$2,$3,$4,$5,$6) RETURNING id")
          .bind(book).bind(user).bind(&work.work_key).bind(input.rating).bind(input.title.trim()).bind(input.body.trim()).fetch_one(&mut *tx).bounded_one().await?
    };
    let rev:i64=sqlx::query_scalar("INSERT INTO review_revisions(review_id,rating,title,body) VALUES($1,$2,$3,$4) RETURNING id")
       .bind(id).bind(input.rating).bind(input.title.trim()).bind(input.body.trim()).fetch_one(&mut *tx).bounded_one().await?;
    sqlx::query("UPDATE reviews SET current_revision=$2,deleted_at=NULL,updated_at=now(),status=CASE WHEN status='published' THEN status ELSE 'pending' END WHERE id=$1")
       .bind(id).bind(rev).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(id)
}
async fn locked_review(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: i64,
) -> Result<Review, AppError> {
    let key: Option<String> = sqlx::query_scalar("SELECT work_key FROM reviews WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut **tx)
        .bounded_one()
        .await?;
    lock_work(tx, &key.ok_or(AppError::NotFound)?).await?;
    sqlx::query_as("SELECT * FROM reviews WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .bounded_one()
        .await?
        .ok_or(AppError::NotFound)
}
async fn event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    r: &Review,
    rev: Option<i64>,
    user: &str,
    action: &str,
    reason: &str,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO review_moderation_events(review_id,revision_id,actor_id,action,reason) VALUES($1,$2,$3,$4,$5)").bind(r.id).bind(rev).bind(user).bind(action).bind(reason).execute(&mut **tx).await?;
    Ok(())
}
pub async fn delete(
    db: &DbPool,
    principal: &crate::access::Principal,
    id: i64,
    expected: i64,
) -> Result<(), AppError> {
    let mut tx = db.begin().await?;
    crate::access::require_active_tx(&mut tx, principal, crate::access::AccountLock::Share).await?;
    let user = principal.user_id.as_str();
    let r = locked_review(&mut tx, id).await?;
    if r.user_id != user {
        return Err(forbidden());
    }
    if r.current_revision != Some(expected) {
        return Err(conflict("Your review changed. Reload before deleting."));
    }
    sqlx::query("UPDATE reviews SET status='removed',deleted_at=now(),published_revision=NULL,title='',body='',updated_at=now(),helpful_count=0,unhelpful_count=0 WHERE id=$1").bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE review_revisions SET title='',body='',reason='',status=CASE WHEN status='pending' THEN 'superseded' ELSE status END WHERE review_id=$1").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM review_votes WHERE review_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE review_reports SET status='resolved',resolved_at=now() WHERE review_id=$1 AND status='open'").bind(id).execute(&mut *tx).await?;
    event(&mut tx, &r, r.current_revision, user, "delete", "").await?;
    tx.commit().await?;
    Ok(())
}
pub async fn moderate(
    db: &DbPool,
    principal: &crate::access::Principal,
    id: i64,
    expected: i64,
    action: &str,
    reason: &str,
) -> Result<(), AppError> {
    let mut tx = db.begin().await?;
    crate::access::require_permission_tx(
        &mut tx,
        principal,
        "reviews.moderate",
        crate::access::AccountLock::Share,
    )
    .await?;
    let user = principal.user_id.as_str();
    let r = locked_review(&mut tx, id).await?;
    if r.deleted_at.is_some() {
        return Err(conflict("The author deleted this review."));
    }
    if r.current_revision != Some(expected) {
        return Err(conflict(
            "The review changed. Reload and inspect the latest revision.",
        ));
    }
    if reason.chars().count() > 1000
        || (matches!(action, "reject" | "remove") && reason.trim().chars().count() < 3)
    {
        return Err(AppError::Validation(
            "Provide a reason of 3–1,000 characters for rejection or removal.".into(),
        ));
    }
    let v: Revision = sqlx::query_as("SELECT * FROM review_revisions WHERE id=$1")
        .bind(expected)
        .fetch_one(&mut *tx)
        .bounded_one()
        .await?;
    match action {
        "approve" | "reject" => {
            if r.user_id == user {
                return Err(forbidden());
            }
            if v.status != "pending" {
                return Err(conflict("This revision has already been moderated."));
            }
            sqlx::query("UPDATE review_revisions SET status=$2,reason=$3 WHERE id=$1")
                .bind(expected)
                .bind(if action == "approve" {
                    "published"
                } else {
                    "rejected"
                })
                .bind(reason.trim())
                .execute(&mut *tx)
                .await?;
            if action == "approve" {
                // Reports about a superseded public version must not target new text.
                let closed=sqlx::query("UPDATE review_reports SET status='resolved',resolved_at=now() WHERE review_id=$1 AND revision_id<>$2 AND status='open'")
                    .bind(id).bind(expected).execute(&mut *tx).await?.rows_affected();
                if closed > 0 {
                    event(
                        &mut tx,
                        &r,
                        Some(expected),
                        user,
                        "report_resolved",
                        "Reported version replaced by approved revision",
                    )
                    .await?;
                }
                // Helpful votes endorsed the old text, not the replacement.
                if r.published_revision.is_some() && r.published_revision != Some(expected) {
                    sqlx::query("DELETE FROM review_votes WHERE review_id=$1")
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                    sqlx::query("UPDATE reviews SET helpful_count=0,unhelpful_count=0 WHERE id=$1")
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                }
                sqlx::query("UPDATE reviews SET rating=$2,title=$3,body=$4,status='published',published_revision=$5,published_at=now(),updated_at=now() WHERE id=$1").bind(id).bind(v.rating).bind(&v.title).bind(&v.body).bind(expected).execute(&mut *tx).await?;
            } else if r.status != "published" {
                sqlx::query("UPDATE reviews SET status='rejected',updated_at=now() WHERE id=$1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        "remove" => {
            sqlx::query("UPDATE reviews SET status='removed',published_revision=NULL,updated_at=now() WHERE id=$1").bind(id).execute(&mut *tx).await?;
            sqlx::query("UPDATE review_revisions SET status='rejected',reason=$2 WHERE id=$1")
                .bind(expected)
                .bind(reason.trim())
                .execute(&mut *tx)
                .await?;
        }
        "report_resolved" => {}
        _ => return Err(AppError::Validation("Unknown moderation action.".into())),
    }
    if matches!(action, "remove" | "report_resolved") {
        sqlx::query("UPDATE review_reports SET status='resolved',resolved_at=now() WHERE review_id=$1 AND status='open'").bind(id).execute(&mut *tx).await?;
    }
    event(&mut tx, &r, Some(expected), user, action, reason.trim()).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn vote(
    db: &DbPool,
    principal: &crate::access::Principal,
    id: i64,
    expected: i64,
    vote: i32,
) -> Result<(), AppError> {
    if ![0, 1].contains(&vote) {
        return Err(AppError::Validation("Invalid helpful vote.".into()));
    }
    let mut tx = db.begin().await?;
    crate::access::require_active_tx(&mut tx, principal, crate::access::AccountLock::Share).await?;
    let user = principal.user_id.as_str();
    let r = locked_review(&mut tx, id).await?;
    if r.user_id == user {
        return Err(AppError::Validation(
            "You cannot vote on your own review.".into(),
        ));
    }
    if r.status != "published" || r.deleted_at.is_some() || r.published_revision != Some(expected) {
        return Err(conflict("This review changed or is no longer public."));
    }
    if vote == 0 {
        sqlx::query("DELETE FROM review_votes WHERE review_id=$1 AND user_id=$2")
            .bind(id)
            .bind(user)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("INSERT INTO review_votes(review_id,user_id,vote) VALUES($1,$2,1) ON CONFLICT(review_id,user_id) DO UPDATE SET vote=1,updated_at=now()").bind(id).bind(user).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE reviews SET helpful_count=(SELECT count(*) FROM review_votes WHERE review_id=$1 AND vote=1),unhelpful_count=0 WHERE id=$1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn report(
    db: &DbPool,
    principal: &crate::access::Principal,
    id: i64,
    expected: i64,
    reason: &str,
) -> Result<(), AppError> {
    if !(3..=1000).contains(&reason.trim().chars().count()) {
        return Err(AppError::Validation(
            "Report reasons must be 3–1,000 characters.".into(),
        ));
    }
    let mut tx = db.begin().await?;
    crate::access::require_active_tx(&mut tx, principal, crate::access::AccountLock::Share).await?;
    let user = principal.user_id.as_str();
    sqlx::query("SELECT pg_advisory_xact_lock(7142,hashtext($1))")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let recent:i64=sqlx::query_scalar("SELECT count(*) FROM review_reports WHERE user_id=$1 AND created_at>now()-interval '1 hour'").bind(user).fetch_one(&mut *tx).bounded_one().await?;
    if recent >= 10 {
        return Err(AppError::Http(
            StatusCode::TOO_MANY_REQUESTS,
            "Please wait before reporting more reviews.".into(),
        ));
    }
    let r = locked_review(&mut tx, id).await?;
    if r.status != "published" || r.deleted_at.is_some() || r.published_revision != Some(expected) {
        return Err(conflict("This review changed or is no longer public."));
    }
    sqlx::query("INSERT INTO review_reports(review_id,revision_id,user_id,reason) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(id).bind(expected).bind(user).bind(reason.trim()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct History {
    pub action: String,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revision_id: Option<i64>,
    pub actor_name: String,
}
pub async fn history(db: &DbPool, id: i64) -> Result<Vec<History>, AppError> {
    Ok(sqlx::query_as("SELECT e.action,e.reason,e.created_at,e.revision_id,COALESCE(NULLIF(u.full_name,''),u.email,'Deleted account') AS actor_name FROM review_moderation_events e LEFT JOIN users u ON u.id=e.actor_id WHERE e.review_id=$1 ORDER BY e.id DESC LIMIT 20").bind(id).fetch_all(db).bounded_rows(20).await?)
}
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Report {
    pub id: i64,
    pub revision_id: i64,
    pub reason: String,
    pub status: String,
}
pub async fn reports(db: &DbPool, id: i64) -> Result<Vec<Report>, AppError> {
    Ok(sqlx::query_as("SELECT id,revision_id,reason,status FROM review_reports WHERE review_id=$1 ORDER BY id DESC LIMIT 20").bind(id).fetch_all(db).bounded_rows(20).await?)
}

pub async fn work(db: &DbPool, book: &str) -> Result<Work, AppError> {
    crate::catalog_cache::cached(
        "review_work",
        book.into(),
        crate::catalog_cache::Lifetime::Metadata,
        async {
            load_work(db, book).await.map_err(|e| match e {
                AppError::Database(e) => e,
                AppError::NotFound => sqlx::Error::RowNotFound,
                _ => sqlx::Error::Protocol(e.to_string()),
            })
        },
    )
    .await
    .map_err(|e| {
        if matches!(e, sqlx::Error::RowNotFound) {
            AppError::NotFound
        } else {
            e.into()
        }
    })
}
pub async fn summary(db: &DbPool, key: &str) -> Result<Summary, AppError> {
    Ok(crate::catalog_cache::cached(
        "review_summary",
        key.into(),
        crate::catalog_cache::Lifetime::Offers,
        async {
            load_summary(db, key).await.map_err(|e| match e {
                AppError::Database(e) => e,
                _ => sqlx::Error::Protocol(e.to_string()),
            })
        },
    )
    .await?)
}
pub async fn public_list(
    db: &DbPool,
    key: &str,
    page: u32,
    sort: &str,
) -> Result<Vec<Entry>, AppError> {
    Ok(crate::catalog_cache::cached(
        "review_list",
        format!("{key}:{page}:{sort}"),
        crate::catalog_cache::Lifetime::Offers,
        async {
            load_public_list(db, key, page, sort)
                .await
                .map_err(|e| match e {
                    AppError::Database(e) => e,
                    _ => sqlx::Error::Protocol(e.to_string()),
                })
        },
    )
    .await?)
}

#[cfg(test)]
mod security_tests {
    include!("review_security_tests.rs");
}
