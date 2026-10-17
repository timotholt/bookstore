use crate::read_budget::{ReadFutureExt, RowsFutureExt};
use crate::{
    access::{self, AccountLock, Principal},
    app::AppState,
    db::DbPool,
    errors::AppError,
};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use tower_sessions::Session;
const ROLES: &[&str] = &[
    "moderator",
    "support",
    "fulfillment",
    "refund",
    "administrator",
];
#[derive(sqlx::FromRow)]
pub struct AccountRow {
    pub id: String,
    pub email: String,
    pub name: String,
    pub status: String,
    pub roles: String,
    pub version: i64,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub line1: String,
    pub line2: String,
    pub city: String,
    pub state: String,
    pub postal: String,
    pub marketing: bool,
}
const ACCOUNT:&str="SELECT u.id,u.email,COALESCE(u.full_name,'') AS name,u.account_status AS status,COALESCE((SELECT string_agg(role,', ' ORDER BY role) FROM user_staff_roles WHERE user_id=u.id),'') AS roles,u.profile_version AS version,COALESCE(u.first_name,'') AS first_name,COALESCE(u.last_name,'') AS last_name,COALESCE(u.phone_number,'') AS phone,COALESCE(u.address_line1,'') AS line1,COALESCE(u.address_line2,'') AS line2,COALESCE(u.address_city,'') AS city,COALESCE(u.address_state,'') AS state,COALESCE(u.address_postal_code,'') AS postal,u.marketing_opt_in AS marketing FROM users u";
#[derive(Clone)]
pub struct FieldView {
    pub id: String,
    pub name: &'static str,
    pub label: &'static str,
    pub value: String,
    pub max: usize,
}
impl FieldView {
    fn new(name: &'static str, label: &'static str, value: String, max: usize) -> Self {
        Self {
            id: name.into(),
            name,
            label,
            value,
            max,
        }
    }
}
pub struct RoleView {
    pub reason: FieldView,
    pub button: crate::ui::ButtonView,
    pub name: &'static str,
    pub granted: bool,
}
#[derive(sqlx::FromRow)]
pub struct AuditRow {
    pub action: String,
    pub actor: String,
    pub reason: String,
    pub metadata: String,
    pub date: String,
}
impl AuditRow {
    fn label(&self) -> &str {
        match self.action.as_str() {
            "profile" => "Profile updated",
            "status" => "Account access updated",
            "sessions" => "Sessions revoked",
            "grant" => "Role granted",
            "revoke" => "Role revoked",
            "--bootstrap-admin" => "Administrator access granted",
            "--recover-admin" => "Administrator access restored",
            "--grant-review-moderator" => "Moderator access granted",
            "--revoke-review-moderator" => "Moderator access revoked",
            _ => "Staff access updated",
        }
    }

    fn details(&self) -> String {
        let data: serde_json::Value = serde_json::from_str(&self.metadata).unwrap_or_default();
        match self.action.as_str() {
            "profile" => "Changed name, phone, address and marketing preference.".into(),
            "status" => format!(
                "Access: {} → {}",
                data["before"].as_str().unwrap_or("unknown"),
                data["after"].as_str().unwrap_or("unknown")
            ),
            "sessions" => "Sessions and outstanding security links revoked.".into(),
            "grant" | "revoke" => {
                format!("Staff role: {}", data["role"].as_str().unwrap_or("unknown"))
            }
            _ => "Staff access updated by an operator.".into(),
        }
    }
}
#[derive(Template)]
#[template(path = "admin_users.html")]
struct UsersTemplate {
    nav: Vec<crate::ui::LinkView>,
    usage_notice: Option<crate::usage::Notice>,
    search: FieldView,
    can_support: bool,
    button: crate::ui::ButtonView,
    users: Vec<AccountRow>,
    q: String,
    page: i64,
    has_next: bool,
}
#[derive(Template)]
#[template(path = "admin_user.html")]
struct UserTemplate {
    nav: Vec<crate::ui::LinkView>,
    profile_reason: FieldView,
    status_reason: FieldView,
    sessions_reason: FieldView,
    profile_button: crate::ui::ButtonView,
    status_button: crate::ui::ButtonView,
    sessions_button: crate::ui::ButtonView,
    user: AccountRow,
    fields: Vec<FieldView>,
    roles: Vec<RoleView>,
    audit: Vec<AuditRow>,
    csrf: String,
    can_staff: bool,
    can_edit: bool,
    is_self: bool,
}
fn private(t: impl Template) -> Result<Response, AppError> {
    let mut r = axum::response::Html(t.render()?).into_response();
    r.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, no-store"),
    );
    Ok(r)
}
#[derive(Default, Deserialize)]
pub struct Search {
    #[serde(default)]
    q: String,
    #[serde(default)]
    page: i64,
}
pub async fn users(
    State(s): State<AppState>,
    session: Session,
    Query(q): Query<Search>,
) -> Result<Response, AppError> {
    let (_p, can_support, _, _) = directory(&s.db, &session).await?;
    let page = q.page.clamp(0, 100000);
    let search = q.q.chars().take(120).collect::<String>();
    let mut rows:Vec<AccountRow>=sqlx::query_as(&format!("{ACCOUNT} WHERE u.email ILIKE $1 OR COALESCE(u.full_name,'') ILIKE $1 OR u.id=$2 ORDER BY u.created_at DESC,u.id LIMIT 26 OFFSET $3")).bind(format!("%{search}%")).bind(&search).bind(page*25).fetch_all(&s.db).bounded_rows(50).await?;
    let has_next = rows.len() > 25;
    rows.truncate(25);
    private(UsersTemplate {
        nav: navigation(&s.db, &_p).await?,
        usage_notice: s.usage.notice(),
        search: FieldView::new("q", "Search name, email or account ID", search.clone(), 120),
        can_support,
        button: admin_button("Search", false),
        users: rows,
        q: search,
        page,
        has_next,
    })
}
pub async fn detail(
    State(s): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let (p, _, can_manage, can_staff) = directory(&s.db, &session).await?;
    let user: AccountRow = sqlx::query_as(&format!("{ACCOUNT} WHERE u.id=$1"))
        .bind(&id)
        .fetch_optional(&s.db)
        .bounded_one()
        .await?
        .ok_or(AppError::NotFound)?;
    let can_edit = can_manage && (user.roles.is_empty() || can_staff);
    let audit = if can_edit || can_staff {
        sqlx::query_as("SELECT a.action,COALESCE(NULLIF(actor.full_name,''),actor.email,a.actor_id,'operator') AS actor,a.reason,a.metadata::text AS metadata,to_char(a.created_at AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI') || ' UTC' AS date FROM admin_audit a LEFT JOIN users actor ON actor.id=a.actor_id WHERE a.target_id=$1 AND (($2 AND a.action IN ('profile','status','sessions')) OR ($3 AND a.action IN ('grant','revoke','--bootstrap-admin','--recover-admin','--grant-review-moderator','--revoke-review-moderator'))) ORDER BY a.id DESC LIMIT 50").bind(&id).bind(can_edit).bind(can_staff).fetch_all(&s.db).bounded_rows(50).await?
    } else {
        Vec::new()
    };
    let fields = vec![
        FieldView::new("first_name", "First name", user.first_name.clone(), 80),
        FieldView::new("last_name", "Last name", user.last_name.clone(), 80),
        FieldView::new("phone_number", "Phone", user.phone.clone(), 40),
        FieldView::new("address_line1", "Address", user.line1.clone(), 120),
        FieldView::new(
            "address_line2",
            "Apartment / suite",
            user.line2.clone(),
            120,
        ),
        FieldView::new("address_city", "City", user.city.clone(), 80),
        FieldView::new("address_state", "State", user.state.clone(), 2),
        FieldView::new(
            "address_postal_code",
            "Postal code",
            user.postal.clone(),
            20,
        ),
    ];
    let roles = ROLES
        .iter()
        .map(|name| RoleView {
            reason: reason_field(&format!("reason-{name}")),
            button: admin_button(
                if user.roles.split(", ").any(|r| r == *name) {
                    "Revoke"
                } else {
                    "Grant"
                },
                p.user_id == id,
            ),
            name,
            granted: user.roles.split(", ").any(|r| r == *name),
        })
        .collect();
    private(UserTemplate {
        nav: navigation(&s.db, &p).await?,
        profile_reason: reason_field("reason-profile"),
        status_reason: reason_field("reason-status"),
        sessions_reason: reason_field("reason-sessions"),
        profile_button: admin_button("Save profile", false),
        status_button: admin_button("Update access", false),
        sessions_button: admin_button("Revoke sessions", false),
        is_self: p.user_id == id,
        user,
        fields,
        roles,
        audit,
        csrf: crate::account_email::csrf(&session).await,
        can_staff,
        can_edit,
    })
}
#[derive(Deserialize)]
pub struct Action {
    csrf: String,
    action: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    version: i64,
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    last_name: String,
    #[serde(default)]
    phone_number: String,
    #[serde(default)]
    address_line1: String,
    #[serde(default)]
    address_line2: String,
    #[serde(default)]
    address_city: String,
    #[serde(default)]
    address_state: String,
    #[serde(default)]
    address_postal_code: String,
    #[serde(default)]
    marketing_opt_in: Option<String>,
}
async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: &str,
    target: &str,
    action: &str,
    reason: &str,
    data: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO admin_audit(actor_id,target_id,action,reason,metadata) VALUES($1,$2,$3,$4,$5)",
    )
    .bind(actor)
    .bind(target)
    .bind(action)
    .bind(reason)
    .bind(data)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
async fn invariant(tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
    if !sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM users u WHERE account_status='active' AND EXISTS(SELECT 1 FROM user_staff_roles WHERE user_id=u.id AND role='administrator') AND EXISTS(SELECT 1 FROM user_staff_roles WHERE user_id=u.id AND role='support'))").fetch_one(&mut **tx).bounded_one().await? {return Err(AppError::Validation("Keep at least one active administrator with user-management permission.".into()))}
    Ok(())
}
pub async fn action(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(f): Form<Action>,
) -> Result<Response, AppError> {
    if !crate::account_email::check_csrf(&session, &headers, &f.csrf).await {
        return Err(access::denied());
    }
    let p = access::principal(&s.db, &session).await?;
    mutate(&s.db, &p, &id, &f).await?;
    Ok(Redirect::to(&format!("/staff/users/{id}")).into_response())
}
pub(crate) async fn mutate(
    db: &DbPool,
    p: &Principal,
    id: &str,
    f: &Action,
) -> Result<(), AppError> {
    let reason = f.reason.trim();
    if reason.len() < 3 || reason.chars().count() > 500 {
        return Err(AppError::Validation(
            "Provide a reason (3–500 characters).".into(),
        ));
    }
    let mut tx = db.begin().await?;
    access::administration_lock(&mut tx).await?;
    let mut ids = vec![p.user_id.as_str(), id];
    ids.sort();
    ids.dedup();
    for account in ids {
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(account)
            .fetch_optional(&mut *tx)
            .bounded_one()
            .await?
            .ok_or(AppError::NotFound)?;
    }
    access::require_permission_tx(
        &mut tx,
        p,
        if f.action == "grant" || f.action == "revoke" {
            "staff.manage"
        } else {
            "users.manage"
        },
        AccountLock::Update,
    )
    .await?;
    let staff: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_staff_roles WHERE user_id=$1)")
            .bind(id)
            .fetch_one(&mut *tx)
            .bounded_one()
            .await?;
    if staff && f.action != "grant" && f.action != "revoke" {
        access::require_permission_tx(&mut tx, p, "staff.manage", AccountLock::Update).await?;
    }
    let data = match f.action.as_str() {
        "grant" | "revoke" => {
            if p.user_id == id {
                return Err(AppError::Validation(
                    "Another administrator must change your staff roles.".into(),
                ));
            }
            if !ROLES.contains(&f.role.as_str()) {
                return Err(AppError::Validation("Unknown role.".into()));
            }
            let changed = if f.action == "grant" {
                sqlx::query("INSERT INTO user_staff_roles(user_id,role) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(id).bind(&f.role).execute(&mut *tx).await?.rows_affected()
            } else {
                sqlx::query("DELETE FROM user_staff_roles WHERE user_id=$1 AND role=$2")
                    .bind(id)
                    .bind(&f.role)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected()
            };
            if changed == 0 {
                tx.commit().await?;
                return Ok(());
            }
            sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            invariant(&mut tx).await?;
            serde_json::json!({"role":f.role})
        }
        "status" => {
            if !["active", "disabled"].contains(&f.status.as_str()) {
                return Err(AppError::Validation("Unknown status.".into()));
            }
            let before: String = sqlx::query_scalar("SELECT account_status FROM users WHERE id=$1")
                .bind(id)
                .fetch_one(&mut *tx)
                .bounded_one()
                .await?;
            if before == f.status {
                tx.commit().await?;
                return Ok(());
            }
            sqlx::query("UPDATE users SET account_status=$2,status_reason=$3,status_changed_by=$4,status_changed_at=now(),auth_version=auth_version+1 WHERE id=$1").bind(id).bind(&f.status).bind(reason).bind(&p.user_id).execute(&mut *tx).await?;
            invariant(&mut tx).await?;
            serde_json::json!({"before":before,"after":f.status})
        }
        "sessions" => {
            sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            serde_json::json!({"sessions":"revoked","challenges":"invalidated"})
        }
        "profile" => {
            let error = |e: crate::auth::AuthError| AppError::Validation(e.to_string());
            let first = crate::auth::optional_profile_text(&f.first_name, 80, "First name")
                .map_err(error)?;
            let last =
                crate::auth::optional_profile_text(&f.last_name, 80, "Last name").map_err(error)?;
            let phone =
                crate::auth::optional_profile_text(&f.phone_number, 40, "Phone").map_err(error)?;
            let line1 = crate::auth::optional_profile_text(&f.address_line1, 120, "Address")
                .map_err(error)?;
            let line2 =
                crate::auth::optional_profile_text(&f.address_line2, 120, "Apartment / suite")
                    .map_err(error)?;
            let city =
                crate::auth::optional_profile_text(&f.address_city, 80, "City").map_err(error)?;
            let state = crate::auth::optional_state(&f.address_state).map_err(error)?;
            let postal =
                crate::auth::optional_profile_text(&f.address_postal_code, 20, "Postal code")
                    .map_err(error)?;
            let changed=sqlx::query("UPDATE users SET first_name=$2,last_name=$3,full_name=NULLIF(trim(concat_ws(' ',$2,$3)),''),phone_number=$4,address_line1=$5,address_line2=$6,address_city=$7,address_state=$8,address_postal_code=$9,marketing_opt_in=$10,profile_version=profile_version+1,updated_at=now() WHERE id=$1 AND profile_version=$11").bind(id).bind(first).bind(last).bind(phone).bind(line1).bind(line2).bind(city).bind(state).bind(postal).bind(f.marketing_opt_in.is_some()).bind(f.version).execute(&mut *tx).await?.rows_affected();
            if changed == 0 {
                return Err(AppError::Http(
                    axum::http::StatusCode::CONFLICT,
                    "Profile changed. Reload before saving.".into(),
                ));
            }
            serde_json::json!({"fields":["name","phone","address","marketing_opt_in"],"profile_version":f.version+1})
        }
        _ => return Err(AppError::Validation("Unknown action.".into())),
    };
    audit(&mut tx, &p.user_id, id, &f.action, reason, data).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn operator(
    db: &DbPool,
    email: &str,
    mode: &str,
    identity: &str,
    reason: &str,
) -> Result<(), AppError> {
    if identity.trim().is_empty() || reason.trim().len() < 3 {
        return Err(AppError::Validation(
            "Operator identity and reason required.".into(),
        ));
    }
    let mut tx = db.begin().await?;
    access::administration_lock(&mut tx).await?;
    let id: String = sqlx::query_scalar(
        "SELECT id FROM users WHERE lower(email)=lower($1) AND account_status='active' FOR UPDATE",
    )
    .bind(email)
    .fetch_optional(&mut *tx)
    .bounded_one()
    .await?
    .ok_or(AppError::NotFound)?;
    let roles = if mode == "--bootstrap-admin" || mode == "--recover-admin" {
        vec!["administrator", "support"]
    } else {
        vec!["moderator"]
    };
    let mut count = 0;
    for role in roles {
        count += if mode == "--revoke-review-moderator" {
            sqlx::query("DELETE FROM user_staff_roles WHERE user_id=$1 AND role=$2")
                .bind(&id)
                .bind(role)
                .execute(&mut *tx)
                .await?
                .rows_affected()
        } else {
            sqlx::query(
                "INSERT INTO user_staff_roles(user_id,role) VALUES($1,$2) ON CONFLICT DO NOTHING",
            )
            .bind(&id)
            .bind(role)
            .execute(&mut *tx)
            .await?
            .rows_affected()
        };
    }
    if count > 0 {
        sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id=$1")
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        audit(
            &mut tx,
            &format!("operator:{identity}"),
            &id,
            mode,
            reason,
            serde_json::json!({"assignments_changed":count}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn admin_button(label: &str, disabled: bool) -> crate::ui::ButtonView {
    let mut b = crate::ui::ButtonView::form_submit(label);
    b.class_name = "ui-admin-button".into();
    b.disabled = disabled;
    b
}
async fn directory(
    db: &DbPool,
    session: &Session,
) -> Result<(Principal, bool, bool, bool), AppError> {
    let p = access::principal(db, session).await?;
    let mut tx = db.begin().await?;
    access::require_active_tx(&mut tx, &p, AccountLock::Share).await?;
    let roles: Vec<String> =
        sqlx::query_scalar("SELECT role FROM user_staff_roles WHERE user_id=$1 LIMIT 5")
            .bind(&p.user_id)
            .fetch_all(&mut *tx)
            .bounded_rows(5)
            .await?;
    let allows = |permission: &str| {
        access::permission_roles(permission)
            .iter()
            .any(|role| roles.iter().any(|r| r == role))
    };
    let view = allows("users.view");
    let manage = allows("users.manage");
    let staff = allows("staff.manage");
    if !view && !staff {
        return Err(access::denied());
    }
    tx.commit().await?;
    Ok((p, view, manage, staff))
}

fn reason_field(id: &str) -> FieldView {
    let mut field = FieldView::new("reason", "Reason", String::new(), 500);
    field.id = id.into();
    field
}
async fn navigation(db: &DbPool, p: &Principal) -> Result<Vec<crate::ui::LinkView>, AppError> {
    let mut links = vec![("Your account", "/account"), ("Users", "/staff/users")];
    if access::has_permission(db, &p.user_id, "reviews.moderate").await? {
        links.push(("Review moderation", "/staff/reviews"));
    }
    links.push(("Storefront", "/"));
    Ok(links
        .into_iter()
        .map(|(label, url)| {
            crate::ui::LinkView::tracked(
                label,
                url,
                "ui-admin-link",
                "",
                "staff",
                "navigation",
                url,
            )
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staff_users_page_shows_the_spending_notice() {
        let page = |usage_notice| {
            UsersTemplate {
                nav: Vec::new(),
                usage_notice,
                search: FieldView::new("q", "Search", String::new(), 120),
                can_support: false,
                button: admin_button("Search", false),
                users: Vec::new(),
                q: String::new(),
                page: 0,
                has_next: false,
            }
            .render()
            .unwrap()
        };
        let notice = crate::usage::Notice {
            title: "Neon spending alert".into(),
            message: "$0.51 today".into(),
            level: "error",
        };
        let body = page(Some(notice));
        assert!(body.contains("Neon spending alert") && body.contains("role=\"alert\""));
        assert!(!page(None).contains("ui-service-notice"));
    }
}
