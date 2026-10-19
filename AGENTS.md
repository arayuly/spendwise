# SpendWise engineering agreements

SpendWise is a Rust modular monolith: Axum REST API, PostgreSQL/SQLx, Leptos CSR
WebAssembly, Trunk, and the official Tailwind CSS v4 CLI. Preserve this stack.
Read README.md and the affected modules before changing code. Do not introduce
placeholder endpoints, fake financial data, an additional frontend framework,
Tailwind v3 configuration, or committed secrets.

## Ownership and wire contracts

- `shared/src/` contains the serializable request and response contracts.
- `backend/src/auth.rs` owns password hashing, JWTs and rotating refresh sessions.
- `backend/src/finance.rs` owns HTTP extraction; `services.rs` owns validation and
  recurrence dates; `repository.rs` owns ownership-scoped SQL and atomic writes.
- `backend/migrations/` is the ordered database history. Add a migration when
  changing a database that has been deployed; never edit an applied migration.
- `frontend/src/` contains Rust UI and API state. Access tokens remain in memory.
  Only theme preference may be persisted in browser storage.
- `scripts/generate_openapi.py` generates `docs/openapi.json`. Update contracts,
  handlers, generator, documentation and client together when an API changes.

## Security and financial invariants

Money uses `rust_decimal::Decimal` and PostgreSQL `NUMERIC`; JSON uses decimal
strings. Never use floating-point values for financial calculations. Positive
amount plus `income`/`expense` determines the sign. A single account currency
applies to every financial record; never perform implicit currency conversion.

Every resource query and mutation must include the authenticated `user_id`.
Never accept `user_id` from input. Categories must belong to the same user and
match the transaction type. Archiving must preserve history. Financial totals
are calculated from transactions, not copied into mutable summary fields.

Recurring generation must retain the original day-of-month anchor and be safe
across retries, deletions of generated transactions, and concurrent processes.
Keep database occurrence uniqueness, row locking and transactions intact.

Passwords use Argon2id. Refresh tokens are random, stored hashed, rotated and
sent only in HttpOnly SameSite cookies. Cookie mutations require Origin and
CSRF verification. Never log credentials, tokens, cookies or request bodies.
Do not trust forwarded IP headers without an explicit trusted-proxy policy.
Errors must preserve the documented envelope and avoid internal diagnostics.

## Frontend build

Use CSS-first Tailwind v4 (`@import`, `@theme`, `@source`, `@custom-variant`).
Keep full utility names statically discoverable in Rust files; do not concatenate
class names. Keep package-lock.json. Build CSS before Trunk:

```sh
cd frontend
npm ci
npm run css:build
trunk build --release --locked
```

Verify forms, loading/error/empty states, keyboard focus, 320px mobile layout,
desktop layout and light/dark/system themes. Do not substitute hardcoded data
when an API request fails.

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked -p backend
cargo check --locked -p frontend --target wasm32-unknown-unknown
python scripts/generate_openapi.py
docker compose config --quiet
docker build -t spendwise:local .
```

`cargo test` integration tests require `DATABASE_URL` pointing to a disposable
PostgreSQL database whose role can create databases (SQLx isolates test cases).
Never run integration tests against a user's production database. Run applicable
checks, fix relevant warnings, and report the exact checks that were unavailable.
Do not claim a Docker image or production deployment was verified without
actually building or exercising it. No deployment/publishing is implied by a
local implementation task.
