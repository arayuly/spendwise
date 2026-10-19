#!/bin/sh
set -eu

# Invoked by the official image only for an empty database volume.
# The application owns its database for SQLx DDL, without cluster privileges.
psql --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
    --set=ON_ERROR_STOP=1 --set=app_password="$SPENDWISE_DB_PASSWORD" <<'SQL'
CREATE ROLE spendwise LOGIN PASSWORD :'app_password'
    NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
ALTER DATABASE spendwise OWNER TO spendwise;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO spendwise;
SQL
