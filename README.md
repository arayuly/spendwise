# SpendWise

SpendWise — приложение для личных финансов: доходы и расходы, категории,
месячные бюджеты, аналитика, регулярные платежи, CSV и управление профилем.
Все данные находятся в PostgreSQL; интерфейс не подменяет ответы API фиктивной
статистикой. Один аккаунт использует одну валюту: KZT по умолчанию, USD или EUR.

Это модульный монолит: **Axum REST API + Leptos CSR/WebAssembly**, единый origin
в production, SQLx/PostgreSQL 18 и Tailwind CSS **v4** через официальный CLI.
Node.js используется только для сборки CSS. Проверки конкретной рабочей среды
записаны в [docs/verification.md](docs/verification.md); наличие конфигурации
production само по себе не означает проведённый аудит или опубликованный сервис.

## Архитектура

```text
Browser: Leptos CSR + SVG charts + Tailwind v4
  └── HTTPS /api/v1 → Caddy → Axum
                             ├── auth: Argon2id, JWT, refresh sessions
                             ├── finance handlers → services → repository
                             ├── SQLx migrations / PostgreSQL
                             └── recurring worker (every 60 seconds, UTC)
```

- `backend/src/` — конфигурация, HTTP, авторизация, одноразовые auth tokens,
  валидация, SQL и планировщик.
- `backend/migrations/` — таблицы, индексы, внешние ключи и финансовые ограничения.
- `backend/tests/` — API и PostgreSQL integration tests.
- `shared/src/` — общие сериализуемые DTO для клиента и сервера.
- `frontend/src/` — страницы, компоненты, клиент API и состояние интерфейса.
- `frontend/styles/input.css` — CSS-first тема Tailwind v4.
- `docker/` — Caddy и инициализация PostgreSQL с отдельной ролью приложения.
- `.github/workflows/ci.yml` — форматирование, Clippy, тесты, WASM и Docker.
- `docs/openapi.json` — OpenAPI 3.1; `scripts/` — его генератор, секреты и backup.

Деньги хранятся как `NUMERIC(18,2)` и `Decimal`, в JSON передаются строками,
например `"12500.50"`. Доходы и расходы задаются положительной суммой и типом
`income`/`expense`. Итоги вычисляются SQL-агрегациями из транзакций. Валюту
можно изменить до создания финансовых записей; исторические суммы не конвертируются.

## Быстрый запуск через Docker

Нужны Docker Engine/Desktop с Linux containers и Docker Compose ≥ 2.24.4.
Серверы PostgreSQL и Node.js на хосте для этого варианта не нужны.
В PowerShell из корня проекта:

```powershell
.\scripts\init-env.ps1
docker compose config --quiet
docker compose up --build -d
docker compose ps
```

Откройте **http://localhost:8080**. База данных наружу не публикуется; приложение
в локальной конфигурации слушает только `127.0.0.1:8080` хоста. Первый запуск
загружает образы, компилирует Rust/WASM и применяет миграции.

`init-env.ps1` создаёт `.env` с независимыми случайными ключами и не перезаписывает
существующий файл. В Linux/macOS скопируйте `.env.example` в `.env` и заполните
`JWT_SECRET` результатом `openssl rand -hex 48`, а оба пароля базы — отдельными
вызовами `openssl rand -hex 32`. `.env` исключён из Git и Docker context.

Создайте первого пользователя на странице регистрации. Нужны корректный email,
имя и пароль длиной 12–128 символов. Регистрация атомарно создаёт пользователя,
сессию и стандартные категории. Администратора или общего пароля по умолчанию нет.
Выберите валюту в настройках **до** внесения финансовых данных.

```powershell
docker compose logs -f app
docker compose stop
docker compose up -d
```

`docker compose down` сохраняет named volume. Опция `down -v` удаляет базу и
не нужна для обновления или обычного перезапуска.

## Локальная разработка без сборки Docker image

Используемые версии: Rust 1.99 (edition 2024), target `wasm32-unknown-unknown`,
Trunk 0.21.14, Node.js 24 LTS, npm и PostgreSQL 18. Нужны системные build tools
Rust; на Windows — MSVC Build Tools. Версии библиотек закреплены Cargo.lock
и frontend/package-lock.json.

```powershell
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
.\scripts\init-env.ps1
```

Для локальной БД можно использовать установленную PostgreSQL: создайте роль
`spendwise` и базу `spendwise`, владельцем которой является эта роль; задайте
пароль из `.env`. Либо создайте отдельный development container, прочитав пароль
из уже подготовленного файла, без вывода секрета в консоль:

```powershell
$dbPassword = ((Get-Content .env | Where-Object { $_ -match '^POSTGRES_PASSWORD=' }) -split '=', 2)[1]
docker run --detach --name spendwise-local-db --publish 127.0.0.1:5432:5432 --env POSTGRES_USER=spendwise --env POSTGRES_DB=spendwise --env "POSTGRES_PASSWORD=$dbPassword" --volume spendwise-local-data:/var/lib/postgresql postgres:18
```

Проверьте `DATABASE_URL` в `.env`. Для режима Trunk измените только
`APP_ORIGIN=http://localhost:3000`; `COOKIE_SECURE=false` разрешён для localhost.
Backend читает `.env` из текущей директории. В трёх терминалах:

```powershell
# Терминал 1, корень проекта
cargo run --locked -p backend
```

```powershell
# Терминал 2
cd frontend
npm ci
npm run css:dev
```

```powershell
# Терминал 3
cd frontend
trunk serve --address 127.0.0.1
```

Откройте http://localhost:3000. Trunk направляет `/api/v1/` на backend:8080.
Не смешивайте `localhost` и `127.0.0.1` в URL браузера: Origin должен совпадать
с настройкой. При запуске готового `frontend/dist` через Axum верните
`APP_ORIGIN=http://localhost:8080`.

### Миграции

По умолчанию сервер применяет встроенные SQLx migrations при старте, прежде
чем начинает слушать HTTP. SQLx использует PostgreSQL advisory lock, поэтому
одновременный старт экземпляров не применяет одну миграцию дважды. Ошибка
подключения или миграции прекращает запуск.

Для отдельного этапа миграции установите SQLx CLI:

```powershell
cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features rustls,postgres
sqlx migrate run --source backend/migrations
```

Команда использует `DATABASE_URL` из `.env`. Затем можно задать
`RUN_MIGRATIONS=false` для экземпляров приложения. При обновлении сначала
сделайте backup. Применённые миграции не редактируйте: добавляйте новые файлы.

## Переменные окружения

| Переменная | Назначение / значение по умолчанию |
| --- | --- |
| `APP_ENV` | `development`, `production` или `test`; по умолчанию development |
| `DATABASE_URL` | Обязательный PostgreSQL URL; Compose задаёт адрес `db` |
| `JWT_SECRET` | Обязательный случайный секрет ≥48 символов, без example значений |
| `POSTGRES_PASSWORD` | Пароль роли приложения; для Compose используйте URL-safe hex |
| `POSTGRES_ADMIN_PASSWORD` | Отдельный пароль postgres; только контейнер БД |
| `APP_ORIGIN` | Точный origin без завершающего `/`; default http://localhost:8080 |
| `COOKIE_SECURE` | false локально, обязательно true в production |
| `HOST` / `PORT` | 127.0.0.1 / 8080 локально; контейнер слушает 0.0.0.0:8080 |
| `FRONTEND_DIR` | frontend/dist локально; /app/public в image |
| `RUN_MIGRATIONS` | true; применять SQLx migrations при запуске |
| `ACCESS_TOKEN_MINUTES` | 10, допустимо 1–15 |
| `REFRESH_TOKEN_DAYS` | 30, допустимо 1–90 |
| `DATABASE_MAX_CONNECTIONS` | 10, допустимо 1–100 |
| `AUTH_RATE_LIMIT_PER_MINUTE` | 20 на peer IP и endpoint, допустимо 1–300 |
| `RUST_LOG` | backend=info,tower_http=info |
| `DOMAIN` / `ACME_EMAIL` | Домен и email ACME для production Caddy |

Production требует HTTPS origin, Secure cookie и собранный `index.html`.
Невалидная конфигурация прекращает запуск с диагностикой. Не используйте
production-секреты в тестах и не записывайте `.env`, tokens или backup в Git.

## API

Префикс `/api/v1`. Спецификация доступна на `/api/v1/openapi.json` и в
[docs/openapi.json](docs/openapi.json). При изменении API обновляйте генератор
и запускайте `python scripts/generate_openapi.py`.

| Маршруты | Действия |
| --- | --- |
| `/auth/register`, `/auth/login`, `/auth/refresh` | POST: регистрация, вход, ротация refresh |
| `/auth/csrf` | GET: восстановление CSRF после перезагрузки |
| `/auth/logout`, `/auth/logout-all` | POST: текущая сессия / все устройства |
| `/auth/me`, `/users/me` | GET: профиль; PATCH `/users/me`: имя и валюта |
| `/users/password` | POST: смена пароля и отзыв всех сессий |
| `/transactions`, `/transactions/{id}` | GET/POST список; GET/PATCH/DELETE запись |
| `/categories`, `/categories/{id}` | GET/POST; PATCH/DELETE (архивирование) |
| `/budgets`, `/budgets/{id}` | GET/POST; PATCH/DELETE |
| `/recurring`, `/recurring/{id}` | GET/POST; PATCH/DELETE |
| `/analytics/summary`, `/analytics/categories` | GET: итоговые суммы / расходы по категориям |
| `/analytics/monthly`, `/analytics/balance` | GET: месячные суммы / ежедневный баланс |
| `/export/transactions` | GET: CSV только собственных записей |

Защищённые endpoints требуют `Authorization: Bearer <access_token>`.
Все изменяющие API-запросы требуют `Origin: <APP_ORIGIN>`, в том числе CLI.
Refresh/logout требуют HttpOnly cookie и `X-CSRF-Token`; logout-all также bearer.
Access token хранится только в памяти клиента. После reload клиент получает
CSRF и обновляет токен через cookie. При ротации старая refresh-сессия отзывается;
её повторное использование отзывает семейство сессий устройства.
Выход с устройства также отзывает его семейство; access JWT проверяется против
сессии в PostgreSQL и после отзыва сразу становится недействительным.
В production cookie называется `__Host-spendwise_refresh` и имеет `Secure`,
`HttpOnly`, `SameSite=Strict`, `Path=/` без Domain. При локальном HTTP используется
`spendwise_refresh` с `Path=/api/v1/auth`.

Транзакции поддерживают `from`, `to` (включительно), `category_id`,
`transaction_type`, `search`, `sort` (`date_desc`, `date_asc`, `amount_desc`,
`amount_asc`), `page` (от 1), `per_page` (1–100, default 20).
Ответ: `{ "items": [], "total": 0, "page": 1, "per_page": 20 }`.
PATCH ресурсов принимает полную DTO; необязательное описание может быть null.
Бюджет месяца использует `YYYY-MM-01`; `category_id=null` означает общий лимит.
Поля `spent` и аналитические суммы вычисляются из реальных транзакций.

Архив категории сохраняет историю и деактивирует её регулярные правила.
Восстановление категории не активирует их автоматически. CSV ограничен 100000
строками: для большей истории сужайте фильтры. CSV передаётся потоком по 500 строк
в согласованном repeatable-read snapshot; на процесс разрешены два параллельных
экспорта, дальнейшие запросы получают 429. Формулы в текстовых полях CSV
нейтрализуются. Графики ограничены 3660 днями; даты планировщика считаются в UTC.
Правило 31-го числа использует последний день короткого месяца, сохраняя
исходный anchor: 31 января → 28/29 февраля → 31 марта. Row locks и уникальные
occurrences обеспечивают идемпотентность и при нескольких экземплярах сервера.
Изменения категорий и генерация используют транзакционные advisory locks
пользователя до row locks: архивирование и смена типа — exclusive, scheduler
и запись recurring — shared. Порядок блокировок пользователей согласован по UUID.

Ошибки имеют вид:

```json
{"error":{"code":"VALIDATION_ERROR","message":"Описание ошибки","details":{}}}
```

Создание возвращает 201, удаление 204, невалидные данные 422, отсутствие
авторизации 401, неправильный Origin/CSRF 403, чужой/несуществующий ресурс 404,
конфликт 409, rate limit 429. `GET /health/live` проверяет процесс;
`GET /health/ready` и `/api/v1/health` проверяют подключение к БД.

## Проверки

SQLx integration tests создают отдельные временные базы, поэтому тестовой роли
нужно `CREATEDB`. Используйте только отдельную тестовую PostgreSQL. В PowerShell:

```powershell
docker run --detach --name spendwise-test-db --publish 127.0.0.1:54329:5432 --env POSTGRES_USER=spendwise_test --env POSTGRES_DB=spendwise_test --env POSTGRES_PASSWORD=spendwise_test_local_only postgres:18
$env:DATABASE_URL='postgresql://spendwise_test:spendwise_test_local_only@127.0.0.1:54329/spendwise_test'
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked -p backend
cargo check --locked -p frontend --target wasm32-unknown-unknown
```

Пароль в этом примере относится только к disposable тестовому контейнеру.
Чтобы backend снова использовал `.env`, удалите временное переопределение:
`Remove-Item Env:DATABASE_URL`. Полная production-сборка frontend:

```powershell
cd frontend
npm ci
npm run css:build
trunk build --release --locked
```

CI повторяет проверки на PostgreSQL service и собирает Docker image без
публикации. Тесты проверяют изоляцию данных, авторизацию, refresh/logout,
финансовую валидацию, бюджеты, агрегации, CSV и recurring idempotency.

## Публичное развёртывание с HTTPS

1. Настройте DNS `DOMAIN` на сервер. Откройте входящие TCP 80/443 и при
   необходимости UDP 443. Не открывайте PostgreSQL или app:8080 наружу.
2. Создайте `.env` с новыми независимыми production-секретами. Укажите
   `DOMAIN=finance.example.com` и `ACME_EMAIL=operator@example.com`.
   Ограничьте доступ к файлу системными правами, на Linux — `chmod 600 .env`.
3. Проверьте и запустите конфигурацию:

```sh
docker compose -f docker-compose.yml -f docker-compose.production.yml config --quiet
docker compose -f docker-compose.yml -f docker-compose.production.yml up --build -d
docker compose -f docker-compose.yml -f docker-compose.production.yml ps
```

Override убирает публикацию app:8080, задаёт `APP_ENV=production`, HTTPS origin
и Secure cookies. Caddy выпускает и обновляет сертификаты; его `/data`
сохраняется в volume. Требования автоматического HTTPS описаны в
[официальной документации Caddy](https://caddyserver.com/docs/automatic-https).
Механизм удаления ports в override —
[Compose !reset](https://docs.docker.com/reference/compose-file/merge/).

Image собирает CSS, затем Rust backend и Trunk WASM. Runtime работает UID 10001,
с read-only filesystem, без Linux capabilities и с ограниченным `/tmp`.
PostgreSQL находится в отдельной внутренней сети; app использует роль-владельца
своей базы без SUPERUSER/CREATEDB/CREATEROLE. Административный пароль не передаётся
приложению. PostgreSQL 18 volume монтируется в `/var/lib/postgresql`, как указано
в [документации official image](https://hub.docker.com/_/postgres).

Для обновления: backup → pull исходников → проверки → `up --build -d`.
Откат приложения возможен только при совместимой схеме; изменения БД проверяйте
на восстановленной копии заранее. Версии образов обновляйте контролируемо;
для строгой воспроизводимости закрепите проверенные image digests в своём release.

### Секреты и эксплуатационные ограничения

`.env` предназначен для одного доверенного оператора хоста; Docker administrators
могут читать container environment. Для другой модели доступа используйте
secret manager и подавайте переменные через deployment environment.
Смена `JWT_SECRET` завершает существующие сеансы; планируйте повторный вход.
Смена пароля в `.env` **не изменяет** существующую роль PostgreSQL: выполните
`ALTER ROLE` через административное подключение и обновите окружение согласованно.
Init script выполняется только на пустом volume.

Rate limiting использует фактический peer IP и память процесса. За Caddy
auth-запросы имеют общий лимит по IP прокси; forwarded headers намеренно
не считаются доверенными. Для большой нагрузки добавьте лимит на доверенном
edge и согласованную proxy/IP policy. Несколько app replicas не разделяют
in-memory rate limit; генерация recurring при этом защищена PostgreSQL.

Публичные workflows email verification и password recovery не включены:
приложение не подтверждает владение email и не отправляет SMTP. Подготовлены
сервис и миграция для случайных одноразовых purpose-scoped tokens с хешированием,
сроком 15 минут и атомарным погашением; они проверяются тестами. Перед публичной
регистрацией подключите SMTP adapter и HTTP workflows подтверждения адреса и
восстановления пароля. Также проведите нагрузочные проверки, dependency/image
scan, проверку backup/restore и конфигурации конкретного сервера.

Проверка Rust-зависимостей через OSV выявила два сообщения: `paste 1.0.15`
([RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html),
библиотека не сопровождается; транзитивная зависимость Leptos) и `rsa 0.9.10`
([RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html),
medium 5.9, исправления нет; транзитивная зависимость JWT crypto provider).
Приложение разрешает только HS256 и не использует RSA-ключи; из проверки кода
следует, что затронутый путь операций с закрытым RSA-ключом в текущей конфигурации
не вызывается. Это оценка достижимости, а не устранение сообщения о зависимости.
Повторяйте проверку при обновлении библиотек или изменении алгоритма JWT.
Отдельный `npm audit` после обновления Tailwind watcher вернул 0 уязвимостей.

## Backup и восстановление

Backup содержит персональные данные. Храните его зашифрованным вне хоста,
ограничивайте доступ и регулярно проверяйте восстановление на отдельной базе.
Встроенные PowerShell scripts используют custom-format pg_dump и `docker cp`,
что сохраняет бинарный dump и в Windows PowerShell 5.1:

```powershell
.\scripts\backup.ps1
# Для deployment с Caddy:
.\scripts\backup.ps1 -Production
```

Для восстановления выберите dump и остановите app; существующие данные
восстанавливаемой базы будут заменены в одной транзакции:

```powershell
docker compose stop app
.\scripts\restore.ps1 -BackupFile .\backups\spendwise-YYYYMMDD-HHMMSS.dump
docker compose up -d app
```

В production добавляйте оба `-f` к compose-командам и `-Production` к script.
В POSIX shell аналогичные команды:

```sh
mkdir -p backups
umask 077
docker compose exec -T db pg_dump -U spendwise -d spendwise -Fc > backups/spendwise.dump
docker compose stop app
docker compose exec -T db pg_restore -U spendwise -d spendwise --clean --if-exists --no-owner --exit-on-error --single-transaction < backups/spendwise.dump
docker compose up -d app
```

После восстановления проверьте readiness, регистрацию/вход и контрольные суммы
своих транзакций. Для изолированной проверки используйте отдельный Compose
project с другим volume и конфигурацией. Не восстанавливайте проверочную копию
поверх единственной рабочей БД.

## Troubleshooting

- **JWT_SECRET required / unsafe configuration**: заполните секреты; production
  требует HTTPS и `COOKIE_SECURE=true`.
- **403 при входе или изменении**: сравните адрес браузера и точный APP_ORIGIN;
  проверьте отсутствие завершающего `/` и CSRF header для refresh/logout.
- **401 после reload**: cookie должен быть доступен для `/api/v1/auth`; Secure
  cookie не работает по обычному HTTP. Старый refresh после ротации недействителен.
- **Connection refused / миграции**: проверьте `DATABASE_URL`, `docker compose ps`
  и `docker compose logs db`; Compose hostname `db` доступен только внутри сети.
- **Password authentication failed после изменения .env**: init script не
  повторяется на существующем volume; измените пароль роли через PostgreSQL.
- **Нет оформления**: сначала `npm ci` и `npm run css:build`, затем Trunk;
  проверьте `@source` на Rust-код. Tailwind CDN не используется.
- **Production frontend bundle missing**: выполните Trunk release build либо
  используйте Dockerfile, который копирует готовый dist в image.
- **Тесты не создают базы**: тестовая роль должна иметь CREATEDB; production-role
  приложения специально не имеет этого права.
- **Порт 5432 занят**: используйте другой локальный порт и обновите DATABASE_URL;
  Compose не публикует DB-port и не конфликтует с локальной PostgreSQL.
