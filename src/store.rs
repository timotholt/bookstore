use sqlx::{SqlitePool, QueryBuilder};
use crate::models::{AnalyticsEventPayload, BookCard, CatalogFilters, VariantAttribute};

const BASE_SELECT: &str = r#"
    SELECT
        b.id as id,
        b.title as title,
        COALESCE(a.name, '') as author,
        COALESCE(a.slug, '') as author_slug,
        COALESCE(g.name, '') as genre,
        COALESCE(g.slug, '') as genre_slug,
        COALESCE(b.year, 0) as year,
        COALESCE(b.isbn, '') as isbn,
        b.cover_color as cover_color,
        b.aspect_ratio as aspect_ratio,
        b.tags as tags,
        b.is_new_arrival as is_new_arrival,
        c.id as copy_id,
        COALESCE(c.condition, '') as condition,
        c.price as price,
        COALESCE(c.notes, '') as notes,
        COALESCE(c.format, 'Standard') as format,
        c.stock as stock,
        c.is_staff_pick as is_staff_pick,
        COALESCE(c.staff_quote, '') as staff_quote,
        c.seal_style as seal_style,
        c.seal_text as seal_text
    FROM books b
    LEFT JOIN authors a ON a.id = b.primary_author_id
    LEFT JOIN genres g ON g.id = b.primary_genre_id
    JOIN book_copies c ON c.book_id = b.id
    WHERE c.is_sold = 0
"#;

pub async fn list_books(db: &SqlitePool, filters: &CatalogFilters) -> Result<Vec<BookCard>, sqlx::Error> {
    let mut query_builder = QueryBuilder::new(BASE_SELECT);

    // Filter out duplicate copies of same book by returning the cheapest copy
    query_builder.push(r#"
        AND c.id = (
            SELECT c2.id 
            FROM book_copies c2 
            WHERE c2.book_id = b.id AND c2.is_sold = 0 
            ORDER BY c2.price ASC 
            LIMIT 1
        )
    "#);

    if let Some(ref q) = filters.q {
        let q_trimmed = q.trim();
        if !q_trimmed.is_empty() {
            query_builder.push(" AND (lower(b.search_text) LIKE ");
            query_builder.push_bind(format!("%{}%", q_trimmed.to_lowercase()));
            query_builder.push(" OR lower(a.name) LIKE ");
            query_builder.push_bind(format!("%{}%", q_trimmed.to_lowercase()));
            query_builder.push(" OR lower(g.name) LIKE ");
            query_builder.push_bind(format!("%{}%", q_trimmed.to_lowercase()));
            query_builder.push(" OR b.isbn LIKE ");
            query_builder.push_bind(format!("%{}%", q_trimmed));
            query_builder.push(" OR lower(b.tags) LIKE ");
            query_builder.push_bind(format!("%{}%", q_trimmed.to_lowercase()));
            query_builder.push(")");
        }
    }

    if let Some(ref author) = filters.author {
        if !author.is_empty() {
            query_builder.push(" AND (a.slug = ");
            query_builder.push_bind(author);
            query_builder.push(" OR a.name = ");
            query_builder.push_bind(author);
            query_builder.push(")");
        }
    }

    if let Some(ref genre) = filters.genre {
        if !genre.is_empty() && genre != "All" {
            query_builder.push(" AND (g.slug = ");
            query_builder.push_bind(genre);
            query_builder.push(" OR g.name = ");
            query_builder.push_bind(genre);
            query_builder.push(")");
        }
    }

    if let Some(ref condition) = filters.condition {
        if !condition.is_empty() && condition != "All" {
            query_builder.push(" AND c.condition = ");
            query_builder.push_bind(condition);
        }
    }

    if let Some(ref format) = filters.format {
        if !format.is_empty() && format != "All" {
            query_builder.push(" AND c.format = ");
            query_builder.push_bind(format);
        }
    }

    if let Some(ref max_price_str) = filters.max_price {
        if let Ok(max_price) = max_price_str.parse::<f64>() {
            if max_price > 0.0 {
                query_builder.push(" AND c.price <= ");
                query_builder.push_bind(max_price);
            }
        }
    }

    let sort = filters.sort.as_deref().unwrap_or("popular");
    match sort {
        "price-asc" => {
            query_builder.push(" ORDER BY c.price ASC, b.title ASC");
        }
        "price-desc" => {
            query_builder.push(" ORDER BY c.price DESC, b.title ASC");
        }
        "year-desc" => {
            query_builder.push(" ORDER BY b.year DESC, b.title ASC");
        }
        _ => {
            query_builder.push(" ORDER BY c.is_staff_pick DESC, b.is_new_arrival DESC, b.title ASC");
        }
    }

    query_builder.build_query_as::<BookCard>().fetch_all(db).await
}

pub async fn collection_books(db: &SqlitePool, slug: &str, limit: i64) -> Result<Vec<BookCard>, sqlx::Error> {
    let query = format!(r#"
        SELECT
            b.id as id,
            b.title as title,
            COALESCE(a.name, '') as author,
            COALESCE(a.slug, '') as author_slug,
            COALESCE(g.name, '') as genre,
            COALESCE(g.slug, '') as genre_slug,
            COALESCE(b.year, 0) as year,
            COALESCE(b.isbn, '') as isbn,
            b.cover_color as cover_color,
            b.aspect_ratio as aspect_ratio,
            b.tags as tags,
            b.is_new_arrival as is_new_arrival,
            c.id as copy_id,
            COALESCE(c.condition, '') as condition,
            c.price as price,
            COALESCE(c.notes, '') as notes,
            COALESCE(c.format, 'Standard') as format,
            c.stock as stock,
            c.is_staff_pick as is_staff_pick,
            COALESCE(c.staff_quote, '') as staff_quote,
            c.seal_style as seal_style,
            c.seal_text as seal_text
        FROM book_collection_items i
        JOIN books b ON b.id = i.book_id
        LEFT JOIN authors a ON a.id = b.primary_author_id
        LEFT JOIN genres g ON g.id = b.primary_genre_id
        JOIN book_copies c ON c.book_id = b.id
        WHERE i.collection_slug = ? AND i.is_active = 1 AND c.is_sold = 0
          AND c.id = (
            SELECT c2.id 
            FROM book_copies c2 
            WHERE c2.book_id = b.id AND c2.is_sold = 0 
            ORDER BY c2.price ASC 
            LIMIT 1
          )
        ORDER BY i.position ASC, c.is_staff_pick DESC, c.price ASC
        LIMIT ?
    "#);

    sqlx::query_as::<_, BookCard>(&query)
        .bind(slug)
        .bind(limit)
        .fetch_all(db)
        .await
}

pub async fn books_by_copy_ids(db: &SqlitePool, copy_ids: &[i64]) -> Result<Vec<BookCard>, sqlx::Error> {
    if copy_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut query_builder = QueryBuilder::new(BASE_SELECT);
    query_builder.push(" AND c.id IN (");
    let mut separated = query_builder.separated(", ");
    for &id in copy_ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    query_builder.build_query_as::<BookCard>().fetch_all(db).await
}

pub async fn book_by_id(db: &SqlitePool, book_id: &str) -> Result<BookCard, sqlx::Error> {
    let query = format!(r#"
        {} AND b.id = ?
        ORDER BY c.is_staff_pick DESC, c.price ASC
        LIMIT 1
    "#, BASE_SELECT);

    sqlx::query_as::<_, BookCard>(&query)
        .bind(book_id)
        .fetch_one(db)
        .await
}

pub async fn copy_stock(db: &SqlitePool, copy_id: i64) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar::<_, i32>("SELECT stock FROM book_copies WHERE id = ? AND is_sold = 0")
        .bind(copy_id)
        .fetch_one(db)
        .await
}

pub async fn copies_by_product_id(db: &SqlitePool, product_id: &str) -> Result<Vec<BookCard>, sqlx::Error> {
    let query = format!(r#"
        {} AND b.id = ?
        ORDER BY c.price ASC
    "#, BASE_SELECT);

    sqlx::query_as::<_, BookCard>(&query)
        .bind(product_id)
        .fetch_all(db)
        .await
}

pub async fn variant_attributes(db: &SqlitePool, book_id: &str) -> Result<Vec<VariantAttribute>, sqlx::Error> {
    sqlx::query_as::<_, VariantAttribute>(r#"
        SELECT a.variant_id, a.name, a.value
        FROM variant_attributes a
        JOIN book_copies c ON c.id = a.variant_id
        WHERE c.book_id = ?
    "#)
    .bind(book_id)
    .fetch_all(db)
    .await
}

pub async fn record_analytics_event(
    db: &SqlitePool,
    session_key: &str,
    payload: &AnalyticsEventPayload,
) -> Result<i64, sqlx::Error> {
    let metadata_json = payload
        .metadata
        .as_ref()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "{}".to_string());

    let result = sqlx::query(
        r#"
        INSERT INTO analytics_events (
            session_key,
            event_name,
            source,
            target_type,
            target_id,
            page_path,
            metadata_json
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(session_key)
    .bind(payload.event_name.trim())
    .bind(payload.source.as_deref().unwrap_or("").trim())
    .bind(payload.target_type.as_deref().unwrap_or("").trim())
    .bind(payload.target_id.as_deref().unwrap_or("").trim())
    .bind(payload.page_path.as_deref().unwrap_or("").trim())
    .bind(metadata_json)
    .execute(db)
    .await?;

    Ok(result.last_insert_rowid())
}
