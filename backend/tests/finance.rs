//! Each test receives a separately migrated database from sqlx::test.
use backend::{error::AppError, repository as repo, services};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use spendwise_shared::*;
use sqlx::PgPool;
use uuid::Uuid;

fn date(value: &str) -> NaiveDate {
    value.parse().expect("test date")
}
fn money(value: &str) -> Decimal {
    value.parse().expect("test amount")
}

async fn user(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,password_hash,display_name) VALUES($1,$2,'test-only-hash','Test User')")
        .bind(id).bind(format!("{id}@example.test")).execute(pool).await.expect("insert user");
    id
}

async fn category(pool: &PgPool, owner: Uuid, kind: &str) -> Category {
    repo::create_category(
        pool,
        owner,
        &CategoryInput {
            name: "Test".into(),
            transaction_type: kind.into(),
            color: "#123456".into(),
            icon: "wallet".into(),
            archived: false,
        },
    )
    .await
    .expect("insert category")
}

fn input(category: &Category, amount: &str, transaction_date: &str) -> TransactionInput {
    TransactionInput {
        category_id: category.id,
        transaction_type: category.transaction_type.clone(),
        amount: money(amount),
        title: "Test transaction".into(),
        description: None,
        transaction_date: date(transaction_date),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn ownership_covers_reads_writes_exports_and_category_links(pool: PgPool) {
    let alice = user(&pool).await;
    let bob = user(&pool).await;
    let alice_category = category(&pool, alice, "expense").await;
    let bob_category = category(&pool, bob, "expense").await;
    let original = input(&alice_category, "42.50", "2026-10-10");
    let item = repo::create_transaction(&pool, alice, &original)
        .await
        .expect("create transaction");
    assert!(matches!(
        repo::transaction(&pool, bob, item.id).await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        repo::update_transaction(
            &pool,
            bob,
            item.id,
            &input(&bob_category, "1", "2026-10-10")
        )
        .await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        repo::delete_transaction(&pool, bob, item.id).await,
        Err(AppError::NotFound)
    ));
    assert!(
        repo::transactions(&pool, bob, &TransactionQuery::default())
            .await
            .expect("list")
            .items
            .is_empty()
    );
    assert!(
        repo::export_transactions(&pool, bob, &TransactionQuery::default())
            .await
            .expect("export")
            .is_empty()
    );
    assert!(matches!(
        repo::create_transaction(&pool, bob, &original).await,
        Err(AppError::Validation(_))
    ));
    // Even bypassing services cannot evade the composite owner foreign key.
    let bypass = sqlx::query("INSERT INTO transactions(id,user_id,category_id,transaction_type,amount,title,transaction_date) VALUES($1,$2,$3,'expense',10,'bypass','2026-10-10')")
        .bind(Uuid::new_v4()).bind(bob).bind(alice_category.id).execute(&pool).await;
    assert!(bypass.is_err());
    assert_eq!(
        repo::transaction(&pool, alice, item.id)
            .await
            .expect("unchanged")
            .amount,
        money("42.50")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn analytics_and_budgets_aggregate_real_period_transactions(pool: PgPool) {
    let owner = user(&pool).await;
    let salary = category(&pool, owner, "income").await;
    let expense = category(&pool, owner, "expense").await;
    for entry in [
        input(&salary, "1000", "2026-09-01"),
        input(&salary, "2000", "2026-10-02"),
        input(&expense, "300", "2026-10-10"),
        input(&expense, "200", "2026-10-15"),
        input(&expense, "50", "2026-11-01"),
    ] {
        repo::create_transaction(&pool, owner, &entry)
            .await
            .expect("insert transaction");
    }
    let period = PeriodQuery {
        from: Some(date("2026-10-01")),
        to: Some(date("2026-10-31")),
    };
    let summary = repo::summary(&pool, owner, &period).await.expect("summary");
    assert_eq!(summary.income, money("2000"));
    assert_eq!(summary.expenses, money("500"));
    assert_eq!(summary.savings, money("1500"));
    assert_eq!(summary.balance, money("2500"));
    let global = repo::create_budget(
        &pool,
        owner,
        &BudgetInput {
            category_id: None,
            month: date("2026-10-01"),
            amount: money("600"),
        },
    )
    .await
    .expect("global budget");
    let limit = repo::create_budget(
        &pool,
        owner,
        &BudgetInput {
            category_id: Some(expense.id),
            month: date("2026-10-01"),
            amount: money("400"),
        },
    )
    .await
    .expect("category budget");
    assert_eq!(global.spent, money("500"));
    assert_eq!(limit.spent, money("500"));
    assert!(limit.spent > limit.amount);
    let categories = repo::category_totals(&pool, owner, &period)
        .await
        .expect("category totals");
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].total, money("500"));
    let monthly = repo::monthly_totals(&pool, owner, date("2026-10-01"), date("2026-12-31"))
        .await
        .expect("monthly totals");
    assert_eq!(monthly.len(), 3);
    assert_eq!(monthly[0].income, money("2000"));
    assert_eq!(monthly[0].expenses, money("500"));
    assert_eq!(monthly[2].income, Decimal::ZERO);
    let balance = repo::balance_points(&pool, owner, date("2026-10-01"), date("2026-10-31"))
        .await
        .expect("balance points");
    assert_eq!(balance.len(), 31);
    assert_eq!(balance[0].balance, money("1000"));
    assert_eq!(balance.last().expect("last point").balance, money("2500"));
    // Budget totals follow edits and deletes rather than cached counters.
    let entries = repo::transactions(
        &pool,
        owner,
        &TransactionQuery {
            transaction_type: Some("expense".into()),
            from: period.from,
            to: period.to,
            ..Default::default()
        },
    )
    .await
    .expect("expense list");
    repo::delete_transaction(&pool, owner, entries.items[0].id)
        .await
        .expect("delete expense");
    assert_eq!(
        repo::budgets(&pool, owner, Some(date("2026-10-01")))
            .await
            .expect("budgets")[0]
            .spent,
        money("300")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn recurring_is_concurrent_idempotent_and_preserves_anchor(pool: PgPool) {
    let owner = user(&pool).await;
    let expense = category(&pool, owner, "expense").await;
    let mut rule = RecurringInput {
        category_id: expense.id,
        transaction_type: "expense".into(),
        amount: money("10"),
        title: "Monthly subscription".into(),
        description: None,
        next_date: date("2024-01-31"),
        active: true,
    };
    let recurring = repo::create_recurring(&pool, owner, &rule)
        .await
        .expect("create recurring");
    let (first, second) = tokio::join!(
        services::process_due(&pool, date("2024-03-31")),
        services::process_due(&pool, date("2024-03-31"))
    );
    assert_eq!(
        first.expect("first scheduler") + second.expect("second scheduler"),
        3
    );
    assert_eq!(
        services::process_due(&pool, date("2024-03-31"))
            .await
            .expect("repeat scheduler"),
        0
    );
    let rows = repo::transactions(
        &pool,
        owner,
        &TransactionQuery {
            sort: Some("date_asc".into()),
            ..Default::default()
        },
    )
    .await
    .expect("generated entries");
    assert_eq!(
        rows.items
            .iter()
            .map(|r| r.transaction_date.to_string())
            .collect::<Vec<_>>(),
        ["2024-01-31", "2024-02-29", "2024-03-31"]
    );
    let schedules = repo::recurring(&pool, owner).await.expect("schedules");
    assert_eq!(schedules[0].next_date, date("2024-04-30"));
    rule.next_date = schedules[0].next_date;
    rule.title = "Renamed subscription".into();
    assert_eq!(
        repo::update_recurring(&pool, owner, recurring.id, &rule)
            .await
            .expect("rename")
            .day_of_month,
        31
    );
    assert_eq!(
        services::process_due(&pool, date("2024-05-31"))
            .await
            .expect("more months"),
        2
    );
    // Deleting a generated transaction and rewinding must not recreate its occurrence.
    repo::delete_transaction(&pool, owner, rows.items[0].id)
        .await
        .expect("delete generated");
    rule.next_date = date("2024-01-31");
    repo::update_recurring(&pool, owner, recurring.id, &rule)
        .await
        .expect("rewind");
    assert_eq!(
        services::process_due(&pool, date("2024-05-31"))
            .await
            .expect("idempotent rewind"),
        0
    );
    assert_eq!(
        repo::transactions(&pool, owner, &TransactionQuery::default())
            .await
            .expect("list")
            .total,
        4
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn budget_uniqueness_types_and_archive_integrity(pool: PgPool) {
    let owner = user(&pool).await;
    let expense = category(&pool, owner, "expense").await;
    let income = category(&pool, owner, "income").await;
    let input = BudgetInput {
        category_id: None,
        month: date("2026-10-01"),
        amount: money("100"),
    };
    repo::create_budget(&pool, owner, &input)
        .await
        .expect("create global");
    assert!(matches!(
        repo::create_budget(&pool, owner, &input).await,
        Err(AppError::Conflict(_))
    ));
    assert!(
        repo::create_budget(
            &pool,
            owner,
            &BudgetInput {
                category_id: Some(income.id),
                ..input.clone()
            }
        )
        .await
        .is_err()
    );
    repo::create_budget(
        &pool,
        owner,
        &BudgetInput {
            category_id: Some(expense.id),
            ..input
        },
    )
    .await
    .expect("expense budget");
    assert!(
        repo::update_category(
            &pool,
            owner,
            expense.id,
            &CategoryInput {
                name: expense.name.clone(),
                transaction_type: "income".into(),
                color: expense.color.clone(),
                icon: expense.icon.clone(),
                archived: false
            }
        )
        .await
        .is_err()
    );
    let transaction =
        repo::create_transaction(&pool, owner, &self::input(&expense, "20", "2026-10-01"))
            .await
            .expect("transaction");
    let rule = repo::create_recurring(
        &pool,
        owner,
        &RecurringInput {
            category_id: expense.id,
            transaction_type: "expense".into(),
            amount: money("10"),
            title: "Archived schedule".into(),
            description: None,
            next_date: date("2026-10-31"),
            active: true,
        },
    )
    .await
    .expect("recurring");
    repo::archive_category(&pool, owner, expense.id)
        .await
        .expect("archive");
    assert_eq!(
        repo::transaction(&pool, owner, transaction.id)
            .await
            .expect("history preserved")
            .amount,
        money("20")
    );
    assert!(
        !repo::categories(&pool, owner, false)
            .await
            .expect("categories")
            .iter()
            .any(|c| c.id == expense.id)
    );
    assert!(
        repo::create_transaction(&pool, owner, &self::input(&expense, "20", "2026-10-02"))
            .await
            .is_err()
    );
    assert!(
        !repo::recurring(&pool, owner)
            .await
            .expect("recurring")
            .iter()
            .find(|r| r.id == rule.id)
            .expect("rule")
            .active
    );
    assert_eq!(
        services::process_due(&pool, date("2026-10-31"))
            .await
            .expect("archived schedule"),
        0
    );
    let direct_insert = sqlx::query("INSERT INTO transactions(id,user_id,category_id,transaction_type,amount,title,transaction_date) VALUES($1,$2,$3,'expense',10,'archived bypass','2026-10-10')")
        .bind(Uuid::new_v4()).bind(owner).bind(expense.id).execute(&pool).await;
    assert!(direct_insert.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn financial_write_serializes_with_currency_change(pool: PgPool) {
    let owner = user(&pool).await;
    let expense = category(&pool, owner, "expense").await;
    let mut currency_change = pool.begin().await.expect("begin currency change");
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(owner)
        .execute(&mut *currency_change)
        .await
        .expect("lock user");
    let insert_pool = pool.clone();
    let mut insert = tokio::spawn(async move {
        repo::create_transaction(&insert_pool, owner, &input(&expense, "100", "2026-10-01")).await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut insert)
            .await
            .is_err(),
        "financial insert must wait for currency lock"
    );
    let finances: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transactions WHERE user_id=$1)")
            .bind(owner)
            .fetch_one(&mut *currency_change)
            .await
            .expect("check finances");
    assert!(!finances);
    sqlx::query("UPDATE users SET currency='USD' WHERE id=$1")
        .bind(owner)
        .execute(&mut *currency_change)
        .await
        .expect("change currency");
    currency_change
        .commit()
        .await
        .expect("commit currency change");
    let created = tokio::time::timeout(std::time::Duration::from_secs(5), insert)
        .await
        .expect("insert completes")
        .expect("task")
        .expect("insert");
    assert_eq!(created.amount, money("100"));
    let currency: String = sqlx::query_scalar("SELECT currency FROM users WHERE id=$1")
        .bind(owner)
        .fetch_one(&pool)
        .await
        .expect("currency");
    assert_eq!(currency, "USD");
}

#[sqlx::test(migrations = "./migrations")]
async fn empty_analytics_include_zero_months_and_daily_opening_balance(pool: PgPool) {
    let owner = user(&pool).await;
    let summary = repo::summary(&pool, owner, &PeriodQuery::default())
        .await
        .expect("empty summary");
    assert_eq!(summary.balance, Decimal::ZERO);
    assert_eq!(summary.income, Decimal::ZERO);
    assert_eq!(summary.expenses, Decimal::ZERO);
    let monthly = repo::monthly_totals(&pool, owner, date("2026-01-01"), date("2026-03-31"))
        .await
        .expect("empty monthly");
    assert_eq!(monthly.len(), 3);
    assert!(
        monthly
            .iter()
            .all(|m| m.income == Decimal::ZERO && m.expenses == Decimal::ZERO)
    );
    let balance = repo::balance_points(&pool, owner, date("2024-02-01"), date("2024-02-29"))
        .await
        .expect("empty daily");
    assert_eq!(balance.len(), 29);
    assert!(balance.iter().all(|b| b.balance == Decimal::ZERO));
}

#[sqlx::test(migrations = "./migrations")]
async fn archive_and_scheduler_use_consistent_lock_order(pool: PgPool) {
    let owner = user(&pool).await;
    let expense = category(&pool, owner, "expense").await;
    let rule = repo::create_recurring(
        &pool,
        owner,
        &RecurringInput {
            category_id: expense.id,
            transaction_type: "expense".into(),
            amount: money("10"),
            title: "Concurrent subscription".into(),
            description: None,
            next_date: date("2026-10-31"),
            active: true,
        },
    )
    .await
    .expect("recurring rule");
    // Hold the same row lock as the scheduler before allowing archive to begin.
    let mut scheduler = pool.begin().await.expect("begin scheduler");
    sqlx::query("SELECT id FROM recurring_transactions WHERE id=$1 FOR UPDATE")
        .bind(rule.id)
        .execute(&mut *scheduler)
        .await
        .expect("lock due rule");
    let archive_pool = pool.clone();
    let mut archive =
        tokio::spawn(async move { repo::archive_category(&archive_pool, owner, expense.id).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut archive)
            .await
            .is_err(),
        "archive must wait for the scheduler's rule lock"
    );
    // An archive that locked the category first would now deadlock with this insert.
    tokio::time::timeout(std::time::Duration::from_secs(3),sqlx::query("INSERT INTO transactions(id,user_id,category_id,transaction_type,amount,title,transaction_date) VALUES($1,$2,$3,'expense',10,'Scheduled payment','2026-10-31')")
        .bind(Uuid::new_v4()).bind(owner).bind(expense.id).execute(&mut *scheduler)).await.expect("scheduler must not wait on category archive").expect("insert generated transaction");
    scheduler.commit().await.expect("commit scheduler");
    tokio::time::timeout(std::time::Duration::from_secs(3), archive)
        .await
        .expect("archive completes")
        .expect("archive task")
        .expect("archive result");
    assert!(!repo::recurring(&pool, owner).await.expect("rules")[0].active);
}
