"""Generate the checked-in OpenAPI 3.1 wire contract; no external dependencies.

Keep schemas synchronized with shared/src and routes with backend/src. This script
deliberately describes externally visible behavior rather than Rust internals.
"""

import json
from pathlib import Path


def ref(name):
    return {"$ref": f"#/components/schemas/{name}"}


def obj(properties, optional=()):
    return {
        "type": "object",
        "properties": properties,
        "required": [name for name in properties if name not in optional],
    }


def array(name):
    return {"type": "array", "items": ref(name)}


def response(schema=None, description="OK", media="application/json"):
    result = {"description": description}
    if schema is not None:
        result["content"] = {media: {"schema": schema}}
    return result


def query(name, schema, description=""):
    return {"name": name, "in": "query", "required": False,
            "schema": schema, "description": description}


uuid = {"type": "string", "format": "uuid"}
date = {"type": "string", "format": "date", "description": "Calendar date, year 1900 through 9998."}
timestamp = {"type": "string", "format": "date-time"}
text = {"type": "string"}
money = {"type": "string", "pattern": r"^-?[0-9]+(\.[0-9]+)?$",
         "description": "Exact decimal string; aggregate balances and savings can be negative.",
         "examples": ["12500.50"]}
input_money = {**money, "description": "Positive exact decimal string, below 10^16, with at most two decimal places."}
kind = {"type": "string", "enum": ["income", "expense"]}
nullable_uuid = {"anyOf": [uuid, {"type": "null"}]}
description = {"type": ["string", "null"], "maxLength": 2000}
schemas = {
    "Error": obj({"error": obj({"code": text, "message": text, "details": {"type": "object"}})}),
    "User": obj({"id": uuid, "email": {"type": "string", "format": "email"},
                 "display_name": text, "currency": {"enum": ["KZT", "USD", "EUR"]}, "created_at": timestamp}),
    "RegisterInput": obj({"email": {"type": "string", "format": "email", "maxLength": 254},
                          "password": {"type": "string", "minLength": 12, "maxLength": 128, "writeOnly": True},
                          "display_name": {"type": "string", "minLength": 1, "maxLength": 80}}),
    "LoginInput": obj({"email": {"type": "string", "format": "email"},
                       "password": {"type": "string", "writeOnly": True}}),
    "AuthResponse": obj({"access_token": text, "user": ref("User"), "csrf_token": text}),
    "CsrfResponse": obj({"csrf_token": text}),
    "ProfileInput": obj({"display_name": {"type": "string", "minLength": 1, "maxLength": 80},
                         "currency": {"type": "string", "enum": ["KZT", "USD", "EUR"]}}),
    "PasswordInput": obj({"current_password": {"type": "string", "writeOnly": True},
                          "new_password": {"type": "string", "minLength": 12, "maxLength": 128, "writeOnly": True}}),
    "Category": obj({"id": uuid, "name": text, "transaction_type": kind, "color": text,
                     "icon": text, "archived": {"type": "boolean"}}),
    "CategoryInput": obj({"name": {"type": "string", "minLength": 1, "maxLength": 80},
                          "transaction_type": kind, "color": {"type": "string", "pattern": "^#[0-9a-fA-F]{6}$"},
                          "icon": {"type": "string", "minLength": 1, "maxLength": 40},
                          "archived": {"type": "boolean", "default": False}}, optional=("archived",)),
    "TransactionInput": obj({"category_id": uuid, "transaction_type": kind, "amount": input_money,
                             "title": {"type": "string", "minLength": 1, "maxLength": 160},
                             "description": description, "transaction_date": date}, optional=("description",)),
    "BudgetInput": obj({"category_id": nullable_uuid, "month": {**date, "description": "First day of the budget month."},
                        "amount": input_money}, optional=("category_id",)),
    "RecurringInput": obj({"category_id": uuid, "transaction_type": kind, "amount": input_money,
                           "title": {"type": "string", "minLength": 1, "maxLength": 160},
                           "description": description, "next_date": date, "active": {"type": "boolean"}},
                          optional=("description",)),
    "AnalyticsSummary": obj({"balance": money, "income": money, "expenses": money, "savings": money}),
    "CategoryTotal": obj({"category_id": uuid, "name": text, "color": text, "total": money}),
    "MonthlyTotal": obj({"month": date, "income": money, "expenses": money}),
    "BalancePoint": obj({"date": date, "balance": money}),
}
schemas["Transaction"] = obj({"id": uuid, **schemas["TransactionInput"]["properties"],
                              "created_at": timestamp, "updated_at": timestamp})
schemas["Budget"] = obj({"id": uuid, **schemas["BudgetInput"]["properties"], "spent": money})
schemas["Recurring"] = obj({"id": uuid, **schemas["RecurringInput"]["properties"],
                            "day_of_month": {"type": "integer", "minimum": 1, "maximum": 31}})
schemas["TransactionPage"] = obj({"items": array("Transaction"), "total": {"type": "integer", "format": "int64"},
                                  "page": {"type": "integer"}, "per_page": {"type": "integer"}})
for schema_name in ("RegisterInput", "LoginInput", "ProfileInput", "PasswordInput", "CategoryInput", "TransactionInput", "BudgetInput", "RecurringInput"):
    schemas[schema_name]["additionalProperties"] = False

paths = {}
errors = {str(code): response(ref("Error"), label) for code, label in [
    (400, "Malformed request path"), (401, "Authentication required or expired"),
    (403, "Origin or CSRF validation failed"),
    (404, "Resource not found, including resources owned by another user"),
    (405, "Method not allowed"), (408, "Request timeout"),
    (409, "Conflict with existing data"), (413, "Request body too large"),
    (415, "Unsupported media type"), (422, "Invalid input"),
    (429, "Authentication rate limit or concurrent export limit exceeded; Retry-After: 60"), (500, "Internal server error")
]}
id_parameter = {"name": "id", "in": "path", "required": True, "schema": uuid}
origin_parameter = {"name": "Origin", "in": "header", "required": True,
                    "schema": text, "description": "Must exactly equal APP_ORIGIN."}
csrf_parameter = {"name": "X-CSRF-Token", "in": "header", "required": True,
                  "schema": text, "description": "Token returned by auth/login, auth/register, auth/refresh or auth/csrf."}
period = [query("from", date, "Inclusive lower bound."), query("to", date, "Inclusive upper bound.")]
filters = [*period, query("category_id", uuid), query("transaction_type", kind),
           query("search", {"type": "string", "maxLength": 200}, "Literal substring in title or description."),
           query("sort", {"type": "string", "enum": ["date_desc", "date_asc", "amount_desc", "amount_asc"], "default": "date_desc"}),
           query("page", {"type": "integer", "minimum": 1, "maximum": 1000000, "default": 1}),
           query("per_page", {"type": "integer", "minimum": 1, "maximum": 100, "default": 20})]
cookie_security = [{"refreshCookie": []}, {"productionRefreshCookie": []}]


def operation(path, method, summary, result=None, body=None, status=200,
              parameters=(), security=None, details=""):
    op = {"summary": summary, "operationId": method + "_" + path.strip("/").replace("/", "_").replace("{", "").replace("}", ""),
          "responses": {str(status): response(result, "No content" if status == 204 else "Success"), **errors},
          "security": [{"bearerAuth": []}] if security is None else security}
    if parameters:
        op["parameters"] = list(parameters)
    if body:
        op["requestBody"] = {"required": True, "content": {"application/json": {"schema": ref(body)}}}
    if details:
        op["description"] = details
    paths.setdefault(path, {})[method] = op
    return op


operation("/auth/register", "post", "Register and create default categories", ref("AuthResponse"), "RegisterInput", 201,
          [origin_parameter], security=[], details="Creates an HttpOnly refresh cookie. The initial currency is KZT.")
operation("/auth/login", "post", "Create a new session", ref("AuthResponse"), "LoginInput",
          parameters=[origin_parameter], security=[])
operation("/auth/csrf", "get", "Recover a CSRF token after a page reload", ref("CsrfResponse"), security=cookie_security)
operation("/auth/refresh", "post", "Rotate refresh session and issue a new access token", ref("AuthResponse"),
          parameters=[origin_parameter, csrf_parameter], security=cookie_security,
          details="The consumed refresh token cannot be reused. Replay revokes its device session family.")
operation("/auth/logout", "post", "Revoke current device session family and clear cookie", status=204,
          parameters=[origin_parameter, csrf_parameter], security=cookie_security,
          details="Access tokens are checked against the session in PostgreSQL; revoking this device family invalidates its issued access tokens immediately.")
operation("/auth/logout-all", "post", "Revoke all refresh sessions and invalidate access tokens", status=204,
          parameters=[origin_parameter, csrf_parameter],
          security=[{"bearerAuth": [], "refreshCookie": []}, {"bearerAuth": [], "productionRefreshCookie": []}])
operation("/auth/me", "get", "Get current user", ref("User"))
operation("/users/me", "get", "Get current user", ref("User"))
operation("/users/me", "patch", "Update profile", ref("User"), "ProfileInput", parameters=[origin_parameter],
          details="Currency may change only before any transactions, budgets or recurring rules exist. No implicit conversion is performed.")
operation("/users/password", "post", "Change password and revoke all sessions", body="PasswordInput", status=204,
          parameters=[origin_parameter], details="The frontend must clear its in-memory access token and sign in again.")

for resource, model in [("categories", "Category"), ("transactions", "Transaction"), ("budgets", "Budget"), ("recurring", "Recurring")]:
    list_result = ref("TransactionPage") if resource == "transactions" else array(model)
    params = filters if resource == "transactions" else (
        [query("include_archived", {"type": "boolean", "default": False})] if resource == "categories" else (
            [query("month", date, "First day of a month; omit to return all budgets.")] if resource == "budgets" else []))
    operation(f"/{resource}", "get", f"List own {resource}", list_result, parameters=params)
    operation(f"/{resource}", "post", f"Create {model.lower()}", ref(model), model + "Input", 201, [origin_parameter])
    operation(f"/{resource}/{{id}}", "patch", f"Update {model.lower()}", ref(model), model + "Input",
              parameters=[id_parameter, origin_parameter], details="Send the full input object, including fields that did not change.")
    operation(f"/{resource}/{{id}}", "delete", "Archive category" if resource == "categories" else f"Delete {model.lower()}",
              status=204, parameters=[id_parameter, origin_parameter],
              details="Archiving preserves historical transactions and disables recurring rules using the category." if resource == "categories" else "")
operation("/transactions/{id}", "get", "Get own transaction", ref("Transaction"), parameters=[id_parameter])
operation("/analytics/summary", "get", "Period income, expenses and savings; cumulative balance", ref("AnalyticsSummary"),
          parameters=period, details="Balance includes all transactions through to; income, expenses and savings use both inclusive period bounds.")
operation("/analytics/categories", "get", "Expense totals by category", array("CategoryTotal"), parameters=period)
operation("/analytics/monthly", "get", "Income and expenses by calendar month", array("MonthlyTotal"), parameters=period,
          details="Defaults to the current month and previous 11 months through today (UTC). Empty months return zeros. Maximum chart range: 3660 days.")
operation("/analytics/balance", "get", "Daily cumulative balance", array("BalancePoint"), parameters=period,
          details="Defaults to the current month through today (UTC), including the opening historical balance. Maximum chart range: 3660 days.")
csv_op = operation("/export/transactions", "get", "Download own transactions as UTF-8 CSV", parameters=filters[:5],
                   details="Up to 100000 matching rows ordered by date, streamed in 500-row pages within a repeatable-read snapshot. Two concurrent exports per process; additional requests receive 429. Narrow filters when the row limit is exceeded. Includes a UTF-8 BOM and protects text fields from spreadsheet formula injection.")
csv_op["responses"]["200"] = response({"type": "string"}, "Attachment spendwise-transactions.csv", "text/csv")
csv_op["responses"]["200"]["headers"] = {"Content-Disposition": {"schema": text, "description": "attachment; filename=\"spendwise-transactions.csv\""}}
operation("/openapi.json", "get", "Get this OpenAPI document", {"type": "object"}, security=[])
operation("/health", "get", "Database readiness", obj({"status": {"const": "ok"}}), security=[])
for probe in ("live", "ready"):
    probe_path = f"/health/{probe}"
    operation(probe_path, "get", "Process liveness" if probe == "live" else "Database readiness",
              obj({"status": {"const": "ok"}}), security=[])
    paths[probe_path]["servers"] = [{"url": "/"}]

document = {
    "openapi": "3.1.0",
    "info": {"title": "SpendWise API", "version": "1.0.0",
             "description": "Same-origin REST API. Every financial resource belongs to the authenticated user. Money is always a decimal string; dates use YYYY-MM-DD. All unsafe requests require an exact allowed Origin. Cookie-authenticated mutations additionally require X-CSRF-Token. No user_id may be supplied in inputs."},
    "servers": [{"url": "/api/v1"}],
    "paths": paths,
    "components": {"securitySchemes": {
        "bearerAuth": {"type": "http", "scheme": "bearer", "bearerFormat": "JWT"},
        "refreshCookie": {"type": "apiKey", "in": "cookie", "name": "spendwise_refresh",
                          "description": "Development HTTP only: HttpOnly; SameSite=Strict; Path=/api/v1/auth."},
        "productionRefreshCookie": {"type": "apiKey", "in": "cookie", "name": "__Host-spendwise_refresh",
                                    "description": "When COOKIE_SECURE=true: HttpOnly; Secure; SameSite=Strict; Path=/; no Domain attribute."}},
        "schemas": schemas},
}

if __name__ == "__main__":
    target = Path(__file__).resolve().parents[1] / "docs" / "openapi.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {len(paths)} paths and {len(schemas)} schemas to {target}")
