use crate::models::{BookCard, CartView, CatalogFilters, VariantAttribute};
use crate::ui::{
    ButtonView, CartLineView, CheckoutLineView, CheckoutSectionView, LinkView, OrderSummaryView,
    ProductCardView, ProductSectionView, RemovedCartNoticeView, SavedLineView,
};
use askama::Template;
use rust_decimal::Decimal;
use std::collections::HashMap;

pub fn format_money(val: &f64) -> String {
    format!("${:.2}", val)
}

pub fn is_selected(current: Option<&str>, option: &str) -> bool {
    current.unwrap_or("") == option
}

pub trait TemplateHelpers {
    fn store_name(&self) -> &'static str {
        crate::brand::STORE_NAME
    }

    fn store_short_name(&self) -> &'static str {
        crate::brand::STORE_SHORT_NAME
    }

    fn store_city(&self) -> &'static str {
        crate::brand::STORE_CITY
    }

    fn store_hours(&self) -> &'static str {
        crate::brand::STORE_HOURS
    }

    fn store_domain_label(&self) -> &'static str {
        crate::brand::STORE_DOMAIN_LABEL
    }

    fn search_placeholder(&self) -> &'static str {
        crate::brand::SEARCH_PLACEHOLDER
    }

    fn money(&self, val: &f64) -> String {
        format_money(val)
    }

    fn money_dec(&self, val: &Decimal) -> String {
        format!("${:.2}", val)
    }

    fn discount_pct(&self, price: &f64, list: &f64) -> i64 {
        if *list <= 0.0 || *list <= *price {
            return 0;
        }
        ((1.0 - *price / *list) * 100.0).round() as i64
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
    pub featured: BookCard,
    pub featured_add_button: ButtonView,
    pub featured_buy_now_button: ButtonView,
    pub quick_fillers: Vec<BookCard>,
    pub product_sections: Vec<ProductSectionView>,
    pub staff_picks: Vec<BookCard>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
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
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
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
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub saved_lines: Vec<SavedLineView>,
    pub saved_count_label: String,
    pub checkout_button: ButtonView,
    pub browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
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
#[template(path = "components/cart_page_content.html")]
pub struct CartPageContentTemplate {
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub saved_lines: Vec<SavedLineView>,
    pub saved_count_label: String,
    pub checkout_button: ButtonView,
    pub browse_books_link: LinkView,
}
impl TemplateHelpers for CartPageContentTemplate {}

impl axum::response::IntoResponse for CartPageContentTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("CartPageContentTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "checkout.html")]
pub struct CheckoutTemplate {
    pub sections: Vec<CheckoutSectionView>,
    pub checkout_lines: Vec<CheckoutLineView>,
    pub summary: OrderSummaryView,
}
impl TemplateHelpers for CheckoutTemplate {}

impl axum::response::IntoResponse for CheckoutTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("CheckoutTemplate rendering failed: {:?}", err);
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
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
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
#[template(path = "search.html")]
pub struct SearchTemplate {
    pub title: String,
    pub query: String,
    pub genres: Vec<String>,
    pub conditions: Vec<String>,
    pub formats: Vec<String>,
    pub show_new_checked: bool,
    pub show_used_checked: bool,
    pub min_rating: String,
    pub catalog_cards: Vec<ProductCardView>,
    pub filters: CatalogFilters,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
}
impl TemplateHelpers for SearchTemplate {}

impl SearchTemplate {
    pub fn rating_star_filled(&self, star: i32) -> bool {
        let active_star = self.min_rating.parse::<i32>().unwrap_or(1);
        star <= active_star
    }
}

impl axum::response::IntoResponse for SearchTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("SearchTemplate rendering failed: {:?}", err);
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

#[derive(Template)]
#[template(path = "signup.html")]
pub struct SignupTemplate {
    pub error_message: Option<String>,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub genres: Vec<String>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
}
impl TemplateHelpers for SignupTemplate {}

impl axum::response::IntoResponse for SignupTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("SignupTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error_message: Option<String>,
    pub email: String,
    pub genres: Vec<String>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
}
impl TemplateHelpers for LoginTemplate {}

impl axum::response::IntoResponse for LoginTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("LoginTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "account_profile.html")]
pub struct AccountProfileTemplate {
    pub user: crate::models::User,
    pub genres: Vec<String>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}
impl TemplateHelpers for AccountProfileTemplate {}

impl axum::response::IntoResponse for AccountProfileTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("AccountProfileTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "account_home.html")]
pub struct AccountHomeTemplate {
    pub genres: Vec<String>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
}
impl TemplateHelpers for AccountHomeTemplate {}

impl axum::response::IntoResponse for AccountHomeTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("AccountHomeTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "account_security.html")]
pub struct AccountSecurityTemplate {
    pub user: crate::models::User,
    pub genres: Vec<String>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
}
impl TemplateHelpers for AccountSecurityTemplate {}

impl axum::response::IntoResponse for AccountSecurityTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("AccountSecurityTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "account_orders.html")]
pub struct AccountOrdersTemplate {
    pub genres: Vec<String>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
}
impl TemplateHelpers for AccountOrdersTemplate {}

impl axum::response::IntoResponse for AccountOrdersTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("AccountOrdersTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Template)]
#[template(path = "account_preferences.html")]
pub struct AccountPreferencesTemplate {
    pub user: crate::models::User,
    pub genres: Vec<String>,
    pub cart: CartView,
    pub cart_lines: Vec<CartLineView>,
    pub removed_notice: Option<RemovedCartNoticeView>,
    pub drawer_checkout_button: ButtonView,
    pub drawer_browse_books_link: LinkView,
    pub current_user: Option<crate::models::User>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}
impl TemplateHelpers for AccountPreferencesTemplate {}

impl axum::response::IntoResponse for AccountPreferencesTemplate {
    fn into_response(self) -> axum::response::Response {
        match self.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(err) => {
                tracing::error!("AccountPreferencesTemplate rendering failed: {:?}", err);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
