use super::*;
use crate::{app::AppContext, components::*, utils::*};
use gloo_net::http::Method;
use leptos::prelude::*;
use spendwise_shared::*;
use uuid::Uuid;

#[component]
pub fn CategoriesPage() -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let data = use_api::<Vec<Category>>(|| "/categories?include_archived=true".into());
    let kind = RwSignal::new(String::new());
    let archived = RwSignal::new(false);
    let editing = RwSignal::new(None::<Option<Category>>);
    let deleting = RwSignal::new(None::<Category>);
    let busy = RwSignal::new(false);
    let delete_ctx = ctx.clone();
    let restore_ctx = ctx.clone();
    view! {<PageHeading title="Категории" description="Создавайте свою систему учёта. Цвет и иконка помогут быстро находить нужное."><Button on_click=Callback::new(move |_|editing.set(Some(None)))><Icon name="plus" class="size-4" />"Новая категория"</Button></PageHeading><div class="mb-5 flex flex-wrap items-center justify-between gap-4"><div class="w-full sm:w-52"><Select label="Тип категории" id="category-filter" value=kind><option value="">"Все категории"</option><option value="expense">"Расходы"</option><option value="income">"Доходы"</option></Select></div><label class="flex items-center gap-2 text-sm text-slate-500"><input type="checkbox" class="size-4 accent-emerald-600" prop:checked=move ||archived.get() on:change=move |e|archived.set(event_target_checked(&e)) />"Показать архивные"</label></div><LoadError error=data.error />
        {move ||if data.loading.get(){view!{<LoadingSpinner />}.into_any()}else{let items=data.data.get().unwrap_or_default().into_iter().filter(|c|(archived.get()||!c.archived)&&(kind.get().is_empty()||c.transaction_type==kind.get())).collect::<Vec<_>>();if items.is_empty(){view!{<div class="panel"><EmptyState title="Категорий пока нет" text="Добавьте категорию доходов или расходов, чтобы начать учёт." /></div>}.into_any()}else{view!{<div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">{items.into_iter().map(|c|{let edit=c.clone();let delete=c.clone();let restore=c.clone();let restore_ctx=restore_ctx.clone();view!{<article class="card"><div class="flex items-start justify-between gap-3"><span class="flex size-12 items-center justify-center rounded-2xl" style=format!("color:{};background-color:{}18",c.color,c.color)><Icon name=c.icon.clone() class="size-6" /></span><div class="flex gap-1"><button class="icon-btn" aria-label="Изменить категорию" on:click=move |_|editing.set(Some(Some(edit.clone())))><Icon name="edit" class="size-4" /></button>{if c.archived{view!{<button class="icon-btn" aria-label="Восстановить категорию" title="Восстановить" on:click=move |_|{let input=CategoryInput{name:restore.name.clone(),transaction_type:restore.transaction_type.clone(),color:restore.color.clone(),icon:restore.icon.clone(),archived:false};let ctx=restore_ctx.clone();let id=restore.id;leptos::task::spawn_local(async move{match ctx.api.mutate::<_,Category>(Method::PATCH,&format!("/categories/{id}"),&input).await{Ok(_)=>{ctx.changed();ctx.notify("Категория восстановлена",true);},Err(e)=>ctx.notify(e,false)}});}><Icon name="recurring" class="size-4" /></button>}.into_any()}else{view!{<button class="icon-btn hover:text-red-600" aria-label="Архивировать категорию" on:click=move |_|deleting.set(Some(delete.clone()))><Icon name="trash" class="size-4" /></button>}.into_any()}}</div></div><h2 class="mt-5 break-words font-semibold">{c.name}</h2><div class="mt-3 flex items-center gap-2"><Badge income=c.transaction_type=="income" /><Show when=move ||c.archived><span class="badge bg-amber-50 text-amber-700 dark:bg-amber-950/30 dark:text-amber-400">"В архиве"</span></Show></div></article>}}).collect_view()}</div>}.into_any()}}}
        {move ||editing.get().map(|category|view!{<CategoryEditor category=category close=Callback::new(move |_|editing.set(None)) />})}
        {move ||deleting.get().map(|c|{let ctx=delete_ctx.clone();let id=c.id;view!{<ConfirmDialog title="Архивировать категорию?" text=format!("Категория «{}» останется в истории операций. Новые операции в ней будут недоступны. Её можно восстановить.",c.name) busy=busy close=Callback::new(move |_|deleting.set(None)) confirm=Callback::new(move |_|{if busy.get_untracked(){return;}busy.set(true);let ctx=ctx.clone();leptos::task::spawn_local(async move{let result=ctx.api.empty(Method::DELETE,&format!("/categories/{id}")).await;busy.try_set(false);match result{Ok(())=>{deleting.try_set(None);ctx.changed();ctx.notify("Категория архивирована",true);},Err(e)=>ctx.notify(e,false)}});}) />}})}
    }
}

#[component]
fn CategoryEditor(category: Option<Category>, close: Callback<()>) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let name = RwSignal::new(
        category
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_default(),
    );
    let kind = RwSignal::new(
        category
            .as_ref()
            .map(|c| c.transaction_type.clone())
            .unwrap_or_else(|| "expense".into()),
    );
    let color = RwSignal::new(
        category
            .as_ref()
            .map(|c| c.color.clone())
            .unwrap_or_else(|| "#10b981".into()),
    );
    let icon = RwSignal::new(
        category
            .as_ref()
            .map(|c| c.icon.clone())
            .unwrap_or_else(|| "wallet".into()),
    );
    let id = category.as_ref().map(|c| c.id);
    let archived = category.as_ref().is_some_and(|c| c.archived);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let name = name.get_untracked().trim().to_string();
        if name.is_empty() {
            error.set(Some("Укажите название категории".into()));
            return;
        }
        let input = CategoryInput {
            name,
            transaction_type: kind.get_untracked(),
            color: color.get_untracked(),
            icon: icon.get_untracked(),
            archived,
        };
        busy.set(true);
        error.set(None);
        let ctx = ctx.clone();
        leptos::task::spawn_local(async move {
            let (method, path) = id
                .map(|id| (Method::PATCH, format!("/categories/{id}")))
                .unwrap_or((Method::POST, "/categories".into()));
            let result = ctx.api.mutate::<_, Category>(method, &path, &input).await;
            busy.try_set(false);
            match result {
                Ok(_) => {
                    close.run(());
                    ctx.changed();
                    ctx.notify("Категория сохранена", true);
                }
                Err(e) => {
                    error.try_set(Some(e));
                }
            }
        });
    };
    view! {<Modal title=if id.is_some(){"Изменить категорию"}else{"Новая категория"} close=close><form class="space-y-5" on:submit=submit><Input label="Название" id="cat-name" value=name required=true maxlength=80 /><Select label="Тип" id="cat-type" value=kind><option value="expense">"Расход"</option><option value="income">"Доход"</option></Select><div class="grid gap-4 sm:grid-cols-2"><Input label="Цвет" id="cat-color" value=color kind="color" /><Select label="Иконка" id="cat-icon" value=icon><option value="wallet">"Кошелёк"</option><option value="budgets">"Бюджет"</option><option value="recurring">"Регулярный платёж"</option><option value="analytics">"Инвестиции"</option><option value="shield">"Здоровье"</option><option value="categories">"Покупки"</option><option value="profile">"Личное"</option><option value="transactions">"Перевод"</option></Select></div><p class="text-xs leading-5 text-slate-400">"Тип категории с существующими операциями изменить нельзя."</p>{move ||error.get().map(|e|view!{<Alert text=e />})}<div class="flex justify-end gap-3"><Button class="btn-secondary" on_click=close>"Отмена"</Button><Button kind="submit" busy=Signal::derive(move ||busy.get())>"Сохранить"</Button></div></form></Modal>}
}

#[component]
pub fn BudgetsPage() -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let month = RwSignal::new(current_month());
    let data = use_api::<Vec<Budget>>(move || format!("/budgets?month={}", period(&month.get()).0));
    let categories = use_api::<Vec<Category>>(|| "/categories?include_archived=true".into());
    let editing = RwSignal::new(None::<Option<Budget>>);
    let deleting = RwSignal::new(None::<Budget>);
    let busy = RwSignal::new(false);
    let currency = ctx.currency();
    let delete_ctx = ctx.clone();
    view! {<PageHeading title="Бюджеты" description="Задайте границы расходов и оставьте место для того, что действительно важно."><label class="sr-only" for="budget-period">"Месяц бюджета"</label><input id="budget-period" type="month" class="input w-auto" prop:value=move ||month.get() on:change=move |e|{let v=event_target_value(&e);if !v.is_empty(){month.set(v);}} /><Button on_click=Callback::new(move |_|editing.set(Some(None)))><Icon name="plus" class="size-4" />"Создать бюджет"</Button></PageHeading><LoadError error=data.error /><LoadError error=categories.error />
        {move ||if data.loading.get(){view!{<LoadingSpinner />}.into_any()}else{let items=data.data.get().unwrap_or_default();if items.is_empty(){view!{<div class="panel"><EmptyState title="Новый месяц — новый план" text="Установите общий лимит расходов или выделите бюджет для отдельных категорий." /></div>}.into_any()}else{view!{<div class="grid gap-5 md:grid-cols-2 xl:grid-cols-3">{items.into_iter().map(|b|{let label=b.category_id.and_then(|id|categories.data.get_untracked().unwrap_or_default().into_iter().find(|c|c.id==id)).map(|c|c.name).unwrap_or_else(||"Общий бюджет".into());let edit=b.clone();let delete=b.clone();view!{<article class="card"><div class="mb-6 flex items-center justify-between"><span class="flex size-10 items-center justify-center rounded-xl bg-brand-50 text-brand-600 dark:bg-brand-950/40"><Icon name="budgets" /></span><div class="flex gap-1"><button class="icon-btn" aria-label="Изменить бюджет" on:click=move |_|editing.set(Some(Some(edit.clone())))><Icon name="edit" class="size-4" /></button><button class="icon-btn hover:text-red-600" aria-label="Удалить бюджет" on:click=move |_|deleting.set(Some(delete.clone()))><Icon name="trash" class="size-4" /></button></div></div><BudgetProgress budget=b label=label currency=currency.clone() /></article>}}).collect_view()}</div>}.into_any()}}}
        <p class="mt-6 max-w-2xl text-xs leading-6 text-slate-400">"Расходы рассчитываются по вашим транзакциям. Общий бюджет включает все категории; лимиты категорий действуют параллельно и не суммируются с ним."</p>
        {move ||editing.get().map(|budget|view!{<BudgetEditor budget=budget month=month.get_untracked() categories=categories.data.get().unwrap_or_default() close=Callback::new(move |_|editing.set(None)) />})}
        {move ||deleting.get().map(|b|{let ctx=delete_ctx.clone();let id=b.id;view!{<ConfirmDialog title="Удалить бюджет?" text="Лимит будет удалён. Ваши транзакции сохранятся." busy=busy close=Callback::new(move |_|deleting.set(None)) confirm=Callback::new(move |_|{if busy.get_untracked(){return;}busy.set(true);let ctx=ctx.clone();leptos::task::spawn_local(async move{let result=ctx.api.empty(Method::DELETE,&format!("/budgets/{id}")).await;busy.try_set(false);match result{Ok(())=>{deleting.try_set(None);ctx.changed();ctx.notify("Бюджет удалён",true);},Err(e)=>ctx.notify(e,false)}});}) />}})}
    }
}

#[component]
fn BudgetEditor(
    budget: Option<Budget>,
    month: String,
    categories: Vec<Category>,
    close: Callback<()>,
) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let amount = RwSignal::new(
        budget
            .as_ref()
            .map(|b| b.amount.to_string())
            .unwrap_or_default(),
    );
    let month = RwSignal::new(
        budget
            .as_ref()
            .map(|b| b.month.format("%Y-%m").to_string())
            .unwrap_or(month),
    );
    let category = RwSignal::new(
        budget
            .as_ref()
            .and_then(|b| b.category_id)
            .map(|id| id.to_string())
            .unwrap_or_default(),
    );
    let id = budget.map(|b| b.id);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let input = (|| {
            let category = category.get_untracked();
            Ok::<_, String>(BudgetInput {
                category_id: if category.is_empty() {
                    None
                } else {
                    Some(Uuid::parse_str(&category).map_err(|_| "Выберите категорию".to_string())?)
                },
                month: parse_date(&format!("{}-01", month.get_untracked()))?,
                amount: parse_money(&amount.get_untracked())?,
            })
        })();
        match input {
            Err(e) => {
                error.try_set(Some(e));
            }
            Ok(input) => {
                busy.set(true);
                error.set(None);
                let ctx = ctx.clone();
                leptos::task::spawn_local(async move {
                    let (method, path) = id
                        .map(|id| (Method::PATCH, format!("/budgets/{id}")))
                        .unwrap_or((Method::POST, "/budgets".into()));
                    let result = ctx.api.mutate::<_, Budget>(method, &path, &input).await;
                    busy.try_set(false);
                    match result {
                        Ok(_) => {
                            close.run(());
                            ctx.changed();
                            ctx.notify("Бюджет сохранён", true);
                        }
                        Err(e) => {
                            error.try_set(Some(e));
                        }
                    }
                });
            }
        }
    };
    view! {<Modal title=if id.is_some(){"Изменить бюджет"}else{"Новый бюджет"} close=close><form class="space-y-5" on:submit=submit><Input label="Месяц" id="budget-month" value=month kind="month" required=true /><Select label="Категория" id="budget-category" value=category><option value="">"Общий бюджет — все расходы"</option>{categories.into_iter().filter(|c|c.transaction_type=="expense"&&!c.archived).map(|c|view!{<option value=c.id.to_string()>{c.name}</option>}).collect_view()}</Select><Input label=format!("Лимит, {}",expect_context::<AppContext>().currency()) id="budget-amount" value=amount required=true placeholder="0,00" />{move ||error.get().map(|e|view!{<Alert text=e />})}<div class="flex justify-end gap-3"><Button class="btn-secondary" on_click=close>"Отмена"</Button><Button kind="submit" busy=Signal::derive(move ||busy.get())>"Сохранить"</Button></div></form></Modal>}
}

#[component]
pub fn RecurringPage() -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let data = use_api::<Vec<Recurring>>(|| "/recurring".into());
    let categories = use_api::<Vec<Category>>(|| "/categories?include_archived=true".into());
    let editing = RwSignal::new(None::<Option<Recurring>>);
    let deleting = RwSignal::new(None::<Recurring>);
    let busy = RwSignal::new(false);
    let toggling = RwSignal::new(None::<Uuid>);
    let delete_ctx = ctx.clone();
    let toggle_ctx = ctx.clone();
    let currency = ctx.currency();
    view! {<PageHeading title="Регулярные платежи" description="Подписки, аренда и зарплата. Ежемесячные операции создаются автоматически."><Button on_click=Callback::new(move |_|editing.set(Some(None)))><Icon name="plus" class="size-4" />"Добавить платёж"</Button></PageHeading><LoadError error=data.error /><LoadError error=categories.error />
        {move ||if data.loading.get(){view!{<LoadingSpinner />}.into_any()}else{let mut items=data.data.get().unwrap_or_default();items.sort_by_key(|r|r.next_date);if items.is_empty(){view!{<div class="panel"><EmptyState title="Освободите время для важного" text="Добавьте регулярные операции. SpendWise будет учитывать их каждый месяц." /></div>}.into_any()}else{view!{<div class="grid gap-5 md:grid-cols-2 xl:grid-cols-3">{items.into_iter().map(|r|{let edit=r.clone();let delete=r.clone();let toggle=r.clone();let toggle_ctx=toggle_ctx.clone();let category=categories.data.get_untracked().unwrap_or_default().into_iter().find(|c|c.id==r.category_id).map(|c|c.name).unwrap_or_default();view!{<article class="card"><div class="flex items-start justify-between gap-2"><span class="flex size-11 items-center justify-center rounded-xl bg-slate-100 text-slate-500 dark:bg-slate-800"><Icon name="recurring" /></span><div class="flex gap-1"><button class="icon-btn" aria-label=if r.active{"Приостановить платёж"}else{"Активировать платёж"} disabled=move ||toggling.get()==Some(r.id) on:click=move |_|{let id=toggle.id;if toggling.get_untracked().is_some(){return;}toggling.set(Some(id));let input=RecurringInput{category_id:toggle.category_id,transaction_type:toggle.transaction_type.clone(),amount:toggle.amount,title:toggle.title.clone(),description:toggle.description.clone(),next_date:toggle.next_date,active:!toggle.active};let ctx=toggle_ctx.clone();leptos::task::spawn_local(async move{let result=ctx.api.mutate::<_,Recurring>(Method::PATCH,&format!("/recurring/{id}"),&input).await;toggling.try_set(None);match result{Ok(_)=>{ctx.changed();ctx.notify("Статус платежа изменён",true);},Err(e)=>ctx.notify(e,false)}});}><Icon name=if r.active{"pause"}else{"play"} class="size-4" /></button><button class="icon-btn" aria-label="Изменить регулярный платёж" on:click=move |_|editing.set(Some(Some(edit.clone())))><Icon name="edit" class="size-4" /></button><button class="icon-btn hover:text-red-600" aria-label="Удалить регулярный платёж" on:click=move |_|deleting.set(Some(delete.clone()))><Icon name="trash" class="size-4" /></button></div></div><h2 class="mt-5 break-words font-semibold">{r.title}</h2><p class="mt-1 text-xs text-slate-400">{category}</p><p class="mt-4 text-xl font-semibold">{money(r.amount,&currency)}</p><div class="mt-4 flex flex-wrap items-center justify-between gap-2"><Badge income=r.transaction_type=="income" /><span class=if r.active{"badge bg-brand-50 text-brand-700 dark:bg-brand-950/40 dark:text-brand-400"}else{"badge bg-amber-50 text-amber-700 dark:bg-amber-950/40 dark:text-amber-400"}>{if r.active{"Активен"}else{"На паузе"}}</span></div><p class="mt-4 border-t border-slate-100 pt-4 text-xs text-slate-500 dark:border-slate-800">{format!("Следующий: {} · Ежемесячно, {} числа",date(r.next_date),r.day_of_month)}</p></article>}}).collect_view()}</div>}.into_any()}}}
        <p class="mt-6 max-w-2xl text-xs leading-6 text-slate-400">"Если в месяце нет выбранного числа, платёж создаётся в последний день месяца. Приостановленные платежи не создают транзакций."</p>
        {move ||editing.get().map(|recurring|view!{<RecurringEditor recurring=recurring categories=categories.data.get().unwrap_or_default() close=Callback::new(move |_|editing.set(None)) />})}
        {move ||deleting.get().map(|r|{let ctx=delete_ctx.clone();let id=r.id;view!{<ConfirmDialog title="Удалить регулярный платёж?" text=format!("Автоматическое создание операции «{}» прекратится. Уже созданные транзакции сохранятся.",r.title) busy=busy close=Callback::new(move |_|deleting.set(None)) confirm=Callback::new(move |_|{if busy.get_untracked(){return;}busy.set(true);let ctx=ctx.clone();leptos::task::spawn_local(async move{let result=ctx.api.empty(Method::DELETE,&format!("/recurring/{id}")).await;busy.try_set(false);match result{Ok(())=>{deleting.try_set(None);ctx.changed();ctx.notify("Регулярный платёж удалён",true);},Err(e)=>ctx.notify(e,false)}});}) />}})}
    }
}

#[component]
fn RecurringEditor(
    recurring: Option<Recurring>,
    categories: Vec<Category>,
    close: Callback<()>,
) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let title = RwSignal::new(
        recurring
            .as_ref()
            .map(|r| r.title.clone())
            .unwrap_or_default(),
    );
    let kind = RwSignal::new(
        recurring
            .as_ref()
            .map(|r| r.transaction_type.clone())
            .unwrap_or_else(|| "expense".into()),
    );
    let amount = RwSignal::new(
        recurring
            .as_ref()
            .map(|r| r.amount.to_string())
            .unwrap_or_default(),
    );
    let description = RwSignal::new(
        recurring
            .as_ref()
            .and_then(|r| r.description.clone())
            .unwrap_or_default(),
    );
    let next_date = RwSignal::new(
        recurring
            .as_ref()
            .map(|r| r.next_date)
            .unwrap_or_else(today)
            .to_string(),
    );
    let category = RwSignal::new(
        recurring
            .as_ref()
            .map(|r| r.category_id.to_string())
            .unwrap_or_default(),
    );
    let active = RwSignal::new(recurring.as_ref().is_none_or(|r| r.active));
    let id = recurring.map(|r| r.id);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let options = categories.clone();
    Effect::new(move |_| {
        let k = kind.get();
        let available = categories
            .iter()
            .filter(|c| c.transaction_type == k && !c.archived)
            .collect::<Vec<_>>();
        if !available
            .iter()
            .any(|c| c.id.to_string() == category.get_untracked())
        {
            category.set(
                available
                    .first()
                    .map(|c| c.id.to_string())
                    .unwrap_or_default(),
            );
        }
    });
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let input = (|| {
            let category_id = Uuid::parse_str(&category.get_untracked())
                .map_err(|_| "Создайте активную категорию".to_string())?;
            let amount = parse_money(&amount.get_untracked())?;
            let next_date = parse_date(&next_date.get_untracked())?;
            let title = title.get_untracked().trim().to_string();
            if title.is_empty() {
                return Err("Укажите название".to_string());
            }
            let d = description.get_untracked().trim().to_string();
            Ok(RecurringInput {
                category_id,
                transaction_type: kind.get_untracked(),
                amount,
                title,
                description: if d.is_empty() { None } else { Some(d) },
                next_date,
                active: active.get_untracked(),
            })
        })();
        match input {
            Err(e) => {
                error.try_set(Some(e));
            }
            Ok(input) => {
                busy.set(true);
                error.set(None);
                let ctx = ctx.clone();
                leptos::task::spawn_local(async move {
                    let (method, path) = id
                        .map(|id| (Method::PATCH, format!("/recurring/{id}")))
                        .unwrap_or((Method::POST, "/recurring".into()));
                    let result = ctx.api.mutate::<_, Recurring>(method, &path, &input).await;
                    busy.try_set(false);
                    match result {
                        Ok(_) => {
                            close.run(());
                            ctx.changed();
                            ctx.notify("Регулярный платёж сохранён", true);
                        }
                        Err(e) => {
                            error.try_set(Some(e));
                        }
                    }
                });
            }
        }
    };
    view! {<Modal title=if id.is_some(){"Изменить регулярный платёж"}else{"Новый регулярный платёж"} close=close><form class="space-y-5" on:submit=submit><Input label="Название" id="rec-title" value=title required=true maxlength=200 /><Select label="Тип операции" id="rec-type" value=kind><option value="expense">"Расход"</option><option value="income">"Доход"</option></Select><div class="grid gap-4 sm:grid-cols-2"><Input label=format!("Сумма, {}",expect_context::<AppContext>().currency()) id="rec-amount" value=amount required=true placeholder="0,00" /><Input label="Дата следующего платежа" id="rec-date" value=next_date kind="date" required=true /></div><Select label="Категория" id="rec-category" value=category>{move ||options.iter().filter(|c|c.transaction_type==kind.get()&&!c.archived).map(|c|view!{<option value=c.id.to_string()>{c.name.clone()}</option>}).collect_view()}</Select><Textarea label="Описание (необязательно)" id="rec-description" value=description /><label class="flex items-center gap-2 text-sm"><input type="checkbox" class="size-4 accent-emerald-600" prop:checked=move ||active.get() on:change=move |e|active.set(event_target_checked(&e)) />"Автоматически создавать каждый месяц"</label>{move ||error.get().map(|e|view!{<Alert text=e />})}<div class="flex justify-end gap-3"><Button class="btn-secondary" on_click=close>"Отмена"</Button><Button kind="submit" busy=Signal::derive(move ||busy.get())>"Сохранить"</Button></div></form></Modal>}
}
