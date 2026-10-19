//! Financial validation and monthly recurrence rules.
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use spendwise_shared::*;
use sqlx::PgPool;

use crate::error::{AppError, AppResult};

pub fn validate_amount(amount: Decimal) -> AppResult<()> {
    if amount <= Decimal::ZERO
        || amount >= Decimal::from(10_000_000_000_000_000_i64)
        || amount.scale() > 2
    {
        return Err(AppError::Validation("Сумма должна быть больше нуля, меньше 10^16 и содержать не более двух знаков после запятой".into()));
    }
    Ok(())
}

pub fn validate_type(value: &str) -> AppResult<()> {
    if !matches!(value, "income" | "expense") {
        return Err(AppError::Validation(
            "Тип операции должен быть income или expense".into(),
        ));
    }
    Ok(())
}

pub fn validate_date(date: NaiveDate) -> AppResult<()> {
    if !(1900..=9998).contains(&date.year()) {
        return Err(AppError::Validation(
            "Год даты должен быть от 1900 до 9998".into(),
        ));
    }
    Ok(())
}

fn clean_text(value: &mut String, name: &str, max: usize) -> AppResult<()> {
    *value = value.trim().to_owned();
    if value.is_empty() || value.chars().count() > max || value.chars().any(|c| c.is_control()) {
        return Err(AppError::Validation(format!(
            "{name}: введите от 1 до {max} символов без управляющих символов"
        )));
    }
    Ok(())
}

fn clean_description(value: &mut Option<String>) -> AppResult<()> {
    if let Some(description) = value {
        *description = description.trim().to_owned();
        if description.chars().count() > 2000
            || description
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(AppError::Validation(
                "Описание превышает 2000 символов или содержит недопустимые управляющие символы"
                    .into(),
            ));
        }
        if description.is_empty() {
            *value = None;
        }
    }
    Ok(())
}

pub fn validate_category(input: &mut CategoryInput) -> AppResult<()> {
    clean_text(&mut input.name, "Название категории", 80)?;
    clean_text(&mut input.icon, "Иконка категории", 40)?;
    validate_type(&input.transaction_type)?;
    let bytes = input.color.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' || !bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err(AppError::Validation(
            "Цвет должен соответствовать формату #RRGGBB".into(),
        ));
    }
    Ok(())
}

pub fn validate_transaction(input: &mut TransactionInput) -> AppResult<()> {
    validate_amount(input.amount)?;
    validate_type(&input.transaction_type)?;
    validate_date(input.transaction_date)?;
    clean_text(&mut input.title, "Название", 160)?;
    clean_description(&mut input.description)
}

pub fn validate_recurring(input: &mut RecurringInput) -> AppResult<()> {
    validate_amount(input.amount)?;
    validate_type(&input.transaction_type)?;
    validate_date(input.next_date)?;
    clean_text(&mut input.title, "Название", 160)?;
    clean_description(&mut input.description)
}

pub fn validate_budget(input: &BudgetInput) -> AppResult<()> {
    validate_amount(input.amount)?;
    validate_date(input.month)?;
    if input.month.day() != 1 {
        return Err(AppError::Validation(
            "Месяц бюджета должен быть указан первым днём месяца".into(),
        ));
    }
    Ok(())
}

pub fn validate_period(from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<()> {
    if let Some(from) = from {
        validate_date(from)?;
    }
    if let Some(to) = to {
        validate_date(to)?;
    }
    if matches!((from, to), (Some(from), Some(to)) if from > to) {
        return Err(AppError::Validation(
            "Начальная дата не должна быть позже конечной".into(),
        ));
    }
    Ok(())
}

pub fn validate_query(query: &mut TransactionQuery) -> AppResult<()> {
    validate_period(query.from, query.to)?;
    if let Some(kind) = &query.transaction_type {
        validate_type(kind)?;
    }
    if let Some(search) = &mut query.search {
        *search = search.trim().to_owned();
        if search.chars().count() > 200 {
            return Err(AppError::Validation(
                "Поисковый запрос ограничен 200 символами".into(),
            ));
        }
    }
    if !matches!(
        query.sort.as_deref(),
        None | Some("date_desc" | "date_asc" | "amount_desc" | "amount_asc")
    ) {
        return Err(AppError::Validation(
            "Неизвестный порядок сортировки".into(),
        ));
    }
    if !(1..=1_000_000).contains(&query.page.unwrap_or(1))
        || !(1..=100).contains(&query.per_page.unwrap_or(20))
    {
        return Err(AppError::Validation(
            "Номер страницы должен быть от 1 до 1000000, размер страницы — от 1 до 100".into(),
        ));
    }
    Ok(())
}

/// Keep the original anchor day so Jan 31 → Feb 28 → Mar 31.
pub fn next_month(date: NaiveDate, anchor: u32) -> AppResult<NaiveDate> {
    let first = date.with_day(1).ok_or(AppError::Internal)?;
    let next = first
        .checked_add_months(chrono::Months::new(1))
        .ok_or(AppError::Internal)?;
    let after = next
        .checked_add_months(chrono::Months::new(1))
        .ok_or(AppError::Internal)?;
    let last = after.pred_opt().ok_or(AppError::Internal)?.day();
    next.with_day(anchor.clamp(1, 31).min(last))
        .ok_or(AppError::Internal)
}

/// Lock due rules and atomically claim each occurrence before creating its transaction.
/// A bounded catch-up batch prevents a decades-old rule monopolizing the scheduler.
pub async fn process_due(pool: &PgPool, today: NaiveDate) -> AppResult<u64> {
    crate::repository::process_due(pool, today).await
}

/// Quote every cell and neutralize spreadsheet formulas, including leading whitespace.
pub fn csv_cell(value: &str) -> String {
    let formula = value.trim_start().starts_with(['=', '+', '-', '@'])
        || value.starts_with(['\t', '\r', '\n']);
    let prefix = if formula { "'" } else { "" };
    format!("\"{prefix}{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn money_limits_and_precision() {
        for invalid in ["0", "-1", "0.001", "10000000000000000", "1.000"] {
            assert!(validate_amount(Decimal::from_str(invalid).expect("test decimal")).is_err());
        }
        for valid in ["0.01", "100", "9999999999999999.99"] {
            assert!(validate_amount(Decimal::from_str(valid).expect("test decimal")).is_ok());
        }
    }

    #[test]
    fn recurrence_preserves_anchor_and_leap_years() {
        let jan = NaiveDate::from_ymd_opt(2024, 1, 31).expect("test date");
        let feb = next_month(jan, 31).expect("next date");
        assert_eq!(feb.to_string(), "2024-02-29");
        assert_eq!(
            next_month(feb, 31).expect("next date").to_string(),
            "2024-03-31"
        );
        assert_eq!(
            next_month(NaiveDate::from_ymd_opt(2023, 1, 31).expect("test date"), 31)
                .expect("next date")
                .to_string(),
            "2023-02-28"
        );
    }

    #[test]
    fn csv_quotes_and_formula_protection() {
        assert_eq!(csv_cell("hello, \"world\""), "\"hello, \"\"world\"\"\"");
        assert_eq!(csv_cell(" =HYPERLINK(1)"), "\"' =HYPERLINK(1)\"");
        assert_eq!(csv_cell("\tcmd"), "\"'\tcmd\"");
        assert_eq!(csv_cell("normal"), "\"normal\"");
    }

    #[test]
    fn rejects_reversed_period_and_invalid_budget_month() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 20).expect("test date");
        assert!(validate_period(Some(date), date.pred_opt()).is_err());
        assert!(
            validate_budget(&BudgetInput {
                category_id: None,
                month: date,
                amount: Decimal::ONE
            })
            .is_err()
        );
    }

    #[test]
    fn financial_inputs_reject_caller_supplied_owner() {
        fn with_owner<T: serde::Serialize>(input: &T) -> serde_json::Value {
            let mut value = serde_json::to_value(input).expect("serialize input");
            value
                .as_object_mut()
                .expect("input object")
                .insert("user_id".into(), serde_json::json!(uuid::Uuid::new_v4()));
            value
        }
        let category = uuid::Uuid::new_v4();
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).expect("test date");
        assert!(
            serde_json::from_value::<CategoryInput>(with_owner(&CategoryInput {
                name: "Test".into(),
                transaction_type: "expense".into(),
                color: "#123456".into(),
                icon: "wallet".into(),
                archived: false
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<TransactionInput>(with_owner(&TransactionInput {
                category_id: category,
                transaction_type: "expense".into(),
                amount: Decimal::ONE,
                title: "Test".into(),
                description: None,
                transaction_date: date
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<BudgetInput>(with_owner(&BudgetInput {
                category_id: Some(category),
                month: date,
                amount: Decimal::ONE
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<RecurringInput>(with_owner(&RecurringInput {
                category_id: category,
                transaction_type: "expense".into(),
                amount: Decimal::ONE,
                title: "Test".into(),
                description: None,
                next_date: date,
                active: true
            }))
            .is_err()
        );
    }
}
