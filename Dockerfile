# syntax=docker/dockerfile:1.7
FROM node:24-bookworm-slim AS styles
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/src ./src
COPY frontend/styles ./styles
COPY frontend/index.html ./index.html
RUN mkdir -p assets && npm run css:build

FROM rust:1.99-bookworm AS build-tools
RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --version 0.21.14 --locked
ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}

FROM build-tools AS backend-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY backend ./backend
COPY frontend/Cargo.toml ./frontend/Cargo.toml
COPY frontend/src/main.rs ./frontend/src/main.rs
COPY docs/openapi.json ./docs/openapi.json
# Cached targets may be newer than source files copied from another checkout.
# Retouch local crate roots so Cargo always rechecks the copied application code.
RUN --mount=type=cache,id=spendwise-backend-target,target=/app/target \
    touch shared/src/lib.rs backend/src/lib.rs backend/src/main.rs docs/openapi.json \
    && cargo build --release --locked -p backend \
    && cp /app/target/release/backend /app/backend-release

FROM build-tools AS frontend-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY backend/Cargo.toml ./backend/Cargo.toml
COPY backend/src/main.rs ./backend/src/main.rs
COPY frontend ./frontend
COPY --from=styles /app/frontend/assets/tailwind.css ./frontend/assets/tailwind.css
RUN --mount=type=cache,id=spendwise-frontend-target,target=/app/target \
    --mount=type=cache,id=spendwise-trunk-tools,target=/root/.cache/trunk \
    touch shared/src/lib.rs frontend/src/main.rs \
    && cd frontend && trunk build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 spendwise \
    && useradd --uid 10001 --gid spendwise --no-create-home --shell /usr/sbin/nologin spendwise
WORKDIR /app
COPY --from=backend-builder /app/backend-release /app/spendwise
COPY --from=frontend-builder /app/frontend/dist /app/public
ENV HOST=0.0.0.0 PORT=8080 FRONTEND_DIR=/app/public
USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:8080/health/ready || exit 1
ENTRYPOINT ["/app/spendwise"]
