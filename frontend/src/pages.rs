mod auth;
mod dashboard;
mod manage;
mod profile;
mod transactions;
pub use auth::*;
pub use dashboard::*;
pub use manage::*;
pub use profile::*;
pub use transactions::*;

use crate::{app::AppContext, components::*};
use leptos::prelude::*;
use serde::de::DeserializeOwned;

pub struct Load<T: 'static> {
    pub data: RwSignal<Option<T>>,
    pub loading: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
}
impl<T> Copy for Load<T> {}
impl<T> Clone for Load<T> {
    fn clone(&self) -> Self {
        *self
    }
}
pub fn use_api<T: DeserializeOwned + Clone + Send + Sync + 'static>(
    path: impl Fn() -> String + Send + Sync + 'static,
) -> Load<T> {
    let ctx = expect_context::<AppContext>();
    let load = Load {
        data: RwSignal::new(None),
        loading: RwSignal::new(true),
        error: RwSignal::new(None),
    };
    let generation = RwSignal::new(0_u64);
    Effect::new(move |_| {
        let _ = ctx.revision.get();
        let url = path();
        let api = ctx.api.clone();
        let id = generation.get_untracked() + 1;
        generation.set(id);
        load.loading.set(true);
        load.error.set(None);
        load.data.set(None);
        leptos::task::spawn_local(async move {
            let result = api.get::<T>(&url).await;
            if generation.try_get_untracked() != Some(id) {
                return;
            }
            match result {
                Ok(data) => {
                    load.data.try_set(Some(data));
                }
                Err(error) => {
                    load.error.try_set(Some(error));
                }
            }
            load.loading.try_set(false);
        });
    });
    load
}

#[component]
pub fn LoadError(error: RwSignal<Option<String>>) -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    view! {{move ||error.get().map(|message|view!{<div class="space-y-3"><Alert text=message /><button type="button" class="btn-secondary" on:click={let ctx=ctx.clone();move |_|ctx.changed()}>"Повторить загрузку"</button></div>})}}
}

#[component]
pub fn PageHeading(
    #[prop(into)] title: String,
    #[prop(into)] description: String,
    children: Children,
) -> impl IntoView {
    view! {<div class="mb-7 flex flex-wrap items-start justify-between gap-4"><div class="min-w-0"><h1 class="text-2xl font-semibold tracking-tight sm:text-3xl">{title}</h1><p class="mt-2 max-w-2xl text-sm leading-6 text-slate-500 dark:text-slate-400">{description}</p></div><div class="flex flex-wrap items-center gap-3">{children()}</div></div>}
}

#[component]
pub fn NotFoundPage() -> impl IntoView {
    let ctx = expect_context::<AppContext>();
    view! {<div class="card mx-auto mt-12 max-w-xl py-14 text-center"><p class="text-6xl font-bold text-brand-600">"404"</p><h1 class="mt-5 text-2xl font-semibold">"Страница не найдена"</h1><p class="mt-3 text-sm text-slate-500">"Проверьте адрес или вернитесь к обзору финансов."</p><button class="btn-primary mt-7" on:click=move |_|ctx.navigate("/")>"На главную"</button></div>}
}
