//! Authenticated REST handlers for personal financial resources.
use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, Query, State},
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{Datelike, Months, NaiveDate, Utc};
use serde::de::DeserializeOwned;
use spendwise_shared::*;
use uuid::Uuid;

use crate::{
    AppState,
    auth::AuthUser,
    error::{AppError, AppJson, AppResult},
    repository as repo, services,
};

/// Query rejections use the same error envelope as domain failures.
struct ApiQuery<T>(T);
impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> AppResult<Self> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::Validation("Некорректные параметры запроса".into()))?;
        Ok(Self(value))
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/transactions",
            get(list_transactions).post(create_transaction),
        )
        .route(
            "/transactions/{id}",
            get(get_transaction)
                .patch(update_transaction)
                .delete(delete_transaction),
        )
        .route("/categories", get(list_categories).post(create_category))
        .route(
            "/categories/{id}",
            axum::routing::patch(update_category).delete(delete_category),
        )
        .route("/budgets", get(list_budgets).post(create_budget))
        .route(
            "/budgets/{id}",
            axum::routing::patch(update_budget).delete(delete_budget),
        )
        .route("/recurring", get(list_recurring).post(create_recurring))
        .route(
            "/recurring/{id}",
            axum::routing::patch(update_recurring).delete(delete_recurring),
        )
        .route("/analytics/summary", get(summary))
        .route("/analytics/categories", get(category_totals))
        .route("/analytics/monthly", get(monthly_totals))
        .route("/analytics/balance", get(balance_points))
        .route("/export/transactions", get(export_transactions))
}

async fn list_transactions(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    ApiQuery(mut query): ApiQuery<TransactionQuery>,
) -> AppResult<Json<Paginated<Transaction>>> {
    services::validate_query(&mut query)?;
    Ok(Json(repo::transactions(&state.pool, user, &query).await?))
}

async fn get_transaction(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Transaction>> {
    Ok(Json(repo::transaction(&state.pool, user, id).await?))
}

async fn create_transaction(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AppJson(mut input): AppJson<TransactionInput>,
) -> AppResult<(StatusCode, Json<Transaction>)> {
    services::validate_transaction(&mut input)?;
    Ok((
        StatusCode::CREATED,
        Json(repo::create_transaction(&state.pool, user, &input).await?),
    ))
}

async fn update_transaction(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    AppJson(mut input): AppJson<TransactionInput>,
) -> AppResult<Json<Transaction>> {
    services::validate_transaction(&mut input)?;
    Ok(Json(
        repo::update_transaction(&state.pool, user, id, &input).await?,
    ))
}

async fn delete_transaction(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    repo::delete_transaction(&state.pool, user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_categories(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    ApiQuery(query): ApiQuery<CategoryQuery>,
) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(
        repo::categories(&state.pool, user, query.include_archived).await?,
    ))
}

async fn create_category(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AppJson(mut input): AppJson<CategoryInput>,
) -> AppResult<(StatusCode, Json<Category>)> {
    services::validate_category(&mut input)?;
    Ok((
        StatusCode::CREATED,
        Json(repo::create_category(&state.pool, user, &input).await?),
    ))
}

async fn update_category(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    AppJson(mut input): AppJson<CategoryInput>,
) -> AppResult<Json<Category>> {
    services::validate_category(&mut input)?;
    Ok(Json(
        repo::update_category(&state.pool, user, id, &input).await?,
    ))
}

async fn delete_category(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    repo::archive_category(&state.pool, user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_budgets(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    ApiQuery(query): ApiQuery<BudgetQuery>,
) -> AppResult<Json<Vec<Budget>>> {
    if let Some(month) = query.month {
        services::validate_date(month)?;
        if month.day() != 1 {
            return Err(AppError::Validation(
                "Месяц бюджета должен быть указан первым днём месяца".into(),
            ));
        }
    }
    Ok(Json(repo::budgets(&state.pool, user, query.month).await?))
}

async fn create_budget(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AppJson(input): AppJson<BudgetInput>,
) -> AppResult<(StatusCode, Json<Budget>)> {
    services::validate_budget(&input)?;
    Ok((
        StatusCode::CREATED,
        Json(repo::create_budget(&state.pool, user, &input).await?),
    ))
}

async fn update_budget(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    AppJson(input): AppJson<BudgetInput>,
) -> AppResult<Json<Budget>> {
    services::validate_budget(&input)?;
    Ok(Json(
        repo::update_budget(&state.pool, user, id, &input).await?,
    ))
}

async fn delete_budget(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    repo::delete_budget(&state.pool, user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_recurring(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Vec<Recurring>>> {
    Ok(Json(repo::recurring(&state.pool, user).await?))
}

async fn create_recurring(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AppJson(mut input): AppJson<RecurringInput>,
) -> AppResult<(StatusCode, Json<Recurring>)> {
    services::validate_recurring(&mut input)?;
    Ok((
        StatusCode::CREATED,
        Json(repo::create_recurring(&state.pool, user, &input).await?),
    ))
}

async fn update_recurring(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    AppJson(mut input): AppJson<RecurringInput>,
) -> AppResult<Json<Recurring>> {
    services::validate_recurring(&mut input)?;
    Ok(Json(
        repo::update_recurring(&state.pool, user, id, &input).await?,
    ))
}

async fn delete_recurring(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    repo::delete_recurring(&state.pool, user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn summary(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    ApiQuery(period): ApiQuery<PeriodQuery>,
) -> AppResult<Json<AnalyticsSummary>> {
    services::validate_period(period.from, period.to)?;
    Ok(Json(repo::summary(&state.pool, user, &period).await?))
}

async fn category_totals(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    ApiQuery(period): ApiQuery<PeriodQuery>,
) -> AppResult<Json<Vec<CategoryTotal>>> {
    services::validate_period(period.from, period.to)?;
    Ok(Json(
        repo::category_totals(&state.pool, user, &period).await?,
    ))
}

fn chart_period(period: &PeriodQuery, monthly: bool) -> AppResult<(NaiveDate, NaiveDate)> {
    let to = period.to.unwrap_or_else(|| Utc::now().date_naive());
    let month_start = to.with_day(1).ok_or(AppError::Internal)?;
    let default_from = if monthly {
        month_start
            .checked_sub_months(Months::new(11))
            .ok_or(AppError::Internal)?
    } else {
        month_start
    };
    let from = period.from.unwrap_or(default_from);
    services::validate_period(Some(from), Some(to))?;
    if to.signed_duration_since(from).num_days() > 3660 {
        return Err(AppError::Validation(
            "Диапазон графика не должен превышать 3660 дней".into(),
        ));
    }
    Ok((from, to))
}

async fn monthly_totals(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    ApiQuery(period): ApiQuery<PeriodQuery>,
) -> AppResult<Json<Vec<MonthlyTotal>>> {
    let (from, to) = chart_period(&period, true)?;
    Ok(Json(
        repo::monthly_totals(&state.pool, user, from, to).await?,
    ))
}

async fn balance_points(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    ApiQuery(period): ApiQuery<PeriodQuery>,
) -> AppResult<Json<Vec<BalancePoint>>> {
    let (from, to) = chart_period(&period, false)?;
    Ok(Json(
        repo::balance_points(&state.pool, user, from, to).await?,
    ))
}

static EXPORT_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

struct ExportState {
    tx: sqlx::Transaction<'static, sqlx::Postgres>,
    user: Uuid,
    query: TransactionQuery,
    after: Option<(NaiveDate, Uuid)>,
    header_sent: bool,
    _slot: tokio::sync::SemaphorePermit<'static>,
}

async fn export_transactions(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    ApiQuery(mut query): ApiQuery<TransactionQuery>,
) -> AppResult<Response> {
    services::validate_query(&mut query)?;
    let slot = EXPORT_SLOTS
        .try_acquire()
        .map_err(|_| AppError::RateLimited)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL idle_in_transaction_session_timeout = '30s'")
        .execute(&mut *tx)
        .await?;
    if repo::export_count(&mut tx, user, &query).await? > 100_000 {
        return Err(AppError::Validation(
            "Экспорт ограничен 100000 записями. Сократите диапазон дат".into(),
        ));
    }
    let stream = futures_util::stream::try_unfold(
        ExportState {
            tx,
            user,
            query,
            after: None,
            header_sent: false,
            _slot: slot,
        },
        |mut state| async move {
            if !state.header_sent {
                state.header_sent = true;
                return Ok::<_, AppError>(Some((
                    String::from("\u{feff}id,date,type,category_id,amount,title,description\r\n"),
                    state,
                )));
            }
            let rows =
                repo::export_page(&mut state.tx, state.user, &state.query, state.after).await?;
            if rows.is_empty() {
                state.tx.commit().await?;
                return Ok(None);
            }
            let mut csv = String::new();
            for item in rows {
                state.after = Some((item.transaction_date, item.id));
                let fields = [
                    item.id.to_string(),
                    item.transaction_date.to_string(),
                    item.transaction_type,
                    item.category_id.to_string(),
                    item.amount.to_string(),
                    item.title,
                    item.description.unwrap_or_default(),
                ];
                csv.push_str(
                    &fields
                        .iter()
                        .map(|value| services::csv_cell(value))
                        .collect::<Vec<_>>()
                        .join(","),
                );
                csv.push_str("\r\n");
            }
            Ok(Some((csv, state)))
        },
    );
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"spendwise-transactions.csv\"",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response())
}
