use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use std::str::FromStr;

pub fn today() -> NaiveDate {
    #[cfg(target_arch = "wasm32")]
    {
        let now = js_sys::Date::new_0();
        if let Some(date) = NaiveDate::from_ymd_opt(
            now.get_full_year() as i32,
            now.get_month() + 1,
            now.get_date(),
        ) {
            return date;
        }
    }
    Utc::now().date_naive()
}
pub fn current_month() -> String {
    today().format("%Y-%m").to_string()
}
pub fn period(month: &str) -> (String, String) {
    let start = NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d")
        .unwrap_or_else(|_| today().with_day(1).unwrap_or_else(today));
    let next = if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
    };
    let end = next.and_then(|d| d.pred_opt()).unwrap_or(start);
    (start.to_string(), end.to_string())
}
pub fn money(amount: Decimal, currency: &str) -> String {
    let raw = format!("{:.2}", amount);
    let (whole, fraction) = raw.split_once('.').unwrap_or((&raw, "00"));
    let mut grouped = String::new();
    let digits = whole.trim_start_matches('-');
    if whole.starts_with('-') {
        grouped.push('−');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push('\u{202f}');
        }
        grouped.push(c);
    }
    let unit = match currency {
        "USD" => "$",
        "EUR" => "€",
        _ => "₸",
    };
    format!("{grouped},{fraction} {unit}")
}
pub fn date(value: NaiveDate) -> String {
    value.format("%d.%m.%Y").to_string()
}
pub fn number(amount: Decimal) -> f64 {
    amount.to_f64().unwrap_or_default()
}
pub fn parse_money(value: &str) -> Result<Decimal, String> {
    let normalized = value.trim().replace(',', ".");
    let amount =
        Decimal::from_str(&normalized).map_err(|_| "Введите корректную сумму".to_string())?;
    if amount <= Decimal::ZERO
        || amount.scale() > 2
        || amount >= Decimal::from(10_000_000_000_000_000_i64)
    {
        return Err("Сумма должна быть положительной, до 16 цифр и 2 знаков после запятой".into());
    }
    Ok(amount)
}
pub fn parse_date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "Выберите корректную дату".into())
}
pub fn percent(spent: Decimal, amount: Decimal) -> f64 {
    if amount <= Decimal::ZERO {
        return 0.0;
    }
    (number(spent) / number(amount) * 100.0).max(0.0)
}
pub fn query(value: &str) -> String {
    js_sys::encode_uri_component(value)
        .as_string()
        .unwrap_or_default()
}
pub fn initials(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|s| s.chars().next())
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leap_year_period() {
        assert_eq!(
            period("2024-02"),
            ("2024-02-01".into(), "2024-02-29".into())
        );
    }
    #[test]
    fn december_period() {
        assert_eq!(period("2026-12").1, "2026-12-31");
    }
    #[test]
    fn money_is_grouped_without_float_rounding() {
        assert_eq!(
            money(Decimal::new(123456789, 2), "KZT"),
            "1\u{202f}234\u{202f}567,89 ₸"
        );
    }
    #[test]
    fn decimal_validation() {
        assert_eq!(parse_money("12,50").ok(), Some(Decimal::new(1250, 2)));
        for invalid in ["0", "-1", "1.001", "NaN", "", "10000000000000000"] {
            assert!(parse_money(invalid).is_err());
        }
    }
    #[test]
    fn overbudget_percentage_is_preserved() {
        assert_eq!(percent(Decimal::from(120), Decimal::from(100)), 120.0);
    }
}
