# Verification record — 2026-10-20

This record distinguishes actual local checks from deployment configuration.
CI repeats compilation, testing, frontend bundling and Docker build on Linux.

## Environment established

- Host: Windows, PowerShell; Rust 1.99.0; Trunk 0.21.14; Node.js 24.21.0.
- Docker Engine 29.8.2 and Docker Compose 5.6.0 are accessible with elevated
  sandbox permission. The original sandbox could not access the engine pipe.
- An isolated PostgreSQL 18 container `spendwise-test-db` was created on
  `127.0.0.1:54329`, separate from the existing `postgres-dev` container.
- Both local and production Compose files passed `docker compose config --quiet`.
- Parsed production Compose configuration confirms zero published app/database
  ports, `APP_ENV=production`, `COOKIE_SECURE=true`, and only Caddy publishes ports.

## Infrastructure checks passed

- `scripts/init-env.ps1` created three independent random secrets of 96/64/64
  hex characters and replaced the native database URL password. The temporary
  environment file was removed without printing its values.
- `python scripts/generate_openapi.py` produced parseable JSON with 26 paths,
  21 schemas and unique operation IDs. Decimal schema matches `12500.50`.
- An isolated PostgreSQL instance exercised `docker/postgres-init.sh` with real
  password authentication. The application role had `rolsuper=false`,
  `rolcreatedb=false`, `rolcreaterole=false`, and could create its own tables.
- Custom-format `pg_dump` / transactional `pg_restore` restored an exact
  `NUMERIC(18,2)` amount of `12500.50` with the non-superuser role.
- A separate Compose project `spendwise-infra-smoke` ran the real PowerShell
  `backup.ps1` and `restore.ps1`: after deleting the source table, restoration
  recovered `43210.65`. Its containers, volume and synthetic backup were removed.
- The official `caddy:2-alpine` image validated the Caddyfile successfully.
  This check did not run a public listener or obtain an ACME certificate.

## Docker build and runtime passed

- `docker build --progress plain --tag spendwise:local .` completed successfully
  with the final pagination and 320px mobile CSS fixes. The image includes the
  Axum release binary, minified Tailwind v4.3.0 CSS, Trunk release JS/WASM and
  external theme initialization. Final runtime image size: 149,728,845 bytes.
- Independent Compose project `spendwise-verify` started on `127.0.0.1:8180` with
  its own freshly initialized PostgreSQL volume. Both services became healthy;
  migrations `202610200001`, `202610200002`, `202610200003` were applied.
- The live application ran as `10001:10001`, with a read-only root filesystem,
  `cap_drop=ALL` and no published database port.
- HTTP smoke checked registration (201), all 13 initial categories, creation of
  income `100000.00` and expense `12500.50`, computed balance `87499.50`, category
  budget spent `12500.50`, CSV download (200), refresh rotation, logout (204),
  and rejection of a logged-out refresh session (401).
- Final HTTP index and CSS returned 200; the served 44,864-byte CSS exactly
  matched the final frontend build. Served OpenAPI exactly matched its current
  checked-in source. `/health/ready` returned `{"status":"ok"}`.
- All ten known CSR routes returned 200 on direct navigation, including
  `/transactions`, `/transactions/new`, `/analytics` and `/profile`.
  `/unknown-client-route` and `/missing.wasm` retained 404. The script CSP used
  hashes and did not permit `unsafe-inline` scripts.
- A separate invocation of the image with production HTTP origin and insecure
  cookies exited 1 with the expected validation diagnostic before DB connection.

## Rust and frontend checks passed

The root and frontend implementation agents completed these checks in the same
workspace and against the isolated PostgreSQL instance:

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed on final Rust source. |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed. |
| `cargo test --workspace --locked` | 30 tests passed before the last three regression tests were added. |
| `cargo test --locked -p backend --test action_tokens` | Three action-token tests passed, including two newly added concurrency and lock-order regressions. |
| `cargo test --locked -p backend --test static_routes` | Newly added direct SPA-navigation status regression passed. |
| `cargo build --release --locked -p backend` | Native release passed; Docker additionally built final Linux release. |
| `cargo test --locked -p frontend` | Five host-side frontend tests passed. |
| `cargo check --locked -p frontend --target wasm32-unknown-unknown` | Passed. |
| `cargo clippy --locked -p frontend --target wasm32-unknown-unknown -- -D warnings` | Passed. |
| `npm ci` / `npm audit --audit-level=high` | Passed; audit reported zero vulnerabilities. |
| `npm run css:build` / `trunk build --release --locked` | Passed; final CSS 44,864 bytes. |

Across the complete workspace and final targeted regression runs, **33 distinct
tests passed**: 9 backend unit, 3 action-token, 8 API, 7 finance database,
1 static-route and 5 frontend tests. Environment-only Cargo cache hardlink
warnings did not fail the checks and were not Clippy findings in application code.

## Browser checks passed

The root agent exercised the final Docker frontend in an actual browser:

- Mobile 320px: `innerWidth=320`, `clientWidth=scrollWidth=305` (vertical scrollbar,
  no horizontal overflow). Tablet 768px: `clientWidth=scrollWidth=753`; desktop
  1280px layout was also checked.
- Correct pagination button text/states after the Leptos macro fix, mobile
  navigation, transaction cards, editing, delete confirmation and Escape close.
- CSV file actually downloaded; cookie session and theme preference restored
  after reload; client Not Found page displayed; no console errors.
- A final direct `/transactions` reload rendered the authenticated table with
  HTTP 200; profile/settings and keyboard logout to `/login` worked. The browser
  theme was returned to system and temporary viewport overrides were reset.
- Light/dark themes, registration, income and expense creation, categories,
  recurring creation/pause, three SVG charts and a real 125% budget warning were
  exercised across the native and Docker previews.

## Dependency review

The root implementation agent queried the official OSV batch API for all 380
registry packages in Cargo.lock. `cargo-audit` was not installed; OSV was used
instead. Two findings remain:

| Dependency | Advisory | Assessment |
| --- | --- | --- |
| paste 1.0.15 | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) | Informational: unmaintained, transitive through Leptos. |
| rsa 0.9.10 | [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html) | Medium 5.9; no available fix, transitive JWT provider dependency. |

The application issues and validates exclusively HS256 JWTs. It does not load or
operate on RSA private keys, so the vulnerable RSA private-key timing path is not
reachable with the current implementation (inference from code review). This
does not remove the transitive advisory and must be reconsidered if JWT algorithms
or providers change. Do not report a zero-warning Rust dependency scan.

The Tailwind build dependency findings were fixed by pinning Tailwind v4.3.0
and its watcher v2.6.0; the frontend agent's final npm audit found 0 vulnerabilities.

An unrecorded check must not be interpreted as passed. Public DNS, certificate
issuance, external hosting and workload testing have not been verified locally.

## Cleanup completed

After the final browser check, only task-created verification resources were
removed: `spendwise-verify` containers, networks and database volume;
`spendwise-test-db` and its anonymous volume; temporary verification `.env` and
Compose override files. The pre-existing `postgres-dev` container was preserved
in its original stopped state. The verified `spendwise:local` image and normal
Docker build cache remain available. No app or test database is left listening.
