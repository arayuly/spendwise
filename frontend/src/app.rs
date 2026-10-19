use crate::{api::Api, components::*, pages::*, utils::*};
use leptos::prelude::*;
use wasm_bindgen::{JsCast, closure::Closure};

#[derive(Clone)]
pub struct AppContext {
    pub api: Api,
    pub path: RwSignal<String>,
    pub revision: RwSignal<u64>,
    pub toast: RwSignal<Option<(u64, String, bool)>>,
    pub mobile_menu: RwSignal<bool>,
    pub toast_sequence: RwSignal<u64>,
}
impl AppContext {
    pub fn navigate(&self, path: &str) {
        if let Some(window) = web_sys::window() {
            if let Ok(history) = window.history() {
                let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path));
            }
            window.scroll_to_with_x_and_y(0.0, 0.0);
        }
        self.path.set(path.to_string());
        self.mobile_menu.set(false);
    }
    pub fn currency(&self) -> String {
        self.api
            .session
            .get_untracked()
            .map(|s| s.user.currency)
            .unwrap_or_else(|| "KZT".into())
    }
    pub fn changed(&self) {
        self.revision.update(|n| *n += 1);
    }
    pub fn notify(&self, message: impl Into<String>, success: bool) {
        let id = self.toast_sequence.get_untracked() + 1;
        self.toast_sequence.set(id);
        self.toast.set(Some((id, message.into(), success)));
        let signal = self.toast;
        if let Some(window) = web_sys::window() {
            let cb = Closure::once_into_js(move || {
                if signal.get_untracked().is_some_and(|t| t.0 == id) {
                    signal.set(None);
                }
            });
            let _ = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 6000);
        }
    }
}

#[component]
pub fn App() -> impl IntoView {
    let path = web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".into());
    let ctx = AppContext {
        api: Api::new(),
        path: RwSignal::new(path),
        revision: RwSignal::new(0),
        toast: RwSignal::new(None),
        mobile_menu: RwSignal::new(false),
        toast_sequence: RwSignal::new(0),
    };
    provide_context(ctx.clone());
    let initialized = RwSignal::new(false);
    let restore_error = RwSignal::new(None::<String>);
    let api = ctx.api.clone();
    leptos::task::spawn_local(async move {
        if let Err(error) = api.restore().await {
            restore_error.set(Some(error));
        }
        initialized.set(true);
    });
    let path_signal = ctx.path;
    if let Some(window) = web_sys::window() {
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if let Some(path) = web_sys::window().and_then(|w| w.location().pathname().ok()) {
                path_signal.set(path);
            }
        });
        let _ = window.add_event_listener_with_callback("popstate", cb.as_ref().unchecked_ref());
        cb.forget();
    }
    let session = ctx.api.session;

    let authenticated = Memo::new(move |_| session.with(|s| s.is_some()));
    view! { <a href="#main-content" class="sr-only fixed left-4 top-4 z-50 rounded-xl bg-brand-600 p-3 text-white focus:not-sr-only">"Перейти к содержимому"</a>
        {move || if !initialized.get() { view! { <div class="flex min-h-screen items-center justify-center"><LoadingSpinner /></div> }.into_any() }
            else if !authenticated.get() { view! { <AuthPage restore_error=restore_error /> }.into_any() }
            else { view! { <Shell /> }.into_any() }}
        <Toast />
    }
}

#[component]
fn NavLink(
    #[prop(into)] href: String,
    #[prop(into)] icon: String,
    #[prop(into)] label: String,
) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let target = href.clone();
    let current = ctx.path;
    view! { <a href=href.clone() class=move || if current.get()==target || (target=="/transactions" && current.get().starts_with("/transactions/")) {"nav-active"} else {"nav-link"} on:click=move |e| {if e.button()==0 && !e.ctrl_key() && !e.meta_key() && !e.shift_key() {e.prevent_default();ctx.navigate(&href);}}><Icon name=icon /><span>{label}</span></a> }
}

#[component]
pub fn Sidebar() -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let menu = ctx.mobile_menu;
    let session = ctx.api.session;
    let ctx_home = ctx.clone();
    view! { <Show when=move || menu.get()><button type="button" class="fixed inset-0 z-30 bg-slate-950/40 backdrop-blur-sm lg:hidden" aria-label="Закрыть меню" on:click=move |_|menu.set(false)></button></Show>
        <aside class=move || if menu.get() {"fixed inset-y-0 left-0 z-40 flex w-64 flex-col border-r border-slate-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-900"} else {"fixed inset-y-0 left-0 z-40 hidden w-64 flex-col border-r border-slate-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-900 lg:flex"} aria-label="Главная навигация">
            <div class="mb-10 flex items-center justify-between"><a href="/" class="flex items-center gap-2.5 text-xl font-bold tracking-tight" on:click=move |e|{e.prevent_default();ctx_home.navigate("/");}><span class="flex size-9 items-center justify-center rounded-xl bg-brand-600 text-white"><Icon name="wallet" /></span>"SpendWise"<span class="sr-only">" — главная"</span></a><button class="icon-btn lg:hidden" aria-label="Закрыть меню" on:click=move |_|menu.set(false)><Icon name="close" /></button></div>
            <p class="mb-3 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-slate-400">"Личные финансы"</p>
            <nav class="space-y-1"><NavLink href="/" icon="dashboard" label="Обзор" /><NavLink href="/transactions" icon="transactions" label="Транзакции" /><NavLink href="/categories" icon="categories" label="Категории" /><NavLink href="/budgets" icon="budgets" label="Бюджеты" /><NavLink href="/recurring" icon="recurring" label="Регулярные платежи" /><NavLink href="/analytics" icon="analytics" label="Аналитика" /></nav>
            <div class="mt-auto pt-8"><div class="mb-5 rounded-xl bg-brand-50 p-4 dark:bg-brand-950/40"><span class="text-brand-600 dark:text-brand-400"><Icon name="shield" /></span><p class="mt-2 text-xs font-semibold text-brand-800 dark:text-brand-300">"Ваши финансы под контролем"</p><p class="mt-1 text-xs leading-5 text-brand-700/70 dark:text-brand-400/70">"Каждая осознанная покупка — шаг к вашим целям."</p></div><NavLink href="/profile" icon="profile" label="Настройки" />
                <div class="mt-4 flex items-center gap-3 border-t border-slate-100 pt-5 dark:border-slate-800"><span class="flex size-9 shrink-0 items-center justify-center rounded-full bg-slate-100 text-xs font-semibold dark:bg-slate-800">{move || session.get().map(|s|initials(&s.user.display_name)).unwrap_or_default()}</span><div class="min-w-0 flex-1"><p class="truncate text-sm font-medium">{move ||session.get().map(|s|s.user.display_name).unwrap_or_default()}</p><p class="mt-0.5 truncate text-xs text-slate-400">{move ||session.get().map(|s|s.user.email).unwrap_or_default()}</p></div></div>
            </div>
        </aside>
    }
}

#[component]
fn Shell() -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let path = ctx.path;
    view! { <div class="min-h-screen"><Sidebar /><div class="min-w-0 lg:pl-64"><Navbar /><main id="main-content" class="mx-auto max-w-[1540px] p-4 sm:p-8">{move || match path.get().as_str() {"/"=>view!{<DashboardPage analytics=false />}.into_any(),"/analytics"=>view!{<DashboardPage analytics=true />}.into_any(),"/transactions"=>view!{<TransactionsPage initial_create=false />}.into_any(),"/transactions/new"=>view!{<TransactionsPage initial_create=true />}.into_any(),"/categories"=>view!{<CategoriesPage />}.into_any(),"/budgets"=>view!{<BudgetsPage />}.into_any(),"/recurring"=>view!{<RecurringPage />}.into_any(),"/profile"=>view!{<ProfilePage />}.into_any(),"/login"|"/register"=>view!{<DashboardPage analytics=false />}.into_any(),_=>view!{<NotFoundPage />}.into_any()}}</main><footer class="mx-auto max-w-[1540px] px-4 pb-6 text-xs text-slate-400 sm:px-8">"SpendWise · Осознанное управление личными финансами"</footer></div></div> }
}
fn page_title(path: &str) -> &'static str {
    match path {
        "/" => "Обзор финансов",
        "/transactions" | "/transactions/new" => "Транзакции",
        "/categories" => "Категории",
        "/budgets" => "Бюджеты",
        "/recurring" => "Регулярные платежи",
        "/analytics" => "Аналитика",
        "/profile" => "Настройки профиля",
        _ => "SpendWise",
    }
}

#[component]
pub fn Navbar() -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let path = ctx.path;
    let menu = ctx.mobile_menu;
    let add = ctx.clone();
    view! {<header class="sticky top-0 z-20 flex h-19 min-w-0 items-center justify-between gap-3 border-b border-slate-200/80 bg-white/90 px-4 backdrop-blur-lg dark:border-slate-800 dark:bg-slate-950/90 sm:px-8">
    <div class="flex min-w-0 items-center gap-3"><button class="icon-btn lg:hidden" aria-label="Открыть меню" aria-expanded=move ||menu.get().to_string() on:click=move |_|menu.set(true)><Icon name="menu" /></button><span class="truncate text-sm font-medium text-slate-500 dark:text-slate-400">{move || page_title(&path.get())}</span></div><div class="flex shrink-0 items-center gap-2 sm:gap-5"><ThemeToggle /><button type="button" class="btn-primary px-3 sm:px-4" on:click=move |_|add.navigate("/transactions/new")><Icon name="plus" class="size-4" /><span class="hidden sm:inline">"Добавить операцию"</span><span class="sr-only sm:hidden">"Добавить операцию"</span></button></div>
    </header>}
}
