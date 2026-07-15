-- Ver docs/database.md y docs/fase1-ingesta.md (variable-create/variable-set/variable-tag).
CREATE TABLE user_variables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    var_type TEXT NOT NULL CHECK(var_type IN ('ordinal','nominal')),
    position INTEGER UNIQUE NOT NULL,
    min_value REAL,
    max_value REAL
);

CREATE TABLE variable_categories (
    variable_id INTEGER NOT NULL,
    code INTEGER NOT NULL,
    label TEXT NOT NULL,
    PRIMARY KEY (variable_id, code),
    FOREIGN KEY (variable_id) REFERENCES user_variables(id)
);

CREATE TABLE image_variable_values (
    image_id INTEGER NOT NULL,
    variable_id INTEGER NOT NULL,
    value REAL,
    PRIMARY KEY (image_id, variable_id)
);
