use super::*;
use crate::access::Principal;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

async fn isolated_db() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect("Set an isolated local DATABASE_URL for tests");
    let parsed: sqlx::postgres::PgConnectOptions = url.parse().unwrap();
    assert!(
        matches!(parsed.get_host(), "localhost" | "127.0.0.1" | "::1")
            || parsed.get_host().starts_with('/'),
        "Never run review tests against hosted databases"
    );
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let schema = format!("review_security_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let schema_for_connect = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .after_connect(move |conn, _| {
            let schema = schema_for_connect.clone();
            Box::pin(async move {
                sqlx::query(&format!("SET search_path TO {schema}"))
                    .execute(&mut *conn)
                    .await?;
                sqlx::query(&format!("SET application_name TO '{schema}'"))
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations_postgres")
        .run(&pool)
        .await
        .unwrap();
    for id in ["author", "moderator", "voter"] {
        sqlx::query("INSERT INTO users(id,email,first_name) VALUES($1,$2,$1)")
            .bind(id)
            .bind(format!("{id}@example.com"))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO user_staff_roles(user_id,role) VALUES('moderator','moderator')")
        .execute(&pool)
        .await
        .unwrap();
    pool
}
fn principal(id: &str, version: i64) -> Principal {
    Principal {
        user_id: id.into(),
        auth_version: version,
    }
}
fn input(revision: i64) -> Input {
    Input {
        rating: 4,
        title: "Security fixture".into(),
        body: "Review body".into(),
        revision,
    }
}
async fn blocked(db: &PgPool, needle: &str) {
    tokio::time::timeout(Duration::from_secs(5),async {
        loop {
            let found:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=current_setting('application_name') AND pid<>pg_backend_pid() AND wait_event_type='Lock' AND query LIKE $1)").bind(format!("%{needle}%")).fetch_one(db).await.unwrap();
            if found {break;}
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("Expected deterministic lock barrier");
}
async fn dispose(db: PgPool) {
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&db)
        .await
        .unwrap();
    // Safe: this helper only ever creates its own random local schema.
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&db)
        .await
        .unwrap();
    db.close().await;
}

#[tokio::test]
async fn revocation_before_review_write_rejects_the_inflight_principal() {
    let db = isolated_db().await;
    let mut revocation = db.begin().await.unwrap();
    sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id='author'")
        .execute(&mut *revocation)
        .await
        .unwrap();
    let worker_db = db.clone();
    let worker = tokio::spawn(async move {
        submit(&worker_db, &principal("author", 0), "b003", &input(0)).await
    });
    blocked(&db, "FROM users WHERE id=").await;
    revocation.commit().await.unwrap();
    assert!(worker.await.unwrap().is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM reviews")
            .fetch_one(&db)
            .await
            .unwrap(),
        0
    );
    let author = principal("author", 1);
    let id = submit(&db, &author, "b003", &input(0)).await.unwrap();
    let key = work(&db, "b003").await.unwrap().work_key;
    let revision = own(&db, &key, "author")
        .await
        .unwrap()
        .unwrap()
        .current_revision
        .unwrap();
    // Permission revocation also waits on and defeats an in-flight moderator.
    let mut revocation = db.begin().await.unwrap();
    sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id='moderator'")
        .execute(&mut *revocation)
        .await
        .unwrap();
    sqlx::query("DELETE FROM user_staff_roles WHERE user_id='moderator'")
        .execute(&mut *revocation)
        .await
        .unwrap();
    let worker_db = db.clone();
    let worker = tokio::spawn(async move {
        moderate(
            &worker_db,
            &principal("moderator", 0),
            id,
            revision,
            "approve",
            "",
        )
        .await
    });
    blocked(&db, "FROM users WHERE id=").await;
    revocation.commit().await.unwrap();
    assert!(worker.await.unwrap().is_err());
    assert_eq!(summary(&db, &key).await.unwrap().published_count, 0);
    // Every mutation uses the account guard, not only moderation.
    let stale = principal("author", 0);
    assert!(delete(&db, &stale, id, revision).await.is_err());
    assert!(vote(&db, &stale, id, revision, 1).await.is_err());
    assert!(report(&db, &stale, id, revision, "Valid report reason")
        .await
        .is_err());
    assert!(submit(&db, &stale, "b003", &input(revision)).await.is_err());
    sqlx::query(
        "UPDATE users SET account_status='disabled',auth_version=auth_version+1 WHERE id='author'",
    )
    .execute(&db)
    .await
    .unwrap();
    assert!(
        submit(&db, &principal("author", 2), "b003", &input(revision))
            .await
            .is_err(),
        "disabled principal with current version was accepted"
    );
    dispose(db).await;
}

#[tokio::test]
async fn review_write_holds_account_authority_until_commit_before_revocation() {
    let db = isolated_db().await;
    let id = submit(&db, &principal("author", 0), "b003", &input(0))
        .await
        .unwrap();
    let key = work(&db, "b003").await.unwrap().work_key;
    let revision = own(&db, &key, "author")
        .await
        .unwrap()
        .unwrap()
        .current_revision
        .unwrap();
    let mut gate = db.begin().await.unwrap();
    sqlx::query("SELECT work_key FROM review_works WHERE work_key=$1 FOR UPDATE")
        .bind(&key)
        .execute(&mut *gate)
        .await
        .unwrap();
    let worker_db = db.clone();
    let writer = tokio::spawn(async move {
        moderate(
            &worker_db,
            &principal("moderator", 0),
            id,
            revision,
            "approve",
            "",
        )
        .await
    });
    blocked(&db, "FROM review_works WHERE work_key=").await;
    let revoke_db = db.clone();
    let revoke = tokio::spawn(async move {
        let mut tx = revoke_db.begin().await.unwrap();
        sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id='moderator'")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("DELETE FROM user_staff_roles WHERE user_id='moderator'")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    });
    blocked(&db, "UPDATE users SET auth_version=").await;
    gate.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), writer)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), revoke)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary(&db, &key).await.unwrap().published_count, 1);
    assert!(!is_moderator(&db, "moderator").await.unwrap());
    assert!(moderate(
        &db,
        &principal("moderator", 0),
        id,
        revision,
        "remove",
        "Policy violation"
    )
    .await
    .is_err());
    assert_eq!(summary(&db, &key).await.unwrap().published_count, 1);
    dispose(db).await;
}

#[tokio::test]
async fn title_author_keys_normalize_edge_whitespace_without_catalog_mutation() {
    let db = isolated_db().await;
    let equivalent:bool=sqlx::query_scalar("SELECT review_title_key(E'\\t Dune \\n',E'\\n Frank\\tHerbert \\t','x')=review_title_key('Dune','Frank Herbert','different-isbn')").fetch_one(&db).await.unwrap();
    assert!(equivalent);
    let unknown:bool=sqlx::query_scalar("SELECT review_title_key('Dune',E'\\t\\n','x')=review_title_key('Dune','','x') AND review_title_key('Dune','','x')<>review_title_key('Dune','','y')").fetch_one(&db).await.unwrap();
    assert!(unknown);
    let before: String = sqlx::query_scalar(
        "SELECT md5(string_agg(row_to_json(b)::text,'' ORDER BY b.id)) FROM books b",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    submit(&db, &principal("author", 0), "b003", &input(0))
        .await
        .unwrap();
    let after: String = sqlx::query_scalar(
        "SELECT md5(string_agg(row_to_json(b)::text,'' ORDER BY b.id)) FROM books b",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(before, after);
    dispose(db).await;
}

#[tokio::test]
async fn normalization_backfill_preserves_review_versions_roles_and_catalog() {
    let db = isolated_db().await;
    // Synthetic edition exists only inside this random local test schema.
    sqlx::query("INSERT INTO books(id,slug,title,primary_author_id,isbn) SELECT 'review-edge','review-edge',E'\\t'||title||E'\\n',primary_author_id,'TEST-EDGE' FROM books WHERE id='b003'").execute(&db).await.unwrap();
    let fingerprint = sqlx::query_scalar::<_, String>(
        "SELECT md5(string_agg(row_to_json(b)::text,'' ORDER BY b.id)) FROM books b",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    sqlx::raw_sql(r#"CREATE OR REPLACE FUNCTION review_title_key(title TEXT, author TEXT, book_id TEXT)
      RETURNS TEXT LANGUAGE SQL IMMUTABLE PARALLEL SAFE AS $$
      SELECT md5(json_build_array(lower(regexp_replace(btrim(title),'\s+',' ','g')),
       CASE WHEN btrim(author)='' THEN 'unknown-author:'||book_id ELSE lower(regexp_replace(btrim(author),'\s+',' ','g')) END)::text) $$;"#).execute(&db).await.unwrap();
    let old_key = work(&db, "review-edge").await.unwrap().work_key;
    let normal_key = work(&db, "b003").await.unwrap().work_key;
    assert_ne!(old_key, normal_key);
    let id = submit(&db, &principal("author", 0), "review-edge", &input(0))
        .await
        .unwrap();
    let before = own(&db, &old_key, "author").await.unwrap().unwrap();
    let revision = before.current_revision.unwrap();
    moderate(&db, &principal("moderator", 0), id, revision, "approve", "")
        .await
        .unwrap();
    let mut migration = db.begin().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations_postgres/20261014003000_review_identity_normalization.sql"
    ))
    .execute(&mut *migration)
    .await
    .unwrap();
    migration.commit().await.unwrap();
    let corrected_key = work(&db, "review-edge").await.unwrap().work_key;
    assert_eq!(normal_key, corrected_key);
    let after = own(&db, &corrected_key, "author").await.unwrap().unwrap();
    assert_eq!(after.id, id);
    assert_eq!(after.current_revision, Some(revision));
    assert_eq!(after.published_revision, Some(revision));
    assert_eq!(
        summary(&db, &corrected_key).await.unwrap().published_count,
        1
    );
    assert_eq!(
        summary(&db, &corrected_key).await.unwrap().average_rating,
        4.
    );
    assert!(is_moderator(&db, "moderator").await.unwrap());
    assert_eq!(
        fingerprint,
        sqlx::query_scalar::<_, String>(
            "SELECT md5(string_agg(row_to_json(b)::text,'' ORDER BY b.id)) FROM books b"
        )
        .fetch_one(&db)
        .await
        .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM review_moderation_events WHERE review_id=$1"
        )
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap(),
        1
    );
    dispose(db).await;
}
