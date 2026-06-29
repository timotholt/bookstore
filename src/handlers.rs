use axum::{
    extract::{Path, Query, State, Form},
    response::{IntoResponse, Response},
    http::HeaderMap,
};
use serde::Deserialize;
use tower_sessions::Session;
use sqlx::SqlitePool;
use rust_decimal::Decimal;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};
use std::str::FromStr;
use std::collections::HashMap;

use crate::errors::AppError;
use crate::models::*;
use crate::templates::*;
use crate::store;
use crate::ui;

// Helper: retrieve cart items from session
async fn get_cart_items(session: &Session) -> Vec<CartItem> {
    session.get::<Vec<CartItem>>("cart")
        .await
        .unwrap_or_default()
        .unwrap_or_default()
}

// Helper: build CartView and cap quantities against DB stock in a batch query
pub async fn cart_view(db: &SqlitePool, session: &Session) -> Result<CartView, AppError> {
    let items = get_cart_items(session).await;
    let copy_ids: Vec<i64> = items.iter().map(|it| it.copy_id).collect();

    let books = store::books_by_copy_ids(db, &copy_ids).await?;
    let mut book_map = HashMap::new();
    for b in books {
        book_map.insert(b.copy_id, b);
    }

    let mut lines = Vec::new();
    let mut item_count = 0;
    let mut subtotal = Decimal::ZERO;
    let mut session_needs_update = false;
    let mut updated_items = Vec::new();

    for mut item in items {
        if item.quantity <= 0 {
            session_needs_update = true;
            continue;
        }
        if let Some(book) = book_map.get(&item.copy_id) {
            let mut qty = item.quantity;
            if qty > book.stock {
                qty = book.stock;
                session_needs_update = true;
            }
            if qty <= 0 {
                session_needs_update = true;
                continue;
            }
            item.quantity = qty;
            updated_items.push(item.clone());

            let price_dec = Decimal::from_f64(book.price).unwrap_or_default();
            let line_total = price_dec * Decimal::from(qty);
            lines.push(CartLine {
                book: book.clone(),
                quantity: qty,
                line_total,
            });
            item_count += qty;
            subtotal += line_total;
        } else {
            // Item copy no longer exists/is sold
            session_needs_update = true;
        }
    }

    if session_needs_update {
        session.insert("cart", updated_items).await?;
    }

    let mut shipping = Decimal::ZERO;
    let free_shipping = subtotal >= Decimal::from(35);
    if subtotal > Decimal::ZERO && subtotal < Decimal::from(35) {
        shipping = Decimal::from_str("4.95").unwrap();
    }

    let total = subtotal + shipping;

    let mut progress_text = String::from("Your cart is empty. Add $35.00 more for free shipping.");
    let mut progress_ratio = 0.0;
    if subtotal > Decimal::ZERO {
        if free_shipping {
            progress_text = String::from("Your stack qualifies for free shipping.");
            progress_ratio = 100.0;
        } else {
            let remaining = Decimal::from(35) - subtotal;
            progress_text = format!("Add ${:.2} more for free shipping.", remaining);
            if let Some(subtotal_f64) = subtotal.to_f64() {
                progress_ratio = (subtotal_f64 / 35.0) * 100.0;
            }
        }
    }

    Ok(CartView {
        lines,
        item_count,
        subtotal,
        shipping,
        total,
        free_shipping,
        progress_text,
        progress_ratio,
    })
}

// Helper: modify item quantity in cart session
async fn set_cart_quantity(session: &Session, copy_id: i64, quantity: i32) -> Result<Vec<CartItem>, AppError> {
    let items = get_cart_items(session).await;
    let mut out = Vec::new();
    let mut found = false;

    for mut item in items {
        if item.copy_id != copy_id {
            out.push(item);
            continue;
        }
        found = true;
        if quantity > 0 {
            item.quantity = quantity;
            out.push(item);
        }
    }
    if !found && quantity > 0 {
        out.push(CartItem { copy_id, quantity });
    }

    session.insert("cart", &out).await?;
    Ok(out)
}

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
pub async fn home(
    State(db): State<SqlitePool>,
    session: Session,
    Query(filters): Query<CatalogFilters>,
) -> Result<impl IntoResponse, AppError> {
    let books = store::list_books(&db, &filters).await?;
    let all_books = store::list_books(&db, &CatalogFilters::default()).await?;
    let best_sellers = store::collection_books(&db, "best-sellers", 6).await?;
    let deals = store::collection_books(&db, "used-deals", 6).await?;
    let staff_picks = store::collection_books(&db, "staff-picks", 3).await?;
    let cart = cart_view(&db, &session).await?;

    let featured = all_books.iter()
        .find(|b| b.id == "b005")
        .cloned()
        .unwrap_or_else(|| all_books[0].clone());

    let quick_fillers: Vec<BookCard> = all_books.iter()
        .filter(|b| b.price < 8.0)
        .take(2)
        .cloned()
        .collect();

    let new_arrivals: Vec<BookCard> = all_books.iter()
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
        quick_fillers,
        product_sections,
        catalog_cards: ui::product_cards(books.clone(), "catalog.results"),
        staff_picks,
        cart,
        filters: result_filters(filters, books.len(), all_books.len()),
    };

    Ok(template)
}

pub async fn catalog(
    State(db): State<SqlitePool>,
    headers: HeaderMap,
    Query(filters): Query<CatalogFilters>,
) -> Result<impl IntoResponse, AppError> {
    let books = store::list_books(&db, &filters).await?;
    let all_books = store::list_books(&db, &CatalogFilters::default()).await?;

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
    State(db): State<SqlitePool>,
    session: Session,
    Path(book_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let book = match store::book_by_id(&db, &book_id).await {
        Ok(b) => b,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound),
        Err(err) => return Err(err.into()),
    };

    let copies = store::copies_by_product_id(&db, &book_id).await?;
    let raw_attribs = store::variant_attributes(&db, &book_id).await?;
    
    let mut attributes = HashMap::new();
    for attr in raw_attribs {
        attributes.entry(attr.variant_id).or_insert_with(Vec::new).push(attr);
    }

    let all_books = store::list_books(&db, &CatalogFilters::default()).await?;
    let cart = cart_view(&db, &session).await?;

    let related: Vec<BookCard> = all_books.iter()
        .filter(|b| b.genre == book.genre && b.id != book.id)
        .take(4)
        .cloned()
        .collect();

    let template = BookDetailTemplate {
        genres: unique_genres(&all_books),
        book,
        copies,
        attributes,
        related_cards: ui::product_cards(related, "book_detail.related"),
        cart,
    };

    Ok(template)
}

#[derive(Deserialize)]
pub struct AddCartForm {
    pub copy_id: String,
}

pub async fn add_cart_item(
    State(db): State<SqlitePool>,
    session: Session,
    Form(form): Form<AddCartForm>,
) -> Result<impl IntoResponse, AppError> {
    let copy_id = form.copy_id.parse::<i64>().map_err(|_| AppError::Validation("invalid copy id".into()))?;
    if copy_id < 1 {
        return Err(AppError::Validation("invalid copy".into()));
    }

    let stock = match store::copy_stock(&db, copy_id).await {
        Ok(s) => s,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound),
        Err(err) => return Err(err.into()),
    };

    let items = get_cart_items(&session).await;
    let current_qty = items.iter()
        .find(|it| it.copy_id == copy_id)
        .map(|it| it.quantity)
        .unwrap_or(0);

    let next_qty = std::cmp::min(current_qty + 1, stock);
    set_cart_quantity(&session, copy_id, next_qty).await?;

    render_cart(State(db), session).await
}

pub async fn increase_cart_item(
    State(db): State<SqlitePool>,
    session: Session,
    Path(copy_id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    change_cart_quantity(State(db), session, copy_id, 1).await
}

pub async fn decrease_cart_item(
    State(db): State<SqlitePool>,
    session: Session,
    Path(copy_id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    change_cart_quantity(State(db), session, copy_id, -1).await
}

pub async fn remove_cart_item(
    State(db): State<SqlitePool>,
    session: Session,
    Path(copy_id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    set_cart_quantity(&session, copy_id, 0).await?;
    render_cart(State(db), session).await
}

async fn change_cart_quantity(
    State(db): State<SqlitePool>,
    session: Session,
    copy_id: i64,
    delta: i32,
) -> Result<impl IntoResponse, AppError> {
    let items = get_cart_items(&session).await;
    let current_qty = items.iter()
        .find(|it| it.copy_id == copy_id)
        .map(|it| it.quantity)
        .unwrap_or(0);

    let mut next_qty = current_qty + delta;
    if next_qty > 0 {
        let stock = store::copy_stock(&db, copy_id).await?;
        next_qty = std::cmp::min(next_qty, stock);
    }

    set_cart_quantity(&session, copy_id, next_qty).await?;
    render_cart(State(db), session).await
}

pub async fn checkout(
    State(db): State<SqlitePool>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let cart = cart_view(&db, &session).await?;
    if cart.item_count == 0 {
        return Err(AppError::Validation("cart is empty".into()));
    }
    
    // In production, this redirect to Stripe Checkout, clear sessions on webhook, etc.
    let body = format!("Stripe Checkout will be connected in Phase 4. Cart total: ${:.2}", cart.total);
    Ok(body)
}

pub async fn cart_page(
    State(db): State<SqlitePool>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let all_books = store::list_books(&db, &CatalogFilters::default()).await?;
    let cart = cart_view(&db, &session).await?;

    let template = CartPageTemplate {
        genres: unique_genres(&all_books),
        cart,
    };

    Ok(template)
}

async fn render_cart(
    State(db): State<SqlitePool>,
    session: Session,
) -> Result<Response, AppError> {
    let cart = cart_view(&db, &session).await?;
    let template = CartDrawerTemplate { cart };
    Ok(template.into_response())
}
