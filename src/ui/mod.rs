use crate::models::BookCard;

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
}

impl HtmxAttrs {
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
    pub analytics: AnalyticsAttrs,
    pub htmx: HtmxAttrs,
}

impl ButtonView {
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
            analytics: AnalyticsAttrs::click(click_event, source, "book", book.id.clone()),
            htmx: HtmxAttrs::post(
                "/cart/items",
                format!(r#"{{"copy_id":"{}"}}"#, book.copy_id),
                "#cartDrawer",
                "outerHTML",
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProductCardView {
    pub book: BookCard,
    pub price_label: String,
    pub rating_count: i32,
    pub extra_class: String,
    pub analytics: AnalyticsAttrs,
    pub title_link: LinkView,
    pub add_button: ButtonView,
    pub buy_now_button: ButtonView,
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
            price_label: format!("${:.2}", book.price),
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
