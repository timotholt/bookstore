use askama::Template;
use rust_decimal::Decimal;
use std::collections::HashMap;
use crate::models::{BookCard, CartView, CatalogFilters, VariantAttribute};
use crate::ui::{ButtonView, CartLineView, ProductCardView, ProductSectionView};

pub fn format_money(val: &f64) -> String {
    format!("${:.2}", val)
}

pub fn free_pickup(val: &f64) -> bool {
    *val > 8.0
}

pub fn is_selected(current: Option<&str>, option: &str) -> bool {
    current.unwrap_or("") == option
}

pub trait TemplateHelpers {
    fn money(&self, val: &f64) -> String {
        format_money(val)
    }

    fn money_val(&self, val: f64) -> String {
        format_money(&val)
    }

    fn money_dec(&self, val: &Decimal) -> String {
        format!("${:.2}", val)
    }

    fn list_price(&self, val: &f64) -> f64 {
        (*val * 1.5 * 100.0).round() / 100.0
    }

    fn price_dollars(&self, val: &f64) -> i64 {
        val.trunc() as i64
    }

    fn price_cents(&self, val: &f64) -> String {
        let cents = ((*val - val.trunc()) * 100.0).round() as i64;
        format!("{:02}", cents)
    }

    fn selected(&self, current: Option<&str>, option: &str) -> bool {
        is_selected(current, option)
    }
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub title: String,
    pub genres: Vec<String>,
    pub conditions: Vec<String>,
    pub formats: Vec<String>,
    pub featured: BookCard,
    pub featured_add_button: ButtonView,
    pub featured_buy_now_button: ButtonView,
    pub quick_fillers: Vec<BookCard>,
    pub product_sections: Vec<ProductSectionView>,
    pub catalog_cards: Vec<ProductCardView>,
    pub staff_picks: Vec<BookCard>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub filters: CatalogFilters,
}
impl TemplateHelpers for HomeTemplate {}

impl axum::response::IntoResponse for HomeTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("HomeTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "book_detail.html")]
pub struct BookDetailTemplate {
    pub genres: Vec<String>,
    pub book: BookCard,
    pub copies: Vec<BookCard>,
    pub attributes: HashMap<i64, Vec<VariantAttribute>>,
    pub related_cards: Vec<ProductCardView>,
    pub add_button: ButtonView,
    pub buy_now_button: ButtonView,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
}
impl TemplateHelpers for BookDetailTemplate {}

impl axum::response::IntoResponse for BookDetailTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("BookDetailTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

impl BookDetailTemplate {
    pub fn json_attributes(&self, copy_id: &i64) -> String {
        if let Some(attrs) = self.attributes.get(copy_id) {
            serde_json::to_string(attrs).unwrap_or_else(|_| "[]".to_string())
        } else {
            "[]".to_string()
        }
    }
}

#[derive(Template)]
#[template(path = "cart.html")]
pub struct CartPageTemplate {
    pub genres: Vec<String>,
    pub cart: CartView,
}
impl TemplateHelpers for CartPageTemplate {}

impl axum::response::IntoResponse for CartPageTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("CartPageTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "components/cart.html")]
pub struct CartDrawerTemplate {
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
}
impl TemplateHelpers for CartDrawerTemplate {}

impl axum::response::IntoResponse for CartDrawerTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("CartDrawerTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "components/catalog_results.html")]
pub struct CatalogResultsTemplate {
    pub catalog_cards: Vec<ProductCardView>,
    pub filters: CatalogFilters,
}
impl TemplateHelpers for CatalogResultsTemplate {}

impl axum::response::IntoResponse for CatalogResultsTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("CatalogResultsTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
