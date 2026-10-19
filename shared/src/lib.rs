//! Shared wire contracts. Money is encoded as decimal strings, never binary floats.
mod finance;
use chrono::{DateTime, Utc};
pub use finance::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub currency: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub user: User,
    pub csrf_token: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CsrfResponse {
    pub csrf_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileInput {
    pub display_name: String,
    pub currency: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordInput {
    pub current_password: String,
    pub new_password: String,
}
