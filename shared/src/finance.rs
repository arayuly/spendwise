use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub transaction_type: String,
    pub color: String,
    pub icon: String,
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryInput {
    pub name: String,
    pub transaction_type: String,
    pub color: String,
    pub icon: String,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Transaction {
    pub id: Uuid,
    pub category_id: Uuid,
    pub transaction_type: String,
    pub amount: Decimal,
    pub title: String,
    pub description: Option<String>,
    pub transaction_date: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionInput {
    pub category_id: Uuid,
    pub transaction_type: String,
    pub amount: Decimal,
    pub title: String,
    pub description: Option<String>,
    pub transaction_date: NaiveDate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TransactionQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub category_id: Option<Uuid>,
    pub transaction_type: Option<String>,
    pub search: Option<String>,
    pub sort: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeriodQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CategoryQuery {
    #[serde(default)]
    pub include_archived: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BudgetQuery {
    pub month: Option<NaiveDate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Budget {
    pub id: Uuid,
    pub category_id: Option<Uuid>,
    pub month: NaiveDate,
    pub amount: Decimal,
    pub spent: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetInput {
    pub category_id: Option<Uuid>,
    pub month: NaiveDate,
    pub amount: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Recurring {
    pub id: Uuid,
    pub category_id: Uuid,
    pub transaction_type: String,
    pub amount: Decimal,
    pub title: String,
    pub description: Option<String>,
    pub next_date: NaiveDate,
    pub day_of_month: i32,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecurringInput {
    pub category_id: Uuid,
    pub transaction_type: String,
    pub amount: Decimal,
    pub title: String,
    pub description: Option<String>,
    pub next_date: NaiveDate,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct AnalyticsSummary {
    pub balance: Decimal,
    pub income: Decimal,
    pub expenses: Decimal,
    pub savings: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct CategoryTotal {
    pub category_id: Uuid,
    pub name: String,
    pub color: String,
    pub total: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct MonthlyTotal {
    pub month: NaiveDate,
    pub income: Decimal,
    pub expenses: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BalancePoint {
    pub date: NaiveDate,
    pub balance: Decimal,
}
