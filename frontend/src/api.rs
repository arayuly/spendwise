//! Same-origin API transport. Access tokens live only in reactive memory.
use async_lock::Mutex;
use gloo_net::http::{Method, Request, RequestBuilder, Response};
use leptos::prelude::*;
use serde::{Serialize, de::DeserializeOwned};
use spendwise_shared::{AuthResponse, CsrfResponse, User};
use std::{cell::RefCell, future::Future, rc::Rc, sync::Arc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, future_to_promise};

#[derive(Clone)]
pub struct Session {
    pub access_token: String,
    pub csrf_token: String,
    pub user: User,
}
impl From<AuthResponse> for Session {
    fn from(auth: AuthResponse) -> Self {
        Self {
            access_token: auth.access_token,
            csrf_token: auth.csrf_token,
            user: auth.user,
        }
    }
}
#[derive(Clone)]
pub struct Api {
    pub session: RwSignal<Option<Session>>,
    refresh_lock: Arc<Mutex<()>>,
}
impl Api {
    pub fn new() -> Self {
        Self {
            session: RwSignal::new(None),
            refresh_lock: Arc::new(Mutex::new(())),
        }
    }
    pub async fn restore(&self) -> Result<(), String> {
        if let Some(auth) = with_session_lock(bootstrap_session()).await? {
            self.session.set(Some(auth.into()));
        }
        Ok(())
    }
    async fn refresh(&self, previous: &str) -> Result<(), String> {
        let _guard = self.refresh_lock.lock().await;
        let session = self
            .session
            .get_untracked()
            .ok_or("Сессия завершена. Войдите снова.")?;
        if session.access_token != previous {
            return Ok(());
        }
        match with_session_lock(bootstrap_session()).await? {
            Some(auth) => self.session.set(Some(auth.into())),
            None => {
                self.session.set(None);
                return Err("Сессия завершена. Войдите снова.".into());
            }
        }
        Ok(())
    }
    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Response, String> {
        let before = self.session.get_untracked();
        let send = |session: Option<Session>| {
            let mut request = RequestBuilder::new(&format!("/api/v1{path}"))
                .method(method.clone())
                .credentials(web_sys::RequestCredentials::SameOrigin)
                .header("Accept", "application/json");
            if let Some(session) = session {
                request = request
                    .header("Authorization", &format!("Bearer {}", session.access_token))
                    .header("X-CSRF-Token", &session.csrf_token);
            }
            let request = match &body {
                Some(body) => request
                    .header("Content-Type", "application/json")
                    .body(body.clone()),
                None => request.build(),
            };
            async move {
                request
                    .map_err(|_| "Не удалось подготовить запрос".to_string())?
                    .send()
                    .await
                    .map_err(|_| {
                        "Не удалось связаться с сервером. Проверьте подключение.".to_string()
                    })
            }
        };
        let mut response = send(before.clone()).await?;
        if response.status() == 401
            && let Some(session) = before
        {
            self.refresh(&session.access_token).await?;
            response = send(self.session.get_untracked()).await?;
        }
        // Another browser tab may have rotated the shared HttpOnly cookie.
        if response.status() == 403
            && path.starts_with("/auth/logout")
            && let Some(token) = fetch_csrf().await?
        {
            self.session.update(|s| {
                if let Some(s) = s {
                    s.csrf_token = token;
                }
            });
            response = send(self.session.get_untracked()).await?;
        }
        if !response.ok() {
            return Err(response_error(response).await);
        }
        Ok(response)
    }
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        self.json(Method::GET, path, None).await
    }
    pub async fn mutate<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let body =
            serde_json::to_string(body).map_err(|_| "Не удалось подготовить данные".to_string())?;
        self.json(method, path, Some(body)).await
    }
    async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<T, String> {
        self.send(method, path, body)
            .await?
            .json()
            .await
            .map_err(|_| "Некорректный ответ сервера".into())
    }
    pub async fn empty(&self, method: Method, path: &str) -> Result<(), String> {
        self.send(method, path, None).await.map(|_| ())
    }
    pub async fn password<B: Serialize>(&self, body: &B) -> Result<(), String> {
        let body =
            serde_json::to_string(body).map_err(|_| "Не удалось подготовить данные".to_string())?;
        self.send(Method::POST, "/users/password", Some(body))
            .await
            .map(|_| ())
    }
    pub async fn export(&self, from: &str, to: &str) -> Result<(), String> {
        let response = self
            .send(
                Method::GET,
                &format!("/export/transactions?from={from}&to={to}"),
                None,
            )
            .await?;
        let bytes = response
            .binary()
            .await
            .map_err(|_| "Не удалось скачать файл".to_string())?;
        let array = js_sys::Uint8Array::from(bytes.as_slice());
        let parts = js_sys::Array::new();
        parts.push(&array.buffer());
        let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)
            .map_err(|_| "Не удалось создать CSV".to_string())?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|_| "Не удалось создать ссылку на файл".to_string())?;
        let result: Result<(), &str> = (|| {
            let document = web_sys::window()
                .and_then(|w| w.document())
                .ok_or("Браузер недоступен")?;
            let anchor = document
                .create_element("a")
                .map_err(|_| "Не удалось скачать файл")?
                .dyn_into::<web_sys::HtmlAnchorElement>()
                .map_err(|_| "Не удалось скачать файл")?;
            anchor.set_href(&url);
            anchor.set_download(&format!("spendwise_{from}_{to}.csv"));
            if let Some(body) = document.body() {
                let _ = body.append_child(&anchor);
            }
            anchor.click();
            anchor.remove();
            Ok(())
        })();
        let _ = web_sys::Url::revoke_object_url(&url);
        result.map_err(String::from)
    }
}

async fn fetch_csrf() -> Result<Option<String>, String> {
    let response = Request::get("/api/v1/auth/csrf")
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .send()
        .await
        .map_err(|_| "Не удалось связаться с сервером. Проверьте подключение.".to_string())?;
    if response.status() == 401 {
        return Ok(None);
    }
    if !response.ok() {
        return Err(response_error(response).await);
    }
    response
        .json::<CsrfResponse>()
        .await
        .map(|r| Some(r.csrf_token))
        .map_err(|_| "Некорректный ответ сервера".into())
}
async fn bootstrap_session() -> Result<Option<AuthResponse>, String> {
    let Some(csrf) = fetch_csrf().await? else {
        return Ok(None);
    };
    let response = Request::post("/api/v1/auth/refresh")
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .header("X-CSRF-Token", &csrf)
        .send()
        .await
        .map_err(|_| "Не удалось восстановить сессию".to_string())?;
    if response.status() == 401 {
        return Ok(None);
    }
    if !response.ok() {
        return Err(response_error(response).await);
    }
    response
        .json()
        .await
        .map(Some)
        .map_err(|_| "Некорректный ответ сервера".into())
}
// Web Locks serialize refresh-cookie rotation across tabs. The in-memory mutex
// above also deduplicates parallel expired requests within this tab.
async fn with_session_lock<T: 'static>(
    operation: impl Future<Output = Result<T, String>> + 'static,
) -> Result<T, String> {
    let locks = js_sys::Reflect::get(&js_sys::global(), &"navigator".into())
        .ok()
        .and_then(|n| js_sys::Reflect::get(&n, &"locks".into()).ok());
    let request = locks
        .as_ref()
        .and_then(|l| js_sys::Reflect::get(l, &"request".into()).ok())
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
    let (Some(locks), Some(request)) = (locks, request) else {
        return operation.await;
    };
    let outcome = Rc::new(RefCell::new(None));
    let result = outcome.clone();
    let callback = Closure::once(move |_: JsValue| {
        future_to_promise(async move {
            result.replace(Some(operation.await));
            Ok(JsValue::UNDEFINED)
        })
    });
    let promise = request
        .call2(&locks, &"spendwise-session".into(), callback.as_ref())
        .map_err(|_| "Не удалось восстановить сессию".to_string())?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "Браузер не поддерживает блокировку сессии".to_string())?;
    JsFuture::from(promise)
        .await
        .map_err(|_| "Не удалось восстановить сессию".to_string())?;
    outcome
        .borrow_mut()
        .take()
        .unwrap_or_else(|| Err("Не удалось восстановить сессию".into()))
}
async fn response_error(response: Response) -> String {
    let status = response.status();
    let message = response
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|value| {
            value
                .get("error")?
                .get("message")?
                .as_str()
                .map(str::to_owned)
        });
    message.unwrap_or_else(|| match status {
        401 => "Сессия завершена. Войдите снова.".into(),
        403 => "Доступ запрещён. Обновите страницу и повторите попытку.".into(),
        429 => "Слишком много попыток. Подождите немного.".into(),
        500..=599 => "Сервер временно недоступен. Попробуйте позже.".into(),
        _ => format!("Запрос не выполнен (код {status})"),
    })
}
