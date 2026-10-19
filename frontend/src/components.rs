use crate::{app::AppContext, utils::*};
use leptos::prelude::*;
use spendwise_shared::{Budget, Category, Transaction};

#[component]
pub fn Icon(
    #[prop(into)] name: String,
    #[prop(default = "size-5")] class: &'static str,
) -> impl IntoView {
    let path = match name.as_str() {
        "dashboard" => "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
        "transactions" => "M4 7h15m-4-4 4 4-4 4M20 17H5m4-4-4 4 4 4",
        "categories" => "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M17.5 14v7M14 17.5h7",
        "budgets" => "M3 7h18v14H3zM3 7V4h15v3M16 13h5v4h-5z",
        "recurring" => "M20 7a8 8 0 0 0-14-3L3 7m0-4v4h4M4 17a8 8 0 0 0 14 3l3-3m0 4v-4h-4",
        "analytics" => "M4 20h17M7 16v-5M12 16V5M17 16V8",
        "profile" => "M20 21v-2a7 7 0 0 0-14 0v2M17 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0",
        "plus" => "M12 5v14M5 12h14",
        "close" => "m6 6 12 12M6 18 18 6",
        "menu" => "M4 6h16M4 12h16M4 18h16",
        "arrow-up" => "M12 19V5m-6 6 6-6 6 6",
        "arrow-down" => "M12 5v14m-6-6 6 6 6-6",
        "edit" => "m16 3 5 5-12 12H4v-5zM14 5l5 5",
        "trash" => "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7",
        "download" => "M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5",
        "logout" => "M9 4H4v16h5M12 12h9m-4-4 4 4-4 4",
        "check" => "m5 12 4 4L19 6",
        "chevron" => "m9 5 7 7-7 7",
        "search" => "M21 21l-5-5M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
        "shield" => "m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6z m-4 9 3 3 5-6",
        "wallet" => "M3 7h18v14H3zM3 7V4h15v3M16 12h5v5h-5z",
        "sun" => {
            "M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5 19 19M5 19l1.5-1.5M17.5 6.5 19 5M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0"
        }
        "warning" => "m12 3 10 18H2zM12 9v5M12 17h.01",
        "pause" => "M8 5v14M16 5v14",
        "play" => "m7 4 14 8-14 8z",
        _ => "M5 4h14v16H5zM8 9h8M8 13h8M8 17h4",
    };
    view! { <svg class=class viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d=path /></svg> }
}

#[component]
pub fn Button(
    #[prop(optional)] on_click: Option<Callback<()>>,
    #[prop(default = "btn-primary")] class: &'static str,
    #[prop(optional)] busy: Signal<bool>,
    #[prop(default = "button")] kind: &'static str,
    children: Children,
) -> impl IntoView {
    view! { <button type=kind class=class disabled=move || busy.get() on:click=move |_| { if let Some(cb) = on_click { cb.run(()); } }>
        <Show when=move || busy.get()><span class="spinner" aria-hidden="true"></span></Show>{children()}
    </button> }
}

#[component]
pub fn Input(
    #[prop(into)] label: String,
    #[prop(into)] id: String,
    value: RwSignal<String>,
    #[prop(default = "text")] kind: &'static str,
    #[prop(default = false)] required: bool,
    #[prop(default = "")] autocomplete: &'static str,
    #[prop(default = "")] placeholder: &'static str,
    #[prop(default = 255)] maxlength: u32,
    #[prop(optional)] min: Option<&'static str>,
) -> impl IntoView {
    view! { <label class="field" for=id.clone()><span>{label}</span><input id=id.clone() type=kind class="input" required=required autocomplete=autocomplete
    placeholder=placeholder maxlength=maxlength min=min prop:value=move || value.get() on:input=move |e| value.set(event_target_value(&e)) /></label> }
}

#[component]
pub fn Select(
    #[prop(into)] label: String,
    #[prop(into)] id: String,
    value: RwSignal<String>,
    children: Children,
) -> impl IntoView {
    view! { <label class="field" for=id.clone()><span>{label}</span><select id=id.clone() class="input" prop:value=move || value.get() on:change=move |e| value.set(event_target_value(&e))>{children()}</select></label> }
}

#[component]
pub fn Textarea(
    #[prop(into)] label: String,
    #[prop(into)] id: String,
    value: RwSignal<String>,
) -> impl IntoView {
    view! { <label class="field" for=id.clone()><span>{label}</span><textarea id=id.clone() class="input min-h-24 resize-y" maxlength="2000" prop:value=move || value.get() on:input=move |e| value.set(event_target_value(&e))></textarea></label> }
}

#[component]
pub fn LoadingSpinner() -> impl IntoView {
    view! { <div class="flex items-center justify-center gap-3 p-10 text-slate-500" role="status"><span class="spinner text-brand-600"></span>"Загрузка…"</div> }
}

#[component]
pub fn Skeleton() -> impl IntoView {
    view! { <div class="grid gap-5 sm:grid-cols-2 xl:grid-cols-4" aria-label="Загрузка данных" role="status">{(0..4).map(|_| view! { <div class="card animate-pulse"><div class="h-3 w-24 rounded bg-slate-200 dark:bg-slate-700"></div><div class="mt-5 h-8 w-36 rounded bg-slate-100 dark:bg-slate-800"></div></div> }).collect_view()}</div> }
}

#[component]
pub fn EmptyState(#[prop(into)] title: String, #[prop(into)] text: String) -> impl IntoView {
    view! { <div class="py-12 px-4 text-center"><span class="mx-auto mb-4 flex size-12 items-center justify-center rounded-2xl bg-brand-50 text-brand-600 dark:bg-brand-950/40"><Icon name="wallet" /></span><h3 class="font-semibold">{title}</h3><p class="mx-auto mt-2 max-w-sm text-sm leading-6 text-slate-500 dark:text-slate-400">{text}</p></div> }
}

#[component]
pub fn Alert(#[prop(into)] text: String) -> impl IntoView {
    view! { <div class="flex items-start gap-3 rounded-xl border border-red-200 bg-red-50 p-4 text-sm leading-6 text-red-800 dark:border-red-900 dark:bg-red-950/30 dark:text-red-300" role="alert"><Icon name="warning" class="mt-0.5 size-5 shrink-0" /><span>{text}</span></div> }
}

#[component]
pub fn Badge(#[prop(default = false)] income: bool) -> impl IntoView {
    view! { <span class=if income { "badge bg-emerald-50 text-emerald-700 dark:bg-emerald-950/50 dark:text-emerald-400" } else { "badge bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300" }>{if income { "Доход" } else { "Расход" }}</span> }
}

#[component]
pub fn StatCard(
    #[prop(into)] label: String,
    #[prop(into)] value: String,
    #[prop(into)] icon: String,
    #[prop(default = false)] highlighted: bool,
    #[prop(into)] caption: String,
) -> impl IntoView {
    view! { <article class=if highlighted { "rounded-2xl bg-brand-700 p-5 text-white shadow-sm" } else { "card" }>
        <div class="flex items-center justify-between gap-2"><h2 class=if highlighted { "text-sm text-emerald-100" } else { "text-sm text-slate-500 dark:text-slate-400" }>{label}</h2><span class=if highlighted { "rounded-xl bg-white/10 p-2" } else { "rounded-xl bg-brand-50 p-2 text-brand-600 dark:bg-brand-950/40 dark:text-brand-400" }><Icon name=icon /></span></div>
        <p class="mt-3 break-words text-2xl font-semibold tracking-tight sm:text-[1.65rem]">{value}</p><p class=if highlighted { "mt-2 text-xs text-emerald-100" } else { "mt-2 text-xs text-slate-500 dark:text-slate-400" }>{caption}</p>
    </article> }
}

#[component]
pub fn Modal(
    #[prop(into)] title: String,
    close: Callback<()>,
    children: Children,
) -> impl IntoView {
    let dialog = NodeRef::<leptos::html::Dialog>::new();
    Effect::new(move |_| {
        if let Some(element) = dialog.get() {
            let _ = element.show_modal();
        }
    });
    view! { <dialog node_ref=dialog class="modal" aria-labelledby="modal-title" on:cancel=move |e: web_sys::Event| { e.prevent_default(); close.run(()); }>
        <div class="flex items-center justify-between gap-4 border-b border-slate-200 px-5 py-4 dark:border-slate-800"><h2 id="modal-title" class="text-lg font-semibold">{title}</h2><button type="button" class="icon-btn" aria-label="Закрыть" on:click=move |_| close.run(())><Icon name="close" /></button></div><div class="p-5">{children()}</div>
    </dialog> }
}

#[component]
pub fn ConfirmDialog(
    #[prop(into)] title: String,
    #[prop(into)] text: String,
    close: Callback<()>,
    confirm: Callback<()>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    view! { <Modal title=title close=close><p class="text-sm leading-6 text-slate-500 dark:text-slate-400">{text}</p><div class="mt-6 flex flex-wrap justify-end gap-3"><Button class="btn-secondary" on_click=close>"Отмена"</Button><Button class="btn-danger" busy=Signal::derive(move || busy.get()) on_click=confirm>"Подтвердить"</Button></div></Modal> }
}

#[component]
pub fn Dropdown(
    #[prop(into)] label: String,
    value: RwSignal<String>,
    on_change: Callback<String>,
    children: Children,
) -> impl IntoView {
    view! {<select class="max-w-28 cursor-pointer rounded-lg border-0 bg-transparent py-2 text-xs focus:ring-2 focus:ring-brand-500" aria-label=label prop:value=move ||value.get() on:change=move |e|on_change.run(event_target_value(&e))>{children()}</select>}
}

#[component]
pub fn ThemeToggle() -> impl IntoView {
    let saved = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("spendwise-theme").ok().flatten())
        .unwrap_or_else(|| "system".into());
    let theme = RwSignal::new(saved);
    let change = Callback::new(move |value: String| {
        theme.set(value.clone());
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.set_item("spendwise-theme", &value);
            }
            if let Ok(event) = web_sys::Event::new("spendwise-theme") {
                let _ = window.dispatch_event(&event);
            }
        }
    });
    view! { <div class="flex items-center gap-2 text-sm text-slate-500 dark:text-slate-400"><Icon name="sun" /><Dropdown label="Тема оформления" value=theme on_change=change><option value="system">"Системная"</option><option value="light">"Светлая"</option><option value="dark">"Тёмная"</option></Dropdown></div> }
}

#[component]
pub fn BudgetProgress(
    budget: Budget,
    #[prop(into)] label: String,
    #[prop(into)] currency: String,
) -> impl IntoView {
    let pct = percent(budget.spent, budget.amount);
    let color = if pct >= 100.0 {
        "h-full rounded-full bg-red-500"
    } else if pct >= 80.0 {
        "h-full rounded-full bg-amber-500"
    } else {
        "h-full rounded-full bg-brand-500"
    };
    let remaining = budget.amount - budget.spent;
    view! { <div><div class="mb-2 flex flex-wrap items-center justify-between gap-2 text-sm"><span class="font-medium">{label}</span><span class="text-slate-500 dark:text-slate-400">{format!("{:.0}%",pct)}</span></div>
        <div class="h-2 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800" role="progressbar" aria-label="Использование бюджета" aria-valuenow=pct.min(100.0).to_string() aria-valuemin="0" aria-valuemax="100"><div class=color style=format!("width:{}%",pct.min(100.0))></div></div>
        <div class="mt-2 flex flex-wrap justify-between gap-1 text-xs text-slate-500 dark:text-slate-400"><span>{format!("{} из {}",money(budget.spent,&currency),money(budget.amount,&currency))}</span><span class={if pct>=100.0 { "font-medium text-red-600 dark:text-red-400" } else { "" }}>{if remaining < rust_decimal::Decimal::ZERO { format!("Превышение на {}",money(-remaining,&currency)) } else { format!("Осталось {}",money(remaining,&currency)) }}</span></div>
        <Show when=move || { pct>=80.0 }><p class={if pct>=100.0 { "mt-2 text-xs text-red-600 dark:text-red-400" } else { "mt-2 text-xs text-amber-600 dark:text-amber-400" }}>{if pct>=100.0 { "Лимит бюджета достигнут" } else { "Вы приближаетесь к лимиту" }}</p></Show>
    </div> }
}

#[component]
pub fn TransactionCard(
    transaction: Transaction,
    categories: Vec<Category>,
    currency: String,
    edit: Callback<Transaction>,
    remove: Callback<Transaction>,
    inspect: Callback<Transaction>,
) -> impl IntoView {
    let category = categories
        .iter()
        .find(|c| c.id == transaction.category_id)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "Архивная категория".into());
    let income = transaction.transaction_type == "income";
    let tx = transaction.clone();
    let tx2 = transaction.clone();
    let tx3 = transaction.clone();
    view! { <article class="border-b border-slate-100 p-4 last:border-0 dark:border-slate-800"><div class="flex min-w-0 items-start justify-between gap-3"><div class="min-w-0"><button type="button" class="break-words text-left text-sm font-medium hover:text-brand-600" on:click=move |_| inspect.run(tx.clone())>{transaction.title}</button><p class="mt-1 text-xs text-slate-500 dark:text-slate-400">{format!("{} · {}",category,date(transaction.transaction_date))}</p></div><span class=if income { "shrink-0 text-sm font-semibold text-brand-600 dark:text-brand-400" } else { "shrink-0 text-sm font-semibold" }>{format!("{}{}",if income {"+"} else {"−"}, money(transaction.amount,&currency))}</span></div><div class="mt-3 flex items-center justify-between"><Badge income=income /><div class="flex gap-1"><button type="button" class="icon-btn" aria-label="Изменить транзакцию" on:click=move |_| edit.run(tx2.clone())><Icon name="edit" class="size-4" /></button><button type="button" class="icon-btn hover:text-red-600" aria-label="Удалить транзакцию" on:click=move |_| remove.run(tx3.clone())><Icon name="trash" class="size-4" /></button></div></div></article> }
}

#[component]
pub fn TransactionTable(
    transactions: Vec<Transaction>,
    categories: Vec<Category>,
    edit: Callback<Transaction>,
    remove: Callback<Transaction>,
    inspect: Callback<Transaction>,
) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let currency = ctx.currency();
    let mobile = transactions.clone();
    let mobile_categories = categories.clone();
    let mobile_currency = currency.clone();
    view! { <div class="md:hidden">{mobile.into_iter().map(|t| view! { <TransactionCard transaction=t categories=mobile_categories.clone() currency=mobile_currency.clone() edit=edit remove=remove inspect=inspect /> }).collect_view()}</div>
        <div class="hidden md:block"><table class="w-full table-fixed text-left text-sm"><thead class="border-b border-slate-100 text-xs font-medium text-slate-500 dark:border-slate-800 dark:text-slate-400"><tr><th class="w-[32%] px-5 py-3 font-medium">"Операция"</th><th class="w-[22%] py-3 font-medium">"Категория"</th><th class="w-[17%] py-3 font-medium">"Дата"</th><th class="w-[19%] py-3 text-right font-medium">"Сумма"</th><th class="w-[10%] px-2 py-3"><span class="sr-only">"Действия"</span></th></tr></thead><tbody>{transactions.into_iter().map(|t| {
            let category = categories.iter().find(|c| c.id==t.category_id).map(|c| c.name.clone()).unwrap_or_else(||"Архивная категория".into());
            let income=t.transaction_type=="income"; let tx=t.clone(); let tx2=t.clone(); let tx3=t.clone();
            view! { <tr class="border-b border-slate-100 last:border-0 dark:border-slate-800"><td class="px-5 py-4"><button type="button" class="line-clamp-2 break-words text-left font-medium hover:text-brand-600" on:click=move |_| inspect.run(tx.clone())>{t.title}</button><span class="mt-1 block text-xs text-slate-500">{if income {"Доход"} else {"Расход"}}</span></td><td class="break-words py-4 pr-3 text-slate-500 dark:text-slate-400">{category}</td><td class="py-4 text-xs text-slate-500 dark:text-slate-400">{date(t.transaction_date)}</td><td class=if income {"break-words py-4 text-right font-semibold text-brand-600 dark:text-brand-400"} else {"break-words py-4 text-right font-semibold"}>{format!("{}{}",if income {"+"}else{"−"},money(t.amount,&currency))}</td><td class="px-2 py-4"><div class="flex flex-wrap justify-end gap-1"><button type="button" class="icon-btn" aria-label="Изменить транзакцию" on:click=move |_| edit.run(tx2.clone())><Icon name="edit" class="size-4" /></button><button type="button" class="icon-btn hover:text-red-600" aria-label="Удалить транзакцию" on:click=move |_| remove.run(tx3.clone())><Icon name="trash" class="size-4" /></button></div></td></tr> }
        }).collect_view()}</tbody></table></div>
    }
}

#[component]
pub fn Toast() -> impl IntoView {
    let toast = expect_context::<AppContext>().toast;
    view! {<div class="pointer-events-none fixed bottom-5 right-4 z-50 w-[calc(100%-2rem)] max-w-sm" aria-live="polite" aria-atomic="true">{move || toast.get().map(|(_,message,success)|view! { <div class=if success {"pointer-events-auto flex items-start gap-3 rounded-xl border border-brand-200 bg-white p-4 text-sm shadow-lg dark:border-brand-800 dark:bg-slate-900"} else {"pointer-events-auto flex items-start gap-3 rounded-xl border border-red-300 bg-white p-4 text-sm shadow-lg dark:border-red-900 dark:bg-slate-900"}><span class=if success {"mt-0.5 text-brand-600"}else{"mt-0.5 text-red-600"}><Icon name=if success {"check"} else {"warning"} /></span><p class="flex-1 leading-6">{message}</p><button class="icon-btn -mr-2 -mt-1" aria-label="Закрыть уведомление" on:click=move |_| toast.set(None)><Icon name="close" class="size-4" /></button></div> })}</div>}
}
