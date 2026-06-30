use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use tower_sessions::Session;

use crate::db::DbPool;
use crate::errors::AppError;
use crate::models::{CartItem, CartLine, CartView, SavedItem, SavedItemsView};
use crate::store;

const CART_SESSION_KEY: &str = "cart_session_key";
const LEGACY_CART_KEY: &str = "cart";
pub const BROWSER_CART_KEY_COOKIE: &str = "davis_cart_key";

pub async fn view(db: &DbPool, session: &Session) -> Result<CartView, AppError> {
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

pub async fn saved_view(db: &DbPool, session: &Session) -> Result<SavedItemsView, AppError> {
    import_legacy_session_cart(db, session).await?;

    let Some(session_key) = session.get::<String>(CART_SESSION_KEY).await? else {
        return Ok(SavedItemsView::default());
    };

    let items = saved_items(db, &session_key).await?;
    build_saved_items_view(db, &session_key, items).await
}

pub async fn add_one(db: &DbPool, session: &Session, copy_id: i64) -> Result<(), AppError> {
    let current_qty = quantity(db, session, copy_id).await?;
    let stock = match store::copy_stock(db, copy_id).await {
        Ok(stock) => stock,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound),
        Err(err) => return Err(err.into()),
    };

    let next_qty = std::cmp::min(current_qty + 1, stock);
    set_quantity(db, session, copy_id, next_qty).await
}

pub async fn save_for_later(db: &DbPool, session: &Session, copy_id: i64) -> Result<(), AppError> {
    if copy_id < 1 {
        return Err(AppError::Validation("invalid copy".into()));
    }

    let Some(cart_id) = optional_cart_id(db, session).await? else {
        return Ok(());
    };
    let Some(quantity) = cart_item_quantity(db, cart_id, copy_id).await? else {
        return Ok(());
    };
    let session_key = cart_session_key(session).await?;
    upsert_saved_item(db, &session_key, copy_id, quantity).await?;
    delete_cart_item(db, cart_id, copy_id).await?;
    Ok(())
}

pub async fn remove_with_notice(
    db: &DbPool,
    session: &Session,
    copy_id: i64,
) -> Result<(), AppError> {
    if copy_id < 1 {
        return Err(AppError::Validation("invalid copy".into()));
    }

    let Some(cart_id) = optional_cart_id(db, session).await? else {
        return Ok(());
    };
    let Some(quantity) = cart_item_quantity(db, cart_id, copy_id).await? else {
        return Ok(());
    };
    let session_key = cart_session_key(session).await?;
    upsert_removed_cart_item(db, &session_key, copy_id, quantity).await?;
    delete_cart_item(db, cart_id, copy_id).await?;
    Ok(())
}

pub async fn restore_removed_item(
    db: &DbPool,
    session: &Session,
    copy_id: i64,
) -> Result<(), AppError> {
    if copy_id < 1 {
        return Err(AppError::Validation("invalid copy".into()));
    }

    let session_key = cart_session_key(session).await?;
    let Some(quantity) = removed_item_quantity(db, &session_key, copy_id).await? else {
        return Ok(());
    };

    set_quantity(db, session, copy_id, quantity).await?;
    delete_removed_cart_item(db, &session_key, copy_id).await?;
    Ok(())
}

pub async fn removed_item_view(
    db: &DbPool,
    session: &Session,
) -> Result<Option<CartLine>, AppError> {
    let Some(session_key) = session.get::<String>(CART_SESSION_KEY).await? else {
        return Ok(None);
    };
    let Some(removed) = removed_cart_item(db, &session_key).await? else {
        return Ok(None);
    };
    if removed.copy_id < 1 || removed.quantity <= 0 {
        delete_removed_cart_item(db, &session_key, removed.copy_id).await?;
        return Ok(None);
    }

    let books = store::books_by_copy_ids(db, &[removed.copy_id]).await?;
    let Some(book) = books.into_iter().next() else {
        delete_removed_cart_item(db, &session_key, removed.copy_id).await?;
        return Ok(None);
    };
    let quantity = std::cmp::min(removed.quantity, book.stock);
    if quantity <= 0 {
        delete_removed_cart_item(db, &session_key, removed.copy_id).await?;
        return Ok(None);
    }
    let line_total = Decimal::from_f64(book.price).unwrap_or_default() * Decimal::from(quantity);
    Ok(Some(CartLine {
        book,
        quantity,
        line_total,
    }))
}

pub async fn move_saved_to_cart(
    db: &DbPool,
    session: &Session,
    copy_id: i64,
) -> Result<(), AppError> {
    if copy_id < 1 {
        return Err(AppError::Validation("invalid copy".into()));
    }

    let session_key = cart_session_key(session).await?;
    let Some(quantity) = saved_item_quantity(db, &session_key, copy_id).await? else {
        return Ok(());
    };
    set_quantity(db, session, copy_id, quantity).await?;
    delete_saved_item(db, &session_key, copy_id).await?;
    Ok(())
}

pub async fn remove_saved_item(
    db: &DbPool,
    session: &Session,
    copy_id: i64,
) -> Result<(), AppError> {
    if copy_id < 1 {
        return Err(AppError::Validation("invalid copy".into()));
    }

    let Some(session_key) = session.get::<String>(CART_SESSION_KEY).await? else {
        return Ok(());
    };
    delete_saved_item(db, &session_key, copy_id).await?;
    Ok(())
}

pub async fn change_quantity(
    db: &DbPool,
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
    db: &DbPool,
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

pub async fn quantity(db: &DbPool, session: &Session, copy_id: i64) -> Result<i32, AppError> {
    let Some(cart_id) = optional_cart_id(db, session).await? else {
        return Ok(0);
    };

    let quantity = cart_item_quantity(db, cart_id, copy_id).await?.unwrap_or(0);

    Ok(quantity)
}

pub async fn adopt_session_key(session: &Session, session_key: &str) -> Result<(), AppError> {
    if session_key.trim().is_empty() || session_key.len() > 128 {
        return Ok(());
    }
    if session.get::<String>(CART_SESSION_KEY).await?.is_none() {
        session.insert(CART_SESSION_KEY, session_key).await?;
    }
    Ok(())
}

pub async fn current_session_key(session: &Session) -> Result<Option<String>, AppError> {
    session
        .get::<String>(CART_SESSION_KEY)
        .await
        .map_err(AppError::from)
}

async fn writable_cart_id(db: &DbPool, session: &Session) -> Result<i64, AppError> {
    import_legacy_session_cart(db, session).await?;
    let session_key = cart_session_key(session).await?;
    ensure_active_cart(db, &session_key)
        .await
        .map_err(AppError::from)
}

async fn optional_cart_id(db: &DbPool, session: &Session) -> Result<Option<i64>, AppError> {
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

async fn active_cart_id(db: &DbPool, session_key: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM carts
        WHERE session_key = $1 AND user_id IS NULL AND status = 'active'
        LIMIT 1
        "#,
    )
    .bind(session_key)
    .fetch_optional(db)
    .await
}

async fn ensure_active_cart(db: &DbPool, session_key: &str) -> Result<i64, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO carts (session_key, status)
        VALUES ($1, 'active')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(session_key)
    .execute(db)
    .await?;

    active_cart_id(db, session_key)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

async fn import_legacy_session_cart(db: &DbPool, session: &Session) -> Result<(), AppError> {
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

async fn cart_items(db: &DbPool, cart_id: i64) -> Result<Vec<CartItem>, sqlx::Error> {
    sqlx::query_as::<_, CartItem>(
        r#"
        SELECT copy_id, quantity
        FROM cart_items
        WHERE cart_id = $1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(cart_id)
    .fetch_all(db)
    .await
}

async fn cart_item_quantity(
    db: &DbPool,
    cart_id: i64,
    copy_id: i64,
) -> Result<Option<i32>, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        "SELECT quantity FROM cart_items WHERE cart_id = $1 AND copy_id = $2",
    )
    .bind(cart_id)
    .bind(copy_id)
    .fetch_optional(db)
    .await
}

async fn saved_items(db: &DbPool, session_key: &str) -> Result<Vec<SavedItem>, sqlx::Error> {
    sqlx::query_as::<_, SavedItem>(
        r#"
        SELECT copy_id, quantity
        FROM saved_items
        WHERE session_key = $1 AND user_id IS NULL
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(session_key)
    .fetch_all(db)
    .await
}

async fn saved_item_quantity(
    db: &DbPool,
    session_key: &str,
    copy_id: i64,
) -> Result<Option<i32>, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        "SELECT quantity FROM saved_items WHERE session_key = $1 AND user_id IS NULL AND copy_id = $2",
    )
    .bind(session_key)
    .bind(copy_id)
    .fetch_optional(db)
    .await
}

async fn removed_cart_item(
    db: &DbPool,
    session_key: &str,
) -> Result<Option<CartItem>, sqlx::Error> {
    sqlx::query_as::<_, CartItem>(
        r#"
        SELECT copy_id, quantity
        FROM removed_cart_items
        WHERE session_key = $1 AND user_id IS NULL
        ORDER BY updated_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(session_key)
    .fetch_optional(db)
    .await
}

async fn removed_item_quantity(
    db: &DbPool,
    session_key: &str,
    copy_id: i64,
) -> Result<Option<i32>, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        "SELECT quantity FROM removed_cart_items WHERE session_key = $1 AND user_id IS NULL AND copy_id = $2",
    )
    .bind(session_key)
    .bind(copy_id)
    .fetch_optional(db)
    .await
}

async fn upsert_cart_item(
    db: &DbPool,
    cart_id: i64,
    copy_id: i64,
    quantity: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO cart_items (cart_id, copy_id, quantity)
        VALUES ($1, $2, $3)
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

async fn upsert_saved_item(
    db: &DbPool,
    session_key: &str,
    copy_id: i64,
    quantity: i32,
) -> Result<(), sqlx::Error> {
    delete_saved_item(db, session_key, copy_id).await?;
    sqlx::query(
        r#"
        INSERT INTO saved_items (session_key, copy_id, quantity)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(session_key)
    .bind(copy_id)
    .bind(std::cmp::max(quantity, 1))
    .execute(db)
    .await?;
    Ok(())
}

async fn upsert_removed_cart_item(
    db: &DbPool,
    session_key: &str,
    copy_id: i64,
    quantity: i32,
) -> Result<(), sqlx::Error> {
    delete_removed_cart_item(db, session_key, copy_id).await?;
    sqlx::query(
        r#"
        INSERT INTO removed_cart_items (session_key, copy_id, quantity)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(session_key)
    .bind(copy_id)
    .bind(std::cmp::max(quantity, 1))
    .execute(db)
    .await?;
    Ok(())
}

async fn delete_cart_item(db: &DbPool, cart_id: i64, copy_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM cart_items WHERE cart_id = $1 AND copy_id = $2")
        .bind(cart_id)
        .bind(copy_id)
        .execute(db)
        .await?;

    touch_cart(db, cart_id).await
}

async fn delete_saved_item(
    db: &DbPool,
    session_key: &str,
    copy_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM saved_items WHERE session_key = $1 AND user_id IS NULL AND copy_id = $2",
    )
    .bind(session_key)
    .bind(copy_id)
    .execute(db)
    .await?;
    Ok(())
}

async fn delete_removed_cart_item(
    db: &DbPool,
    session_key: &str,
    copy_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM removed_cart_items WHERE session_key = $1 AND user_id IS NULL AND copy_id = $2",
    )
    .bind(session_key)
    .bind(copy_id)
    .execute(db)
    .await?;
    Ok(())
}

async fn touch_cart(db: &DbPool, cart_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE carts SET updated_at = CURRENT_TIMESTAMP WHERE id = $1")
        .bind(cart_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn build_cart_view(
    db: &DbPool,
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

async fn build_saved_items_view(
    db: &DbPool,
    session_key: &str,
    items: Vec<SavedItem>,
) -> Result<SavedItemsView, AppError> {
    let copy_ids: Vec<i64> = items.iter().map(|it| it.copy_id).collect();
    let books = store::books_by_copy_ids(db, &copy_ids).await?;
    let mut book_map = HashMap::new();
    for book in books {
        book_map.insert(book.copy_id, book);
    }

    let mut lines = Vec::new();
    let mut item_count = 0;
    for item in items {
        let Some(book) = book_map.get(&item.copy_id) else {
            delete_saved_item(db, session_key, item.copy_id).await?;
            continue;
        };
        if item.quantity <= 0 || book.stock <= 0 {
            delete_saved_item(db, session_key, item.copy_id).await?;
            continue;
        }

        let quantity = std::cmp::max(item.quantity, 1);
        let price_dec = Decimal::from_f64(book.price).unwrap_or_default();
        let line_total = price_dec * Decimal::from(quantity);
        lines.push(CartLine {
            book: book.clone(),
            quantity,
            line_total,
        });
        item_count += quantity;
    }

    Ok(SavedItemsView { lines, item_count })
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
