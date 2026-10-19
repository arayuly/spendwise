use crate::{app::AppContext, components::*};
use gloo_net::http::Method;
use leptos::prelude::*;
use spendwise_shared::{AuthResponse, LoginInput, RegisterInput};

#[component]
pub fn AuthPage(restore_error: RwSignal<Option<String>>) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    let path = ctx.path;
    let register = move || path.get() == "/register";
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let submit_ctx = ctx.clone();
    let switch_ctx = ctx.clone();
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let is_register = register();
        let password_value = password.get_untracked();
        if is_register && (password_value.len() < 12 || password_value.len() > 128) {
            error.set(Some(
                "Пароль должен содержать от 12 до 128 символов.".into(),
            ));
            return;
        }
        if is_register && name.get_untracked().trim().is_empty() {
            error.set(Some("Укажите ваше имя.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        let ctx = submit_ctx.clone();
        let email_value = email.get_untracked().trim().to_string();
        let display_name = name.get_untracked().trim().to_string();
        leptos::task::spawn_local(async move {
            let result: Result<AuthResponse, String> = if is_register {
                ctx.api
                    .mutate(
                        Method::POST,
                        "/auth/register",
                        &RegisterInput {
                            email: email_value,
                            password: password_value,
                            display_name,
                        },
                    )
                    .await
            } else {
                ctx.api
                    .mutate(
                        Method::POST,
                        "/auth/login",
                        &LoginInput {
                            email: email_value,
                            password: password_value,
                        },
                    )
                    .await
            };
            password.try_set(String::new());
            busy.try_set(false);
            match result {
                Ok(auth) => {
                    ctx.api.session.set(Some(auth.into()));
                    ctx.navigate("/");
                }
                Err(message) => {
                    error.try_set(Some(message));
                }
            }
        });
    };
    view! {<main id="main-content" class="grid min-h-screen lg:grid-cols-2"><section class="relative hidden overflow-hidden bg-brand-800 p-14 text-white lg:flex lg:flex-col"><div class="flex items-center gap-3 text-2xl font-bold"><span class="rounded-xl bg-white/10 p-2"><Icon name="wallet" class="size-7" /></span>"SpendWise"</div><div class="my-auto max-w-lg py-20"><span class="mb-6 inline-flex items-center gap-2 rounded-full border border-white/20 px-3 py-1.5 text-xs text-emerald-100"><span class="size-1.5 rounded-full bg-emerald-300"></span>"Всё о ваших финансах — в одном месте"</span><h1 class="text-5xl font-semibold leading-[1.15] tracking-tight">"Меньше суеты."<br/>"Больше ясности."</h1><p class="mt-6 max-w-md text-lg leading-8 text-emerald-100/75">"Знайте, куда уходят деньги. Планируйте расходы и создавайте привычки, которые приближают к целям."</p><div class="mt-10 grid grid-cols-3 gap-4 border-t border-white/15 pt-7">{[("transactions","Удобный учёт"),("budgets","Гибкие бюджеты"),("analytics","Ясная аналитика")].into_iter().map(|(icon,label)|view!{<div><span class="text-emerald-300"><Icon name=icon /></span><p class="mt-3 text-xs text-emerald-100">{label}</p></div>}).collect_view()}</div></div><p class="text-xs text-emerald-100/50">"Ваш следующий шаг к финансовому спокойствию."</p><div class="pointer-events-none absolute -bottom-52 -right-36 size-[500px] rounded-full border-[60px] border-white/[0.025]"></div></section>
    <section class="flex min-w-0 flex-col px-5 py-6 sm:px-12"><div class="flex items-center justify-between"><a href="/" class="flex items-center gap-2 font-bold lg:invisible"><span class="text-brand-600"><Icon name="wallet" /></span>"SpendWise"</a><ThemeToggle /></div><div class="mx-auto my-auto w-full max-w-sm py-12"><span class="mb-6 flex size-12 items-center justify-center rounded-2xl bg-brand-50 text-brand-600 dark:bg-brand-950/40"><Icon name="shield" class="size-6" /></span><h2 class="text-3xl font-semibold tracking-tight">{move ||if register(){"Создайте аккаунт"}else{"С возвращением"}}</h2><p class="mt-3 text-sm leading-6 text-slate-500 dark:text-slate-400">{move ||if register(){"Начните путь к осознанным финансам."}else{"Войдите, чтобы продолжить управлять финансами."}}</p>
        <form class="mt-8 space-y-5" on:submit=submit><Show when=register><Input label="Ваше имя" id="auth-name" value=name required=true autocomplete="name" maxlength=100 /></Show><Input label="Email" id="auth-email" value=email kind="email" required=true autocomplete="email" maxlength=254 /><Input label="Пароль" id="auth-password" value=password kind="password" required=true autocomplete="current-password" maxlength=128 />
            <Show when=register><p class="-mt-2 text-xs leading-5 text-slate-500">"От 12 до 128 символов. Используйте уникальный пароль."</p></Show>
            {move ||error.get().map(|e|view!{<Alert text=e />})}{move ||restore_error.get().map(|e|view!{<Alert text=e />})}
            <Button kind="submit" class="btn-primary w-full" busy=Signal::derive(move ||busy.get())>{move ||if register(){"Создать аккаунт"}else{"Войти в аккаунт"}}<Icon name="chevron" class="size-4" /></Button>
        </form><p class="mt-7 text-center text-sm text-slate-500">{move ||if register(){"Уже есть аккаунт? "}else{"Впервые здесь? "}}<button type="button" class="font-semibold text-brand-600 hover:text-brand-700 dark:text-brand-400" on:click=move |_|{error.set(None);password.try_set(String::new());switch_ctx.navigate(if register(){"/login"}else{"/register"});}>{move ||if register(){"Войти"}else{"Зарегистрироваться"}}</button></p>
    </div><p class="text-center text-xs leading-5 text-slate-400">"SpendWise · Личные финансы с ясной картиной"</p></section></main>}
}
