use crate::catalog_cache::{cached, Lifetime};
use crate::db::{Db, DbPool};
use crate::models::{AnalyticsEventPayload, BookCard, CatalogFilters, VariantAttribute};
use crate::read_budget::{ReadFutureExt, RowsFutureExt};
use rust_decimal::Decimal;
use sqlx::QueryBuilder;

const BASE_SELECT: &str = r#"
    SELECT
        b.id as id,
        left(b.title, 512) as title,
        left(COALESCE(a.name, ''), 512) as author,
        left(COALESCE(a.slug, ''),512) as author_slug,
        left(COALESCE(g.name, ''),512) as genre,
        left(COALESCE(g.slug, ''),512) as genre_slug,
        COALESCE(b.year, 0) as year,
        left(COALESCE(b.isbn, ''),32) as isbn,
        ''::text as description,
        left(COALESCE(NULLIF(b.cover_url, ''), '/assets/covers/' || b.id || '.jpg'),2048) as cover_url,
        left(b.cover_color,64) as cover_color,
        b.aspect_ratio::float8 as aspect_ratio,
        ''::text as tags,
        b.is_new_arrival as is_new_arrival,
        c.id as copy_id,
        COALESCE(c.condition, '') as condition,
        c.is_new as is_new,
        c.price::float8 as price,
        c.list_price::float8 as list_price,
        left(COALESCE(c.notes, ''), 2048) as notes,
        left(COALESCE(c.format, 'Standard'),128) as format,
        c.stock as stock,
        c.is_staff_pick as is_staff_pick,
        left(COALESCE(c.staff_quote, ''), 2048) as staff_quote,
        left(c.seal_style,64) as seal_style,
        left(c.seal_text,128) as seal_text
    FROM books b
    LEFT JOIN authors a ON a.id = b.primary_author_id
    LEFT JOIN genres g ON g.id = b.primary_genre_id
    JOIN book_copies c ON c.book_id = b.id
    LEFT JOIN review_aggregates ra ON ra.book_id = b.id
    WHERE c.is_sold = false
"#;

async fn load_list_books(
    db: &DbPool,
    filters: &CatalogFilters,
) -> Result<Vec<BookCard>, sqlx::Error> {
    let mut query_builder: QueryBuilder<Db> = QueryBuilder::new(BASE_SELECT);

    append_catalog_filters(&mut query_builder, filters);

    let sort = filters.sort.as_deref().unwrap_or("popular");
    match sort {
        "price-asc" => {
            query_builder.push(" ORDER BY c.price ASC, b.title ASC, b.id ASC");
        }
        "price-desc" => {
            query_builder.push(" ORDER BY c.price DESC, b.title ASC, b.id ASC");
        }
        "year-desc" => {
            query_builder.push(" ORDER BY b.year DESC, b.title ASC, b.id ASC");
        }
        _ => {
            query_builder.push(
                " ORDER BY c.is_staff_pick DESC, b.is_new_arrival DESC, b.title ASC, b.id ASC",
            );
        }
    }

    let per_page = filters.per_page.unwrap_or(24) as usize;
    crate::read_budget::limit(per_page)?;
    let page = filters.page.unwrap_or(1);
    if page == 0 {
        return Err(sqlx::Error::Protocol("page must be positive".into()));
    }
    query_builder
        .push(" LIMIT ")
        .push_bind(per_page as i64)
        .push(" OFFSET ")
        .push_bind((i64::from(page) - 1) * per_page as i64);

    query_builder
        .build_query_as::<BookCard>()
        .fetch_all(db)
        .bounded_rows(per_page)
        .await
}

async fn load_count_books(db: &DbPool, filters: &CatalogFilters) -> Result<i64, sqlx::Error> {
    let from = BASE_SELECT
        .split_once("    FROM books b")
        .expect("canonical catalog projection")
        .1;
    let mut query_builder: QueryBuilder<Db> =
        QueryBuilder::new(format!("SELECT COUNT(*) FROM books b{from}"));
    append_catalog_filters(&mut query_builder, filters);
    let row: (i64,) = query_builder
        .build_query_as()
        .fetch_one(db)
        .bounded_one()
        .await?;
    Ok(row.0)
}

fn append_catalog_filters<'a>(
    query_builder: &mut QueryBuilder<'a, Db>,
    filters: &'a CatalogFilters,
) {
    // Filter out duplicate copies of same book by returning the cheapest copy
    query_builder.push(
        r#"
        AND c.id = (
            SELECT c2.id
            FROM book_copies c2
            WHERE c2.book_id = b.id AND c2.is_sold = false
            ORDER BY c2.price ASC, c2.id ASC
            LIMIT 1
        )
    "#,
    );

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

    let wants_new = filters.listing.as_deref().unwrap_or("").trim() == "new";
    let wants_used = filters.listing.as_deref().unwrap_or("").trim() == "used";
    match (wants_new, wants_used) {
        (true, false) => query_builder.push(" AND c.is_new = true"),
        (false, true) => query_builder.push(" AND c.is_new = false"),
        _ => &mut *query_builder,
    };

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

    if let Some(ref min_rating_str) = filters.min_rating {
        if let Ok(min_rating) = min_rating_str.parse::<f64>() {
            if (1.0..=5.0).contains(&min_rating) {
                query_builder.push(
                    " AND (ra.book_id IS NULL OR ra.published_count = 0 OR ra.average_rating ",
                );
                if min_rating >= 5.0 {
                    query_builder.push("> ");
                    query_builder.push_bind(Decimal::from(4));
                } else {
                    query_builder.push(">= ");
                    query_builder.push_bind(Decimal::from(min_rating as i32));
                }
                query_builder.push(")");
            }
        }
    }
}

async fn load_catalog_facets(
    db: &DbPool,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), sqlx::Error> {
    let genres = sqlx::query_scalar::<Db, String>(
        "SELECT DISTINCT g.name FROM genres g JOIN books b ON b.primary_genre_id = g.id WHERE g.name <> '' ORDER BY g.name LIMIT 24 OFFSET $1",
    ).bind(crate::pages::offset("genre_page",24))
    .fetch_all(db).bounded_rows(24)
    .await?;
    let conditions = sqlx::query_scalar::<Db, String>(
        "SELECT DISTINCT c.condition FROM book_copies c WHERE c.is_sold = false AND c.condition <> '' ORDER BY c.condition LIMIT 8 OFFSET $1",
    ).bind(crate::pages::offset("condition_page",8))
    .fetch_all(db).bounded_rows(8)
    .await?;
    let formats = sqlx::query_scalar::<Db, String>(
        "SELECT DISTINCT c.format FROM book_copies c WHERE c.is_sold = false AND COALESCE(c.format, '') <> '' ORDER BY c.format LIMIT 12 OFFSET $1",
    ).bind(crate::pages::offset("format_page",12))
    .fetch_all(db).bounded_rows(12)
    .await?;
    Ok((genres, conditions, formats))
}

async fn load_collection_books(
    db: &DbPool,
    slug: &str,
    limit: i64,
) -> Result<Vec<BookCard>, sqlx::Error> {
    let limit = usize::try_from(limit)
        .map_err(|_| sqlx::Error::Protocol("invalid collection limit".into()))?;
    crate::read_budget::limit(limit)?;
    let query = r#"
        SELECT
            b.id as id,
            left(b.title, 512) as title,
            left(COALESCE(a.name, ''), 512) as author,
            left(COALESCE(a.slug, ''),512) as author_slug,
            left(COALESCE(g.name, ''),512) as genre,
            left(COALESCE(g.slug, ''),512) as genre_slug,
            COALESCE(b.year, 0) as year,
            left(COALESCE(b.isbn, ''),32) as isbn,
            ''::text as description,
        left(COALESCE(NULLIF(b.cover_url, ''), '/assets/covers/' || b.id || '.jpg'),2048) as cover_url,
        left(b.cover_color,64) as cover_color,
            b.aspect_ratio::float8 as aspect_ratio,
            ''::text as tags,
            b.is_new_arrival as is_new_arrival,
            c.id as copy_id,
            COALESCE(c.condition, '') as condition,
            c.is_new as is_new,
            c.price::float8 as price,
            c.list_price::float8 as list_price,
            left(COALESCE(c.notes, ''), 2048) as notes,
            left(COALESCE(c.format, 'Standard'),128) as format,
            c.stock as stock,
            c.is_staff_pick as is_staff_pick,
            left(COALESCE(c.staff_quote, ''), 2048) as staff_quote,
            left(c.seal_style,64) as seal_style,
            left(c.seal_text,128) as seal_text
        FROM book_collection_items i
        JOIN books b ON b.id = i.book_id
        LEFT JOIN authors a ON a.id = b.primary_author_id
        LEFT JOIN genres g ON g.id = b.primary_genre_id
        JOIN book_copies c ON c.book_id = b.id
        WHERE i.collection_slug = $1 AND i.is_active = true AND c.is_sold = false
          AND c.id = (
            SELECT c2.id
            FROM book_copies c2
            WHERE c2.book_id = b.id AND c2.is_sold = false
            ORDER BY c2.price ASC, c2.id ASC
            LIMIT 1
          )
        ORDER BY i.position ASC, c.is_staff_pick DESC, c.price ASC
        LIMIT $2
    "#;

    sqlx::query_as::<_, BookCard>(query)
        .bind(slug)
        .bind(limit as i64)
        .fetch_all(db)
        .bounded_rows(limit)
        .await
}

pub async fn books_by_copy_ids(
    db: &DbPool,
    copy_ids: &[i64],
) -> Result<Vec<BookCard>, sqlx::Error> {
    let mut copy_ids = copy_ids.to_vec();
    copy_ids.sort_unstable();
    copy_ids.dedup();
    if copy_ids.len() > 100 {
        return Err(sqlx::Error::Protocol("copy batch exceeds 100".into()));
    }
    if copy_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut query_builder: QueryBuilder<Db> = QueryBuilder::new(BASE_SELECT);
    query_builder.push(" AND c.id IN (");
    let mut separated = query_builder.separated(", ");
    for &id in &copy_ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    query_builder.push(" LIMIT 100");
    query_builder
        .build_query_as::<BookCard>()
        .fetch_all(db)
        .bounded_rows(copy_ids.len())
        .await
}

async fn load_book_by_id(db: &DbPool, book_id: &str) -> Result<BookCard, sqlx::Error> {
    let query = format!(
        r#"
        {} AND b.id = $1
        ORDER BY c.is_staff_pick DESC, c.price ASC
        LIMIT 1
    "#,
        BASE_SELECT
    );

    sqlx::query_as::<_, BookCard>(&query)
        .bind(book_id)
        .fetch_one(db)
        .bounded_one()
        .await
}

pub async fn copy_stock(db: &DbPool, copy_id: i64) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar::<_, i32>("SELECT stock FROM book_copies WHERE id = $1 AND is_sold = false")
        .bind(copy_id)
        .fetch_one(db)
        .bounded_one()
        .await
}

async fn load_copies_by_product_id(
    db: &DbPool,
    product_id: &str,
) -> Result<Vec<BookCard>, sqlx::Error> {
    let query = format!(
        r#"
        {} AND b.id = $1
        ORDER BY c.price ASC, c.id ASC LIMIT 10 OFFSET $2
    "#,
        BASE_SELECT
    );

    sqlx::query_as::<_, BookCard>(&query)
        .bind(product_id)
        .bind(crate::pages::offset("copy_page", 10))
        .fetch_all(db)
        .bounded_rows(10)
        .await
}

async fn load_variant_attributes(
    db: &DbPool,
    book_id: &str,
) -> Result<Vec<VariantAttribute>, sqlx::Error> {
    sqlx::query_as::<_, VariantAttribute>(
        r#"
        SELECT a.variant_id, left(a.name,128) as name, left(a.value,2048) as value
        FROM variant_attributes a
        JOIN book_copies c ON c.id = a.variant_id
        WHERE c.book_id = $1 AND c.id IN (SELECT id FROM book_copies WHERE book_id=$1 AND is_sold=false ORDER BY price,id LIMIT 10 OFFSET $2)
        ORDER BY a.variant_id, a.name LIMIT 50 OFFSET $3
    "#,
    )
    .bind(book_id).bind(crate::pages::offset("copy_page",10)).bind(crate::pages::offset("attribute_page",50))
    .fetch_all(db)
    .bounded_rows(50)
    .await
}

pub async fn record_analytics_event(
    db: &DbPool,
    session_key: &str,
    payload: &AnalyticsEventPayload,
) -> Result<i64, sqlx::Error> {
    let metadata_json = payload
        .metadata
        .as_ref()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "{}".to_string());

    sqlx::query_scalar::<_, i64>(
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
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(session_key)
    .bind(payload.event_name.trim())
    .bind(payload.source.as_deref().unwrap_or("").trim())
    .bind(payload.target_type.as_deref().unwrap_or("").trim())
    .bind(payload.target_id.as_deref().unwrap_or("").trim())
    .bind(payload.page_path.as_deref().unwrap_or("").trim())
    .bind(metadata_json)
    .fetch_one(db)
    .bounded_one()
    .await
}

pub async fn list_books(
    db: &DbPool,
    filters: &CatalogFilters,
) -> Result<Vec<BookCard>, sqlx::Error> {
    let filters = normalize_filters(filters)?;

    cached(
        "list_books",
        serde_json::to_string(&filters).map_err(|e| sqlx::Error::Encode(Box::new(e)))?,
        Lifetime::Offers,
        load_list_books(db, &filters),
    )
    .await
}

pub async fn count_books(db: &DbPool, filters: &CatalogFilters) -> Result<i64, sqlx::Error> {
    let filters = normalize_filters(filters)?;
    let mut filters = filters;
    filters.page = None;
    filters.per_page = None;
    filters.sort = None;

    cached(
        "count_books",
        serde_json::to_string(&filters).map_err(|e| sqlx::Error::Encode(Box::new(e)))?,
        Lifetime::Counts,
        load_count_books(db, &filters),
    )
    .await
}

pub async fn catalog_facets(
    db: &DbPool,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), sqlx::Error> {
    cached(
        "catalog_facets",
        format!(
            "{}:{}:{}",
            crate::pages::number("genre_page"),
            crate::pages::number("condition_page"),
            crate::pages::number("format_page")
        ),
        Lifetime::Metadata,
        load_catalog_facets(db),
    )
    .await
}

pub async fn collection_books(
    db: &DbPool,
    slug: &str,
    limit: i64,
) -> Result<Vec<BookCard>, sqlx::Error> {
    cached(
        "collection_books",
        format!("{slug}:{limit}"),
        Lifetime::Offers,
        load_collection_books(db, slug, limit),
    )
    .await
}

pub async fn book_by_id(db: &DbPool, book_id: &str) -> Result<BookCard, sqlx::Error> {
    cached(
        "book_by_id",
        book_id.to_owned(),
        Lifetime::Offers,
        load_book_by_id(db, book_id),
    )
    .await
}

pub async fn copies_by_product_id(
    db: &DbPool,
    product_id: &str,
) -> Result<Vec<BookCard>, sqlx::Error> {
    cached(
        "copies_by_product_id",
        format!("{product_id}:{}", crate::pages::number("copy_page")),
        Lifetime::Offers,
        load_copies_by_product_id(db, product_id),
    )
    .await
}

pub async fn variant_attributes(
    db: &DbPool,
    book_id: &str,
) -> Result<Vec<VariantAttribute>, sqlx::Error> {
    cached(
        "variant_attributes",
        format!(
            "{book_id}:{}:{}",
            crate::pages::number("copy_page"),
            crate::pages::number("attribute_page")
        ),
        Lifetime::Metadata,
        load_variant_attributes(db, book_id),
    )
    .await
}

/// Small homepage/related shelves; predicates are chosen here, never supplied as SQL by callers.
pub async fn shelf(
    db: &DbPool,
    kind: &str,
    genre: &str,
    exclude: &str,
) -> Result<Vec<BookCard>, sqlx::Error> {
    let (predicate, max) = match kind {
        "fillers" => ("c.price < 8", 4),
        "arrivals" => ("b.is_new_arrival = true", 6),
        "related" => ("COALESCE(g.name, '') = $1 AND b.id <> $2", 4),
        "featured" => ("true", 1),
        _ => return Err(sqlx::Error::Protocol("unknown shelf".into())),
    };
    cached("shelf",format!("{kind}:{genre}:{exclude}"),Lifetime::Offers,async {
        let sql=format!("{BASE_SELECT} AND {predicate} AND c.id = (SELECT c2.id FROM book_copies c2 WHERE c2.book_id=b.id AND c2.is_sold=false ORDER BY c2.price,c2.id LIMIT 1) ORDER BY c.is_staff_pick DESC,b.is_new_arrival DESC,b.title,b.id LIMIT {max}");
        let mut q=sqlx::query_as::<_,BookCard>(&sql);
        if kind=="related" { q=q.bind(genre).bind(exclude); }
        q.fetch_all(db).bounded_rows(max).await
    }).await
}

pub async fn genres(db: &DbPool) -> Result<Vec<String>, sqlx::Error> {
    cached("genres",String::new(),Lifetime::Metadata,async {
        sqlx::query_scalar("SELECT DISTINCT left(g.name,512) FROM genres g JOIN books b ON b.primary_genre_id=g.id WHERE g.name<>'' ORDER BY 1 LIMIT 24")
            .fetch_all(db).bounded_rows(24).await
    }).await
}

pub async fn description(db: &DbPool, id: &str) -> Result<String, sqlx::Error> {
    cached("description", id.to_owned(), Lifetime::Metadata, async {
        sqlx::query_scalar("SELECT left(description,32768) FROM books WHERE id=$1 LIMIT 1")
            .bind(id)
            .fetch_one(db)
            .bounded_one()
            .await
    })
    .await
}

pub async fn facet_counts(db: &DbPool) -> Result<(i64, i64, i64), sqlx::Error> {
    cached("facet_counts",String::new(),Lifetime::Metadata,async {
        sqlx::query_as("SELECT (SELECT count(DISTINCT g.name) FROM genres g JOIN books b ON b.primary_genre_id=g.id WHERE g.name<>''),(SELECT count(DISTINCT condition) FROM book_copies WHERE is_sold=false AND condition<>''),(SELECT count(DISTINCT format) FROM book_copies WHERE is_sold=false AND COALESCE(format,'')<>'')")
            .fetch_one(db).bounded_one().await
    }).await
}
pub async fn detail_counts(db: &DbPool, id: &str) -> Result<(i64, i64), sqlx::Error> {
    cached("detail_counts",format!("{id}:{}",crate::pages::number("copy_page")),Lifetime::Offers,async {
        sqlx::query_as("SELECT (SELECT count(*) FROM book_copies WHERE book_id=$1 AND is_sold=false),(SELECT count(*) FROM variant_attributes WHERE variant_id IN(SELECT id FROM book_copies WHERE book_id=$1 AND is_sold=false ORDER BY price,id LIMIT 10 OFFSET $2))")
            .bind(id).bind(crate::pages::offset("copy_page",10)).fetch_one(db).bounded_one().await
    }).await
}

fn normalize_filters(filters: &CatalogFilters) -> Result<CatalogFilters, sqlx::Error> {
    let mut filters = filters.clone();
    for value in [
        &mut filters.q,
        &mut filters.author,
        &mut filters.genre,
        &mut filters.condition,
        &mut filters.listing,
        &mut filters.max_price,
        &mut filters.format,
        &mut filters.min_rating,
        &mut filters.sort,
    ] {
        if let Some(text) = value {
            if text.len() > 256 {
                return Err(sqlx::Error::Protocol("catalog filter is too long".into()));
            }
            *text = text.trim().to_owned();
            if text.is_empty() {
                *value = None;
            }
        }
    }
    for value in [
        &mut filters.genre,
        &mut filters.condition,
        &mut filters.format,
    ] {
        if value.as_deref() == Some("All") {
            *value = None;
        }
    }
    if let Some(q) = &mut filters.q {
        *q = q.to_lowercase();
    }
    filters.page = Some(filters.page.unwrap_or(1).max(1));
    filters.per_page = Some(filters.per_page.unwrap_or(24));
    crate::read_budget::limit(filters.per_page.unwrap() as usize)?;
    if !matches!(
        filters.sort.as_deref(),
        Some("price-asc" | "price-desc" | "year-desc")
    ) {
        filters.sort = None;
    }
    Ok(filters)
}
