use crate::models::User;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use secrecy::{ExposeSecret, Secret};
use sqlx::{PgPool, Row};
use tower_sessions::Session;
use uuid::Uuid;

const USER_SESSION_KEY: &str = "user_id";

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Session error: {0}")]
    Session(#[from] tower_sessions::session::Error),
    #[error("Password hash error: {0}")]
    HashError(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Invalid email or password")]
    InvalidCredentials,
    #[error("User already exists")]
    UserExists,
}

pub async fn hash_password(password: Secret<String>) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.expose_secret().as_bytes(), &salt)
        .map_err(|e| AuthError::HashError(e.to_string()))?
        .to_string();
    Ok(password_hash)
}

pub async fn verify_password(
    password: Secret<String>,
    password_hash: &str,
) -> Result<bool, AuthError> {
    let expected_password_hash =
        PasswordHash::new(password_hash).map_err(|e| AuthError::HashError(e.to_string()))?;
    let argon2 = Argon2::default();
    Ok(argon2
        .verify_password(password.expose_secret().as_bytes(), &expected_password_hash)
        .is_ok())
}

pub async fn register_user(
    db: &PgPool,
    first_name: &str,
    last_name: &str,
    email: &str,
    password: Secret<String>,
) -> Result<User, AuthError> {
    let first_name = required_profile_text(first_name, 80, "First name")?;
    let last_name = required_profile_text(last_name, 80, "Last name")?;
    let full_name = joined_full_name(Some(&first_name), Some(&last_name));
    let email = normalize_email(email)?;
    validate_password(password.expose_secret())?;
    let password_hash = hash_password(password).await?;
    let id = Uuid::new_v4().to_string();

    let mut tx = db.begin().await?;

    let user_result = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, email, full_name, first_name, last_name)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id,
            email,
            full_name,
            first_name,
            last_name,
            phone_number,
            address_line1,
            address_line2,
            address_city,
            address_state,
            address_postal_code,
            marketing_opt_in
        "#,
    )
    .bind(&id)
    .bind(&email)
    .bind(full_name)
    .bind(first_name)
    .bind(last_name)
    .fetch_one(&mut *tx)
    .await;

    let user = match user_result {
        Ok(u) => u,
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            return Err(AuthError::UserExists)
        }
        Err(e) => return Err(AuthError::Db(e)),
    };

    sqlx::query(
        r#"
        INSERT INTO user_identities (user_id, provider, provider_id)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(&id)
    .bind("password")
    .bind(&id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO password_credentials (user_id, password_hash)
        VALUES ($1, $2)
        "#,
    )
    .bind(&id)
    .bind(&password_hash)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(user)
}

pub async fn login_user(
    db: &PgPool,
    session: &Session,
    email: &str,
    password: Secret<String>,
) -> Result<User, AuthError> {
    let email = normalize_email(email)?;
    let record = sqlx::query(
        r#"
        SELECT
            u.id,
            u.email,
            u.full_name,
            u.first_name,
            u.last_name,
            u.phone_number,
            u.address_line1,
            u.address_line2,
            u.address_city,
            u.address_state,
            u.address_postal_code,
            u.marketing_opt_in,
            pc.password_hash
        FROM users u
        JOIN password_credentials pc ON pc.user_id = u.id
        WHERE u.email = $1
        "#,
    )
    .bind(&email)
    .fetch_optional(db)
    .await?;

    let record = match record {
        Some(r) => r,
        None => return Err(AuthError::InvalidCredentials),
    };

    let password_hash: String = record.try_get("password_hash")?;
    let id: String = record.try_get("id")?;
    let db_email: String = record.try_get("email")?;
    let full_name: Option<String> = record.try_get("full_name")?;
    let first_name: Option<String> = record.try_get("first_name")?;
    let last_name: Option<String> = record.try_get("last_name")?;
    let phone_number: Option<String> = record.try_get("phone_number")?;
    let address_line1: Option<String> = record.try_get("address_line1")?;
    let address_line2: Option<String> = record.try_get("address_line2")?;
    let address_city: Option<String> = record.try_get("address_city")?;
    let address_state: Option<String> = record.try_get("address_state")?;
    let address_postal_code: Option<String> = record.try_get("address_postal_code")?;
    let marketing_opt_in: bool = record.try_get("marketing_opt_in")?;

    if !verify_password(password, &password_hash).await? {
        return Err(AuthError::InvalidCredentials);
    }

    let user = User {
        id,
        email: db_email,
        full_name,
        first_name,
        last_name,
        phone_number,
        address_line1,
        address_line2,
        address_city,
        address_state,
        address_postal_code,
        marketing_opt_in,
    };

    sign_in_user(session, &user.id).await?;

    Ok(user)
}

pub async fn sign_in_user(session: &Session, user_id: &str) -> Result<(), AuthError> {
    session.cycle_id().await?;
    session.insert(USER_SESSION_KEY, user_id).await?;
    Ok(())
}

pub async fn logout_user(session: &Session) {
    let _ = session.delete().await;
}

pub async fn get_current_user(db: &PgPool, session: &Session) -> Result<Option<User>, sqlx::Error> {
    let user_id: Option<String> = session.get(USER_SESSION_KEY).await.unwrap_or(None);
    match user_id {
        Some(id) => {
            let user = sqlx::query_as::<_, User>(
                r#"
                SELECT
                    id,
                    email,
                    full_name,
                    first_name,
                    last_name,
                    phone_number,
                    address_line1,
                    address_line2,
                    address_city,
                    address_state,
                    address_postal_code,
                    marketing_opt_in
                FROM users
                WHERE id = $1
                "#,
            )
            .bind(&id)
            .fetch_optional(db)
            .await?;
            Ok(user)
        }
        None => Ok(None),
    }
}

pub async fn update_user_profile(
    db: &PgPool,
    user_id: &str,
    first_name: &str,
    last_name: &str,
    email: &str,
    phone_number: &str,
    address_line1: &str,
    address_line2: &str,
    address_city: &str,
    address_state: &str,
    address_postal_code: &str,
    marketing_opt_in: bool,
) -> Result<User, AuthError> {
    let email = normalize_email(email)?;
    let first_name = optional_profile_text(first_name, 80, "First name")?;
    let last_name = optional_profile_text(last_name, 80, "Last name")?;
    let full_name = joined_full_name(first_name.as_deref(), last_name.as_deref());
    let phone_number = optional_profile_text(phone_number, 40, "Phone number")?;
    let address_line1 = optional_profile_text(address_line1, 120, "Address")?;
    let address_line2 = optional_profile_text(address_line2, 120, "Apartment, suite, or unit")?;
    let address_city = optional_profile_text(address_city, 80, "City")?;
    let address_state = optional_state(address_state)?;
    let address_postal_code = optional_profile_text(address_postal_code, 20, "ZIP code")?;

    let result = sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET
            full_name = $2,
            first_name = $3,
            last_name = $4,
            email = $5,
            phone_number = $6,
            address_line1 = $7,
            address_line2 = $8,
            address_city = $9,
            address_state = $10,
            address_postal_code = $11,
            marketing_opt_in = $12,
            updated_at = now()
        WHERE id = $1
        RETURNING
            id,
            email,
            full_name,
            first_name,
            last_name,
            phone_number,
            address_line1,
            address_line2,
            address_city,
            address_state,
            address_postal_code,
            marketing_opt_in
        "#,
    )
    .bind(user_id)
    .bind(full_name)
    .bind(first_name)
    .bind(last_name)
    .bind(email)
    .bind(phone_number)
    .bind(address_line1)
    .bind(address_line2)
    .bind(address_city)
    .bind(address_state)
    .bind(address_postal_code)
    .bind(marketing_opt_in)
    .fetch_one(db)
    .await;

    match result {
        Ok(user) => Ok(user),
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => Err(AuthError::UserExists),
        Err(err) => Err(AuthError::Db(err)),
    }
}

fn normalize_email(email: &str) -> Result<String, AuthError> {
    let email = email.trim().to_lowercase();
    if email.is_empty() || email.len() > 254 || !email.contains('@') {
        return Err(AuthError::Validation("Enter a valid email address.".into()));
    }
    Ok(email)
}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.len() < 8 {
        return Err(AuthError::Validation(
            "Password must be at least 8 characters.".into(),
        ));
    }
    Ok(())
}

fn optional_profile_text(
    value: &str,
    max_len: usize,
    label: &str,
) -> Result<Option<String>, AuthError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > max_len {
        return Err(AuthError::Validation(format!("{label} is too long.")));
    }
    Ok(Some(value.to_string()))
}

fn required_profile_text(value: &str, max_len: usize, label: &str) -> Result<String, AuthError> {
    optional_profile_text(value, max_len, label)?
        .ok_or_else(|| AuthError::Validation(format!("{label} is required.")))
}

fn optional_state(value: &str) -> Result<Option<String>, AuthError> {
    let value = value.trim().to_uppercase();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(AuthError::Validation(
            "Enter a valid 2-letter US state.".into(),
        ));
    }
    Ok(Some(value))
}

fn joined_full_name(first_name: Option<&str>, last_name: Option<&str>) -> Option<String> {
    let name = [first_name.unwrap_or(""), last_name.unwrap_or("")]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}
