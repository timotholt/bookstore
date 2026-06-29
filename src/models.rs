use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct BookCard {
    #[sqlx(rename = "id")]
    pub id: String,
    pub title: String,
    pub author: String,
    pub author_slug: String,
    pub genre: String,
    pub genre_slug: String,
    pub year: i32,
    pub isbn: String,
    pub cover_color: String,
    pub aspect_ratio: f64,
    pub tags: String,
    pub is_new_arrival: bool,
    #[sqlx(rename = "copy_id")]
    pub copy_id: i64,
    pub condition: String,
    pub price: f64,
    pub notes: Option<String>,
    pub format: String,
    pub stock: i32,
    pub is_staff_pick: bool,
    pub staff_quote: String,
    pub seal_style: String,
    pub seal_text: String,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct VariantAttribute {
    #[sqlx(rename = "variant_id")]
    pub variant_id: i64,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CatalogFilters {
    pub q: Option<String>,
    pub author: Option<String>,
    pub genre: Option<String>,
    pub condition: Option<String>,
    pub max_price: Option<String>,
    pub format: Option<String>,
    pub sort: Option<String>,
    #[serde(skip)]
    pub result_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CartItem {
    pub copy_id: i64,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartLine {
    pub book: BookCard,
    pub quantity: i32,
    pub line_total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CartView {
    pub lines: Vec<CartLine>,
    pub item_count: i32,
    pub subtotal: Decimal,
    pub shipping: Decimal,
    pub total: Decimal,
    pub free_shipping: bool,
    pub progress_text: String,
    pub progress_ratio: f64,
}
