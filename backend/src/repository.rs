//! Parameterized, ownership-scoped PostgreSQL access.
use chrono::{Datelike, NaiveDate};
use spendwise_shared::*;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

const TRANSACTION_COLUMNS: &str = "id, category_id, transaction_type, amount, title, description, transaction_date, created_at, updated_at";
const CATEGORY_COLUMNS: &str = "id, name, transaction_type, color, icon, archived";
const RECURRING_COLUMNS: &str = "id, category_id, transaction_type, amount, title, description, next_date, day_of_month, active";
const FILTER: &str = "user_id = $1 AND ($2::date IS NULL OR transaction_date >= $2) AND ($3::date IS NULL OR transaction_date <= $3) AND ($4::uuid IS NULL OR category_id = $4) AND ($5::text IS NULL OR transaction_type = $5) AND ($6::text IS NULL OR title ILIKE $6 OR COALESCE(description, '') ILIKE $6)";

pub fn database_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(ref db) = error {
        match db.code().as_deref() {
            Some("23505") => {
                return AppError::Conflict("Запись с такими параметрами уже существует".into());
            }
            Some("23503") => {
                return AppError::Validation(
                    "Категория используется другими записями или не соответствует операции".into(),
                );
            }
            Some("23514") | Some("22003") => {
                return AppError::Validation("Финансовые данные нарушают правила валидации".into());
            }
            _ => {}
        }
    }
    error.into()
}

/// Archive must exclude newly inserted/moved rules as well as existing row locks.
/// Workers share this lock before claiming rows; unrelated users remain independent.
async fn lock_recurring_user(
    connection: &mut sqlx::PgConnection,
    user: Uuid,
    exclusive: bool,
) -> AppResult<()> {
    let statement = if exclusive {
        "SELECT pg_advisory_xact_lock(hashtextextended($1::text, 220106))"
    } else {
        "SELECT pg_advisory_xact_lock_shared(hashtextextended($1::text, 220106))"
    };
    sqlx::query(statement)
        .bind(user.to_string())
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn require_category<'e, E>(
    executor: E,
    user: Uuid,
    category: Uuid,
    kind: &str,
) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM categories WHERE user_id=$1 AND id=$2 AND transaction_type=$3 AND NOT archived)")
        .bind(user).bind(category).bind(kind).fetch_one(executor).await?;
    if !exists {
        return Err(AppError::Validation(
            "Выберите активную категорию, соответствующую типу операции".into(),
        ));
    }
    Ok(())
}

fn search_pattern(query: &TransactionQuery) -> Option<String> {
    query.search.as_ref().filter(|s| !s.is_empty()).map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    })
}

pub async fn transactions(
    pool: &PgPool,
    user: Uuid,
    query: &TransactionQuery,
) -> AppResult<Paginated<Transaction>> {
    let pattern = search_pattern(query);
    let total: i64 =
        sqlx::query_scalar(&format!("SELECT count(*) FROM transactions WHERE {FILTER}"))
            .bind(user)
            .bind(query.from)
            .bind(query.to)
            .bind(query.category_id)
            .bind(&query.transaction_type)
            .bind(&pattern)
            .fetch_one(pool)
            .await?;
    let order = match query.sort.as_deref() {
        Some("date_asc") => "transaction_date ASC, id ASC",
        Some("amount_desc") => "amount DESC, id DESC",
        Some("amount_asc") => "amount ASC, id ASC",
        _ => "transaction_date DESC, id DESC",
    };
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);
    let items = sqlx::query_as(&format!("SELECT {TRANSACTION_COLUMNS} FROM transactions WHERE {FILTER} ORDER BY {order} LIMIT $7 OFFSET $8"))
        .bind(user).bind(query.from).bind(query.to).bind(query.category_id).bind(&query.transaction_type).bind(&pattern)
        .bind(per_page).bind((page-1)*per_page).fetch_all(pool).await?;
    Ok(Paginated {
        items,
        total,
        page,
        per_page,
    })
}

pub async fn transaction(pool: &PgPool, user: Uuid, id: Uuid) -> AppResult<Transaction> {
    sqlx::query_as(&format!(
        "SELECT {TRANSACTION_COLUMNS} FROM transactions WHERE user_id=$1 AND id=$2"
    ))
    .bind(user)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub async fn create_transaction(
    pool: &PgPool,
    user: Uuid,
    input: &TransactionInput,
) -> AppResult<Transaction> {
    require_category(pool, user, input.category_id, &input.transaction_type).await?;
    sqlx::query_as(&format!("INSERT INTO transactions(id,user_id,category_id,transaction_type,amount,title,description,transaction_date) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING {TRANSACTION_COLUMNS}"))
        .bind(Uuid::new_v4()).bind(user).bind(input.category_id).bind(&input.transaction_type).bind(input.amount)
        .bind(&input.title).bind(&input.description).bind(input.transaction_date).fetch_one(pool).await.map_err(database_error)
}

pub async fn update_transaction(
    pool: &PgPool,
    user: Uuid,
    id: Uuid,
    input: &TransactionInput,
) -> AppResult<Transaction> {
    // Resolve ownership first; an unrelated category cannot reveal whether another user's id exists.
    transaction(pool, user, id).await?;
    require_category(pool, user, input.category_id, &input.transaction_type).await?;
    sqlx::query_as(&format!("UPDATE transactions SET category_id=$3,transaction_type=$4,amount=$5,title=$6,description=$7,transaction_date=$8,updated_at=now() WHERE user_id=$1 AND id=$2 RETURNING {TRANSACTION_COLUMNS}"))
        .bind(user).bind(id).bind(input.category_id).bind(&input.transaction_type).bind(input.amount)
        .bind(&input.title).bind(&input.description).bind(input.transaction_date).fetch_optional(pool).await.map_err(database_error)?.ok_or(AppError::NotFound)
}

pub async fn delete_transaction(pool: &PgPool, user: Uuid, id: Uuid) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM transactions WHERE user_id=$1 AND id=$2")
        .bind(user)
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn categories(
    pool: &PgPool,
    user: Uuid,
    include_archived: bool,
) -> AppResult<Vec<Category>> {
    Ok(sqlx::query_as(&format!("SELECT {CATEGORY_COLUMNS} FROM categories WHERE user_id=$1 AND ($2 OR NOT archived) ORDER BY transaction_type, lower(name), id"))
        .bind(user).bind(include_archived).fetch_all(pool).await?)
}

pub async fn create_category(
    pool: &PgPool,
    user: Uuid,
    input: &CategoryInput,
) -> AppResult<Category> {
    sqlx::query_as(&format!("INSERT INTO categories(id,user_id,name,transaction_type,color,icon,archived) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING {CATEGORY_COLUMNS}"))
        .bind(Uuid::new_v4()).bind(user).bind(&input.name).bind(&input.transaction_type).bind(&input.color).bind(&input.icon).bind(input.archived)
        .fetch_one(pool).await.map_err(database_error)
}

pub async fn update_category(
    pool: &PgPool,
    user: Uuid,
    id: Uuid,
    input: &CategoryInput,
) -> AppResult<Category> {
    let mut tx = pool.begin().await?;
    lock_recurring_user(&mut tx, user, true).await?;
    lock_category_rules(&mut tx, user, id).await?;
    let current_type: String =
        sqlx::query_scalar("SELECT transaction_type FROM categories WHERE user_id=$1 AND id=$2")
            .bind(user)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
    if current_type != input.transaction_type {
        let used: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transactions WHERE user_id=$1 AND category_id=$2) OR EXISTS(SELECT 1 FROM budgets WHERE user_id=$1 AND category_id=$2) OR EXISTS(SELECT 1 FROM recurring_transactions WHERE user_id=$1 AND category_id=$2)")
            .bind(user).bind(id).fetch_one(&mut *tx).await?;
        if used {
            return Err(AppError::Conflict(
                "Тип используемой категории изменить нельзя. Создайте новую категорию".into(),
            ));
        }
    }
    let category = sqlx::query_as(&format!("UPDATE categories SET name=$3,transaction_type=$4,color=$5,icon=$6,archived=$7,updated_at=now() WHERE user_id=$1 AND id=$2 RETURNING {CATEGORY_COLUMNS}"))
        .bind(user).bind(id).bind(&input.name).bind(&input.transaction_type).bind(&input.color).bind(&input.icon).bind(input.archived)
        .fetch_optional(&mut *tx).await.map_err(database_error)?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(category)
}

/// Match scheduler's rule-before-category lock order before the archive trigger runs.
async fn lock_category_rules(
    connection: &mut sqlx::PgConnection,
    user: Uuid,
    category: Uuid,
) -> AppResult<()> {
    sqlx::query("SELECT id FROM recurring_transactions WHERE user_id=$1 AND category_id=$2 ORDER BY next_date,id FOR UPDATE")
        .bind(user).bind(category).execute(connection).await?;
    Ok(())
}

/// Preserve historical transactions and budgets by archiving instead of deleting referenced categories.
pub async fn archive_category(pool: &PgPool, user: Uuid, id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    lock_recurring_user(&mut tx, user, true).await?;
    lock_category_rules(&mut tx, user, id).await?;
    let result = sqlx::query(
        "UPDATE categories SET archived=true,updated_at=now() WHERE user_id=$1 AND id=$2",
    )
    .bind(user)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tx.commit().await?;
    Ok(())
}

const BUDGET_SELECT: &str = "SELECT b.id,b.category_id,b.month,b.amount,COALESCE((SELECT sum(t.amount) FROM transactions t WHERE t.user_id=b.user_id AND t.transaction_type='expense' AND t.transaction_date>=b.month AND t.transaction_date<(b.month+interval '1 month') AND (b.category_id IS NULL OR t.category_id=b.category_id)),0)::numeric AS spent FROM budgets b";

pub async fn budgets(
    pool: &PgPool,
    user: Uuid,
    month: Option<NaiveDate>,
) -> AppResult<Vec<Budget>> {
    Ok(sqlx::query_as(&format!("{BUDGET_SELECT} WHERE b.user_id=$1 AND ($2::date IS NULL OR b.month=$2) ORDER BY b.month DESC,b.category_id NULLS FIRST,b.id"))
        .bind(user).bind(month).fetch_all(pool).await?)
}

async fn budget(pool: &PgPool, user: Uuid, id: Uuid) -> AppResult<Budget> {
    sqlx::query_as(&format!("{BUDGET_SELECT} WHERE b.user_id=$1 AND b.id=$2"))
        .bind(user)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn create_budget(pool: &PgPool, user: Uuid, input: &BudgetInput) -> AppResult<Budget> {
    if let Some(category) = input.category_id {
        require_category(pool, user, category, "expense").await?;
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO budgets(id,user_id,category_id,month,amount) VALUES($1,$2,$3,$4,$5)")
        .bind(id)
        .bind(user)
        .bind(input.category_id)
        .bind(input.month)
        .bind(input.amount)
        .execute(pool)
        .await
        .map_err(database_error)?;
    budget(pool, user, id).await
}

pub async fn update_budget(
    pool: &PgPool,
    user: Uuid,
    id: Uuid,
    input: &BudgetInput,
) -> AppResult<Budget> {
    budget(pool, user, id).await?;
    if let Some(category) = input.category_id {
        require_category(pool, user, category, "expense").await?;
    }
    let result = sqlx::query("UPDATE budgets SET category_id=$3,month=$4,amount=$5,updated_at=now() WHERE user_id=$1 AND id=$2")
        .bind(user).bind(id).bind(input.category_id).bind(input.month).bind(input.amount).execute(pool).await.map_err(database_error)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    budget(pool, user, id).await
}

pub async fn delete_budget(pool: &PgPool, user: Uuid, id: Uuid) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM budgets WHERE user_id=$1 AND id=$2")
        .bind(user)
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn recurring(pool: &PgPool, user: Uuid) -> AppResult<Vec<Recurring>> {
    Ok(sqlx::query_as(&format!("SELECT {RECURRING_COLUMNS} FROM recurring_transactions WHERE user_id=$1 ORDER BY active DESC,next_date,id"))
        .bind(user).fetch_all(pool).await?)
}

pub async fn create_recurring(
    pool: &PgPool,
    user: Uuid,
    input: &RecurringInput,
) -> AppResult<Recurring> {
    let mut tx = pool.begin().await?;
    lock_recurring_user(&mut tx, user, false).await?;
    require_category(&mut *tx, user, input.category_id, &input.transaction_type).await?;
    let recurring = sqlx::query_as(&format!("INSERT INTO recurring_transactions(id,user_id,category_id,transaction_type,amount,title,description,next_date,day_of_month,active) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING {RECURRING_COLUMNS}"))
        .bind(Uuid::new_v4()).bind(user).bind(input.category_id).bind(&input.transaction_type).bind(input.amount)
        .bind(&input.title).bind(&input.description).bind(input.next_date).bind(input.next_date.day() as i32).bind(input.active)
        .fetch_one(&mut *tx).await.map_err(database_error)?;
    tx.commit().await?;
    Ok(recurring)
}

pub async fn update_recurring(
    pool: &PgPool,
    user: Uuid,
    id: Uuid,
    input: &RecurringInput,
) -> AppResult<Recurring> {
    let mut tx = pool.begin().await?;
    lock_recurring_user(&mut tx, user, false).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM recurring_transactions WHERE user_id=$1 AND id=$2)",
    )
    .bind(user)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    require_category(&mut *tx, user, input.category_id, &input.transaction_type).await?;
    // Preserve the original anchor when only metadata changes after a short month.
    let recurring = sqlx::query_as(&format!("UPDATE recurring_transactions SET category_id=$3,transaction_type=$4,amount=$5,title=$6,description=$7,day_of_month=CASE WHEN next_date=$8 THEN day_of_month ELSE $9 END,next_date=$8,active=$10,updated_at=now() WHERE user_id=$1 AND id=$2 RETURNING {RECURRING_COLUMNS}"))
        .bind(user).bind(id).bind(input.category_id).bind(&input.transaction_type).bind(input.amount)
        .bind(&input.title).bind(&input.description).bind(input.next_date).bind(input.next_date.day() as i32).bind(input.active)
        .fetch_optional(&mut *tx).await.map_err(database_error)?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(recurring)
}

pub async fn delete_recurring(pool: &PgPool, user: Uuid, id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    lock_recurring_user(&mut tx, user, false).await?;
    let result = sqlx::query("DELETE FROM recurring_transactions WHERE user_id=$1 AND id=$2")
        .bind(user)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tx.commit().await?;
    Ok(())
}

pub async fn summary(
    pool: &PgPool,
    user: Uuid,
    period: &PeriodQuery,
) -> AppResult<AnalyticsSummary> {
    Ok(sqlx::query_as("SELECT COALESCE(sum(CASE WHEN transaction_type='income' THEN amount ELSE -amount END) FILTER(WHERE $3::date IS NULL OR transaction_date<=$3),0)::numeric AS balance,COALESCE(sum(amount) FILTER(WHERE transaction_type='income' AND ($2::date IS NULL OR transaction_date>=$2) AND ($3::date IS NULL OR transaction_date<=$3)),0)::numeric AS income,COALESCE(sum(amount) FILTER(WHERE transaction_type='expense' AND ($2::date IS NULL OR transaction_date>=$2) AND ($3::date IS NULL OR transaction_date<=$3)),0)::numeric AS expenses,COALESCE(sum(CASE WHEN transaction_type='income' THEN amount ELSE -amount END) FILTER(WHERE ($2::date IS NULL OR transaction_date>=$2) AND ($3::date IS NULL OR transaction_date<=$3)),0)::numeric AS savings FROM transactions WHERE user_id=$1")
        .bind(user).bind(period.from).bind(period.to).fetch_one(pool).await?)
}

pub async fn category_totals(
    pool: &PgPool,
    user: Uuid,
    period: &PeriodQuery,
) -> AppResult<Vec<CategoryTotal>> {
    Ok(sqlx::query_as("SELECT c.id AS category_id,c.name,c.color,sum(t.amount)::numeric AS total FROM transactions t JOIN categories c ON c.id=t.category_id AND c.user_id=t.user_id WHERE t.user_id=$1 AND t.transaction_type='expense' AND ($2::date IS NULL OR t.transaction_date>=$2) AND ($3::date IS NULL OR t.transaction_date<=$3) GROUP BY c.id,c.name,c.color ORDER BY total DESC,c.id")
        .bind(user).bind(period.from).bind(period.to).fetch_all(pool).await?)
}

pub async fn monthly_totals(
    pool: &PgPool,
    user: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> AppResult<Vec<MonthlyTotal>> {
    Ok(sqlx::query_as("SELECT m.month::date AS month,COALESCE(sum(t.amount) FILTER(WHERE t.transaction_type='income'),0)::numeric AS income,COALESCE(sum(t.amount) FILTER(WHERE t.transaction_type='expense'),0)::numeric AS expenses FROM generate_series(date_trunc('month',$2::date)::timestamp,date_trunc('month',$3::date)::timestamp,interval '1 month') AS m(month) LEFT JOIN transactions t ON t.user_id=$1 AND date_trunc('month',t.transaction_date)=m.month AND t.transaction_date>=$2 AND t.transaction_date<=$3 GROUP BY m.month ORDER BY m.month")
        .bind(user).bind(from).bind(to).fetch_all(pool).await?)
}

pub async fn balance_points(
    pool: &PgPool,
    user: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> AppResult<Vec<BalancePoint>> {
    Ok(sqlx::query_as("WITH opening AS (SELECT COALESCE(sum(CASE WHEN transaction_type='income' THEN amount ELSE -amount END),0)::numeric AS amount FROM transactions WHERE user_id=$1 AND transaction_date<$2),daily AS (SELECT transaction_date, sum(CASE WHEN transaction_type='income' THEN amount ELSE -amount END)::numeric AS amount FROM transactions WHERE user_id=$1 AND transaction_date>=$2 AND transaction_date<=$3 GROUP BY transaction_date) SELECT d.day::date AS date,(opening.amount+sum(COALESCE(daily.amount,0)) OVER(ORDER BY d.day))::numeric AS balance FROM generate_series($2::timestamp,$3::timestamp,interval '1 day') d(day) CROSS JOIN opening LEFT JOIN daily ON daily.transaction_date=d.day::date ORDER BY d.day")
        .bind(user).bind(from).bind(to).fetch_all(pool).await?)
}

pub async fn export_transactions(
    pool: &PgPool,
    user: Uuid,
    query: &TransactionQuery,
) -> AppResult<Vec<Transaction>> {
    let pattern = search_pattern(query);
    let items: Vec<Transaction> = sqlx::query_as(&format!("SELECT {TRANSACTION_COLUMNS} FROM transactions WHERE {FILTER} ORDER BY transaction_date,id LIMIT 100001"))
        .bind(user).bind(query.from).bind(query.to).bind(query.category_id).bind(&query.transaction_type).bind(&pattern).fetch_all(pool).await?;
    if items.len() > 100_000 {
        return Err(AppError::Validation(
            "Экспорт ограничен 100000 записями. Сократите диапазон дат".into(),
        ));
    }
    Ok(items)
}

pub async fn export_count(
    connection: &mut sqlx::PgConnection,
    user: Uuid,
    query: &TransactionQuery,
) -> AppResult<i64> {
    let pattern = search_pattern(query);
    Ok(
        sqlx::query_scalar(&format!("SELECT count(*) FROM transactions WHERE {FILTER}"))
            .bind(user)
            .bind(query.from)
            .bind(query.to)
            .bind(query.category_id)
            .bind(&query.transaction_type)
            .bind(&pattern)
            .fetch_one(connection)
            .await?,
    )
}

/// Stable keyset pages within the caller's repeatable-read export snapshot.
pub async fn export_page(
    connection: &mut sqlx::PgConnection,
    user: Uuid,
    query: &TransactionQuery,
    after: Option<(NaiveDate, Uuid)>,
) -> AppResult<Vec<Transaction>> {
    let pattern = search_pattern(query);
    Ok(sqlx::query_as(&format!("SELECT {TRANSACTION_COLUMNS} FROM transactions WHERE {FILTER} AND ($7::date IS NULL OR (transaction_date,id)>($7,$8::uuid)) ORDER BY transaction_date,id LIMIT 500"))
        .bind(user).bind(query.from).bind(query.to).bind(query.category_id).bind(&query.transaction_type).bind(&pattern)
        .bind(after.map(|v|v.0)).bind(after.map(|v|v.1)).fetch_all(connection).await?)
}

#[derive(sqlx::FromRow)]
struct DueRule {
    id: Uuid,
    user_id: Uuid,
    category_id: Uuid,
    transaction_type: String,
    amount: rust_decimal::Decimal,
    title: String,
    description: Option<String>,
    next_date: NaiveDate,
    day_of_month: i32,
}

pub async fn process_due(pool: &PgPool, today: NaiveDate) -> AppResult<u64> {
    let mut tx = pool.begin().await?;
    let users: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT user_id FROM recurring_transactions WHERE active AND next_date<=$1 ORDER BY user_id LIMIT 100")
        .bind(today).fetch_all(&mut *tx).await?;
    for user in &users {
        lock_recurring_user(&mut tx, *user, false).await?;
    }
    let rules: Vec<DueRule> = sqlx::query_as("SELECT id,user_id,category_id,transaction_type,amount,title,description,next_date,day_of_month FROM recurring_transactions WHERE active AND next_date<=$1 AND user_id=ANY($2) ORDER BY next_date,id LIMIT 100 FOR UPDATE SKIP LOCKED")
        .bind(today).bind(&users).fetch_all(&mut *tx).await?;
    let mut generated = 0;
    for rule in rules {
        let mut next = rule.next_date;
        for _ in 0..120 {
            if next > today {
                break;
            }
            let claimed = sqlx::query("INSERT INTO recurring_occurrences(recurring_id,occurrence_date) VALUES($1,$2) ON CONFLICT(recurring_id,occurrence_date) DO NOTHING")
                .bind(rule.id).bind(next).execute(&mut *tx).await?.rows_affected() == 1;
            if claimed {
                let id = Uuid::new_v4();
                sqlx::query("INSERT INTO transactions(id,user_id,category_id,transaction_type,amount,title,description,transaction_date) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
                    .bind(id).bind(rule.user_id).bind(rule.category_id).bind(&rule.transaction_type).bind(rule.amount).bind(&rule.title).bind(&rule.description).bind(next)
                    .execute(&mut *tx).await?;
                sqlx::query("UPDATE recurring_occurrences SET transaction_id=$3 WHERE recurring_id=$1 AND occurrence_date=$2")
                    .bind(rule.id).bind(next).bind(id).execute(&mut *tx).await?;
                generated += 1;
            }
            next = crate::services::next_month(next, rule.day_of_month as u32)?;
        }
        sqlx::query("UPDATE recurring_transactions SET next_date=$2,updated_at=now() WHERE id=$1")
            .bind(rule.id)
            .bind(next)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(generated)
}
