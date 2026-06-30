use axum::{
    extract::{Form, Json, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::HashMap;
use tower_sessions::Session;

use crate::app::AppState;
use crate::cart;
use crate::db::DbPool;
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
    sqlx::query("SELECT 1").execute(&state.db).await?;
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
    headers: HeaderMap,
    Query(filters): Query<CatalogFilters>,
) -> Result<impl IntoResponse, AppError> {
    restore_cart_session(&headers, &session).await?;
    let db = &state.db;
    let books = store::list_books(db, &filters).await?;
    let all_books = store::list_books(db, &CatalogFilters::default()).await?;
    let best_sellers = store::collection_books(db, "best-sellers", 6).await?;
    let deals = store::collection_books(db, "used-deals", 6).await?;
    let staff_picks = store::collection_books(db, "staff-picks", 3).await?;
    let cart = cart::view(db, &session).await?;
    let cart_lines = ui::cart_lines(cart.lines.clone(), "#cartDrawer");
    let removed_notice = ui::removed_notice(
        cart::removed_item_view(db, &session).await?,
        "#cartDrawer",
        "cart.drawer",
    );

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
        .filter(|b| b.is_new_arrival())
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
        drawer_checkout_button: ui::checkout_start_button("cart.drawer", cart.item_count == 0),
        drawer_browse_books_link: ui::browse_books_link("cart.drawer.empty", "secondary-button"),
        cart,
        cart_lines,
        removed_notice,
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
    headers: HeaderMap,
    Path(book_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    restore_cart_session(&headers, &session).await?;
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
    let removed_notice = ui::removed_notice(
        cart::removed_item_view(db, &session).await?,
        "#cartDrawer",
        "cart.drawer",
    );

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
        drawer_checkout_button: ui::checkout_start_button("cart.drawer", cart.item_count == 0),
        drawer_browse_books_link: ui::browse_books_link("cart.drawer.empty", "secondary-button"),
        cart,
        cart_lines,
        removed_notice,
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
    headers: HeaderMap,
    Form(form): Form<AddCartForm>,
) -> Result<impl IntoResponse, AppError> {
    restore_cart_session(&headers, &session).await?;
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
    restore_cart_session(&headers, &session).await?;
    cart::change_quantity(&state.db, &session, copy_id, 1).await?;
    if wants_cart_page_fragment(&headers) {
        return render_cart_page_content(&state.db, &session).await;
    }
    render_cart(&state.db, session).await
}

pub async fn decrease_cart_item(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    cart::change_quantity(&state.db, &session, copy_id, -1).await?;
    if wants_cart_page_fragment(&headers) {
        return render_cart_page_content(&state.db, &session).await;
    }
    render_cart(&state.db, session).await
}

pub async fn remove_cart_item(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    cart::remove_with_notice(&state.db, &session, copy_id).await?;
    if wants_cart_page_fragment(&headers) {
        return render_cart_page_content(&state.db, &session).await;
    }
    render_cart(&state.db, session).await
}

pub async fn restore_cart_item(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    cart::restore_removed_item(&state.db, &session, copy_id).await?;
    if wants_cart_page_fragment(&headers) {
        return render_cart_page_content(&state.db, &session).await;
    }
    render_cart(&state.db, session).await
}

pub async fn save_cart_item_for_later(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    cart::save_for_later(&state.db, &session, copy_id).await?;
    if wants_cart_page_fragment(&headers) {
        return render_cart_page_content(&state.db, &session).await;
    }
    render_cart(&state.db, session).await
}

pub async fn move_saved_item_to_cart(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    cart::move_saved_to_cart(&state.db, &session, copy_id).await?;
    if wants_cart_page_fragment(&headers) {
        return render_cart_page_content(&state.db, &session).await;
    }
    render_cart(&state.db, session).await
}

pub async fn remove_saved_item(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(copy_id): Path<i64>,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    cart::remove_saved_item(&state.db, &session, copy_id).await?;
    if wants_cart_page_fragment(&headers) {
        return render_cart_page_content(&state.db, &session).await;
    }
    render_cart(&state.db, session).await
}

pub async fn checkout(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    let db = &state.db;
    let cart = cart::view(db, &session).await?;
    if cart.item_count == 0 {
        return Err(AppError::Validation("cart is empty".into()));
    }

    let checkout_lines = ui::checkout_lines(cart.lines.clone());
    let summary = ui::order_summary(&cart, "checkout.summary");
    let response = CheckoutTemplate {
        sections: ui::checkout_sections(),
        checkout_lines,
        summary,
    }
    .into_response();
    attach_cart_cookie(response, &session).await
}

pub async fn cart_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    let db = &state.db;
    let all_books = store::list_books(db, &CatalogFilters::default()).await?;
    let content = cart_page_content_template(db, &session).await?;

    let template = CartPageTemplate {
        genres: unique_genres(&all_books),
        cart: content.cart,
        cart_lines: content.cart_lines,
        removed_notice: content.removed_notice,
        saved_lines: content.saved_lines,
        saved_count_label: content.saved_count_label,
        checkout_button: content.checkout_button,
        browse_books_link: content.browse_books_link,
    };

    attach_cart_cookie(template.into_response(), &session).await
}

async fn cart_page_content_template(
    db: &DbPool,
    session: &Session,
) -> Result<CartPageContentTemplate, AppError> {
    let cart = cart::view(db, session).await?;
    let saved = cart::saved_view(db, session).await?;
    let cart_lines = ui::cart_page_lines(cart.lines.clone());
    let removed_notice = ui::cart_page_removed_notice(cart::removed_item_view(db, session).await?);
    let saved_lines = ui::saved_lines(saved.lines);
    let saved_count_label = match saved.item_count {
        1 => String::from("1 saved item"),
        count => format!("{} saved items", count),
    };

    let mut checkout_button = ui::ButtonView::tracked(
        "Proceed to Checkout",
        "primary-button checkout-button checkout-button--page",
        "submit",
        "checkout",
        "Proceed to checkout",
        "checkout_started",
        "cart.page",
        "checkout",
        "current",
    );
    checkout_button.disabled = cart.item_count == 0;

    Ok(CartPageContentTemplate {
        cart,
        cart_lines,
        removed_notice,
        saved_lines,
        saved_count_label,
        checkout_button,
        browse_books_link: ui::browse_books_link("cart.page.empty", "primary-button"),
    })
}

async fn render_cart_page_content(db: &DbPool, session: &Session) -> Result<Response, AppError> {
    let template = cart_page_content_template(db, session).await?;
    attach_cart_cookie(template.into_response(), session).await
}

async fn render_cart(db: &DbPool, session: Session) -> Result<Response, AppError> {
    let cart = cart::view(db, &session).await?;
    let cart_lines = ui::cart_lines(cart.lines.clone(), "#cartDrawer");
    let removed_notice = ui::removed_notice(
        cart::removed_item_view(db, &session).await?,
        "#cartDrawer",
        "cart.drawer",
    );
    let drawer_checkout_button = ui::checkout_start_button("cart.drawer", cart.item_count == 0);
    let template = CartDrawerTemplate {
        cart,
        cart_lines,
        removed_notice,
        drawer_checkout_button,
        drawer_browse_books_link: ui::browse_books_link("cart.drawer.empty", "secondary-button"),
    };
    attach_cart_cookie(template.into_response(), &session).await
}

fn wants_cart_page_fragment(headers: &HeaderMap) -> bool {
    headers
        .get("X-Cart-View")
        .and_then(|value| value.to_str().ok())
        == Some("page")
}

async fn restore_cart_session(headers: &HeaderMap, session: &Session) -> Result<(), AppError> {
    if cart::current_session_key(session).await?.is_some() {
        return Ok(());
    }
    if let Some(session_key) = cart_cookie(headers) {
        cart::adopt_session_key(session, &session_key).await?;
    }
    Ok(())
}

fn cart_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        if name == cart::BROWSER_CART_KEY_COOKIE && !value.is_empty() {
            Some(value.to_string())
        } else {
            None
        }
    })
}

async fn attach_cart_cookie(
    mut response: Response,
    session: &Session,
) -> Result<Response, AppError> {
    let Some(session_key) = cart::current_session_key(session).await? else {
        return Ok(response);
    };
    let mut cookie = format!(
        "{}={}; Path=/; Max-Age=2592000; SameSite=Lax; HttpOnly",
        cart::BROWSER_CART_KEY_COOKIE,
        session_key
    );
    if std::env::var("APP_ENV").unwrap_or_default() == "production" {
        cookie.push_str("; Secure");
    }
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    Ok(response)
}
