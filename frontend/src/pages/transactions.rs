use super::*;
use crate::{app::AppContext, components::*, utils::*};
use gloo_net::http::Method;
use leptos::prelude::*;
use spendwise_shared::*;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct TransactionActions {
    pub editing: RwSignal<Option<Option<Transaction>>>,
    pub deleting: RwSignal<Option<Transaction>>,
    pub viewing: RwSignal<Option<Transaction>>,
}
impl TransactionActions {
    pub fn new() -> Self {
        Self {
            editing: RwSignal::new(None),
            deleting: RwSignal::new(None),
            viewing: RwSignal::new(None),
        }
    }
    pub fn edit(self) -> Callback<Transaction> {
        Callback::new(move |t| self.editing.set(Some(Some(t))))
    }
    pub fn remove(self) -> Callback<Transaction> {
        Callback::new(move |t| self.deleting.set(Some(t)))
    }
    pub fn inspect(self) -> Callback<Transaction> {
        Callback::new(move |t| self.viewing.set(Some(t)))
    }
}

#[component]
pub fn TransactionsPage(initial_create: bool) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let (f, t) = period(&current_month());
    let from = RwSignal::new(f);
    let to = RwSignal::new(t);
    let search = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let kind = RwSignal::new(String::new());
    let sort = RwSignal::new("date_desc".to_string());
    let page = RwSignal::new(1_i64);
    let filter_error = RwSignal::new(None::<String>);
    let applied = RwSignal::new(format!(
        "from={}&to={}&sort=date_desc",
        from.get_untracked(),
        to.get_untracked()
    ));
    let transactions = use_api::<Paginated<Transaction>>(move || {
        format!(
            "/transactions?{}&page={}&per_page=15",
            applied.get(),
            page.get()
        )
    });
    let categories = use_api::<Vec<Category>>(|| "/categories?include_archived=true".into());
    let actions = TransactionActions::new();
    if initial_create {
        actions.editing.set(Some(None));
    }
    let edit = actions.edit();
    let remove = actions.remove();
    let inspect = actions.inspect();
    let export_busy = RwSignal::new(false);
    let export_ctx = ctx.clone();
    let apply = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if from.get_untracked() > to.get_untracked() {
            filter_error.set(Some("Начальная дата не может быть позже конечной.".into()));
            return;
        }
        filter_error.set(None);
        page.set(1);
        applied.set(
            format!(
                "from={}&to={}&search={}&category_id={}&transaction_type={}&sort={}",
                from.get_untracked(),
                to.get_untracked(),
                query(&search.get_untracked()),
                category.get_untracked(),
                kind.get_untracked(),
                sort.get_untracked()
            )
            .replace("&category_id=&", "&")
            .replace("&transaction_type=&", "&"),
        );
    };
    view! {<PageHeading title="Транзакции" description="Все доходы и расходы. Находите нужное и следите за каждой операцией."><Button class="btn-secondary" busy=Signal::derive(move ||export_busy.get()) on_click=Callback::new(move |_|{if from.get_untracked()>to.get_untracked(){export_ctx.notify("Проверьте диапазон дат экспорта",false);return;}export_busy.set(true);let ctx=export_ctx.clone();leptos::task::spawn_local(async move{let result=ctx.api.export(&from.get_untracked(),&to.get_untracked()).await;export_busy.try_set(false);match result{Ok(())=>ctx.notify("CSV-файл скачан",true),Err(e)=>ctx.notify(e,false)}});})><Icon name="download" class="size-4" />"Экспорт CSV"</Button><Button on_click=Callback::new(move |_|actions.editing.set(Some(None)))><Icon name="plus" class="size-4" />"Добавить"</Button></PageHeading>
        <form class="card mb-5" on:submit=apply><div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-4"><Input label="Поиск" id="tx-search" value=search placeholder="Название или описание" maxlength=200 /><Input label="С даты" id="tx-from" value=from kind="date" required=true /><Input label="По дату" id="tx-to" value=to kind="date" required=true /><Select label="Тип операции" id="tx-type-filter" value=kind><option value="">"Все операции"</option><option value="income">"Доходы"</option><option value="expense">"Расходы"</option></Select><Select label="Категория" id="tx-category-filter" value=category><option value="">"Все категории"</option>{move ||categories.data.get().unwrap_or_default().into_iter().map(|c|view!{<option value=c.id.to_string()>{format!("{}{}",c.name,if c.archived{" (архив)"}else{""})}</option>}).collect_view()}</Select><Select label="Сортировка" id="tx-sort" value=sort><option value="date_desc">"Сначала новые"</option><option value="date_asc">"Сначала старые"</option><option value="amount_desc">"Сначала крупные суммы"</option><option value="amount_asc">"Сначала небольшие суммы"</option></Select><div class="flex flex-wrap items-end gap-3 sm:col-span-2"><Button kind="submit"><Icon name="search" class="size-4" />"Применить"</Button><button type="button" class="btn-secondary" on:click=move |_|{let(f,t)=period(&current_month());from.set(f.clone());to.set(t.clone());search.set(String::new());category.set(String::new());kind.set(String::new());sort.set("date_desc".into());page.set(1);applied.set(format!("from={f}&to={t}&sort=date_desc"));filter_error.set(None);}>"Сбросить"</button></div></div>{move ||filter_error.get().map(|e|view!{<div class="mt-4"><Alert text=e /></div>})}<p class="mt-4 text-xs leading-5 text-slate-400">"CSV-экспорт использует выбранный диапазон дат."</p></form>
        <LoadError error=categories.error /><LoadError error=transactions.error /><section class="panel">{move ||if transactions.loading.get(){view!{<LoadingSpinner />}.into_any()}else{let data=transactions.data.get();match data{Some(data) if !data.items.is_empty()=>{let total=data.total;let max=(total+14)/15;view!{<TransactionTable transactions=data.items categories=categories.data.get().unwrap_or_default() edit=edit remove=remove inspect=inspect /><div class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 p-4 text-xs text-slate-500 dark:border-slate-800"><span>{format!("{}–{} из {} операций",(data.page-1)*15+1,(data.page*15).min(total),total)}</span><div class="flex items-center gap-2"><button type="button" class="btn-secondary px-3 py-2 text-xs" disabled={move || page.get() <= 1} on:click=move |_|page.update(|p|*p-=1)>"Назад"</button><span>{move ||format!("{} / {}",page.get(),max)}</span><button type="button" class="btn-secondary px-3 py-2 text-xs" disabled={move || page.get() >= max} on:click=move |_|page.update(|p|*p+=1)>"Вперёд"</button></div></div>}.into_any()},_=>view!{<EmptyState title="Операции не найдены" text="Измените фильтры или добавьте первую транзакцию за выбранный период." />}.into_any()}}}</section>
        {move ||view!{<TransactionModals actions=actions categories=categories.data.get().unwrap_or_default() />}}
    }
}

#[component]
pub fn TransactionModals(actions: TransactionActions, categories: Vec<Category>) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let busy = RwSignal::new(false);
    let edit_categories = categories.clone();
    let details_categories = categories.clone();
    let close_ctx = ctx.clone();
    let delete_ctx = ctx.clone();
    let close = Callback::new(move |_| {
        actions.editing.try_set(None);
        if close_ctx.path.get_untracked() == "/transactions/new" {
            close_ctx.navigate("/transactions");
        }
    });
    view! {{move ||actions.editing.get().map(|tx|view!{<TransactionEditor transaction=tx categories=edit_categories.clone() close=close />})}
        {move ||actions.deleting.get().map(|tx|{let ctx=delete_ctx.clone();let id=tx.id;view!{<ConfirmDialog title="Удалить транзакцию?" text=format!("Операция «{}» будет удалена. Показатели и бюджет пересчитаются.",tx.title) busy=busy close=Callback::new(move |_|actions.deleting.set(None)) confirm=Callback::new(move |_|{if busy.get_untracked(){return;}busy.set(true);let ctx=ctx.clone();leptos::task::spawn_local(async move{let result=ctx.api.empty(Method::DELETE,&format!("/transactions/{id}")).await;busy.try_set(false);match result{Ok(())=>{actions.deleting.try_set(None);ctx.changed();ctx.notify("Транзакция удалена",true);},Err(e)=>ctx.notify(e,false)}});}) />}})}
        {move ||actions.viewing.get().map(|tx|{let currency=ctx.currency();let category=details_categories.iter().find(|c|c.id==tx.category_id).map(|c|c.name.clone()).unwrap_or_default();view!{<Modal title="Детали транзакции" close=Callback::new(move |_|actions.viewing.set(None))><p class="break-words text-xl font-semibold">{tx.title}</p><p class="mt-3 text-2xl font-semibold text-brand-600 dark:text-brand-400">{money(tx.amount,&currency)}</p><dl class="mt-6 grid grid-cols-2 gap-4 text-sm"><div><dt class="text-slate-400">"Тип"</dt><dd class="mt-1">{if tx.transaction_type=="income"{"Доход"}else{"Расход"}}</dd></div><div><dt class="text-slate-400">"Категория"</dt><dd class="mt-1 break-words">{category}</dd></div><div><dt class="text-slate-400">"Дата операции"</dt><dd class="mt-1">{date(tx.transaction_date)}</dd></div><div><dt class="text-slate-400">"Добавлена"</dt><dd class="mt-1">{tx.created_at.format("%d.%m.%Y").to_string()}</dd></div></dl><p class="mt-6 whitespace-pre-wrap break-words text-sm leading-6 text-slate-500 dark:text-slate-400">{tx.description.unwrap_or_else(||"Описание не добавлено".into())}</p></Modal>}})}
    }
}

#[component]
pub fn TransactionEditor(
    transaction: Option<Transaction>,
    categories: Vec<Category>,
    close: Callback<()>,
) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let title = RwSignal::new(
        transaction
            .as_ref()
            .map(|t| t.title.clone())
            .unwrap_or_default(),
    );
    let kind = RwSignal::new(
        transaction
            .as_ref()
            .map(|t| t.transaction_type.clone())
            .unwrap_or_else(|| "expense".into()),
    );
    let amount = RwSignal::new(
        transaction
            .as_ref()
            .map(|t| t.amount.to_string())
            .unwrap_or_default(),
    );
    let description = RwSignal::new(
        transaction
            .as_ref()
            .and_then(|t| t.description.clone())
            .unwrap_or_default(),
    );
    let date_value = RwSignal::new(
        transaction
            .as_ref()
            .map(|t| t.transaction_date)
            .unwrap_or_else(today)
            .to_string(),
    );
    let category = RwSignal::new(
        transaction
            .as_ref()
            .map(|t| t.category_id.to_string())
            .unwrap_or_default(),
    );
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let id = transaction.as_ref().map(|t| t.id);
    let options = categories.clone();
    Effect::new(move |_| {
        let selected_kind = kind.get();
        let available = categories
            .iter()
            .filter(|c| c.transaction_type == selected_kind && !c.archived)
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
                .map_err(|_| "Создайте активную категорию для этого типа операции".to_string())?;
            let amount = parse_money(&amount.get_untracked())?;
            let transaction_date = parse_date(&date_value.get_untracked())?;
            let title = title.get_untracked().trim().to_string();
            if title.is_empty() {
                return Err("Укажите название операции".to_string());
            }
            let d = description.get_untracked().trim().to_string();
            Ok(TransactionInput {
                category_id,
                transaction_type: kind.get_untracked(),
                amount,
                title,
                description: if d.is_empty() { None } else { Some(d) },
                transaction_date,
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
                        .map(|id| (Method::PATCH, format!("/transactions/{id}")))
                        .unwrap_or((Method::POST, "/transactions".into()));
                    let result = ctx
                        .api
                        .mutate::<_, Transaction>(method, &path, &input)
                        .await;
                    busy.try_set(false);
                    match result {
                        Ok(_) => {
                            close.run(());
                            ctx.changed();
                            ctx.notify("Транзакция сохранена", true);
                        }
                        Err(e) => {
                            error.try_set(Some(e));
                        }
                    }
                });
            }
        }
    };
    view! {<Modal title=if id.is_some(){"Изменить транзакцию"}else{"Новая транзакция"} close=close><form class="space-y-5" on:submit=submit><Select label="Тип операции" id="editor-type" value=kind><option value="expense">"Расход"</option><option value="income">"Доход"</option></Select><Input label="Название" id="editor-title" value=title required=true maxlength=200 placeholder="Например, покупка продуктов" /><div class="grid gap-4 sm:grid-cols-2"><Input label=format!("Сумма, {}",expect_context::<AppContext>().currency()) id="editor-amount" value=amount required=true placeholder="0,00" /><Input label="Дата операции" id="editor-date" value=date_value kind="date" required=true /></div><Select label="Категория" id="editor-category" value=category>{move ||options.iter().filter(|c|c.transaction_type==kind.get()&&!c.archived).map(|c|view!{<option value=c.id.to_string()>{c.name.clone()}</option>}).collect_view()}</Select><Textarea label="Описание (необязательно)" id="editor-description" value=description />{move ||error.get().map(|e|view!{<Alert text=e />})}<div class="flex flex-wrap justify-end gap-3 border-t border-slate-100 pt-5 dark:border-slate-800"><Button class="btn-secondary" on_click=close>"Отмена"</Button><Button kind="submit" busy=Signal::derive(move ||busy.get())>"Сохранить"</Button></div></form></Modal>}
}
