//! SVG geometry is derived only from API aggregates; money itself remains Decimal.
use crate::{components::EmptyState, utils::*};
use leptos::prelude::*;
use spendwise_shared::{BalancePoint, CategoryTotal, MonthlyTotal};

#[component]
pub fn DonutChart(data: Vec<CategoryTotal>, currency: String) -> impl IntoView {
    let total: f64 = data.iter().map(|d| number(d.total)).sum();
    if total <= 0.0 {
        return view! { <EmptyState title="Расходов пока нет" text="Добавьте первую расходную операцию, чтобы увидеть распределение по категориям." /> }.into_any();
    }
    let circumference = std::f64::consts::TAU * 70.0;
    let mut offset = 0.0;
    let circles = data.iter().map(|d| {
        let length = number(d.total)/total*circumference;
        let start = offset; offset += length;
        view! { <circle cx="100" cy="100" r="70" fill="none" stroke=d.color.clone() stroke-width="25"
            stroke-dasharray=format!("{length} {}",circumference-length) stroke-dashoffset=(-start).to_string() transform="rotate(-90 100 100)"><title>{format!("{}: {}",d.name,money(d.total,&currency))}</title></circle> }
    }).collect_view();
    view! { <div class="flex flex-col items-center gap-4 p-5 sm:flex-row lg:flex-col xl:flex-row"><svg viewBox="0 0 200 200" class="w-44 shrink-0" role="img" aria-label="Распределение расходов по категориям">{circles}<text x="100" y="96" text-anchor="middle" class="fill-slate-500 text-[11px] dark:fill-slate-400">"Категорий"</text><text x="100" y="119" text-anchor="middle" class="fill-slate-900 text-xl font-semibold dark:fill-slate-100">{data.len()}</text></svg><div class="w-full min-w-0 space-y-3">{data.into_iter().map(|d| view! { <div class="flex min-w-0 items-center justify-between gap-3 text-xs"><span class="flex min-w-0 items-center gap-2"><span class="size-2.5 shrink-0 rounded-full" style=format!("background-color:{}",d.color)></span><span class="truncate">{d.name}</span></span><span class="shrink-0 font-medium">{money(d.total,&currency)}</span></div> }).collect_view()}</div></div> }.into_any()
}

#[component]
pub fn BarChart(data: Vec<MonthlyTotal>, currency: String) -> impl IntoView {
    if data.is_empty() {
        return view! { <EmptyState title="Здесь появится история" text="Ваши доходы и расходы будут отображаться на графике по месяцам." /> }.into_any();
    }
    let max = data
        .iter()
        .map(|d| number(d.income).max(number(d.expenses)))
        .fold(1.0_f64, f64::max);
    let width = 500.0 / data.len().max(1) as f64;
    let bar_width = (width * 0.28).min(24.0);
    view! { <div class="px-3 pb-3 pt-6 sm:px-5"><div class="mb-4 flex justify-end gap-4 text-xs text-slate-500"><span class="flex items-center gap-1.5"><span class="size-2 rounded-full bg-brand-500"></span>"Доходы"</span><span class="flex items-center gap-1.5"><span class="size-2 rounded-full bg-slate-300 dark:bg-slate-600"></span>"Расходы"</span></div><svg viewBox="0 0 580 225" class="w-full" role="img" aria-label="Доходы и расходы по месяцам">
        {(0..=4).map(|i| { let y=20.0+i as f64*40.0; view! { <line x1="65" y1=y.to_string() x2="570" y2=y.to_string() class="stroke-slate-100 dark:stroke-slate-800" /><text x="58" y=(y+4.0).to_string() text-anchor="end" class="fill-slate-400 text-[10px]">{compact(max*(4-i) as f64/4.0)}</text> } }).collect_view()}
        {data.into_iter().enumerate().map(|(i,d)| { let center=65.0+width*(i as f64+0.5); let income=number(d.income)/max*160.0; let expense=number(d.expenses)/max*160.0; view! { <g><rect x=(center-bar_width-2.0).to_string() y=(180.0-income).to_string() width=bar_width.to_string() height=income.to_string() rx="4" class="fill-brand-500"><title>{format!("{}: доходы {}",d.month.format("%m.%Y"),money(d.income,&currency))}</title></rect><rect x=(center+2.0).to_string() y=(180.0-expense).to_string() width=bar_width.to_string() height=expense.to_string() rx="4" class="fill-slate-300 dark:fill-slate-600"><title>{format!("{}: расходы {}",d.month.format("%m.%Y"),money(d.expenses,&currency))}</title></rect><text x=center.to_string() y="206" text-anchor="middle" class="fill-slate-400 text-[10px]">{d.month.format("%m.%y").to_string()}</text></g> } }).collect_view()}
    </svg></div> }.into_any()
}

#[component]
pub fn LineChart(data: Vec<BalancePoint>, currency: String) -> impl IntoView {
    if data.is_empty() {
        return view! { <EmptyState title="Динамики пока нет" text="Создавайте транзакции, чтобы отслеживать изменение баланса." /> }.into_any();
    }
    let low = data
        .iter()
        .map(|d| number(d.balance))
        .fold(0.0_f64, f64::min);
    let high = data
        .iter()
        .map(|d| number(d.balance))
        .fold(1.0_f64, f64::max);
    let range = (high - low).max(1.0);
    let points = data
        .iter()
        .enumerate()
        .map(|(i, d)| {
            (
                65.0 + i as f64 / (data.len() - 1).max(1) as f64 * 495.0,
                180.0 - (number(d.balance) - low) / range * 155.0,
            )
        })
        .collect::<Vec<_>>();
    let line = points
        .iter()
        .map(|(x, y)| format!("{x},{y}"))
        .collect::<Vec<_>>()
        .join(" ");
    let area = format!(
        "65,180 {line} {},180",
        points.last().map(|p| p.0).unwrap_or(65.0)
    );
    let len = data.len();
    view! { <div class="px-3 pb-3 pt-6 sm:px-5"><svg viewBox="0 0 580 225" class="w-full" role="img" aria-label="Динамика баланса">
        {(0..=4).map(|i| { let y=25.0+i as f64*38.75; view! { <line x1="65" y1=y.to_string() x2="570" y2=y.to_string() class="stroke-slate-100 dark:stroke-slate-800" /><text x="58" y=(y+4.0).to_string() text-anchor="end" class="fill-slate-400 text-[10px]">{compact(high-range*i as f64/4.0)}</text> } }).collect_view()}
        <polygon points=area class="fill-brand-500/10" /><polyline points=line fill="none" class="stroke-brand-500" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
        {data.into_iter().zip(points).enumerate().map(|(i,(d,(x,y)))| view! { <g><circle cx=x.to_string() cy=y.to_string() r="3" class="fill-brand-500"><title>{format!("{}: {}",date(d.date),money(d.balance,&currency))}</title></circle><Show when=move || i==0 || i==len-1 || i==len/2><text x=x.to_string() y="208" text-anchor="middle" class="fill-slate-400 text-[10px]">{d.date.format("%d.%m").to_string()}</text></Show></g> }).collect_view()}
    </svg></div> }.into_any()
}
fn compact(value: f64) -> String {
    if value.abs() >= 1_000_000.0 {
        format!("{:.1}м", value / 1_000_000.0)
    } else if value.abs() >= 1000.0 {
        format!("{:.0}к", value / 1000.0)
    } else {
        format!("{value:.0}")
    }
}
