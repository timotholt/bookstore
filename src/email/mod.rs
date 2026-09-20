//! Transactional account mail. Secrets and payloads intentionally never implement Debug.
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use askama::Template;
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::{PgConnection, PgPool, Row};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("email configuration invalid: {0}")]
    Config(&'static str),
    #[error("email unavailable")]
    Disabled,
    #[error("email storage failed")]
    Database(#[from] sqlx::Error),
    #[error("email encryption failed")]
    Encryption,
    #[error("email rendering failed")]
    Rendering,
    #[error("invalid webhook")]
    Webhook,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EmailKind {
    Verification,
    PasswordReset,
    PasswordChanged,
    EmailChange,
    EmailChangeRequested,
    EmailChanged,
}
impl EmailKind {
    fn name(self) -> &'static str {
        match self {
            Self::Verification => "verification",
            Self::PasswordReset => "password_reset",
            Self::PasswordChanged => "password_changed",
            Self::EmailChange => "email_change",
            Self::EmailChangeRequested => "email_change_requested",
            Self::EmailChanged => "email_changed",
        }
    }
    fn copy(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    ) {
        match self {
 Self::Verification=>("Verify your email","Welcome to Chantel’s Corner, your neighborhood bookstore. Confirm your email address to finish setting up your account.","Verify email","This link expires in 24 hours.","Didn’t create an account? You can ignore this email."),
 Self::PasswordReset=>("Reset your password","We received a request to reset your Chantel’s Corner bookstore password.","Reset password","This link expires in 30 minutes.","Didn’t request this? Ignore this email. Your password stays unchanged."),
 Self::EmailChange=>("Confirm your new email","Confirm this address to use it for your Chantel’s Corner bookstore account.","Confirm email","This link expires in 24 hours.","Didn’t request this change? Ignore this email."),
 Self::PasswordChanged=>("Your password changed","The password for your Chantel’s Corner bookstore account was changed.","","","If this wasn’t you, visit the bookstore and use Forgot password immediately to secure your account."),
 Self::EmailChangeRequested=>("Email change requested","Someone requested a new email address for your Chantel’s Corner bookstore account.","","","If this wasn’t you, visit the bookstore and reset your password to cancel pending account changes."),
 Self::EmailChanged=>("Your email address changed","The email address for your Chantel’s Corner bookstore account was changed.","","","If this wasn’t you, contact the bookstore through its website immediately."),
 }
    }
}
#[derive(Template)]
#[template(path = "email/account.html")]
struct HtmlMail<'a> {
    heading: &'a str,
    message: &'a str,
    button: &'a str,
    expiry: &'a str,
    safety: &'a str,
    has_action: bool,
    action_url: &'a str,
    base_url: &'a str,
    image_url: &'a str,
}
#[derive(Template)]
#[template(path = "email/account.txt", escape = "none")]
struct TextMail<'a> {
    heading: &'a str,
    message: &'a str,
    button: &'a str,
    expiry: &'a str,
    safety: &'a str,
    has_action: bool,
    action_url: &'a str,
    base_url: &'a str,
}
pub fn render_preview(
    kind: EmailKind,
    action_url: &str,
    image_url: &str,
) -> Result<(String, String), EmailError> {
    render(
        kind,
        action_url,
        "https://www.chantelscorner.com",
        image_url,
    )
}
fn render(
    kind: EmailKind,
    action_url: &str,
    base_url: &str,
    image_url: &str,
) -> Result<(String, String), EmailError> {
    let (heading, message, button, expiry, safety) = kind.copy();
    let has_action = !button.is_empty();
    Ok((
        HtmlMail {
            heading,
            message,
            button,
            expiry,
            safety,
            has_action,
            action_url,
            base_url,
            image_url,
        }
        .render()
        .map_err(|_| EmailError::Rendering)?,
        TextMail {
            heading,
            message,
            button,
            expiry,
            safety,
            has_action,
            action_url,
            base_url,
        }
        .render()
        .map_err(|_| EmailError::Rendering)?,
    ))
}
#[derive(Clone, Copy, PartialEq)]
enum Provider {
    Disabled,
    Capture,
    Resend,
}
#[derive(Serialize, Deserialize)]
struct Message {
    from: String,
    to: Vec<String>,
    subject: String,
    html: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<InlineImage>,
}
#[derive(Serialize, Deserialize)]
struct InlineImage {
    filename: String,
    content: String,
    content_id: String,
    content_type: String,
}
pub struct EmailService {
    provider: Provider,
    key: Option<[u8; 32]>,
    key_id: String,
    api_key: String,
    webhook_key: Vec<u8>,
    base_url: String,
    from: String,
    reply_to: Option<String>,
    quota: i32,
    client: reqwest::Client,
    capture_dir: Option<std::path::PathBuf>,
}
fn mailbox(value: &str) -> bool {
    !value.contains(['\r', '\n']) && validator::validate_email(value)
}
impl EmailService {
    #[cfg(test)]
    pub fn test_capture() -> Self {
        Self::from_lookup(false, |k| match k {
            "EMAIL_PROVIDER" => Some("capture".into()),
            "EMAIL_OUTBOX_KEY" => Some(STANDARD.encode([7u8; 32])),
            "EMAIL_OUTBOX_KEY_ID" => Some("test-v1".into()),
            "EMAIL_FROM" => Some("Chantel’s Corner <accounts@example.com>".into()),
            "EMAIL_CAPTURE_DIR" => Some("/tmp/chantels-email-tests".into()),
            _ => None,
        })
        .unwrap()
    }
    pub fn from_env(production: bool) -> Result<Self, EmailError> {
        Self::from_lookup(production, |k| std::env::var(k).ok())
    }
    fn from_lookup(
        production: bool,
        get: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, EmailError> {
        let provider = match get("EMAIL_PROVIDER").as_deref().unwrap_or("disabled") {
            "disabled" => Provider::Disabled,
            "capture" if !production => Provider::Capture,
            "resend" => Provider::Resend,
            _ => return Err(EmailError::Config("EMAIL_PROVIDER")),
        };
        let base_url =
            get("PUBLIC_BASE_URL").unwrap_or_else(|| "https://www.chantelscorner.com".into());
        let url =
            reqwest::Url::parse(&base_url).map_err(|_| EmailError::Config("PUBLIC_BASE_URL"))?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || ((production || provider == Provider::Resend)
                && base_url != "https://www.chantelscorner.com")
            || (!production
                && url.scheme() != "https"
                && !(url.scheme() == "http"
                    && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))))
        {
            return Err(EmailError::Config("PUBLIC_BASE_URL"));
        }
        let enabled = provider != Provider::Disabled;
        let key = if enabled {
            Some(
                STANDARD
                    .decode(get("EMAIL_OUTBOX_KEY").ok_or(EmailError::Config("EMAIL_OUTBOX_KEY"))?)
                    .map_err(|_| EmailError::Config("EMAIL_OUTBOX_KEY"))?
                    .try_into()
                    .map_err(|_| EmailError::Config("EMAIL_OUTBOX_KEY must decode to 32 bytes"))?,
            )
        } else {
            None
        };
        let key_id = get("EMAIL_OUTBOX_KEY_ID").unwrap_or_default();
        if enabled && (key_id.is_empty() || key_id.len() > 64) {
            return Err(EmailError::Config("EMAIL_OUTBOX_KEY_ID"));
        }
        let from = get("EMAIL_FROM").unwrap_or_default();
        let address = from
            .split('<')
            .next_back()
            .unwrap_or("")
            .trim_end_matches('>');
        if enabled && (from.contains(['\r', '\n']) || !mailbox(address)) {
            return Err(EmailError::Config("EMAIL_FROM"));
        }
        let reply_to = get("EMAIL_REPLY_TO").filter(|x| !x.is_empty());
        if reply_to.as_deref().is_some_and(|x| !mailbox(x)) {
            return Err(EmailError::Config("EMAIL_REPLY_TO"));
        }
        let api_key = get("RESEND_API_KEY").unwrap_or_default();
        let webhook_key = match get("RESEND_WEBHOOK_SECRET") {
            Some(s) => STANDARD
                .decode(s.strip_prefix("whsec_").unwrap_or(&s))
                .map_err(|_| EmailError::Config("RESEND_WEBHOOK_SECRET"))?,
            None => vec![],
        };
        if provider == Provider::Resend && (!api_key.starts_with("re_") || webhook_key.len() < 16) {
            return Err(EmailError::Config("Resend credentials"));
        }
        let quota = get("EMAIL_SENDS_PER_DAY")
            .unwrap_or_else(|| "90".into())
            .parse::<i32>()
            .map_err(|_| EmailError::Config("EMAIL_SENDS_PER_DAY"))?;
        if !(10..=100000).contains(&quota) {
            return Err(EmailError::Config("EMAIL_SENDS_PER_DAY"));
        }
        let capture_dir = if provider == Provider::Capture {
            Some(
                get("EMAIL_CAPTURE_DIR")
                    .ok_or(EmailError::Config("EMAIL_CAPTURE_DIR required"))?
                    .into(),
            )
        } else {
            None
        };
        Ok(Self {
            provider,
            key,
            key_id,
            api_key,
            webhook_key,
            base_url: base_url.trim_end_matches('/').into(),
            from,
            reply_to,
            quota,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| EmailError::Config("HTTP client"))?,
            capture_dir,
        })
    }
    pub fn available(&self) -> bool {
        self.provider != Provider::Disabled
    }
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
    fn encrypt(&self, id: Uuid, plain: &[u8]) -> Result<Vec<u8>, EmailError> {
        let cipher = Aes256Gcm::new_from_slice(self.key.as_ref().ok_or(EmailError::Disabled)?)
            .map_err(|_| EmailError::Encryption)?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let mut result = nonce.to_vec();
        result.extend(
            cipher
                .encrypt(
                    &nonce,
                    Payload {
                        msg: plain,
                        aad: id.as_bytes(),
                    },
                )
                .map_err(|_| EmailError::Encryption)?,
        );
        Ok(result)
    }
    fn decrypt(&self, id: Uuid, data: &[u8]) -> Result<Vec<u8>, EmailError> {
        if data.len() < 28 {
            return Err(EmailError::Encryption);
        }
        Aes256Gcm::new_from_slice(self.key.as_ref().ok_or(EmailError::Disabled)?)
            .map_err(|_| EmailError::Encryption)?
            .decrypt(
                Nonce::from_slice(&data[..12]),
                Payload {
                    msg: &data[12..],
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| EmailError::Encryption)
    }
    pub async fn enqueue(
        &self,
        db: &mut PgConnection,
        kind: EmailKind,
        recipient: &str,
        action_url: &str,
        token_id: Option<Uuid>,
        expires_at: DateTime<Utc>,
    ) -> Result<Uuid, EmailError> {
        if !self.available() {
            return Err(EmailError::Disabled);
        }
        if !mailbox(recipient) {
            return Err(EmailError::Config("recipient"));
        }
        if !kind.copy().2.is_empty()
            && (!action_url.starts_with(&format!("{}/", self.base_url))
                || action_url.contains(['\r', '\n']))
        {
            return Err(EmailError::Config("action URL"));
        }
        let (html, text) = render(
            kind,
            action_url,
            &self.base_url,
            "cid:chantels-corner-header",
        )?;
        let message = Message {
            from: self.from.clone(),
            to: vec![recipient.into()],
            subject: format!("{} — Chantel’s Corner bookstore", kind.copy().0),
            html,
            text,
            reply_to: self.reply_to.clone(),
            attachments: vec![InlineImage {
                filename: "chantels-corner-header.jpg".into(),
                content: STANDARD.encode(include_bytes!(
                    "../../assets/email/chantels-corner-header.jpg"
                )),
                content_id: "chantels-corner-header".into(),
                content_type: "image/jpeg".into(),
            }],
        };
        let id = Uuid::new_v4();
        let payload = self.encrypt(
            id,
            &serde_json::to_vec(&message).map_err(|_| EmailError::Rendering)?,
        )?;
        sqlx::query("INSERT INTO email_outbox(id,kind,recipient,token_id,payload,key_id,expires_at) VALUES($1,$2,$3,$4,$5,$6,LEAST($7,now()+interval '1 hour'))").bind(id).bind(kind.name()).bind(recipient).bind(token_id).bind(payload).bind(&self.key_id).bind(expires_at).execute(db).await?;
        Ok(id)
    }
    pub fn spawn_worker(self: Arc<Self>, pool: PgPool) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if !self.available() {
                return;
            }
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                if let Err(e) = self.work_once(&pool).await {
                    tracing::error!(error=%e,"email worker failed");
                }
            }
        })
    }
    pub async fn work_once(&self, pool: &PgPool) -> Result<bool, EmailError> {
        if !self.available() {
            return Ok(false);
        }
        sqlx::query("DELETE FROM email_outbox WHERE id IN (SELECT id FROM email_outbox WHERE created_at<now()-interval '30 days' AND status NOT IN ('pending','sending') LIMIT 100)").execute(pool).await?;
        sqlx::query("DELETE FROM email_delivery_events WHERE event_id IN (SELECT event_id FROM email_delivery_events WHERE created_at<now()-interval '30 days' LIMIT 100)").execute(pool).await?;
        sqlx::query("DELETE FROM email_send_budget WHERE day<CURRENT_DATE-30")
            .execute(pool)
            .await?;
        sqlx::query("UPDATE email_outbox SET status='expired',payload=NULL,lease_owner=NULL,lease_until=NULL WHERE status IN ('pending','sending') AND expires_at<=now()").execute(pool).await?;
        let owner = Uuid::new_v4();
        let row=sqlx::query("WITH candidate AS (SELECT id FROM email_outbox WHERE (status='pending' OR (status='sending' AND lease_until<now())) AND next_attempt_at<=now() AND expires_at>now() ORDER BY CASE WHEN kind='verification' THEN 1 ELSE 0 END,created_at FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE email_outbox o SET status='sending',lease_owner=$1,lease_until=now()+interval '60 seconds' FROM candidate c WHERE o.id=c.id RETURNING o.*").bind(owner).fetch_optional(pool).await?;
        let Some(row) = row else { return Ok(false) };
        let id: Uuid = row.get("id");
        let recipient: String = row.get("recipient");
        let token: Option<Uuid> = row.get("token_id");
        let suppressed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM email_suppressions WHERE recipient=$1)",
        )
        .bind(&recipient)
        .fetch_one(pool)
        .await?;
        let current = if let Some(token) = token {
            sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM account_tokens t JOIN users u ON u.id=t.user_id WHERE t.id=$1 AND t.consumed_at IS NULL AND t.revoked_at IS NULL AND t.expires_at>now() AND t.auth_version=u.auth_version AND t.target_email=$2 AND (t.purpose='change_email' OR u.email=t.target_email))").bind(token).bind(&recipient).fetch_one(pool).await?
        } else {
            true
        };
        if suppressed || !current {
            self.finish(
                pool,
                id,
                owner,
                "cancelled",
                None,
                Some("recipient_or_token_ineligible"),
            )
            .await?;
            return Ok(true);
        }
        let key_id: String = row.get("key_id");
        let payload: Option<Vec<u8>> = row.get("payload");
        let decoded = payload
            .as_deref()
            .filter(|_| key_id == self.key_id)
            .and_then(|p| self.decrypt(id, p).ok())
            .and_then(|p| serde_json::from_slice::<Message>(&p).ok());
        let Some(message) = decoded else {
            self.finish(pool, id, owner, "failed", None, Some("payload_unavailable"))
                .await?;
            return Ok(true);
        };
        let verification: bool = row.get::<String, _>("kind") == "verification";
        let budget=sqlx::query("INSERT INTO email_send_budget(day,total,verification,last_send_at) VALUES(CURRENT_DATE,1,$1,now()) ON CONFLICT(day) DO UPDATE SET total=email_send_budget.total+1,verification=email_send_budget.verification+$1,last_send_at=now() WHERE email_send_budget.total<$2 AND ($1=0 OR email_send_budget.verification<$3) AND email_send_budget.last_send_at<now()-interval '1 second' RETURNING total").bind(if verification{1}else{0}).bind(self.quota).bind(self.quota*4/5).fetch_optional(pool).await?;
        if budget.is_none() {
            self.retry(pool, id, owner, "quota_or_rate_limit", 60, false)
                .await?;
            return Ok(true);
        }
        sqlx::query("UPDATE email_outbox SET attempts=attempts+1 WHERE id=$1 AND lease_owner=$2")
            .bind(id)
            .bind(owner)
            .execute(pool)
            .await?;
        let outcome = self.send(id, &message).await;
        match outcome {
            Ok(provider_id) => {
                self.finish(pool, id, owner, "accepted", Some(&provider_id), None)
                    .await?
            }
            Err((category, retry_after)) => {
                let attempts: i32 = row.get("attempts");
                if retry_after.is_some() && attempts < 7 {
                    let delay = retry_after
                        .unwrap()
                        .max((2_i64.pow((attempts + 1) as u32) * 5).min(300))
                        + (id.as_bytes()[0] as i64 % 7);
                    self.retry(pool, id, owner, category, delay, true).await?
                } else {
                    self.finish(pool, id, owner, "failed", None, Some(category))
                        .await?
                }
            }
        }
        Ok(true)
    }
    async fn finish(
        &self,
        pool: &PgPool,
        id: Uuid,
        owner: Uuid,
        status: &str,
        provider: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), EmailError> {
        sqlx::query("UPDATE email_outbox SET status=$3,provider_id=$4,error_category=$5,payload=NULL,lease_owner=NULL,lease_until=NULL,accepted_at=CASE WHEN $3='accepted' THEN now() ELSE NULL END WHERE id=$1 AND lease_owner=$2 AND status='sending'").bind(id).bind(owner).bind(status).bind(provider).bind(error).execute(pool).await?;
        Ok(())
    }
    async fn retry(
        &self,
        pool: &PgPool,
        id: Uuid,
        owner: Uuid,
        error: &str,
        delay: i64,
        _attempted: bool,
    ) -> Result<(), EmailError> {
        sqlx::query("UPDATE email_outbox SET status='pending',error_category=$3,next_attempt_at=now()+($4*interval '1 second'),lease_owner=NULL,lease_until=NULL WHERE id=$1 AND lease_owner=$2 AND status='sending'").bind(id).bind(owner).bind(error).bind(delay as f64).execute(pool).await?;
        Ok(())
    }
    async fn send(
        &self,
        id: Uuid,
        message: &Message,
    ) -> Result<String, (&'static str, Option<i64>)> {
        if self.provider == Provider::Capture {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let dir = self.capture_dir.as_ref().ok_or(("capture_config", None))?;
            std::fs::create_dir_all(dir).map_err(|_| ("capture_io", None))?;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| ("capture_io", None))?;
            let path = dir.join(format!("{id}.json"));
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true).mode(0o600);
            match opts.open(path) {
                Ok(mut file) => {
                    use std::io::Write;
                    file.write_all(
                        &serde_json::to_vec(message).map_err(|_| ("capture_encoding", None))?,
                    )
                    .map_err(|_| ("capture_io", None))?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(("capture_io", None)),
            };
            return Ok(format!("capture-{id}"));
        }
        self.send_http(id, message, "https://api.resend.com/emails")
            .await
    }
    async fn send_http(
        &self,
        id: Uuid,
        message: &Message,
        endpoint: &str,
    ) -> Result<String, (&'static str, Option<i64>)> {
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(&self.api_key)
            .header("Idempotency-Key", id.to_string())
            .json(message)
            .send()
            .await
            .map_err(|_| ("transport", Some(0)))?;
        let status = response.status();
        if status.is_success() {
            let value: serde_json::Value = response
                .json()
                .await
                .map_err(|_| ("provider_response", Some(0)))?;
            return value
                .get("id")
                .and_then(|x| x.as_str())
                .filter(|x| !x.is_empty() && x.len() < 256)
                .map(str::to_owned)
                .ok_or(("provider_response", Some(0)));
        }
        if status.as_u16() == 429 || status.is_server_error() {
            let delay = response
                .headers()
                .get("retry-after")
                .and_then(|x| x.to_str().ok())
                .and_then(|x| x.parse::<i64>().ok())
                .unwrap_or(0)
                .clamp(0, 3600);
            Err(("provider_transient", Some(delay)))
        } else {
            Err(("provider_rejected", None))
        }
    }
    pub async fn verify_and_record(
        &self,
        pool: &PgPool,
        headers: &axum::http::HeaderMap,
        body: &[u8],
    ) -> Result<(), EmailError> {
        if self.provider != Provider::Resend || body.len() > 65536 {
            return Err(EmailError::Webhook);
        }
        let get = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .ok_or(EmailError::Webhook)
        };
        let id = get("svix-id")?;
        let timestamp = get("svix-timestamp")?;
        let signatures = get("svix-signature")?;
        verify_signature(
            &self.webhook_key,
            id,
            timestamp,
            signatures,
            body,
            Utc::now().timestamp(),
        )?;
        let event: serde_json::Value =
            serde_json::from_slice(body).map_err(|_| EmailError::Webhook)?;
        let kind = event
            .get("type")
            .and_then(|x| x.as_str())
            .ok_or(EmailError::Webhook)?;
        let data = event.get("data").ok_or(EmailError::Webhook)?;
        let provider_id = data
            .get("email_id")
            .and_then(|x| x.as_str())
            .ok_or(EmailError::Webhook)?;
        if id.len() > 256 || kind.len() > 100 || provider_id.len() > 256 {
            return Err(EmailError::Webhook);
        }
        let mut tx = pool.begin().await?;
        let inserted=sqlx::query("INSERT INTO email_delivery_events(event_id,provider_id,kind) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(id).bind(provider_id).bind(kind).execute(&mut *tx).await?.rows_affected()>0;
        let hard_bounce = kind == "email.bounced"
            && data
                .pointer("/bounce/type")
                .and_then(|x| x.as_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("Permanent"));
        if inserted && (kind == "email.complained" || kind == "email.suppressed" || hard_bounce) {
            if let Some(recipients) = data.get("to").and_then(|x| x.as_array()) {
                for recipient in recipients {
                    if let Some(r) = recipient.as_str().filter(|r| mailbox(r)) {
                        sqlx::query("INSERT INTO email_suppressions(recipient,reason) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(r.to_lowercase()).bind(kind).execute(&mut *tx).await?;
                    }
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
fn verify_signature(
    key: &[u8],
    id: &str,
    timestamp: &str,
    signatures: &str,
    body: &[u8],
    now: i64,
) -> Result<(), EmailError> {
    let time = timestamp.parse::<i64>().map_err(|_| EmailError::Webhook)?;
    if now.abs_diff(time) > 300 || key.is_empty() {
        return Err(EmailError::Webhook);
    }
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).map_err(|_| EmailError::Webhook)?;
    mac.update(id.as_bytes());
    mac.update(b".");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    if signatures
        .split_whitespace()
        .filter_map(|s| s.strip_prefix("v1,"))
        .filter_map(|s| STANDARD.decode(s).ok())
        .any(|sig| mac.clone().verify_slice(&sig).is_ok())
    {
        Ok(())
    } else {
        Err(EmailError::Webhook)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn capture() -> EmailService {
        EmailService::from_lookup(false, |k| match k {
            "EMAIL_PROVIDER" => Some("capture".into()),
            "EMAIL_OUTBOX_KEY" => Some(STANDARD.encode([7u8; 32])),
            "EMAIL_OUTBOX_KEY_ID" => Some("test-v1".into()),
            "EMAIL_FROM" => Some("Chantel’s Corner <accounts@example.com>".into()),
            "EMAIL_CAPTURE_DIR" => Some("/tmp/chantels-email-tests".into()),
            _ => None,
        })
        .unwrap()
    }
    #[test]
    fn configuration_fails_closed() {
        assert!(matches!(
            EmailService::from_lookup(false, |k| match k {
                "EMAIL_PROVIDER" => Some("resend".into()),
                "PUBLIC_BASE_URL" => Some("http://127.0.0.1:8088".into()),
                _ => None,
            }),
            Err(EmailError::Config("PUBLIC_BASE_URL"))
        ));
        assert!(!EmailService::from_lookup(true, |_| None)
            .unwrap()
            .available());
        assert!(EmailService::from_lookup(true, |k| (k == "EMAIL_PROVIDER")
            .then(|| "capture".into()))
        .is_err());
        assert!(EmailService::from_lookup(true, |k| (k == "PUBLIC_BASE_URL")
            .then(|| "https://evil.example".into()))
        .is_err());
        assert!(EmailService::from_lookup(false, |k| (k == "EMAIL_PROVIDER")
            .then(|| "resend".into()))
        .is_err());
    }
    #[test]
    fn encryption_is_authenticated_and_bound_to_job() {
        let svc = capture();
        let id = Uuid::new_v4();
        let value = svc.encrypt(id, b"reset token secret").unwrap();
        assert!(!value.windows(5).any(|x| x == b"token"));
        assert_eq!(svc.decrypt(id, &value).unwrap(), b"reset token secret");
        assert!(svc.decrypt(Uuid::new_v4(), &value).is_err());
        let mut changed = value.clone();
        changed[15] ^= 1;
        assert!(svc.decrypt(id, &changed).is_err());
        assert_ne!(value, svc.encrypt(id, b"reset token secret").unwrap());
    }
    #[test]
    fn templates_escape_and_keep_accessible_text() {
        let (html, text) = render_preview(
            EmailKind::Verification,
            "https://www.chantelscorner.com/verify?token=<script>\"",
            "https://www.chantelscorner.com/assets/email/chantels-corner-header.jpg",
        )
        .unwrap();
        assert!(!html.contains("<script>"));
        assert!(html.contains("Verify email"));
        assert!(text.contains("24 hours"));
        assert!(html.contains("Chantel’s Corner"));
        assert!(text.contains("bookstore"));
        let (notice, _) = render_preview(EmailKind::PasswordChanged, "", "").unwrap();
        assert!(!notice.contains("Button not working"));
    }
    #[test]
    fn signatures_reject_tampering_staleness_and_accept_rotation() {
        let key = b"a real test signing key";
        let body = b"{\"type\":\"email.delivered\"}";
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap();
        mac.update(b"evt_1.1000.");
        mac.update(body);
        let signature = format!("v1,bad v1,{}", STANDARD.encode(mac.finalize().into_bytes()));
        assert!(verify_signature(key, "evt_1", "1000", &signature, body, 1001).is_ok());
        assert!(verify_signature(key, "evt_1", "1000", &signature, b"{}", 1001).is_err());
        assert!(verify_signature(key, "evt_1", "1000", &signature, body, 1400).is_err());
        assert!(verify_signature(key, "evt_2", "1000", &signature, body, 1001).is_err());
    }
    #[tokio::test]
    async fn fake_provider_classifies_errors_and_preserves_idempotency() {
        use axum::{
            http::{HeaderMap, StatusCode},
            response::IntoResponse,
            routing::post,
            Router,
        };
        let requests = Arc::new(tokio::sync::Mutex::new(Vec::<(String, String)>::new()));
        let captured = requests.clone();
        let app = Router::new().route(
            "/emails",
            post(move |headers: HeaderMap, body: String| {
                let captured = captured.clone();
                async move {
                    let mut calls = captured.lock().await;
                    calls.push((
                        headers
                            .get("idempotency-key")
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .into(),
                        body,
                    ));
                    match calls.len() {
                        1 => (
                            StatusCode::TOO_MANY_REQUESTS,
                            [("retry-after", "120")],
                            "{}",
                        )
                            .into_response(),
                        2 => (StatusCode::INTERNAL_SERVER_ERROR, "{}").into_response(),
                        3 => {
                            (StatusCode::UNAUTHORIZED, "private provider response").into_response()
                        }
                        _ => (
                            StatusCode::OK,
                            axum::Json(serde_json::json!({"id":"provider-id"})),
                        )
                            .into_response(),
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/emails", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let svc = capture();
        let id = Uuid::new_v4();
        let msg = Message {
            from: "accounts@example.com".into(),
            to: vec!["reader@example.com".into()],
            subject: "Reset".into(),
            html: "immutable".into(),
            text: "immutable".into(),
            reply_to: None,
            attachments: vec![],
        };
        assert_eq!(
            svc.send_http(id, &msg, &endpoint).await.unwrap_err(),
            ("provider_transient", Some(120))
        );
        assert_eq!(
            svc.send_http(id, &msg, &endpoint).await.unwrap_err(),
            ("provider_transient", Some(0))
        );
        assert_eq!(
            svc.send_http(id, &msg, &endpoint).await.unwrap_err(),
            ("provider_rejected", None)
        );
        assert_eq!(
            svc.send_http(id, &msg, &endpoint).await.unwrap(),
            "provider-id"
        );
        let calls = requests.lock().await;
        assert_eq!(calls.len(), 4);
        assert!(calls.iter().all(|x| *x == calls[0]));
        server.abort();
    }
    #[tokio::test]
    #[ignore = "requires isolated local Postgres EMAIL_TEST_DATABASE_URL"]
    async fn outbox_atomic_encryption_lease_and_suppression() {
        let url = std::env::var("EMAIL_TEST_DATABASE_URL").expect("local test URL");
        assert!(url.contains("127.0.0.1"));
        let schema = format!("email_test_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let schema2 = schema.clone();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .after_connect(move |conn, _| {
                let query = format!("SET search_path TO {schema2}");
                Box::pin(async move {
                    sqlx::query(&query).execute(conn).await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations_postgres/20260920001000_email_delivery.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let mut service = capture();
        let dir = std::env::temp_dir().join(&schema);
        service.capture_dir = Some(dir.clone());
        let mut tx = pool.begin().await.unwrap();
        service
            .enqueue(
                &mut tx,
                EmailKind::PasswordReset,
                "reader@example.com",
                "https://www.chantelscorner.com/reset?token=secret",
                None,
                Utc::now() + chrono::Duration::minutes(30),
            )
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM email_outbox")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        let mut tx = pool.begin().await.unwrap();
        let id = service
            .enqueue(
                &mut tx,
                EmailKind::PasswordReset,
                "reader@example.com",
                "https://www.chantelscorner.com/reset?token=secret",
                None,
                Utc::now() + chrono::Duration::minutes(30),
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let payload: Vec<u8> = sqlx::query_scalar("SELECT payload FROM email_outbox WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!payload.windows(6).any(|x| x == b"secret"));
        sqlx::query("UPDATE email_outbox SET status='sending',lease_owner=$1,lease_until=now()-interval '1 second' WHERE id=$2").bind(Uuid::new_v4()).bind(id).execute(&pool).await.unwrap();
        let (a, b) = tokio::join!(service.work_once(&pool), service.work_once(&pool));
        assert!(a.unwrap() || b.unwrap());
        let row = sqlx::query("SELECT status,payload,attempts FROM email_outbox WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("status"), "accepted");
        assert!(row.get::<Option<Vec<u8>>, _>("payload").is_none());
        assert_eq!(row.get::<i32, _>("attempts"), 1);
        assert!(dir.join(format!("{id}.json")).exists());
        sqlx::query("INSERT INTO email_suppressions(recipient,reason) VALUES('reader@example.com','complaint')").execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let blocked = service
            .enqueue(
                &mut tx,
                EmailKind::PasswordChanged,
                "reader@example.com",
                "",
                None,
                Utc::now() + chrono::Duration::minutes(30),
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        service.work_once(&pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT status FROM email_outbox WHERE id=$1")
                .bind(blocked)
                .fetch_one(&pool)
                .await
                .unwrap(),
            "cancelled"
        );
        // Exhausted shared budget defers; expiry clears the sensitive payload.
        sqlx::query("UPDATE email_send_budget SET total=100000")
            .execute(&pool)
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let quota_job = service
            .enqueue(
                &mut tx,
                EmailKind::PasswordChanged,
                "quota@example.com",
                "",
                None,
                Utc::now() + chrono::Duration::minutes(30),
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        service.work_once(&pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT error_category FROM email_outbox WHERE id=$1")
                .bind(quota_job)
                .fetch_one(&pool)
                .await
                .unwrap(),
            "quota_or_rate_limit"
        );
        sqlx::query("UPDATE email_outbox SET expires_at=now()-interval '1 second' WHERE id=$1")
            .bind(quota_job)
            .execute(&pool)
            .await
            .unwrap();
        service.work_once(&pool).await.unwrap();
        let expired = sqlx::query("SELECT status,payload FROM email_outbox WHERE id=$1")
            .bind(quota_job)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(expired.get::<String, _>("status"), "expired");
        assert!(expired.get::<Option<Vec<u8>>, _>("payload").is_none());
        // Authenticated duplicate and out-of-order webhooks cannot erase suppression.
        service.provider = Provider::Resend;
        service.webhook_key = b"webhook test secret material".to_vec();
        let timestamp = Utc::now().timestamp().to_string();
        for (event_id, kind) in [
            ("evt_complaint", "email.complained"),
            ("evt_complaint", "email.complained"),
            ("evt_late_delivery", "email.delivered"),
        ] {
            let body=serde_json::to_vec(&serde_json::json!({"type":kind,"data":{"email_id":"provider-known","to":["signed@example.com"]}})).unwrap();
            let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&service.webhook_key).unwrap();
            mac.update(format!("{event_id}.{timestamp}.").as_bytes());
            mac.update(&body);
            let mut headers = axum::http::HeaderMap::new();
            headers.insert("svix-id", event_id.parse().unwrap());
            headers.insert("svix-timestamp", timestamp.parse().unwrap());
            headers.insert(
                "svix-signature",
                format!("v1,{}", STANDARD.encode(mac.finalize().into_bytes()))
                    .parse()
                    .unwrap(),
            );
            service
                .verify_and_record(&pool, &headers, &body)
                .await
                .unwrap();
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM email_delivery_events")
                .fetch_one(&pool)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT reason FROM email_suppressions WHERE recipient='signed@example.com'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "email.complained"
        );
        pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
