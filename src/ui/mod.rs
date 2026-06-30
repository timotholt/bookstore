use crate::models::{BookCard, CartLine};

#[derive(Debug, Clone)]
pub struct AnalyticsAttrs {
    pub impression_event: &'static str,
    pub click_event: &'static str,
    pub source: String,
    pub target_type: &'static str,
    pub target_id: String,
}

impl AnalyticsAttrs {
    pub fn product(source: impl Into<String>, target_id: impl Into<String>) -> Self {
        Self {
            impression_event: "product_impression",
            click_event: "product_clicked",
            source: source.into(),
            target_type: "book",
            target_id: target_id.into(),
        }
    }

    pub fn click(
        click_event: &'static str,
        source: impl Into<String>,
        target_type: &'static str,
        target_id: impl Into<String>,
    ) -> Self {
        Self {
            impression_event: "",
            click_event,
            source: source.into(),
            target_type,
            target_id: target_id.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HtmxAttrs {
    pub enabled: bool,
    pub post_url: String,
    pub vals: String,
    pub target: String,
    pub swap: String,
    pub headers: String,
}

impl HtmxAttrs {
    pub fn none() -> Self {
        Self {
            enabled: false,
            post_url: String::new(),
            vals: String::new(),
            target: String::new(),
            swap: String::new(),
            headers: String::new(),
        }
    }

    pub fn post(
        post_url: impl Into<String>,
        vals: impl Into<String>,
        target: impl Into<String>,
        swap: impl Into<String>,
    ) -> Self {
        Self {
            enabled: true,
            post_url: post_url.into(),
            vals: vals.into(),
            target: target.into(),
            swap: swap.into(),
            headers: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinkView {
    pub label: String,
    pub href: String,
    pub class_name: String,
    pub strong: bool,
    pub analytics: AnalyticsAttrs,
}

impl LinkView {
    pub fn tracked(
        label: impl Into<String>,
        href: impl Into<String>,
        class_name: impl Into<String>,
        click_event: &'static str,
        source: impl Into<String>,
        target_type: &'static str,
        target_id: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
            class_name: class_name.into(),
            strong: false,
            analytics: AnalyticsAttrs::click(click_event, source, target_type, target_id),
        }
    }

    pub fn product_title(book: &BookCard, source: impl Into<String>) -> Self {
        Self {
            label: book.title.clone(),
            href: format!("/books/{}", book.id),
            class_name: "card-title-link".to_string(),
            strong: true,
            analytics: AnalyticsAttrs::click("product_clicked", source, "book", book.id.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ButtonView {
    pub label: String,
    pub class_name: String,
    pub button_type: &'static str,
    pub disabled: bool,
    pub data_action: String,
    pub target_id: String,
    pub aria_label: String,
    pub analytics: AnalyticsAttrs,
    pub htmx: HtmxAttrs,
}

impl ButtonView {
    pub fn tracked(
        label: impl Into<String>,
        class_name: impl Into<String>,
        button_type: &'static str,
        data_action: impl Into<String>,
        aria_label: impl Into<String>,
        click_event: &'static str,
        source: impl Into<String>,
        target_type: &'static str,
        target_id: impl Into<String>,
    ) -> Self {
        let target_id = target_id.into();
        Self {
            label: label.into(),
            class_name: class_name.into(),
            button_type,
            disabled: false,
            data_action: data_action.into(),
            target_id: target_id.clone(),
            aria_label: aria_label.into(),
            analytics: AnalyticsAttrs::click(click_event, source, target_type, target_id),
            htmx: HtmxAttrs::none(),
        }
    }

    pub fn cart_action(
        label: impl Into<String>,
        class_name: impl Into<String>,
        data_action: impl Into<String>,
        click_event: &'static str,
        book: &BookCard,
        source: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            class_name: class_name.into(),
            button_type: "button",
            disabled: false,
            data_action: data_action.into(),
            target_id: book.id.clone(),
            aria_label: String::new(),
            analytics: AnalyticsAttrs::click(click_event, source, "book", book.id.clone()),
            htmx: HtmxAttrs::post(
                "/cart/items",
                format!(r#"{{"copy_id":"{}"}}"#, book.copy_id),
                "#cartDrawer",
                "outerHTML show:none",
            ),
        }
    }

    pub fn cart_line_action(
        label: impl Into<String>,
        class_name: impl Into<String>,
        aria_label: impl Into<String>,
        click_event: &'static str,
        copy_id: i64,
        url: impl Into<String>,
        target: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            class_name: class_name.into(),
            button_type: "button",
            disabled: false,
            data_action: String::new(),
            target_id: copy_id.to_string(),
            aria_label: aria_label.into(),
            analytics: AnalyticsAttrs::click(click_event, source, "copy", copy_id.to_string()),
            htmx: HtmxAttrs::post(url, "", target, "outerHTML show:none"),
        }
    }

    pub fn checkout_place_order(source: impl Into<String>) -> Self {
        Self::tracked(
            "Place your order",
            "primary-button checkout-place-order-button",
            "submit",
            "place-order",
            "Place your order",
            "checkout_place_order_clicked",
            source,
            "checkout",
            "current",
        )
    }
}

#[derive(Debug, Clone)]
pub struct ProductCardView {
    pub book: BookCard,
    pub rating_count: i32,
    pub extra_class: String,
    pub analytics: AnalyticsAttrs,
    pub title_link: LinkView,
    pub add_button: ButtonView,
    pub buy_now_button: ButtonView,
}

#[derive(Debug, Clone)]
pub struct CartLineView {
    pub line: CartLine,
    pub unit_price_label: String,
    pub line_total_label: String,
    pub option_label: String,
    pub stock_limit_label: String,
    pub decrease_button: ButtonView,
    pub increase_button: ButtonView,
    pub remove_button: ButtonView,
    pub save_for_later_button: ButtonView,
}

impl CartLineView {
    pub fn from_line(line: CartLine, htmx_target: impl Into<String>) -> Self {
        Self::from_line_with_context(line, htmx_target, "cart.line", false)
    }

    pub fn for_cart_page(line: CartLine) -> Self {
        Self::from_line_with_context(line, "#cartPageMain", "cart.page", true)
    }

    fn from_line_with_context(
        line: CartLine,
        htmx_target: impl Into<String>,
        source: impl Into<String>,
        page_redirect: bool,
    ) -> Self {
        let htmx_target = htmx_target.into();
        let source = source.into();
        let copy_id = line.book.copy_id;
        let option_label = if line.book.condition.is_empty() {
            line.book.format.clone()
        } else {
            format!("{} · {}", line.book.condition, line.book.format)
        };
        let at_stock_limit = line.quantity >= line.book.stock;
        let stock_limit_label = if at_stock_limit {
            format!("Only {} available", line.book.stock)
        } else {
            String::new()
        };
        let mut increase_button = ButtonView::cart_line_action(
            "+",
            "stepper-button",
            "Increase quantity",
            "cart_quantity_increased",
            copy_id,
            format!("/cart/items/{}/increase", copy_id),
            htmx_target.clone(),
            source.clone(),
        );
        if page_redirect {
            increase_button.htmx.swap = "outerHTML show:none".to_string();
            increase_button.htmx.headers = r#"{"X-Cart-View":"page"}"#.to_string();
        }
        increase_button.disabled = at_stock_limit;
        let mut decrease_button = ButtonView::cart_line_action(
            "-",
            "stepper-button",
            "Decrease quantity",
            "cart_quantity_decreased",
            copy_id,
            format!("/cart/items/{}/decrease", copy_id),
            htmx_target.clone(),
            source.clone(),
        );
        let mut remove_button = ButtonView::cart_line_action(
            "Remove",
            "remove-button",
            "Remove item",
            "cart_item_removed",
            copy_id,
            format!("/cart/items/{}/remove", copy_id),
            htmx_target.clone(),
            source.clone(),
        );
        let mut save_for_later_button = ButtonView::cart_line_action(
            "Save for later",
            "cart-secondary-action",
            "Save for later",
            "cart_item_saved_for_later",
            copy_id,
            format!("/cart/items/{}/save-for-later", copy_id),
            htmx_target,
            source,
        );
        if page_redirect {
            decrease_button.htmx.swap = "outerHTML show:none".to_string();
            decrease_button.htmx.headers = r#"{"X-Cart-View":"page"}"#.to_string();
            remove_button.htmx.swap = "outerHTML show:none".to_string();
            remove_button.htmx.headers = r#"{"X-Cart-View":"page"}"#.to_string();
            save_for_later_button.htmx.swap = "outerHTML show:none".to_string();
            save_for_later_button.htmx.headers = r#"{"X-Cart-View":"page"}"#.to_string();
        } else {
            save_for_later_button.htmx.target = "#cartDrawer".to_string();
        }

        Self {
            unit_price_label: format!("${:.2} each", line.book.price),
            line_total_label: format!("${:.2}", line.line_total),
            option_label,
            stock_limit_label,
            decrease_button,
            increase_button,
            remove_button,
            save_for_later_button,
            line,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SavedLineView {
    pub line: CartLine,
    pub line_total_label: String,
    pub option_label: String,
    pub quantity_label: String,
    pub move_to_cart_button: ButtonView,
    pub remove_button: ButtonView,
}

#[derive(Debug, Clone)]
pub struct RemovedCartNoticeView {
    pub title: String,
    pub restore_button: ButtonView,
}

impl RemovedCartNoticeView {
    pub fn from_line_with_context(
        line: CartLine,
        htmx_target: impl Into<String>,
        source: impl Into<String>,
        page_redirect: bool,
    ) -> Self {
        let copy_id = line.book.copy_id;
        let mut restore_button = ButtonView::cart_line_action(
            "Undo",
            "cart-undo-link",
            "Undo remove item",
            "cart_item_remove_undone",
            copy_id,
            format!("/cart/items/{}/restore", copy_id),
            htmx_target,
            source,
        );
        if page_redirect {
            restore_button.htmx.swap = "outerHTML show:none".to_string();
            restore_button.htmx.headers = r#"{"X-Cart-View":"page"}"#.to_string();
        }

        Self {
            title: line.book.title,
            restore_button,
        }
    }
}

impl SavedLineView {
    pub fn from_line(line: CartLine) -> Self {
        let copy_id = line.book.copy_id;
        let option_label = if line.book.condition.is_empty() {
            line.book.format.clone()
        } else {
            format!("{} · {}", line.book.condition, line.book.format)
        };
        let mut move_to_cart_button = ButtonView::cart_line_action(
            "Move to cart",
            "cart-secondary-action saved-line-primary",
            "Move to cart",
            "saved_item_moved_to_cart",
            copy_id,
            format!("/saved-items/{}/move-to-cart", copy_id),
            "#cartPageMain",
            "cart.saved",
        );
        move_to_cart_button.htmx.swap = "outerHTML show:none".to_string();
        move_to_cart_button.htmx.headers = r#"{"X-Cart-View":"page"}"#.to_string();
        let mut remove_button = ButtonView::cart_line_action(
            "Remove",
            "remove-button",
            "Remove saved item",
            "saved_item_removed",
            copy_id,
            format!("/saved-items/{}/remove", copy_id),
            "#cartPageMain",
            "cart.saved",
        );
        remove_button.htmx.swap = "outerHTML show:none".to_string();
        remove_button.htmx.headers = r#"{"X-Cart-View":"page"}"#.to_string();

        Self {
            line_total_label: format!("${:.2}", line.line_total),
            quantity_label: format!("Saved quantity: {}", line.quantity),
            option_label,
            move_to_cart_button,
            remove_button,
            line,
        }
    }
}

pub fn cart_lines(lines: Vec<CartLine>, htmx_target: impl Into<String>) -> Vec<CartLineView> {
    let htmx_target = htmx_target.into();
    lines
        .into_iter()
        .map(|line| CartLineView::from_line(line, htmx_target.clone()))
        .collect()
}

pub fn cart_page_lines(lines: Vec<CartLine>) -> Vec<CartLineView> {
    lines.into_iter().map(CartLineView::for_cart_page).collect()
}

pub fn saved_lines(lines: Vec<CartLine>) -> Vec<SavedLineView> {
    lines.into_iter().map(SavedLineView::from_line).collect()
}

pub fn removed_notice(
    line: Option<CartLine>,
    htmx_target: impl Into<String>,
    source: impl Into<String>,
) -> Option<RemovedCartNoticeView> {
    line.map(|line| RemovedCartNoticeView::from_line_with_context(line, htmx_target, source, false))
}

pub fn cart_page_removed_notice(line: Option<CartLine>) -> Option<RemovedCartNoticeView> {
    line.map(|line| {
        RemovedCartNoticeView::from_line_with_context(line, "#cartPageMain", "cart.page", true)
    })
}

#[derive(Debug, Clone)]
pub struct CheckoutSectionView {
    pub class_name: String,
    pub eyebrow: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub has_link: bool,
    pub link: LinkView,
}

impl CheckoutSectionView {
    pub fn informational(
        class_name: impl Into<String>,
        eyebrow: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            class_name: class_name.into(),
            eyebrow: eyebrow.into(),
            title: title.into(),
            body: body.into(),
            status: status.into(),
            has_link: false,
            link: LinkView::tracked(
                "",
                "#checkout",
                "",
                "checkout_section_link_hidden",
                "checkout.section",
                "checkout",
                "current",
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckoutLineView {
    pub line: CartLine,
    pub line_total_label: String,
    pub option_label: String,
    pub quantity_label: String,
    pub pickup_name: String,
}

impl CheckoutLineView {
    pub fn from_line(line: CartLine) -> Self {
        let option_label = if line.book.condition.is_empty() {
            line.book.format.clone()
        } else {
            format!("{} · {}", line.book.condition, line.book.format)
        };
        Self {
            line_total_label: format!("${:.2}", line.line_total),
            quantity_label: format!("Quantity: {}", line.quantity),
            pickup_name: format!("pickup-{}", line.book.copy_id),
            option_label,
            line,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderSummaryView {
    pub subtotal_label: String,
    pub shipping_label: String,
    pub tax_label: String,
    pub total_label: String,
    pub place_order_button: ButtonView,
}

pub fn checkout_sections() -> Vec<CheckoutSectionView> {
    vec![
        CheckoutSectionView::informational(
            "checkout-address-section",
            "1. Delivery",
            "Delivering to La Habra, CA",
            "Store pickup at Davis's Books, 1261 Smoke Tree Dr, La Habra, CA 90631.",
            "Ready in 1-2 days",
        ),
        CheckoutSectionView::informational(
            "",
            "2. Payment",
            "Secure card payment",
            "Stripe Checkout will collect card details in the next phase. No card data is stored by Davis's Books.",
            "Not charged yet",
        ),
    ]
}

pub fn checkout_lines(lines: Vec<CartLine>) -> Vec<CheckoutLineView> {
    lines.into_iter().map(CheckoutLineView::from_line).collect()
}

pub fn order_summary(
    cart: &crate::models::CartView,
    source: impl Into<String>,
) -> OrderSummaryView {
    OrderSummaryView {
        subtotal_label: format!("${:.2}", cart.subtotal),
        shipping_label: if cart.free_shipping {
            "Free".to_string()
        } else {
            format!("${:.2}", cart.shipping)
        },
        tax_label: "Calculated at payment".to_string(),
        total_label: format!("${:.2}", cart.total),
        place_order_button: ButtonView::checkout_place_order(source),
    }
}

pub fn checkout_start_button(source: impl Into<String>, disabled: bool) -> ButtonView {
    let mut button = ButtonView::tracked(
        "Checkout",
        "primary-button checkout-button",
        "submit",
        "checkout",
        "Checkout",
        "checkout_started",
        source,
        "checkout",
        "current",
    );
    button.disabled = disabled;
    button
}

pub fn browse_books_link(source: impl Into<String>, class_name: impl Into<String>) -> LinkView {
    LinkView::tracked(
        "Browse used books",
        "/#catalog",
        class_name,
        "browse_books_clicked",
        source,
        "catalog",
        "used-books",
    )
}

impl ProductCardView {
    pub fn from_book(book: BookCard, source: impl Into<String>) -> Self {
        let source = source.into();
        let title_link = LinkView::product_title(&book, source.clone());
        let add_button = ButtonView::cart_action(
            "Add to Cart",
            "card-btn add-btn",
            "add",
            "add_to_cart_clicked",
            &book,
            source.clone(),
        );
        let buy_now_button = ButtonView::cart_action(
            "Buy Now",
            "card-btn buy-now-btn",
            "buy-now-card",
            "buy_now_clicked",
            &book,
            source.clone(),
        );

        Self {
            rating_count: 48,
            extra_class: String::new(),
            analytics: AnalyticsAttrs::product(source, book.id.clone()),
            title_link,
            add_button,
            buy_now_button,
            book,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProductSectionView {
    pub id: String,
    pub title_id: String,
    pub row_id: String,
    pub eyebrow: String,
    pub title: String,
    pub cta_href: String,
    pub cta_label: String,
    pub has_cta: bool,
    pub cards: Vec<ProductCardView>,
}

impl ProductSectionView {
    pub fn new(
        id: impl Into<String>,
        eyebrow: impl Into<String>,
        title: impl Into<String>,
        row_id: impl Into<String>,
        source: impl Into<String>,
        books: Vec<BookCard>,
    ) -> Self {
        let id = id.into();
        let source = source.into();
        let cards = books
            .into_iter()
            .map(|book| ProductCardView::from_book(book, source.clone()))
            .collect();

        Self {
            title_id: format!("{}Title", to_camel_id(&id)),
            row_id: row_id.into(),
            id,
            eyebrow: eyebrow.into(),
            title: title.into(),
            cta_href: String::new(),
            cta_label: String::new(),
            has_cta: false,
            cards,
        }
    }

    pub fn with_cta(mut self, href: impl Into<String>, label: impl Into<String>) -> Self {
        self.cta_href = href.into();
        self.cta_label = label.into();
        self.has_cta = true;
        self
    }
}

pub fn product_shelf(
    id: impl Into<String>,
    eyebrow: impl Into<String>,
    title: impl Into<String>,
    row_id: impl Into<String>,
    source: impl Into<String>,
    books: Vec<BookCard>,
) -> ProductSectionView {
    ProductSectionView::new(id, eyebrow, title, row_id, source, books)
}

pub fn product_cards(books: Vec<BookCard>, source: impl Into<String>) -> Vec<ProductCardView> {
    let source = source.into();
    books
        .into_iter()
        .map(|book| ProductCardView::from_book(book, source.clone()))
        .collect()
}

fn to_camel_id(id: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = false;
    for ch in id.chars() {
        if ch == '-' || ch == '_' || ch == ' ' {
            uppercase_next = true;
            continue;
        }
        if uppercase_next {
            out.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}
