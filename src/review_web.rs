use crate::{
    app::AppState,
    auth,
    errors::AppError,
    reviews,
    templates::TemplateHelpers,
    ui::{ButtonView, LinkView},
};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

#[derive(Clone, Debug)]
pub struct ReviewSummaryView {
    pub count: i64,
    pub label: String,
    pub stars: String,
}
impl ReviewSummaryView {
    pub fn new(count: i64, average: f64) -> Self {
        let n = average.round().clamp(0., 5.) as usize;
        Self {
            count,
            label: if count == 0 {
                "No reviews yet".into()
            } else {
                format!(
                    "{average:.1} out of 5 · {count} {}",
                    if count == 1 { "review" } else { "reviews" }
                )
            },
            stars: format!("{}{}", "★".repeat(n), "☆".repeat(5 - n)),
        }
    }
}
pub fn button(label: &str) -> ButtonView {
    let mut b = ButtonView::form_submit(label);
    b.class_name = "primary-button ui-review-button".into();
    b
}
pub fn moderation_button(label: &str, action: &str) -> ButtonView {
    let mut b = button(label);
    b.submit_name = "action".into();
    b.submit_value = action.into();
    b
}
pub fn link(label: &str, url: &str) -> LinkView {
    LinkView::tracked(
        label,
        url,
        "ui-review-link",
        "review_navigation",
        "reviews",
        "page",
        url,
    )
}
#[derive(Clone)]
pub struct ReviewCardView {
    pub entry: reviews::Entry,
    pub stars: String,
    pub date: String,
    pub status_label: String,
    pub book_link: LinkView,
    pub edit_link: LinkView,
    pub detail_link: LinkView,
    pub delete_button: ButtonView,
    pub helpful_button: ButtonView,
    pub unvote_button: ButtonView,
    pub report_button: ButtonView,
    pub report_field: ReviewFieldView,
}
impl ReviewCardView {
    fn new(e: reviews::Entry) -> Self {
        let label = match (e.review_status.as_str(), e.status.as_str()) {
            ("removed", _) => "Removed by moderation — see reason",
            (_, "pending") if e.published_revision.is_some() => {
                "Changes awaiting moderation; approved version remains public"
            }
            (_, "pending") => "Awaiting moderation",
            (_, "rejected") if e.published_revision.is_some() => {
                "Changes rejected; approved version remains public"
            }
            (_, "rejected") => "Not published — see moderator reason",
            (_, "published") => "Published",
            _ => "Removed",
        };
        Self {
            stars: format!(
                "{}{}",
                "★".repeat(e.rating as usize),
                "☆".repeat(5 - e.rating as usize)
            ),
            date: e.created_at.format("%b %e, %Y").to_string(),
            status_label: label.into(),
            book_link: link(&e.book_title, &format!("/books/{}", e.book_id)),
            detail_link: link("Inspect and moderate", &format!("/staff/reviews/{}", e.id)),
            edit_link: link("Edit review", &format!("/books/{}/reviews/edit", e.book_id)),
            delete_button: button("Delete review"),
            helpful_button: button("Helpful"),
            unvote_button: button("Remove my vote"),
            report_button: button("Send report"),
            report_field: ReviewFieldView::new(
                format!("report-{}", e.id),
                "reason",
                "Reason for reporting",
                String::new(),
                1000,
                3,
                3,
                true,
            ),
            entry: e,
        }
    }
}
#[derive(Clone)]
pub struct ReviewFieldView {
    pub id: String,
    pub name: &'static str,
    pub label: &'static str,
    pub value: String,
    pub max: usize,
    pub min: usize,
    pub rows: usize,
    pub required: bool,
}
impl ReviewFieldView {
    // Keep the complete HTML field constraints in one view constructor.
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: String,
        name: &'static str,
        label: &'static str,
        value: String,
        max: usize,
        min: usize,
        rows: usize,
        required: bool,
    ) -> Self {
        Self {
            id,
            name,
            label,
            value,
            max,
            min,
            rows,
            required,
        }
    }
}
#[derive(Clone)]
pub struct ReviewFormView {
    pub input: reviews::Input,
    pub action: String,
    pub submit: ButtonView,
    pub fields: Vec<ReviewFieldView>,
}
impl ReviewFormView {
    pub fn selected(&self, rating: &i32) -> bool {
        self.input.rating == *rating
    }
}
#[derive(Clone)]
pub struct ReviewPanelView {
    pub summary: reviews::Summary,
    pub cards: Vec<ReviewCardView>,
    pub navigation: crate::ui::PageNavView,
    pub write_link: LinkView,
    pub logged_in: bool,
    pub user_id: String,
    pub csrf: String,
    pub sort: String,
    pub book_id: String,
    pub review_summary: ReviewSummaryView,
    pub sort_button: ButtonView,
}
pub async fn panel(
    db: &crate::db::DbPool,
    session: &Session,
    book: &str,
) -> Result<ReviewPanelView, AppError> {
    let work = reviews::work(db, book).await?;
    let user = auth::get_current_user(db, session).await?;
    let sort = crate::pages::value("review_sort").unwrap_or_else(|| "recent".into());
    let page = crate::pages::number("review_page");
    let summary = reviews::summary(db, &work.work_key).await?;
    let cards = reviews::public_list(db, &work.work_key, page, &sort)
        .await?
        .into_iter()
        .map(ReviewCardView::new)
        .collect();
    let own = if let Some(u) = &user {
        reviews::own(db, &work.work_key, &u.id).await?
    } else {
        None
    };
    Ok(ReviewPanelView {
        navigation: crate::ui::page_navigation(
            &format!("/books/{book}"),
            "review_page",
            page,
            summary.published_count,
            20,
        ),
        review_summary: ReviewSummaryView::new(summary.published_count, summary.average_rating),
        summary,
        cards,
        write_link: link(
            if own.is_some() {
                "Manage your review"
            } else {
                "Write a review"
            },
            &format!("/books/{book}/reviews/edit"),
        ),
        logged_in: user.is_some(),
        user_id: user.map(|u| u.id).unwrap_or_default(),
        csrf: crate::account_email::csrf(session).await,
        sort,
        book_id: book.into(),
        sort_button: button("Sort"),
    })
}
#[derive(Template)]
#[template(path = "reviews.html")]
pub struct ReviewPage {
    pub heading: String,
    pub message: String,
    pub error: String,
    pub cards: Vec<ReviewCardView>,
    pub csrf: String,
    pub navigation: crate::ui::PageNavView,
    pub form: Option<ReviewFormView>,
    pub own: bool,
    pub staff: bool,
    pub moderation: bool,
    pub history: Vec<reviews::History>,
    pub reports: Vec<reviews::Report>,
    pub published: Option<ReviewCardView>,
    pub detail_id: i64,
    pub filter: String,
    pub moderation_reason: ReviewFieldView,
    pub approve_button: ButtonView,
    pub reject_button: ButtonView,
    pub remove_button: ButtonView,
    pub resolve_button: ButtonView,
    pub filter_links: Vec<LinkView>,
    pub home_link: LinkView,
    pub account_link: LinkView,
    pub my_link: LinkView,
    pub staff_link: LinkView,
}
impl TemplateHelpers for ReviewPage {}
impl ReviewPage {
    fn new(heading: &str, csrf: String) -> Self {
        Self {
            heading: heading.into(),
            message: String::new(),
            error: String::new(),
            cards: vec![],
            csrf,
            navigation: crate::ui::page_navigation("/account/reviews", "review_page", 1, 0, 20),
            form: None,
            own: false,
            staff: false,
            moderation: false,
            history: vec![],
            reports: vec![],
            published: None,
            detail_id: 0,
            filter: "pending".into(),
            moderation_reason: ReviewFieldView::new(
                "moderation-reason".into(),
                "reason",
                "Reason (required for rejection or removal)",
                String::new(),
                1000,
                0,
                3,
                false,
            ),
            approve_button: moderation_button("Approve", "approve"),
            reject_button: moderation_button("Reject", "reject"),
            remove_button: moderation_button("Remove published review", "remove"),
            resolve_button: moderation_button("Resolve reports", "report_resolved"),
            filter_links: ["pending", "published", "reported", "rejected", "removed"]
                .into_iter()
                .map(|v| link(v, &format!("/staff/reviews?filter={v}")))
                .collect(),
            home_link: link("Browse books", "/search"),
            account_link: link("Your account", "/account"),
            my_link: link("My reviews", "/account/reviews"),
            staff_link: link("Review moderation", "/staff/reviews"),
        }
    }
    fn response(self, status: StatusCode) -> Result<Response, AppError> {
        Ok((
            status,
            [(header::CACHE_CONTROL, "private, no-store")],
            axum::response::Html(self.render()?),
        )
            .into_response())
    }
}
#[derive(Default, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub notice: String,
}
async fn user(db: &crate::db::DbPool, s: &Session) -> Result<crate::models::User, AppError> {
    auth::get_current_user(db, s).await?.ok_or(AppError::Http(
        StatusCode::UNAUTHORIZED,
        "Please sign in to manage reviews.".into(),
    ))
}
async fn csrf(s: &Session, h: &HeaderMap, token: &str) -> Result<(), AppError> {
    if crate::account_email::check_csrf(s, h, token).await {
        Ok(())
    } else {
        Err(AppError::Http(
            StatusCode::FORBIDDEN,
            "Your form expired. Reload and try again.".into(),
        ))
    }
}
pub async fn my_reviews(
    State(state): State<AppState>,
    s: Session,
    Query(q): Query<ListQuery>,
) -> Result<Response, AppError> {
    let Some(u) = auth::get_current_user(&state.db, &s).await? else {
        return Ok(Redirect::to("/login").into_response());
    };
    let (entries, count) =
        reviews::mine(&state.db, &u.id, crate::pages::number("review_page")).await?;
    let mut p = ReviewPage::new("My reviews", crate::account_email::csrf(&s).await);
    p.own = true;
    p.staff = reviews::is_moderator(&state.db, &u.id).await?;
    p.message = match q.notice.as_str() {
        "saved" => "Your review was saved and is awaiting moderation.",
        "deleted" => "Your review was deleted and no longer contributes to public ratings.",
        _ => "Reviews are shared across editions with the same title and author.",
    }
    .into();
    p.cards = entries.into_iter().map(ReviewCardView::new).collect();
    p.navigation = crate::ui::page_navigation(
        "/account/reviews",
        "review_page",
        crate::pages::number("review_page"),
        count,
        20,
    );
    p.response(StatusCode::OK)
}
async fn edit_page(
    state: &AppState,
    s: &Session,
    book: &str,
    input: Option<reviews::Input>,
    error: String,
    status: StatusCode,
) -> Result<Response, AppError> {
    let Some(u) = auth::get_current_user(&state.db, s).await? else {
        return Ok(Redirect::to("/login").into_response());
    };
    let work = reviews::work(&state.db, book).await?;
    let own = reviews::own(&state.db, &work.work_key, &u.id).await?;
    let input = if let Some(i) = input {
        i
    } else if let Some(r) = own {
        let v = reviews::revision(&state.db, r.current_revision.ok_or(AppError::NotFound)?).await?;
        reviews::Input {
            rating: v.rating,
            title: v.title,
            body: v.body,
            revision: v.id,
        }
    } else {
        reviews::Input {
            rating: 0,
            title: String::new(),
            body: String::new(),
            revision: 0,
        }
    };
    let mut p = ReviewPage::new(
        &format!("Review {} by {}", work.title, work.author),
        crate::account_email::csrf(s).await,
    );
    p.message="Review the book itself. Copy condition, delivery, and order concerns belong with customer support. Only approved reviews contribute to stars. Changes to a published review await moderation while the approved version stays public.".into();
    p.error = error;
    p.form = Some(ReviewFormView {
        fields: vec![
            ReviewFieldView::new(
                "review-title".into(),
                "title",
                "Review title (optional, up to 120 characters)",
                input.title.clone(),
                120,
                0,
                0,
                false,
            ),
            ReviewFieldView::new(
                "review-body".into(),
                "body",
                "Your review (optional, up to 5,000 characters)",
                input.body.clone(),
                5000,
                0,
                7,
                false,
            ),
        ],
        input,
        action: format!("/books/{book}/reviews"),
        submit: button("Submit review"),
    });
    p.response(status)
}
pub async fn edit(
    State(state): State<AppState>,
    s: Session,
    Path(book): Path<String>,
) -> Result<Response, AppError> {
    edit_page(&state, &s, &book, None, String::new(), StatusCode::OK).await
}
#[derive(Deserialize)]
pub struct Submit {
    pub csrf: String,
    pub rating: i32,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub revision: i64,
}
pub async fn submit(
    State(state): State<AppState>,
    s: Session,
    h: HeaderMap,
    Path(book): Path<String>,
    Form(f): Form<Submit>,
) -> Result<Response, AppError> {
    csrf(&s, &h, &f.csrf).await?;
    let principal = crate::access::principal(&state.db, &s).await?;
    let input = reviews::Input {
        rating: f.rating,
        title: f.title,
        body: f.body,
        revision: f.revision,
    };
    match reviews::submit(&state.db, &principal, &book, &input).await {
        Ok(_) => Ok(Redirect::to("/account/reviews?notice=saved").into_response()),
        Err(AppError::Validation(msg)) => {
            edit_page(&state, &s, &book, Some(input), msg, StatusCode::BAD_REQUEST).await
        }
        Err(AppError::Http(code, msg)) => {
            edit_page(&state, &s, &book, Some(input), msg, code).await
        }
        Err(e) => Err(e),
    }
}
#[derive(Deserialize)]
pub struct Action {
    pub csrf: String,
    pub revision: i64,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub vote: i32,
}
pub async fn delete(
    State(state): State<AppState>,
    s: Session,
    h: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<Action>,
) -> Result<Response, AppError> {
    csrf(&s, &h, &f.csrf).await?;
    let principal = crate::access::principal(&state.db, &s).await?;
    reviews::delete(&state.db, &principal, id, f.revision).await?;
    Ok(Redirect::to("/account/reviews?notice=deleted").into_response())
}
pub async fn vote(
    State(state): State<AppState>,
    s: Session,
    h: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<Action>,
) -> Result<Response, AppError> {
    csrf(&s, &h, &f.csrf).await?;
    let principal = crate::access::principal(&state.db, &s).await?;
    reviews::vote(&state.db, &principal, id, f.revision, f.vote).await?;
    Ok(Redirect::to("/account/reviews/activity?notice=voted").into_response())
}
pub async fn report(
    State(state): State<AppState>,
    s: Session,
    h: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<Action>,
) -> Result<Response, AppError> {
    csrf(&s, &h, &f.csrf).await?;
    let principal = crate::access::principal(&state.db, &s).await?;
    reviews::report(&state.db, &principal, id, f.revision, &f.reason).await?;
    Ok(Redirect::to("/account/reviews/activity?notice=reported").into_response())
}
pub async fn activity(
    State(state): State<AppState>,
    s: Session,
    Query(q): Query<ListQuery>,
) -> Result<Response, AppError> {
    user(&state.db, &s).await?;
    let mut p = ReviewPage::new("Review activity", crate::account_email::csrf(&s).await);
    p.message=if q.notice=="reported"{"Your report was sent to the moderation team. The review remains visible while it is assessed."}else{"Your helpful vote was saved."}.into();
    p.response(StatusCode::OK)
}
pub async fn staff(
    State(state): State<AppState>,
    s: Session,
    Query(q): Query<ListQuery>,
) -> Result<Response, AppError> {
    crate::access::require_permission(&state.db, &s, "reviews.moderate").await?;
    let (entries, count) =
        reviews::queue(&state.db, &q.filter, crate::pages::number("review_page")).await?;
    let mut p = ReviewPage::new("Review moderation", crate::account_email::csrf(&s).await);
    p.staff = true;
    p.moderation = true;
    p.filter = if ["pending", "published", "reported", "rejected", "removed"]
        .contains(&q.filter.as_str())
    {
        q.filter
    } else {
        "pending".into()
    };
    p.message="Approve honest opinions regardless of rating. Reject spam, harassment, personal information, or unrelated content. Never rewrite a reader’s review. You cannot approve your own review.".into();
    p.cards = entries.into_iter().map(ReviewCardView::new).collect();
    p.navigation = crate::ui::page_navigation(
        "/staff/reviews",
        "review_page",
        crate::pages::number("review_page"),
        count,
        20,
    );
    p.response(StatusCode::OK)
}
pub async fn staff_detail(
    State(state): State<AppState>,
    s: Session,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    staff_detail_page(&state, &s, id, String::new(), StatusCode::OK).await
}
async fn staff_detail_page(
    state: &AppState,
    s: &Session,
    id: i64,
    error: String,
    status: StatusCode,
) -> Result<Response, AppError> {
    crate::access::require_permission(&state.db, s, "reviews.moderate").await?;
    let r: reviews::Review =
        sqlx::query_as("SELECT * FROM reviews WHERE id=$1 AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&state.db)
            .bounded_one()
            .await?
            .ok_or(AppError::NotFound)?;
    let work = reviews::work(&state.db, &r.book_id).await?;
    let v = reviews::revision(&state.db, r.current_revision.ok_or(AppError::NotFound)?).await?;
    let entry = |v: reviews::Revision| reviews::Entry {
        id: r.id,
        book_id: r.book_id.clone(),
        user_id: r.user_id.clone(),
        book_title: work.title.clone(),
        author: work.author.clone(),
        display_name: "Reader".into(),
        rating: v.rating,
        title: v.title,
        body: v.body,
        status: v.status,
        review_status: r.status.clone(),
        reason: v.reason,
        revision: v.id,
        published_revision: r.published_revision,
        created_at: v.created_at,
        helpful_count: 0,
        report_count: 0,
    };
    let mut p = ReviewPage::new("Moderate review", crate::account_email::csrf(s).await);
    p.error = error;
    p.staff = true;
    p.moderation = true;
    p.detail_id = id;
    p.cards = vec![ReviewCardView::new(entry(v))];
    if let Some(rev) = r.published_revision {
        p.published = Some(ReviewCardView::new(entry(
            reviews::revision(&state.db, rev).await?,
        )));
    }
    p.history = reviews::history(&state.db, id).await?;
    p.reports = reviews::reports(&state.db, id).await?;
    p.response(status)
}
use crate::read_budget::ReadFutureExt;
pub async fn moderate(
    State(state): State<AppState>,
    s: Session,
    h: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<Action>,
) -> Result<Response, AppError> {
    csrf(&s, &h, &f.csrf).await?;
    let principal = crate::access::principal(&state.db, &s).await?;
    match reviews::moderate(&state.db, &principal, id, f.revision, &f.action, &f.reason).await {
        Ok(()) => Ok(Redirect::to(&format!("/staff/reviews/{id}")).into_response()),
        Err(AppError::Validation(msg)) => {
            staff_detail_page(&state, &s, id, msg, StatusCode::BAD_REQUEST).await
        }
        Err(AppError::Http(code, msg)) if code == StatusCode::CONFLICT => {
            match staff_detail_page(&state, &s, id, msg.clone(), code).await {
                Err(AppError::NotFound) => {
                    let mut p =
                        ReviewPage::new("Review unavailable", crate::account_email::csrf(&s).await);
                    p.error = msg;
                    p.response(code)
                }
                other => other,
            }
        }
        Err(e) => Err(e),
    }
}
