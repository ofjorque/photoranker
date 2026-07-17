//! `variable-create` / `variable-list` / `variable-set` / `variable-delete` —
//! ver docs/fase1-ingesta.md y docs/database.md (`user_variables`,
//! `variable_categories`, `image_variable_values`).

use crate::db;
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
use std::path::Path;

pub struct NewCategory {
    pub code: i64,
    pub label: String,
}

pub fn create(
    conn: &mut Connection,
    name: &str,
    var_type: &str,
    min: Option<f64>,
    max: Option<f64>,
    categories: &[NewCategory],
) -> AppResult<serde_json::Value> {
    if var_type != "ordinal" && var_type != "nominal" {
        return Err(AppError::InvalidArgument(format!(
            "var_type debe ser 'ordinal' o 'nominal', recibido '{var_type}'"
        )));
    }
    if var_type == "nominal" && categories.is_empty() {
        return Err(AppError::InvalidArgument(
            "una variable nominal requiere al menos una categoría (--categories)".to_string(),
        ));
    }

    let tx = conn.transaction()?;
    let next_position: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM user_variables",
        [],
        |r| r.get(0),
    )?;

    tx.execute(
        "INSERT INTO user_variables (name, var_type, position, min_value, max_value) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, var_type, next_position, min, max],
    )?;
    let variable_id = tx.last_insert_rowid();

    for category in categories {
        tx.execute(
            "INSERT INTO variable_categories (variable_id, code, label) VALUES (?1, ?2, ?3)",
            params![variable_id, category.code, category.label],
        )?;
    }
    tx.commit()?;

    Ok(json!({
        "id": variable_id,
        "name": name,
        "var_type": var_type,
        "position": next_position,
    }))
}

pub fn list(conn: &Connection) -> AppResult<serde_json::Value> {
    let mut stmt = conn.prepare("SELECT id, name, var_type, position, min_value, max_value FROM user_variables ORDER BY position")?;
    let variables = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<f64>>(4)?,
            row.get::<_, Option<f64>>(5)?,
        ))
    })?;

    let mut result = Vec::new();
    let mut cat_stmt = conn.prepare(
        "SELECT code, label FROM variable_categories WHERE variable_id = ?1 ORDER BY code",
    )?;
    for row in variables {
        let (id, name, var_type, position, min_value, max_value) = row?;
        let categories: Vec<serde_json::Value> = cat_stmt
            .query_map(params![id], |r| {
                Ok(json!({"code": r.get::<_, i64>(0)?, "label": r.get::<_, String>(1)?}))
            })?
            .filter_map(|r| r.ok())
            .collect();

        result.push(json!({
            "id": id,
            "name": name,
            "var_type": var_type,
            "position": position,
            "min_value": min_value,
            "max_value": max_value,
            "categories": categories,
        }));
    }

    Ok(json!(result))
}

fn find_variable(
    conn: &Connection,
    name: &str,
) -> AppResult<(i64, String, Option<f64>, Option<f64>)> {
    conn.query_row(
        "SELECT id, var_type, min_value, max_value FROM user_variables WHERE name = ?1",
        params![name],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .optional()?
    .ok_or_else(|| AppError::VariableNotFound(name.to_string()))
}

pub fn set(
    conn: &mut Connection,
    variable_name: &str,
    values: &[(i64, f64)],
) -> AppResult<serde_json::Value> {
    let (variable_id, var_type, min_value, max_value) = find_variable(conn, variable_name)?;

    let valid_codes: Vec<i64> = if var_type == "nominal" {
        let mut stmt =
            conn.prepare("SELECT code FROM variable_categories WHERE variable_id = ?1")?;
        stmt.query_map(params![variable_id], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        Vec::new()
    };

    for &(image_id, value) in values {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM images WHERE id = ?1)",
            params![image_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(AppError::ImageNotFound(image_id));
        }
        if var_type == "nominal" && !valid_codes.contains(&(value as i64)) {
            return Err(AppError::InvalidArgument(format!(
                "código {value} inválido para la variable nominal '{variable_name}'"
            )));
        }
        if var_type == "ordinal"
            && let (Some(min), Some(max)) = (min_value, max_value)
            && (value < min || value > max)
        {
            return Err(AppError::InvalidArgument(format!(
                "valor {value} fuera de rango [{min}, {max}] para la variable ordinal '{variable_name}'"
            )));
        }
    }

    let tx = conn.transaction()?;
    for &(image_id, value) in values {
        tx.execute(
            "INSERT INTO image_variable_values (image_id, variable_id, value) VALUES (?1, ?2, ?3)
             ON CONFLICT(image_id, variable_id) DO UPDATE SET value = excluded.value",
            params![image_id, variable_id, value],
        )?;
    }
    tx.commit()?;

    Ok(json!({
        "variable": variable_name,
        "values_set": values.len(),
    }))
}

/// `variable-delete --variable <name>`: borra por completo una variable
/// personalizada — sus categorías (si es nominal) y **todos** los valores ya
/// asignados a imágenes (`image_variable_values`), además de la fila en
/// `user_variables`. Agregado por feedback de uso real ("debería poder
/// eliminar las variables definidas si así lo requiero" — ej. para corregir
/// una variable creada con un nombre mal tipeado). Es destructivo e
/// irreversible (a diferencia de `tournament-undo`/`burst-undo`, no hay
/// snapshot que restaurar), así que dispara `db::backup` igual que las demás
/// operaciones irreversibles del CLI.
pub fn delete(
    conn: &mut Connection,
    db_path: &Path,
    variable_name: &str,
) -> AppResult<serde_json::Value> {
    let (variable_id, _var_type, _min, _max) = find_variable(conn, variable_name)?;

    db::backup(conn, db_path)?;

    let tx = conn.transaction()?;
    let values_deleted = tx.execute(
        "DELETE FROM image_variable_values WHERE variable_id = ?1",
        params![variable_id],
    )?;
    tx.execute(
        "DELETE FROM variable_categories WHERE variable_id = ?1",
        params![variable_id],
    )?;
    tx.execute(
        "DELETE FROM user_variables WHERE id = ?1",
        params![variable_id],
    )?;
    tx.commit()?;

    Ok(json!({
        "variable": variable_name,
        "values_deleted": values_deleted,
    }))
}

/// `get-variable-values --variable <name>`: solo lectura, sin backup —
/// devuelve el valor actual (o `null` si nunca se asignó) de una variable
/// para cada imagen activa (`rejected=0 AND missing=0`, mismo criterio que
/// entra a `clustMD`, ver fase2-clustering.md). Existe para que la GUI pueda
/// mostrar/editar valores foto por foto (ver fase5-gui.md, "clasificación
/// visual") sin tener que adivinar qué imágenes ya están etiquetadas — antes
/// de este comando no había forma de leer `image_variable_values` por fuera
/// del modo TUI `variable-tag`.
pub fn get_values(conn: &Connection, variable_name: &str) -> AppResult<serde_json::Value> {
    let (variable_id, _var_type, _min, _max) = find_variable(conn, variable_name)?;

    let mut stmt = conn.prepare(
        "SELECT images.id, images.file_path, v.value \
         FROM images \
         LEFT JOIN image_variable_values v \
           ON v.image_id = images.id AND v.variable_id = ?1 \
         WHERE images.rejected = 0 AND images.missing = 0 \
         ORDER BY images.id",
    )?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map(params![variable_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "file_path": row.get::<_, String>(1)?,
                "value": row.get::<_, Option<f64>>(2)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!(rows))
}
