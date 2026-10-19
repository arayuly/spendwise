use super::*;
use crate::{app::AppContext, charts::*, components::*, utils::*};
use leptos::prelude::*;
use spendwise_shared::*;

#[component]
pub fn DashboardPage(analytics: bool) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let month = RwSignal::new(current_month());
    let mode = RwSignal::new("month".to_string());
    let dates = move || {
        let m = month.get();
        let (from, to) = period(&m);
        if mode.get() == "year" {
            let year = &from[..4];
            (format!("{year}-01-01"), format!("{year}-12-31"))
        } else {
            (from, to)
        }
    };
    let summary = use_api::<AnalyticsSummary>(move || {
        let (f, t) = dates();
        format!("/analytics/summary?from={f}&to={t}")
    });
    let category_totals = use_api::<Vec<CategoryTotal>>(move || {
        let (f, t) = dates();
        format!("/analytics/categories?from={f}&to={t}")
    });
    let monthly = use_api::<Vec<MonthlyTotal>>(move || {
        let (f, _) = dates();
        let year = &f[..4];
        format!("/analytics/monthly?from={year}-01-01&to={year}-12-31")
    });
    let balance = use_api::<Vec<BalancePoint>>(move || {
        let (f, t) = dates();
        format!("/analytics/balance?from={f}&to={t}")
    });
    let categories = use_api::<Vec<Category>>(|| "/categories?include_archived=true".into());
    let transactions = use_api::<Paginated<Transaction>>(move || {
        let (f, t) = dates();
        format!("/transactions?from={f}&to={t}&per_page=5&sort=date_desc")
    });
    let budgets =
        use_api::<Vec<Budget>>(move || format!("/budgets?month={}", period(&month.get()).0));
    let recurring = use_api::<Vec<Recurring>>(|| "/recurring".into());
    let currency = ctx.currency();
    let actions = TransactionActions::new();
    let edit = actions.edit();
    let remove = actions.remove();
    let inspect = actions.inspect();
    let nav = ctx.clone();
    let nav2 = ctx.clone();
    let nav3 = ctx.clone();
    let name = ctx
        .api
        .session
        .get_untracked()
        .map(|s| {
            s.user
                .display_name
                .split_whitespace()
                .next()
                .unwrap_or("друг")
                .to_string()
        })
        .unwrap_or_default();
    let title = if analytics {
        "Финансы в деталях".into()
    } else {
        format!("Добрый день, {name}!")
    };
    view! {<PageHeading title=title description=if analytics {"Замечайте закономерности и принимайте взвешенные финансовые решения."}else{"Ваши деньги, планы и привычки — всё в одном обзоре."}><label class="sr-only" for="dashboard-period">"Месяц и год"</label><input id="dashboard-period" aria-label="Месяц и год" type="month" class="input w-auto" prop:value=move ||month.get() on:change=move |e|{let v=event_target_value(&e);if !v.is_empty(){month.set(v);}} /><label class="sr-only" for="period-mode">"Период аналитики"</label><select id="period-mode" class="input w-auto" prop:value=move ||mode.get() on:change=move |e|mode.set(event_target_value(&e))><option value="month">"За месяц"</option><option value="year">"За год"</option></select></PageHeading>
        <LoadError error=summary.error />
        {move ||if summary.loading.get(){view!{<Skeleton />}.into_any()}else{summary.data.get().map(|s|view!{<div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-4"><StatCard label="Общий баланс" value=money(s.balance,&currency) icon="wallet" highlighted=true caption="На конец выбранного периода" /><StatCard label="Доходы" value=money(s.income,&currency) icon="arrow-down" caption="За выбранный период" /><StatCard label="Расходы" value=money(s.expenses,&currency) icon="arrow-up" caption="За выбранный период" /><StatCard label="Сбережения" value=money(s.savings,&currency) icon="budgets" caption="Доходы минус расходы периода" /></div>}).into_any()}}
        <div class="mt-6 grid gap-5 xl:grid-cols-[1.6fr_1fr]"><section class="panel"><div class="panel-head"><h2 class="section-title">"Доходы и расходы"</h2><span class="text-xs text-slate-400">{move ||format!("В течение {} года",&period(&month.get()).0[..4])}</span></div><LoadError error=monthly.error />{move ||if monthly.loading.get(){view!{<LoadingSpinner />}.into_any()}else{view!{<BarChart data=monthly.data.get().unwrap_or_default() currency=ctx.currency() />}.into_any()}}</section><section class="panel"><div class="panel-head"><h2 class="section-title">"Расходы по категориям"</h2></div><LoadError error=category_totals.error />{move ||if category_totals.loading.get(){view!{<LoadingSpinner />}.into_any()}else{view!{<DonutChart data=category_totals.data.get().unwrap_or_default() currency={expect_context::<AppContext>().currency()} />}.into_any()}}</section></div>
        <Show when=move ||analytics><section class="panel mt-5"><div class="panel-head"><h2 class="section-title">"Динамика баланса"</h2><span class="text-xs text-slate-400">"За выбранный период"</span></div><LoadError error=balance.error />{move ||if balance.loading.get(){view!{<LoadingSpinner />}.into_any()}else{view!{<LineChart data=balance.data.get().unwrap_or_default() currency={expect_context::<AppContext>().currency()} />}.into_any()}}</section></Show>
        <div class="mt-5 grid gap-5 xl:grid-cols-[1.6fr_1fr]"><section class="panel"><div class="panel-head"><h2 class="section-title">"Последние транзакции"</h2><button class="text-xs font-semibold text-brand-600 hover:underline dark:text-brand-400" on:click=move |_|nav.navigate("/transactions")>"Все операции →"</button></div><LoadError error=transactions.error />{move ||if transactions.loading.get(){view!{<LoadingSpinner />}.into_any()}else{let items=transactions.data.get().map(|d|d.items).unwrap_or_default();if items.is_empty(){view!{<EmptyState title="Начните с первой операции" text="Добавьте доход или расход. SpendWise соберёт картину ваших финансов." />}.into_any()}else{view!{<TransactionTable transactions=items categories=categories.data.get().unwrap_or_default() edit=edit remove=remove inspect=inspect />}.into_any()}}}</section>
        <section class="panel"><div class="panel-head"><h2 class="section-title">"Месячный бюджет"</h2><button class="text-xs font-semibold text-brand-600 hover:underline dark:text-brand-400" on:click=move |_|nav2.navigate("/budgets")>"Управление →"</button></div><LoadError error=budgets.error />{move ||if budgets.loading.get(){view!{<LoadingSpinner />}.into_any()}else{let items=budgets.data.get().unwrap_or_default();if items.is_empty(){view!{<EmptyState title="Планируйте с уверенностью" text="Установите бюджет на месяц или лимиты по категориям, чтобы держать расходы под контролем." />}.into_any()}else{view!{<div class="space-y-6 p-5">{items.into_iter().take(5).map(|b|{let label=b.category_id.and_then(|id|categories.data.get_untracked().unwrap_or_default().into_iter().find(|c|c.id==id)).map(|c|c.name).unwrap_or_else(||"Общий бюджет".into());view!{<BudgetProgress budget=b label=label currency={expect_context::<AppContext>().currency()} />}}).collect_view()}</div>}.into_any()}}}</section></div>
        <section class="panel mt-5"><div class="panel-head"><h2 class="section-title">"Ближайшие регулярные платежи"</h2><button class="text-xs font-semibold text-brand-600 hover:underline dark:text-brand-400" on:click=move |_|nav3.navigate("/recurring")>"Все платежи →"</button></div><LoadError error=recurring.error />{move ||if recurring.loading.get(){view!{<LoadingSpinner />}.into_any()}else{let mut items=recurring.data.get().unwrap_or_default().into_iter().filter(|r|r.active).collect::<Vec<_>>();items.sort_by_key(|r|r.next_date);if items.is_empty(){view!{<EmptyState title="Регулярных платежей нет" text="Добавьте аренду, подписку или ежемесячный доход — операции будут создаваться автоматически." />}.into_any()}else{view!{<div class="grid divide-y divide-slate-100 p-2 dark:divide-slate-800 sm:grid-cols-2 sm:divide-y-0 xl:grid-cols-4">{items.into_iter().take(4).map(|r|view!{<div class="flex items-center gap-3 p-4"><span class="rounded-xl bg-slate-100 p-3 text-slate-500 dark:bg-slate-800"><Icon name="recurring" /></span><div class="min-w-0"><p class="truncate text-sm font-medium">{r.title}</p><p class="mt-1 text-xs text-slate-400">{format!("{} · {}",date(r.next_date),money(r.amount,&expect_context::<AppContext>().currency()))}</p></div></div>}).collect_view()}</div>}.into_any()}}}</section>
        {move ||view!{<TransactionModals actions=actions categories=categories.data.get().unwrap_or_default() />}}
    }
}
