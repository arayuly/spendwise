use super::*;
use crate::{app::AppContext, components::*};
use gloo_net::http::Method;
use leptos::prelude::*;
use spendwise_shared::*;

#[component]
pub fn ProfilePage() -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let session = ctx.api.session.get_untracked();
    let name = RwSignal::new(
        session
            .as_ref()
            .map(|s| s.user.display_name.clone())
            .unwrap_or_default(),
    );
    let currency = RwSignal::new(
        session
            .as_ref()
            .map(|s| s.user.currency.clone())
            .unwrap_or_else(|| "KZT".into()),
    );
    let email = session.map(|s| s.user.email).unwrap_or_default();
    let current = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let confirm_password = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let password_busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let password_error = RwSignal::new(None::<String>);
    let logout_busy = RwSignal::new(false);
    let logout_all = RwSignal::new(false);
    let profile_ctx = ctx.clone();
    let password_ctx = ctx.clone();
    let logout_ctx = ctx.clone();
    let logout_all_ctx = ctx.clone();
    let save = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let name = name.get_untracked().trim().to_string();
        if name.is_empty() {
            error.set(Some("Укажите ваше имя".into()));
            return;
        }
        let input = ProfileInput {
            display_name: name,
            currency: currency.get_untracked(),
        };
        busy.set(true);
        error.set(None);
        let ctx = profile_ctx.clone();
        leptos::task::spawn_local(async move {
            let result = ctx
                .api
                .mutate::<_, User>(Method::PATCH, "/users/me", &input)
                .await;
            busy.try_set(false);
            match result {
                Ok(user) => {
                    ctx.api.session.update(|s| {
                        if let Some(s) = s {
                            s.user = user;
                        }
                    });
                    ctx.notify("Профиль обновлён", true);
                }
                Err(e) => {
                    error.try_set(Some(e));
                }
            }
        });
    };
    let change_password = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if password_busy.get_untracked() {
            return;
        }
        let new_password = password.get_untracked();
        if new_password.len() < 12 || new_password.len() > 128 {
            password_error.set(Some(
                "Новый пароль должен содержать от 12 до 128 символов".into(),
            ));
            return;
        }
        if new_password != confirm_password.get_untracked() {
            password_error.set(Some("Пароли не совпадают".into()));
            return;
        }
        let input = PasswordInput {
            current_password: current.get_untracked(),
            new_password,
        };
        password_busy.set(true);
        password_error.set(None);
        let ctx = password_ctx.clone();
        leptos::task::spawn_local(async move {
            let result = ctx.api.password(&input).await;
            password_busy.try_set(false);
            current.try_set(String::new());
            password.try_set(String::new());
            confirm_password.try_set(String::new());
            match result {
                Ok(()) => {
                    ctx.api.session.set(None);
                    ctx.navigate("/login");
                    ctx.notify("Пароль изменён. Войдите с новым паролем.", true);
                }
                Err(e) => {
                    password_error.try_set(Some(e));
                }
            }
        });
    };
    let confirm_logout_all = Callback::new(move |_| {
        if logout_busy.get_untracked() {
            return;
        }
        logout_busy.set(true);
        let ctx = logout_all_ctx.clone();
        leptos::task::spawn_local(async move {
            let result = ctx.api.empty(Method::POST, "/auth/logout-all").await;
            logout_busy.try_set(false);
            match result {
                Ok(()) => {
                    ctx.api.session.set(None);
                    ctx.navigate("/login");
                    ctx.notify("Все сессии завершены", true);
                }
                Err(e) => ctx.notify(e, false),
            }
        });
    });
    view! {<PageHeading title="Настройки профиля" description="Личные данные, валюта учёта и безопасность аккаунта."><span></span></PageHeading><div class="grid gap-5 xl:grid-cols-2"><section class="card"><div class="mb-6 flex items-center gap-3"><span class="rounded-xl bg-brand-50 p-3 text-brand-600 dark:bg-brand-950/40"><Icon name="profile" /></span><div><h2 class="section-title">"Личные данные"</h2><p class="mt-1 text-xs text-slate-400">"Как обращаться к вам в SpendWise"</p></div></div><form class="space-y-5" on:submit=save><Input label="Имя" id="profile-name" value=name required=true autocomplete="name" maxlength=100 /><div class="field"><span>"Email"</span><p class="input bg-slate-50 text-slate-500 dark:bg-slate-950">{email}</p></div><Select label="Основная валюта" id="profile-currency" value=currency><option value="KZT">"KZT — Казахстанский тенге"</option><option value="USD">"USD — Доллар США"</option><option value="EUR">"EUR — Евро"</option></Select><p class="text-xs leading-6 text-slate-400">"Все суммы ведутся в одной валюте. Выберите её до добавления финансовых данных: после создания операций, бюджетов или регулярных платежей смена валюты недоступна. Конвертации нет."</p>{move ||error.get().map(|e|view!{<Alert text=e />})}<Button kind="submit" busy=Signal::derive(move ||busy.get())>"Сохранить изменения"</Button></form></section>
        <section class="card"><div class="mb-6 flex items-center gap-3"><span class="rounded-xl bg-brand-50 p-3 text-brand-600 dark:bg-brand-950/40"><Icon name="shield" /></span><div><h2 class="section-title">"Смена пароля"</h2><p class="mt-1 text-xs text-slate-400">"Все текущие сессии будут завершены"</p></div></div><form class="space-y-5" on:submit=change_password><Input label="Текущий пароль" id="password-current" value=current kind="password" required=true autocomplete="current-password" maxlength=128 /><Input label="Новый пароль" id="password-new" value=password kind="password" required=true autocomplete="new-password" maxlength=128 /><Input label="Повторите новый пароль" id="password-confirm" value=confirm_password kind="password" required=true autocomplete="new-password" maxlength=128 /><p class="text-xs text-slate-400">"От 12 до 128 символов. Используйте уникальный пароль."</p>{move ||password_error.get().map(|e|view!{<Alert text=e />})}<Button kind="submit" busy=Signal::derive(move ||password_busy.get())>"Изменить пароль"</Button></form></section>
        <section class="card xl:col-span-2"><h2 class="section-title">"Сессии и оформление"</h2><p class="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">"Вы можете выйти на этом устройстве или завершить все активные сессии аккаунта."</p><div class="mt-6 flex flex-wrap items-center gap-3"><Button class="btn-secondary" busy=Signal::derive(move ||logout_busy.get()) on_click=Callback::new(move |_|{if logout_busy.get_untracked(){return;}logout_busy.set(true);let ctx=logout_ctx.clone();leptos::task::spawn_local(async move{let result=ctx.api.empty(Method::POST,"/auth/logout").await;logout_busy.try_set(false);match result{Ok(())=>{ctx.api.session.set(None);ctx.navigate("/login");},Err(e)=>ctx.notify(e,false)}});})><Icon name="logout" class="size-4" />"Выйти"</Button><Button class="btn-secondary text-red-600 dark:text-red-400" on_click=Callback::new(move |_|logout_all.set(true))>"Выйти на всех устройствах"</Button><div class="sm:ml-auto"><ThemeToggle /></div></div></section></div>
        <Show when=move ||logout_all.get()><ConfirmDialog title="Завершить все сессии?" text="Вы выйдете из SpendWise на всех устройствах, включая это. Для продолжения потребуется войти снова." busy=logout_busy close=Callback::new(move |_|logout_all.set(false)) confirm=confirm_logout_all /></Show>
    }
}
