CREATE TABLE users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE CHECK (length(email) BETWEEN 3 AND 254),
    password_hash TEXT NOT NULL,
    display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 100),
    currency TEXT NOT NULL DEFAULT 'KZT' CHECK (currency IN ('KZT', 'USD', 'EUR')),
    token_version BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (email = lower(email))
);

CREATE TABLE refresh_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    csrf_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at)
);
CREATE INDEX refresh_sessions_user_idx ON refresh_sessions(user_id);
CREATE INDEX refresh_sessions_expiry_idx ON refresh_sessions(expires_at);

CREATE TABLE categories (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    transaction_type TEXT NOT NULL CHECK (transaction_type IN ('income', 'expense')),
    color TEXT NOT NULL CHECK (color ~ '^#[0-9a-fA-F]{6}$'),
    icon TEXT NOT NULL CHECK (length(icon) BETWEEN 1 AND 40),
    archived BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, name, transaction_type),
    UNIQUE (user_id, id, transaction_type),
    UNIQUE (user_id, id)
);
CREATE INDEX categories_user_idx ON categories(user_id, archived);

CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    category_id UUID NOT NULL,
    transaction_type TEXT NOT NULL CHECK (transaction_type IN ('income', 'expense')),
    amount NUMERIC(18,2) NOT NULL CHECK (amount > 0),
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 160),
    description TEXT CHECK (length(description) <= 2000),
    transaction_date DATE NOT NULL CHECK (transaction_date BETWEEN DATE '1900-01-01' AND DATE '9998-12-31'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (user_id, category_id, transaction_type)
        REFERENCES categories(user_id, id, transaction_type) ON DELETE RESTRICT
);
CREATE INDEX transactions_user_date_idx ON transactions(user_id, transaction_date DESC, id);
CREATE INDEX transactions_user_category_idx ON transactions(user_id, category_id, transaction_date);
CREATE INDEX transactions_user_type_idx ON transactions(user_id, transaction_type, transaction_date);
CREATE INDEX transactions_user_amount_idx ON transactions(user_id, amount, id);

CREATE TABLE budgets (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    category_id UUID,
    month DATE NOT NULL CHECK (extract(day FROM month) = 1 AND month BETWEEN DATE '1900-01-01' AND DATE '9998-12-01'),
    amount NUMERIC(18,2) NOT NULL CHECK (amount > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (user_id, category_id) REFERENCES categories(user_id, id) ON DELETE RESTRICT
);
CREATE UNIQUE INDEX budgets_category_month_unique ON budgets(user_id, category_id, month) WHERE category_id IS NOT NULL;
CREATE UNIQUE INDEX budgets_global_month_unique ON budgets(user_id, month) WHERE category_id IS NULL;
CREATE INDEX budgets_user_month_idx ON budgets(user_id, month);

CREATE TABLE recurring_transactions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    category_id UUID NOT NULL,
    transaction_type TEXT NOT NULL CHECK (transaction_type IN ('income', 'expense')),
    amount NUMERIC(18,2) NOT NULL CHECK (amount > 0),
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 160),
    description TEXT CHECK (length(description) <= 2000),
    next_date DATE NOT NULL CHECK (next_date BETWEEN DATE '1900-01-01' AND DATE '9998-12-31'),
    day_of_month INTEGER NOT NULL CHECK (day_of_month BETWEEN 1 AND 31),
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (user_id, category_id, transaction_type)
        REFERENCES categories(user_id, id, transaction_type) ON DELETE RESTRICT
);
CREATE INDEX recurring_due_idx ON recurring_transactions(next_date, id) WHERE active;
CREATE INDEX recurring_user_idx ON recurring_transactions(user_id);

CREATE TABLE recurring_occurrences (
    recurring_id UUID NOT NULL REFERENCES recurring_transactions(id) ON DELETE CASCADE,
    occurrence_date DATE NOT NULL,
    transaction_id UUID REFERENCES transactions(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (recurring_id, occurrence_date),
    UNIQUE (transaction_id)
);

-- Budgets only target expense categories, including under concurrent category edits.
CREATE FUNCTION enforce_expense_budget() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.category_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM categories WHERE id = NEW.category_id AND user_id = NEW.user_id AND transaction_type = 'expense' FOR SHARE
    ) THEN
        RAISE EXCEPTION 'Budget category must be an expense category' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER budgets_expense_category BEFORE INSERT OR UPDATE ON budgets
    FOR EACH ROW EXECUTE FUNCTION enforce_expense_budget();

CREATE FUNCTION protect_budget_category_type() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.transaction_type <> OLD.transaction_type AND NEW.transaction_type <> 'expense' AND EXISTS (
        SELECT 1 FROM budgets WHERE category_id = OLD.id AND user_id = OLD.user_id
    ) THEN
        RAISE EXCEPTION 'Category is used by a budget' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER categories_budget_type BEFORE UPDATE ON categories
    FOR EACH ROW EXECUTE FUNCTION protect_budget_category_type();

-- Serialize the first financial write with an empty-account currency change.
-- Currency changes take FOR UPDATE on users before checking financial tables.
CREATE FUNCTION lock_financial_user() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    PERFORM 1 FROM users WHERE id = NEW.user_id FOR SHARE;
    RETURN NEW;
END;
$$;
CREATE TRIGGER transactions_currency_lock BEFORE INSERT OR UPDATE ON transactions
    FOR EACH ROW EXECUTE FUNCTION lock_financial_user();
CREATE TRIGGER budgets_currency_lock BEFORE INSERT OR UPDATE ON budgets
    FOR EACH ROW EXECUTE FUNCTION lock_financial_user();
CREATE TRIGGER recurring_currency_lock BEFORE INSERT OR UPDATE ON recurring_transactions
    FOR EACH ROW EXECUTE FUNCTION lock_financial_user();

CREATE FUNCTION deactivate_archived_recurring() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.archived AND NOT OLD.archived THEN
        UPDATE recurring_transactions SET active = false, updated_at = now()
            WHERE user_id = NEW.user_id AND category_id = NEW.id AND active;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER categories_archive_recurring AFTER UPDATE ON categories
    FOR EACH ROW EXECUTE FUNCTION deactivate_archived_recurring();

CREATE FUNCTION enforce_active_category() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE category_archived BOOLEAN;
BEGIN
    IF NEW.category_id IS NULL THEN RETURN NEW; END IF;
    -- An unchanged inactive rule must still be deactivatable when archiving its category.
    IF TG_TABLE_NAME = 'recurring_transactions' THEN
        IF NOT NEW.active THEN RETURN NEW; END IF;
    END IF;
    SELECT archived INTO category_archived FROM categories
        WHERE id = NEW.category_id AND user_id = NEW.user_id FOR SHARE;
    IF category_archived IS NULL OR category_archived THEN
        RAISE EXCEPTION 'Category is unavailable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER transactions_active_category BEFORE INSERT OR UPDATE ON transactions
    FOR EACH ROW EXECUTE FUNCTION enforce_active_category();
CREATE TRIGGER budgets_active_category BEFORE INSERT OR UPDATE ON budgets
    FOR EACH ROW EXECUTE FUNCTION enforce_active_category();
CREATE TRIGGER recurring_active_category BEFORE INSERT OR UPDATE ON recurring_transactions
    FOR EACH ROW EXECUTE FUNCTION enforce_active_category();
