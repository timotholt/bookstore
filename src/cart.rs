use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::str::FromStr;
use tower_sessions::Session;

use crate::errors::AppError;
use crate::models::{CartItem, CartLine, CartView};
use crate::store;

const CART_SESSION_KEY: &str = "cart_session_key";
const LEGACY_CART_KEY: &str = "cart";

pub async fn view(db: &SqlitePool, session: &Session) -> Result<CartView, AppError> {
    import_legacy_session_cart(db, session).await?;

    let Some(session_key) = session.get::<String>(CART_SESSION_KEY).await? else {
        return Ok(empty_cart_view());
    };
    let Some(cart_id) = active_cart_id(db, &session_key).await? else {
        return Ok(empty_cart_view());
    };

    let items = cart_items(db, cart_id).await?;
    build_cart_view(db, Some(cart_id), items).await
}

pub async fn add_one(db: &SqlitePool, session: &Session, copy_id: i64) -> Result<(), AppError> {
    let current_qty = quantity(db, session, copy_id).await?;
    let stock = match store::copy_stock(db, copy_id).await {
        Ok(stock) => stock,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound),
        Err(err) => return Err(err.into()),
    };

    let next_qty = std::cmp::min(current_qty + 1, stock);
    set_quantity(db, session, copy_id, next_qty).await
}

pub async fn change_quantity(
    db: &SqlitePool,
    session: &Session,
    copy_id: i64,
    delta: i32,
) -> Result<(), AppError> {
    let current_qty = quantity(db, session, copy_id).await?;
    let mut next_qty = current_qty + delta;
    if next_qty > 0 {
        let stock = store::copy_stock(db, copy_id).await?;
        next_qty = std::cmp::min(next_qty, stock);
    }

    set_quantity(db, session, copy_id, next_qty).await
}

pub async fn set_quantity(
    db: &SqlitePool,
    session: &Session,
    copy_id: i64,
    quantity: i32,
) -> Result<(), AppError> {
    if copy_id < 1 {
        return Err(AppError::Validation("invalid copy".into()));
    }

    if quantity <= 0 {
        if let Some(cart_id) = optional_cart_id(db, session).await? {
            delete_cart_item(db, cart_id, copy_id).await?;
        }
        return Ok(());
    }

    let stock = match store::copy_stock(db, copy_id).await {
        Ok(stock) => stock,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound),
        Err(err) => return Err(err.into()),
    };
    let capped_quantity = std::cmp::min(quantity, stock);
    if capped_quantity <= 0 {
        if let Some(cart_id) = optional_cart_id(db, session).await? {
            delete_cart_item(db, cart_id, copy_id).await?;
        }
        return Ok(());
    }

    let cart_id = writable_cart_id(db, session).await?;
    upsert_cart_item(db, cart_id, copy_id, capped_quantity).await?;
    Ok(())
}

pub async fn quantity(db: &SqlitePool, session: &Session, copy_id: i64) -> Result<i32, AppError> {
    let Some(cart_id) = optional_cart_id(db, session).await? else {
        return Ok(0);
    };

    let quantity = sqlx::query_scalar::<_, i32>(
        "SELECT quantity FROM cart_items WHERE cart_id = ? AND copy_id = ?",
    )
    .bind(cart_id)
    .bind(copy_id)
    .fetch_optional(db)
    .await?
    .unwrap_or(0);

    Ok(quantity)
}

async fn writable_cart_id(db: &SqlitePool, session: &Session) -> Result<i64, AppError> {
    import_legacy_session_cart(db, session).await?;
    let session_key = cart_session_key(session).await?;
    ensure_active_cart(db, &session_key)
        .await
        .map_err(AppError::from)
}

async fn optional_cart_id(db: &SqlitePool, session: &Session) -> Result<Option<i64>, AppError> {
    import_legacy_session_cart(db, session).await?;
    let Some(session_key) = session.get::<String>(CART_SESSION_KEY).await? else {
        return Ok(None);
    };
    active_cart_id(db, &session_key)
        .await
        .map_err(AppError::from)
}

async fn cart_session_key(session: &Session) -> Result<String, AppError> {
    if let Some(session_key) = session.get::<String>(CART_SESSION_KEY).await? {
        return Ok(session_key);
    }

    let session_key = uuid::Uuid::new_v4().to_string();
    session.insert(CART_SESSION_KEY, &session_key).await?;
    Ok(session_key)
}

async fn active_cart_id(db: &SqlitePool, session_key: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM carts
        WHERE session_key = ? AND user_id IS NULL AND status = 'active'
        LIMIT 1
        "#,
    )
    .bind(session_key)
    .fetch_optional(db)
    .await
}

async fn ensure_active_cart(db: &SqlitePool, session_key: &str) -> Result<i64, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO carts (session_key, status)
        VALUES (?, 'active')
        "#,
    )
    .bind(session_key)
    .execute(db)
    .await?;

    active_cart_id(db, session_key)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

async fn import_legacy_session_cart(db: &SqlitePool, session: &Session) -> Result<(), AppError> {
    let Some(items) = session.remove::<Vec<CartItem>>(LEGACY_CART_KEY).await? else {
        return Ok(());
    };
    if items.is_empty() {
        return Ok(());
    }

    let session_key = cart_session_key(session).await?;
    let cart_id = ensure_active_cart(db, &session_key).await?;
    for item in items {
        if item.copy_id > 0 && item.quantity > 0 {
            let stock = store::copy_stock(db, item.copy_id).await.unwrap_or(0);
            let quantity = std::cmp::min(item.quantity, stock);
            if quantity > 0 {
                upsert_cart_item(db, cart_id, item.copy_id, quantity).await?;
            }
        }
    }

    Ok(())
}

async fn cart_items(db: &SqlitePool, cart_id: i64) -> Result<Vec<CartItem>, sqlx::Error> {
    sqlx::query_as::<_, CartItem>(
        r#"
        SELECT copy_id, quantity
        FROM cart_items
        WHERE cart_id = ?
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(cart_id)
    .fetch_all(db)
    .await
}

async fn upsert_cart_item(
    db: &SqlitePool,
    cart_id: i64,
    copy_id: i64,
    quantity: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO cart_items (cart_id, copy_id, quantity)
        VALUES (?, ?, ?)
        ON CONFLICT(cart_id, copy_id) DO UPDATE SET
            quantity = excluded.quantity,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(cart_id)
    .bind(copy_id)
    .bind(quantity)
    .execute(db)
    .await?;

    touch_cart(db, cart_id).await
}

async fn delete_cart_item(db: &SqlitePool, cart_id: i64, copy_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM cart_items WHERE cart_id = ? AND copy_id = ?")
        .bind(cart_id)
        .bind(copy_id)
        .execute(db)
        .await?;

    touch_cart(db, cart_id).await
}

async fn touch_cart(db: &SqlitePool, cart_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE carts SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(cart_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn build_cart_view(
    db: &SqlitePool,
    cart_id: Option<i64>,
    items: Vec<CartItem>,
) -> Result<CartView, AppError> {
    let copy_ids: Vec<i64> = items.iter().map(|it| it.copy_id).collect();
    let books = store::books_by_copy_ids(db, &copy_ids).await?;
    let mut book_map = HashMap::new();
    for book in books {
        book_map.insert(book.copy_id, book);
    }

    let mut lines = Vec::new();
    let mut item_count = 0;
    let mut subtotal = Decimal::ZERO;

    for item in items {
        if item.quantity <= 0 {
            if let Some(cart_id) = cart_id {
                delete_cart_item(db, cart_id, item.copy_id).await?;
            }
            continue;
        }

        let Some(book) = book_map.get(&item.copy_id) else {
            if let Some(cart_id) = cart_id {
                delete_cart_item(db, cart_id, item.copy_id).await?;
            }
            continue;
        };

        let quantity = std::cmp::min(item.quantity, book.stock);
        if quantity <= 0 {
            if let Some(cart_id) = cart_id {
                delete_cart_item(db, cart_id, item.copy_id).await?;
            }
            continue;
        }
        if quantity != item.quantity {
            if let Some(cart_id) = cart_id {
                upsert_cart_item(db, cart_id, item.copy_id, quantity).await?;
            }
        }

        let price_dec = Decimal::from_f64(book.price).unwrap_or_default();
        let line_total = price_dec * Decimal::from(quantity);
        lines.push(CartLine {
            book: book.clone(),
            quantity,
            line_total,
        });
        item_count += quantity;
        subtotal += line_total;
    }

    Ok(cart_view_from_totals(lines, item_count, subtotal))
}

fn empty_cart_view() -> CartView {
    cart_view_from_totals(Vec::new(), 0, Decimal::ZERO)
}

fn cart_view_from_totals(lines: Vec<CartLine>, item_count: i32, subtotal: Decimal) -> CartView {
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

    CartView {
        lines,
        item_count,
        subtotal,
        shipping,
        total,
        free_shipping,
        progress_text,
        progress_ratio,
    }
}
