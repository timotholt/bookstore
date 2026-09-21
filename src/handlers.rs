use axum::{
    extract::{Form, Json, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use secrecy::ExposeSecret;
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

fn result_filters(filters: CatalogFilters, count: usize, total: usize) -> CatalogFilters {
    let mut out = filters;
    let per_page = out
        .per_page
        .filter(|value| matches!(value, 24 | 48 | 96))
        .unwrap_or(24);
    let total_pages = if total == 0 {
        1
    } else {
        (total as u32).div_ceil(per_page)
    };
    let page = out.page.unwrap_or(1).max(1).min(total_pages);
    let first = if total == 0 {
        0
    } else {
        ((page - 1) as usize * per_page as usize) + 1
    };
    let last = if total == 0 {
        0
    } else {
        first + count.saturating_sub(1)
    };
    out.result_text = if total == 0 {
        "No results".to_string()
    } else if total == 1 {
        "Showing 1 result".to_string()
    } else {
        format!("Showing {}–{} of {} results", first, last.min(total), total)
    };
    out.total_items = total;
    out.total_pages = total_pages;
    out.page = Some(page);
    out
}

fn catalog_results_response(
    books: Vec<BookCard>,
    filters: CatalogFilters,
    total: usize,
    source: &'static str,
) -> Response {
    let count = books.len();
    CatalogResultsTemplate {
        catalog_cards: ui::product_cards(books, source),
        filters: result_filters(filters, count, total),
    }
    .into_response()
}

fn listing_checked(filters: &CatalogFilters, option: &str) -> bool {
    filters
        .listing
        .as_deref()
        .map(|listing| listing.trim().is_empty() || listing.trim() == option)
        .unwrap_or(true)
}

struct StoreChrome {
    genres: Vec<String>,
    cart: CartView,
    cart_lines: Vec<ui::CartLineView>,
    removed_notice: Option<ui::RemovedCartNoticeView>,
    drawer_checkout_button: ui::ButtonView,
    drawer_browse_books_link: ui::LinkView,
}

async fn store_chrome(db: &DbPool, session: &Session) -> Result<StoreChrome, AppError> {
    let all_books = store::list_books(db, &CatalogFilters::default()).await?;
    let cart = cart::view(db, session).await?;
    let cart_is_empty = cart.item_count == 0;
    let cart_lines = ui::cart_lines(cart.lines.clone(), "#cartDrawer");
    let removed_notice = ui::removed_notice(
        cart::removed_item_view(db, session).await?,
        "#cartDrawer",
        "cart.drawer",
    );

    Ok(StoreChrome {
        genres: unique_genres(&all_books),
        cart,
        cart_lines,
        removed_notice,
        drawer_checkout_button: ui::checkout_start_button("cart.drawer", cart_is_empty),
        drawer_browse_books_link: ui::browse_books_link("cart.drawer.empty", "secondary-button"),
    })
}

async fn signup_template_response(
    db: &DbPool,
    session: &Session,
    error_message: Option<String>,
    email: String,
    first_name: String,
    last_name: String,
) -> Result<Response, AppError> {
    let chrome = store_chrome(db, session).await?;
    let current_user = crate::auth::get_current_user(db, session).await?;

    Ok(SignupTemplate {
        csrf: crate::account_email::csrf(session).await,
        error_message,
        email,
        first_name,
        last_name,
        genres: chrome.genres,
        cart: chrome.cart,
        cart_lines: chrome.cart_lines,
        removed_notice: chrome.removed_notice,
        drawer_checkout_button: chrome.drawer_checkout_button,
        drawer_browse_books_link: chrome.drawer_browse_books_link,
        current_user,
    }
    .into_response())
}

async fn login_template_response(
    db: &DbPool,
    session: &Session,
    error_message: Option<String>,
    email: String,
) -> Result<Response, AppError> {
    let error_message = if session
        .remove::<bool>("reset_completed")
        .await
        .ok()
        .flatten()
        == Some(true)
    {
        Some("Your password was reset. Other sessions have been signed out. Sign in with your new password.".into())
    } else {
        error_message
    };
    let chrome = store_chrome(db, session).await?;
    let current_user = crate::auth::get_current_user(db, session).await?;

    Ok(LoginTemplate {
        csrf: crate::account_email::csrf(session).await,
        error_message,
        email,
        genres: chrome.genres,
        cart: chrome.cart,
        cart_lines: chrome.cart_lines,
        removed_notice: chrome.removed_notice,
        drawer_checkout_button: chrome.drawer_checkout_button,
        drawer_browse_books_link: chrome.drawer_browse_books_link,
        current_user,
    }
    .into_response())
}

// Handlers
pub async fn healthz() -> &'static str {
    "ok"
}

pub async fn version() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "service": "chantels-corner",
        "version": env!("CARGO_PKG_VERSION"),
        "commit": std::env::var("RAILWAY_GIT_COMMIT_SHA")
            .or_else(|_| std::env::var("BUILD_COMMIT"))
            .unwrap_or_else(|_| "unknown".into()),
        "deployment": std::env::var("RAILWAY_DEPLOYMENT_ID")
            .unwrap_or_else(|_| "unknown".into()),
        "branch": std::env::var("RAILWAY_GIT_BRANCH")
            .unwrap_or_else(|_| "unknown".into()),
        "commit_message": std::env::var("RAILWAY_GIT_COMMIT_MESSAGE")
            .unwrap_or_else(|_| "unknown".into()),
        "built_at": option_env!("BUILD_TIMESTAMP_PACIFIC").unwrap_or("unknown"),
        "built_timezone": "America/Los_Angeles",
        "built_at_utc": option_env!("BUILD_TIMESTAMP_UTC").unwrap_or("unknown"),
    }))
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
) -> Result<impl IntoResponse, AppError> {
    restore_cart_session(&headers, &session).await?;
    let db = &state.db;
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
        .with_cta("/search", "Browse new arrivals"),
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
        title: format!("{} | Used Books Online", crate::brand::STORE_NAME),
        genres: unique_genres(&all_books),
        featured,
        featured_add_button,
        featured_buy_now_button,
        quick_fillers,
        product_sections,
        staff_picks,
        drawer_checkout_button: ui::checkout_start_button("cart.drawer", cart.item_count == 0),
        drawer_browse_books_link: ui::browse_books_link("cart.drawer.empty", "secondary-button"),
        cart,
        cart_lines,
        removed_notice,
        current_user: crate::auth::get_current_user(db, &session)
            .await
            .unwrap_or(None),
    };

    Ok(template)
}

pub async fn catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(mut filters): Query<CatalogFilters>,
) -> Result<impl IntoResponse, AppError> {
    let db = &state.db;
    filters.page = Some(filters.page.unwrap_or(1).max(1));
    filters.per_page = Some(
        filters
            .per_page
            .filter(|value| matches!(value, 24 | 48 | 96))
            .unwrap_or(24),
    );
    let books = store::list_books(db, &filters).await?;
    let total = store::count_books(db, &filters).await? as usize;

    if headers.get("HX-Request").and_then(|v| v.to_str().ok()) == Some("true") {
        Ok(catalog_results_response(
            books,
            filters,
            total,
            "catalog.results",
        ))
    } else {
        let redirect = axum::response::Redirect::to("/search");
        Ok(redirect.into_response())
    }
}

pub async fn search_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(mut filters): Query<CatalogFilters>,
) -> Result<impl IntoResponse, AppError> {
    restore_cart_session(&headers, &session).await?;
    let db = &state.db;
    filters.page = Some(filters.page.unwrap_or(1).max(1));
    filters.per_page = Some(
        filters
            .per_page
            .filter(|value| matches!(value, 24 | 48 | 96))
            .unwrap_or(24),
    );
    let books = store::list_books(db, &filters).await?;
    let total = store::count_books(db, &filters).await? as usize;

    if headers.get("HX-Request").and_then(|v| v.to_str().ok()) == Some("true") {
        return Ok(catalog_results_response(
            books,
            filters,
            total,
            "search.results",
        ));
    }

    let (genres, conditions, formats) = store::catalog_facets(db).await?;
    let cart = cart::view(db, &session).await?;
    let cart_lines = ui::cart_lines(cart.lines.clone(), "#cartDrawer");
    let removed_notice = ui::removed_notice(
        cart::removed_item_view(db, &session).await?,
        "#cartDrawer",
        "cart.drawer",
    );
    let query = filters.q.clone().unwrap_or_default();
    let show_new_checked = listing_checked(&filters, "new");
    let show_used_checked = listing_checked(&filters, "used");
    let min_rating = filters.min_rating.clone().unwrap_or_default();

    let template = SearchTemplate {
        title: if query.is_empty() {
            format!("Search | {}", crate::brand::STORE_NAME)
        } else {
            format!(
                "Search results for \"{}\" | {}",
                query,
                crate::brand::STORE_NAME
            )
        },
        query,
        genres,
        conditions,
        formats,
        show_new_checked,
        show_used_checked,
        min_rating,
        catalog_cards: ui::product_cards(books.clone(), "search.results"),
        drawer_checkout_button: ui::checkout_start_button("cart.drawer", cart.item_count == 0),
        drawer_browse_books_link: ui::browse_books_link("cart.drawer.empty", "secondary-button"),
        cart,
        cart_lines,
        removed_notice,
        filters: result_filters(filters, books.len(), total),
        current_user: crate::auth::get_current_user(db, &session)
            .await
            .unwrap_or(None),
    };

    Ok(template.into_response())
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
        current_user: crate::auth::get_current_user(db, &session)
            .await
            .unwrap_or(None),
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
        current_user: crate::auth::get_current_user(db, &session)
            .await
            .unwrap_or(None),
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
        "View Checkout Preview",
        "primary-button checkout-button checkout-button--page",
        "submit",
        "checkout",
        "View checkout preview",
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

// Authentication Handlers

pub async fn signup_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    signup_template_response(
        &state.db,
        &session,
        None,
        String::new(),
        String::new(),
        String::new(),
    )
    .await
}

#[derive(Deserialize)]
pub struct SignupForm {
    #[serde(default)]
    pub csrf: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: secrecy::Secret<String>,
    pub password_confirm: secrecy::Secret<String>,
}

#[derive(Deserialize)]
pub struct AuthForm {
    #[serde(default)]
    pub csrf: String,
    pub email: String,
    pub password: secrecy::Secret<String>,
}

pub async fn signup_action(
    State(state): State<AppState>,
    session: Session,
    peer: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    headers: HeaderMap,
    Form(form): Form<SignupForm>,
) -> Result<Response, AppError> {
    use crate::auth::{register_user, AuthError};

    if !crate::account_email::check_csrf(&session, &headers, &form.csrf).await {
        return Ok(axum::http::StatusCode::FORBIDDEN.into_response());
    }
    restore_cart_session(&headers, &session).await?;

    if form.email.trim().is_empty() {
        return signup_template_response(
            &state.db,
            &session,
            Some("Email is required".into()),
            form.email,
            form.first_name,
            form.last_name,
        )
        .await;
    }

    if form.password.expose_secret() != form.password_confirm.expose_secret() {
        return signup_template_response(
            &state.db,
            &session,
            Some("Passwords must match.".into()),
            form.email,
            form.first_name,
            form.last_name,
        )
        .await;
    }

    if !crate::account_email::signup_allowed(
        &state.db,
        &form.email.trim().to_lowercase(),
        peer.map(|p| p.0),
    )
    .await
    {
        return signup_template_response(
            &state.db,
            &session,
            Some("Too many requests. Please try again later.".into()),
            form.email,
            form.first_name,
            form.last_name,
        )
        .await;
    }
    match register_user(
        &state.db,
        &state.email,
        &form.first_name,
        &form.last_name,
        form.email.trim(),
        form.password,
    )
    .await
    {
        Ok(user) => {
            crate::auth::sign_in_user(&session, &user.id, 0)
                .await
                .map_err(|err| AppError::Validation(err.to_string()))?;
            Ok(axum::response::Redirect::to("/account/profile").into_response())
        }
        Err(AuthError::EmailUnavailable) => {
            signup_template_response(
                &state.db,
                &session,
                Some("Email is temporarily unavailable. Please try again later.".into()),
                form.email,
                form.first_name,
                form.last_name,
            )
            .await
        }
        Err(AuthError::UserExists) => {
            signup_template_response(
                &state.db,
                &session,
                Some("An account with this email already exists.".into()),
                form.email,
                form.first_name,
                form.last_name,
            )
            .await
        }
        Err(AuthError::Validation(message)) => {
            signup_template_response(
                &state.db,
                &session,
                Some(message),
                form.email,
                form.first_name,
                form.last_name,
            )
            .await
        }
        Err(e) => {
            tracing::error!("Signup error: {:?}", e);
            signup_template_response(
                &state.db,
                &session,
                Some("An unexpected error occurred. Please try again.".into()),
                form.email,
                form.first_name,
                form.last_name,
            )
            .await
        }
    }
}

pub async fn login_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    login_template_response(&state.db, &session, None, String::new()).await
}

pub async fn login_action(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<AuthForm>,
) -> Result<Response, AppError> {
    use crate::auth::{login_user, AuthError};

    if !crate::account_email::check_csrf(&session, &headers, &form.csrf).await {
        return Ok(axum::http::StatusCode::FORBIDDEN.into_response());
    }
    restore_cart_session(&headers, &session).await?;

    if form.email.trim().is_empty() {
        return login_template_response(
            &state.db,
            &session,
            Some("Email is required".into()),
            form.email,
        )
        .await;
    }

    match login_user(&state.db, &session, form.email.trim(), form.password).await {
        Ok(_) => {
            // TODO: Merge cart from anonymous session to user session if needed
            Ok(axum::response::Redirect::to("/").into_response())
        }
        Err(AuthError::InvalidCredentials) => {
            login_template_response(
                &state.db,
                &session,
                Some("Invalid email or password.".into()),
                form.email,
            )
            .await
        }
        Err(AuthError::Validation(message)) => {
            login_template_response(&state.db, &session, Some(message), form.email).await
        }
        Err(e) => {
            tracing::error!("Login error: {:?}", e);
            login_template_response(
                &state.db,
                &session,
                Some("An unexpected error occurred. Please try again.".into()),
                form.email,
            )
            .await
        }
    }
}

pub async fn logout_action(session: Session) -> Result<impl IntoResponse, AppError> {
    crate::auth::logout_user(&session).await;
    Ok(axum::response::Redirect::to("/"))
}

async fn current_user_or_login(
    db: &DbPool,
    session: &Session,
) -> Result<Result<User, Response>, AppError> {
    match crate::auth::get_current_user(db, session).await? {
        Some(user) => Ok(Ok(user)),
        None => Ok(Err(axum::response::Redirect::to("/login").into_response())),
    }
}

#[derive(Deserialize)]
pub struct ProfileForm {
    #[serde(default)]
    pub csrf: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone_number: String,
    pub address_line1: String,
    pub address_line2: String,
    pub address_city: String,
    pub address_state: String,
    pub address_postal_code: String,
    pub marketing_opt_in: Option<String>,
}

pub async fn account_home_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    let user = match current_user_or_login(&state.db, &session).await? {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let chrome = store_chrome(&state.db, &session).await?;

    Ok(AccountHomeTemplate {
        current_user: Some(user.clone()),
        genres: chrome.genres,
        cart: chrome.cart,
        cart_lines: chrome.cart_lines,
        removed_notice: chrome.removed_notice,
        drawer_checkout_button: chrome.drawer_checkout_button,
        drawer_browse_books_link: chrome.drawer_browse_books_link,
    }
    .into_response())
}

pub async fn profile_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    let user = match current_user_or_login(&state.db, &session).await? {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let chrome = store_chrome(&state.db, &session).await?;

    Ok(AccountProfileTemplate {
        csrf: crate::account_email::csrf(&session).await,
        current_user: Some(user.clone()),
        user,
        genres: chrome.genres,
        cart: chrome.cart,
        cart_lines: chrome.cart_lines,
        removed_notice: chrome.removed_notice,
        drawer_checkout_button: chrome.drawer_checkout_button,
        drawer_browse_books_link: chrome.drawer_browse_books_link,
        success_message: None,
        error_message: None,
    }
    .into_response())
}

pub async fn profile_action(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<ProfileForm>,
) -> Result<Response, AppError> {
    use crate::auth::{update_user_profile, AuthError, ProfileUpdate};

    if !crate::account_email::check_csrf(&session, &headers, &form.csrf).await {
        return Ok(axum::http::StatusCode::FORBIDDEN.into_response());
    }
    restore_cart_session(&headers, &session).await?;
    let current_user = match current_user_or_login(&state.db, &session).await? {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let chrome = store_chrome(&state.db, &session).await?;

    match update_user_profile(
        &state.db,
        &current_user.id,
        ProfileUpdate {
            first_name: &form.first_name,
            last_name: &form.last_name,
            email: &form.email,
            phone_number: &form.phone_number,
            address_line1: &form.address_line1,
            address_line2: &form.address_line2,
            address_city: &form.address_city,
            address_state: &form.address_state,
            address_postal_code: &form.address_postal_code,
            marketing_opt_in: form.marketing_opt_in.is_some(),
        },
    )
    .await
    {
        Ok(user) => Ok(AccountProfileTemplate {
            csrf: crate::account_email::csrf(&session).await,
            current_user: Some(user.clone()),
            user,
            genres: chrome.genres,
            cart: chrome.cart,
            cart_lines: chrome.cart_lines,
            removed_notice: chrome.removed_notice,
            drawer_checkout_button: chrome.drawer_checkout_button,
            drawer_browse_books_link: chrome.drawer_browse_books_link,
            success_message: Some("Profile saved.".into()),
            error_message: None,
        }
        .into_response()),
        Err(AuthError::UserExists) => Ok(AccountProfileTemplate {
            csrf: crate::account_email::csrf(&session).await,
            current_user: Some(current_user.clone()),
            user: current_user,
            genres: chrome.genres,
            cart: chrome.cart,
            cart_lines: chrome.cart_lines,
            removed_notice: chrome.removed_notice,
            drawer_checkout_button: chrome.drawer_checkout_button,
            drawer_browse_books_link: chrome.drawer_browse_books_link,
            success_message: None,
            error_message: Some("That email is already used by another account.".into()),
        }
        .into_response()),
        Err(AuthError::Validation(message)) => Ok(AccountProfileTemplate {
            csrf: crate::account_email::csrf(&session).await,
            current_user: Some(current_user.clone()),
            user: current_user,
            genres: chrome.genres,
            cart: chrome.cart,
            cart_lines: chrome.cart_lines,
            removed_notice: chrome.removed_notice,
            drawer_checkout_button: chrome.drawer_checkout_button,
            drawer_browse_books_link: chrome.drawer_browse_books_link,
            success_message: None,
            error_message: Some(message),
        }
        .into_response()),
        Err(err) => {
            tracing::error!("Profile update error: {:?}", err);
            Ok(AccountProfileTemplate {
                csrf: crate::account_email::csrf(&session).await,
                current_user: Some(current_user.clone()),
                user: current_user,
                genres: chrome.genres,
                cart: chrome.cart,
                cart_lines: chrome.cart_lines,
                removed_notice: chrome.removed_notice,
                drawer_checkout_button: chrome.drawer_checkout_button,
                drawer_browse_books_link: chrome.drawer_browse_books_link,
                success_message: None,
                error_message: Some("An unexpected error occurred. Please try again.".into()),
            }
            .into_response())
        }
    }
}

pub async fn security_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    let user = match current_user_or_login(&state.db, &session).await? {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let chrome = store_chrome(&state.db, &session).await?;

    Ok(AccountSecurityTemplate {
        current_user: Some(user.clone()),
        user,
        genres: chrome.genres,
        cart: chrome.cart,
        cart_lines: chrome.cart_lines,
        removed_notice: chrome.removed_notice,
        drawer_checkout_button: chrome.drawer_checkout_button,
        drawer_browse_books_link: chrome.drawer_browse_books_link,
    }
    .into_response())
}

pub async fn orders_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    let user = match current_user_or_login(&state.db, &session).await? {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let chrome = store_chrome(&state.db, &session).await?;

    Ok(AccountOrdersTemplate {
        current_user: Some(user.clone()),
        genres: chrome.genres,
        cart: chrome.cart,
        cart_lines: chrome.cart_lines,
        removed_notice: chrome.removed_notice,
        drawer_checkout_button: chrome.drawer_checkout_button,
        drawer_browse_books_link: chrome.drawer_browse_books_link,
    }
    .into_response())
}

pub async fn preferences_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    restore_cart_session(&headers, &session).await?;
    let user = match current_user_or_login(&state.db, &session).await? {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let chrome = store_chrome(&state.db, &session).await?;

    Ok(AccountPreferencesTemplate {
        csrf: crate::account_email::csrf(&session).await,
        current_user: Some(user.clone()),
        user,
        genres: chrome.genres,
        cart: chrome.cart,
        cart_lines: chrome.cart_lines,
        removed_notice: chrome.removed_notice,
        drawer_checkout_button: chrome.drawer_checkout_button,
        drawer_browse_books_link: chrome.drawer_browse_books_link,
        success_message: None,
        error_message: None,
    }
    .into_response())
}

#[derive(Deserialize)]
pub struct PreferencesForm {
    #[serde(default)]
    pub csrf: String,
    pub marketing_opt_in: Option<String>,
}

pub async fn preferences_action(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<PreferencesForm>,
) -> Result<Response, AppError> {
    use crate::auth::{update_user_profile, AuthError, ProfileUpdate};

    if !crate::account_email::check_csrf(&session, &headers, &form.csrf).await {
        return Ok(axum::http::StatusCode::FORBIDDEN.into_response());
    }
    restore_cart_session(&headers, &session).await?;
    let current_user = match current_user_or_login(&state.db, &session).await? {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let chrome = store_chrome(&state.db, &session).await?;

    let first_name = current_user.first_name_value().to_string();
    let last_name = current_user.last_name_value().to_string();
    let phone_number = current_user.phone_number_value().to_string();
    let address_line1 = current_user.address_line1_value().to_string();
    let address_line2 = current_user.address_line2_value().to_string();
    let address_city = current_user.address_city_value().to_string();
    let address_state = current_user.address_state_value().to_string();
    let address_postal_code = current_user.address_postal_code_value().to_string();

    match update_user_profile(
        &state.db,
        &current_user.id,
        ProfileUpdate {
            first_name: &first_name,
            last_name: &last_name,
            email: &current_user.email,
            phone_number: &phone_number,
            address_line1: &address_line1,
            address_line2: &address_line2,
            address_city: &address_city,
            address_state: &address_state,
            address_postal_code: &address_postal_code,
            marketing_opt_in: form.marketing_opt_in.is_some(),
        },
    )
    .await
    {
        Ok(user) => Ok(AccountPreferencesTemplate {
            csrf: crate::account_email::csrf(&session).await,
            current_user: Some(user.clone()),
            user,
            genres: chrome.genres,
            cart: chrome.cart,
            cart_lines: chrome.cart_lines,
            removed_notice: chrome.removed_notice,
            drawer_checkout_button: chrome.drawer_checkout_button,
            drawer_browse_books_link: chrome.drawer_browse_books_link,
            success_message: Some("Preferences saved.".into()),
            error_message: None,
        }
        .into_response()),
        Err(AuthError::Validation(message)) => Ok(AccountPreferencesTemplate {
            csrf: crate::account_email::csrf(&session).await,
            current_user: Some(current_user.clone()),
            user: current_user,
            genres: chrome.genres,
            cart: chrome.cart,
            cart_lines: chrome.cart_lines,
            removed_notice: chrome.removed_notice,
            drawer_checkout_button: chrome.drawer_checkout_button,
            drawer_browse_books_link: chrome.drawer_browse_books_link,
            success_message: None,
            error_message: Some(message),
        }
        .into_response()),
        Err(err) => {
            tracing::error!("Preferences update error: {:?}", err);
            Ok(AccountPreferencesTemplate {
                csrf: crate::account_email::csrf(&session).await,
                current_user: Some(current_user.clone()),
                user: current_user,
                genres: chrome.genres,
                cart: chrome.cart,
                cart_lines: chrome.cart_lines,
                removed_notice: chrome.removed_notice,
                drawer_checkout_button: chrome.drawer_checkout_button,
                drawer_browse_books_link: chrome.drawer_browse_books_link,
                success_message: None,
                error_message: Some("An unexpected error occurred. Please try again.".into()),
            }
            .into_response())
        }
    }
}
