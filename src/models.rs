use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub description: String,
    pub cover_url: String,
    pub cover_color: String,
    pub aspect_ratio: f64,
    pub tags: String,
    pub is_new_arrival: bool,
    #[sqlx(rename = "copy_id")]
    pub copy_id: i64,
    pub condition: String,
    pub is_new: bool,
    pub price: f64,
    pub list_price: Option<f64>,
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
    pub listing: Option<String>,
    pub max_price: Option<String>,
    pub format: Option<String>,
    pub min_rating: Option<String>,
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    #[serde(skip)]
    pub result_text: String,
    #[serde(skip)]
    pub total_items: usize,
    #[serde(skip)]
    pub total_pages: u32,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize, Default)]
pub struct CartItem {
    pub copy_id: i64,
    pub quantity: i32,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize, Default)]
pub struct SavedItem {
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
    pub page: u32,
    pub total_lines: i64,
    pub lines: Vec<CartLine>,
    pub item_count: i32,
    pub subtotal: Decimal,
    pub shipping: Decimal,
    pub total: Decimal,
    pub free_shipping: bool,
    pub progress_text: String,
    pub progress_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedItemsView {
    pub page: u32,
    pub total_lines: i64,
    pub lines: Vec<CartLine>,
    pub item_count: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnalyticsEventPayload {
    pub event_name: String,
    pub source: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub page_path: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone_number: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub address_city: Option<String>,
    pub address_state: Option<String>,
    pub address_postal_code: Option<String>,
    pub marketing_opt_in: bool,
}

impl User {
    pub fn display_name(&self) -> String {
        let name = [self.first_name_value(), self.last_name_value()]
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if !name.is_empty() {
            return name;
        }

        self.full_name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&self.email)
            .to_string()
    }

    pub fn header_name(&self) -> String {
        if let Some(first_name) = self
            .first_name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
        {
            first_name.to_string()
        } else {
            self.display_name()
        }
    }

    pub fn first_name_value(&self) -> &str {
        self.first_name.as_deref().unwrap_or("")
    }

    pub fn last_name_value(&self) -> &str {
        self.last_name.as_deref().unwrap_or("")
    }

    pub fn phone_number_value(&self) -> &str {
        self.phone_number.as_deref().unwrap_or("")
    }

    pub fn address_line1_value(&self) -> &str {
        self.address_line1.as_deref().unwrap_or("")
    }

    pub fn address_line2_value(&self) -> &str {
        self.address_line2.as_deref().unwrap_or("")
    }

    pub fn address_city_value(&self) -> &str {
        self.address_city.as_deref().unwrap_or("")
    }

    pub fn address_state_value(&self) -> &str {
        self.address_state.as_deref().unwrap_or("")
    }

    pub fn address_postal_code_value(&self) -> &str {
        self.address_postal_code.as_deref().unwrap_or("")
    }
}
