use axum::{
    extract::{Form, Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use tower_sessions::Session;

use crate::app::AppState;
use crate::cart;
use crate::errors::AppError;
use crate::models::*;
use crate::store;
use crate::templates::*;
use crate::ui;

// Helpers for catalog filters
fn unique_genres(books: &[BookCard]) -> Vec<String> {
    let mut genres = Vec::new();
    for b in books {
        if !b.genre.is_empty() && !genres.contains(&b.genre) {
            genres.push(b.genre.clone());
        }
    }
    genres.sort();
    genres
}

fn unique_conditions(books: &[BookCard]) -> Vec<String> {
    let mut conditions = Vec::new();
    for b in books {
        if !b.condition.is_empty() && !conditions.contains(&b.condition) {
            conditions.push(b.condition.clone());
        }
    }
    conditions.sort();
    conditions
}

fn unique_formats(books: &[BookCard]) -> Vec<String> {
    let mut formats = Vec::new();
    for b in books {
        if !b.format.is_empty() && !formats.contains(&b.format) {
            formats.push(b.format.clone());
        }
    }
    formats.sort();
    formats
}

fn result_filters(filters: CatalogFilters, count: usize, total: usize) -> CatalogFilters {
    let mut out = filters;
    out.result_text = format!("{} of {} used books shown", count, total);
    out
}

// Handlers
pub async fn healthz() -> &'static str {
    "ok"
}

pub async fn readyz(State(state): State<AppState>) -> Result<&'static str, AppError> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await?;
    Ok("ready")
}

pub async fn record_event(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<AnalyticsEventPayload>,
) -> Result<StatusCode, AppError> {
    validate_analytics_event(&payload)?;
    let session_key = analytics_session_key(&session).await?;
    store::record_analytics_event(&state.db, &session_key, &payload).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn analytics_session_key(session: &Session) -> Result<String, AppError> {
    if let Some(session_key) = session.get::<String>("analytics_session_key").await? {
        return Ok(session_key);
    }

    let session_key = uuid::Uuid::new_v4().to_string();
    session
        .insert("analytics_session_key", &session_key)
        .await?;
    Ok(session_key)
}

fn validate_analytics_event(payload: &AnalyticsEventPayload) -> Result<(), AppError> {
    let event_name = payload.event_name.trim();
    if event_name.is_empty() || event_name.len() > 80 {
        return Err(AppError::Validation("invalid analytics event".into()));
    }

    for value in [
        payload.source.as_deref(),
        payload.target_type.as_deref(),
        payload.target_id.as_deref(),
        payload.page_path.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if value.len() > 512 {
            return Err(AppError::Validation("analytics field too long".into()));
        }
    }

    Ok(())
}

pub async fn home(
    State(state): State<AppState>,
    session: Session,
    Query(filters): Query<CatalogFilters>,
) -> Result<impl IntoResponse, AppError> {
    let db = &state.db;
    let books = store::list_books(db, &filters).await?;
    let all_books = store::list_books(db, &CatalogFilters::default()).await?;
    let best_sellers = store::collection_books(db, "best-sellers", 6).await?;
    let deals = store::collection_books(db, "used-deals", 6).await?;
    let staff_picks = store::collection_books(db, "staff-picks", 3).await?;
    let cart = cart::view(db, &session).await?;
    let cart_lines = ui::cart_lines(cart.lines.clone(), "#cartDrawer");

    let featured = all_books
        .iter()
        .find(|b| b.id == "b005")
        .cloned()
        .unwrap_or_else(|| all_books[0].clone());
    let featured_add_button = ui::ButtonView::cart_action(
        "Add to Cart",
        "card-btn add-btn",
        "add",
        "add_to_cart_clicked",
        &featured,
        "home.featured_deal",
    );
    let featured_buy_now_button = ui::ButtonView::cart_action(
        "Buy Now",
        "card-btn buy-now-btn",
        "buy-now-card",
        "buy_now_clicked",
        &featured,
        "home.featured_deal",
    );

    let quick_fillers: Vec<BookCard> = all_books
        .iter()
        .filter(|b| b.price < 8.0)
        .take(2)
        .cloned()
        .collect();

    let new_arrivals: Vec<BookCard> = all_books
        .iter()
        .filter(|b| b.is_new_arrival)
        .take(6)
        .cloned()
        .collect();

    let product_sections = vec![
        ui::product_shelf(
            "best-sellers",
            "Best sellers",
            "Readers keep grabbing these",
            "bestSellerShelf",
            "home.best_sellers",
            best_sellers,
        ),
        ui::product_shelf(
            "new-arrivals",
            "New arrivals",
            "Just checked in",
            "newArrivalShelf",
            "home.new_arrivals",
            new_arrivals,
        )
        .with_cta("#catalog", "Browse new arrivals"),
        ui::product_shelf(
            "deals",
            "Deals",
            "Used books under $8",
            "dealShelf",
            "home.deals",
            deals,
        ),
    ];

    let template = HomeTemplate {
        title: String::from("Davis's Books | Used Books Online"),
        genres: unique_genres(&all_books),
        conditions: unique_conditions(&all_books),
        formats: unique_formats(&all_books),
        featured,
        featured_add_button,
        featured_buy_now_button,
        quick_fillers,
        product_sections,
        catalog_cards: ui::product_cards(books.clone(), "catalog.results"),
        staff_picks,
        cart,
        cart_lines,
        filters: result_filters(filters, books.len(), all_books.len()),
    };

    Ok(template)
}

pub async fn catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filters): Query<CatalogFilters>,
) -> Result<impl IntoResponse, AppError> {
    let db = &state.db;
    let books = store::list_books(db, &filters).await?;
    let all_books = store::list_books(db, &CatalogFilters::default()).await?;

    if headers.get("HX-Request").and_then(|v| v.to_str().ok()) == Some("true") {
        let template = CatalogResultsTemplate {
            catalog_cards: ui::product_cards(books.clone(), "catalog.results"),
            filters: result_filters(filters, books.len(), all_books.len()),
        };
        Ok(template.into_response())
    } else {
        // Redirect non-HTMX requests to homepage with catalog anchor
        let redirect = axum::response::Redirect::to("/#catalog");
        Ok(redirect.into_response())
    }
}

pub async fn book_detail(
    State(state): State<AppState>,
    session: Session,
    Path(book_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let db = &state.db;
    let book = match store::book_by_id(db, &book_id).await {
        Ok(b) => b,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound),
        Err(err) => return Err(err.into()),
    };

    let copies = store::copies_by_product_id(db, &book_id).await?;
    let raw_attribs = store::variant_attributes(db, &book_id).await?;

    let mut attributes = HashMap::new();
    for attr in raw_attribs {
        attributes
            .entry(attr.variant_id)
            .or_insert_with(Vec::new)
            .push(attr);
    }

    let all_books = store::list_books(db, &CatalogFilters::default()).await?;
    let cart = cart::view(db, &session).await?;
    let cart_lines = ui::cart_lines(cart.lines.clone(), "#cartDrawer");

    let related: Vec<BookCard> = all_books
        .iter()
        .filter(|b| b.genre == book.genre && b.id != book.id)
        .take(4)
        .cloned()
        .collect();
    let add_button = ui::ButtonView::cart_action(
        "Add to Stack",
        "buybox-btn add-to-stack",
        "add",
        "add_to_cart_clicked",
        &book,
        "book_detail.buybox",
    );
    let buy_now_button = ui::ButtonView::cart_action(
        "Buy Now",
        "buybox-btn buy-now",
        "buy-now-card",
        "buy_now_clicked",
        &book,
        "book_detail.buybox",
    );

    let template = BookDetailTemplate {
        genres: unique_genres(&all_books),
        book,
        copies,
        attributes,
        related_cards: ui::product_cards(related, "book_detail.related"),
        add_button,
        buy_now_button,
        cart,
        cart_lines,
    };

    Ok(template)
}

#[derive(Deserialize)]
pub struct AddCartForm {
    pub copy_id: String,
}

pub async fn add_cart_item(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<AddCartForm>,
) -> Result<impl IntoResponse, AppError> {
    let db = &state.db;
    let copy_id = form
        .copy_id
        .parse::<i64>()
        .map_err(|_| AppError::Validation("invalid copy id".into()))?;
    if copy_id < 1 {
        return Err(AppError::Validation("invalid copy".into()));
    }

    cart::add_one(db, &session, copy_id).await?;

    render_cart(db, session).await
}

pub async fn increase_cart_item(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    cart::change_quantity(&state.db, &session, copy_id, 1).await?;
    if wants_cart_page_redirect(&headers) {
        return Ok(cart_page_redirect());
    }
    render_cart(&state.db, session).await
}

pub async fn decrease_cart_item(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    cart::change_quantity(&state.db, &session, copy_id, -1).await?;
    if wants_cart_page_redirect(&headers) {
        return Ok(cart_page_redirect());
    }
    render_cart(&state.db, session).await
}

pub async fn remove_cart_item(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    cart::set_quantity(&state.db, &session, copy_id, 0).await?;
    if wants_cart_page_redirect(&headers) {
        return Ok(cart_page_redirect());
    }
    render_cart(&state.db, session).await
}

pub async fn checkout(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let db = &state.db;
    let cart = cart::view(db, &session).await?;
    if cart.item_count == 0 {
        return Err(AppError::Validation("cart is empty".into()));
    }

    // In production, this redirect to Stripe Checkout, clear sessions on webhook, etc.
    let body = format!(
        "Stripe Checkout will be connected in Phase 4. Cart total: ${:.2}",
        cart.total
    );
    Ok(body)
}

pub async fn cart_page(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let db = &state.db;
    let all_books = store::list_books(db, &CatalogFilters::default()).await?;
    let cart = cart::view(db, &session).await?;

    let template = CartPageTemplate {
        genres: unique_genres(&all_books),
        cart,
    };

    Ok(template)
}

async fn render_cart(db: &SqlitePool, session: Session) -> Result<Response, AppError> {
    let cart = cart::view(db, &session).await?;
    let cart_lines = ui::cart_lines(cart.lines.clone(), "#cartDrawer");
    let template = CartDrawerTemplate { cart, cart_lines };
    Ok(template.into_response())
}

fn wants_cart_page_redirect(headers: &HeaderMap) -> bool {
    headers
        .get("X-Cart-View")
        .and_then(|value| value.to_str().ok())
        == Some("page")
}

fn cart_page_redirect() -> Response {
    (StatusCode::NO_CONTENT, [("HX-Redirect", "/cart")]).into_response()
}
