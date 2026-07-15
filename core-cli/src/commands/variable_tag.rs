//! `photoranker variable-tag --variable <nombre>` — modo TUI, ver
//! docs/fase1-ingesta.md y "Interacción por Teclado" en docs/fase3-torneo.md.

use crate::error::{AppError, AppResult};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Size};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::picker::Picker;
use ratatui_image::{Image, Resize};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;

struct Category {
    code: i64,
    label: String,
}

struct VariableDef {
    id: i64,
    var_type: String,
    min_value: Option<f64>,
    max_value: Option<f64>,
    categories: Vec<Category>,
}

fn load_variable(conn: &Connection, name: &str) -> AppResult<VariableDef> {
    let row = conn
        .query_row(
            "SELECT id, var_type, min_value, max_value FROM user_variables WHERE name = ?1",
            params![name],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<f64>>(2)?,
                    r.get::<_, Option<f64>>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((id, var_type, min_value, max_value)) = row else {
        return Err(AppError::VariableNotFound(name.to_string()));
    };

    let mut stmt = conn.prepare(
        "SELECT code, label FROM variable_categories WHERE variable_id = ?1 ORDER BY code",
    )?;
    let categories = stmt
        .query_map(params![id], |r| {
            Ok(Category {
                code: r.get(0)?,
                label: r.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(VariableDef {
        id,
        var_type,
        min_value,
        max_value,
        categories,
    })
}

/// Imágenes activas (`missing=0`) que aún no tienen valor asignado para esta
/// variable — así reanudar `variable-tag` continúa donde el usuario dejó el
/// etiquetado en la sesión anterior, sin repetir lo ya hecho.
fn pending_image_ids(conn: &Connection, variable_id: i64) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM images
         WHERE missing = 0
           AND id NOT IN (SELECT image_id FROM image_variable_values WHERE variable_id = ?1)
         ORDER BY id",
    )?;
    let ids = stmt
        .query_map(params![variable_id], |r| r.get::<_, i64>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

fn load_thumbnail(conn: &Connection, image_id: i64) -> AppResult<Option<Vec<u8>>> {
    conn.query_row(
        "SELECT thumbnail FROM images WHERE id = ?1",
        params![image_id],
        |r| r.get::<_, Option<Vec<u8>>>(0),
    )
    .map_err(AppError::from)
}

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> AppResult<Self> {
        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

pub fn run(conn: &mut Connection, variable_name: &str) -> AppResult<serde_json::Value> {
    let variable = load_variable(conn, variable_name)?;
    let mut queue = pending_image_ids(conn, variable.id)?;
    if queue.is_empty() {
        return Ok(json!({"variable": variable_name, "tagged_this_session": 0, "remaining": 0}));
    }

    let _guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    let mut index = 0usize;
    let mut tagged = 0u32;

    loop {
        if index >= queue.len() {
            break;
        }
        let image_id = queue[index];
        let thumbnail = load_thumbnail(conn, image_id)?;
        let dyn_img = thumbnail
            .as_deref()
            .and_then(|bytes| image::load_from_memory(bytes).ok());

        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Min(3)])
                .split(area);

            if let Some(img) = &dyn_img {
                let image_area = Size::new(chunks[0].width, chunks[0].height);
                if let Ok(protocol) =
                    picker.new_protocol(img.clone(), image_area, Resize::Fit(None))
                {
                    frame.render_widget(Image::new(&protocol), chunks[0]);
                }
            } else {
                frame.render_widget(
                    Paragraph::new("(sin miniatura disponible)")
                        .block(Block::default().borders(Borders::ALL)),
                    chunks[0],
                );
            }

            let help = help_text(&variable);
            let title = format!(
                "variable-tag — imagen {} de {} (id {}) — {} etiquetadas esta sesión",
                index + 1,
                queue.len(),
                image_id,
                tagged
            );
            let lines: Vec<Line> =
                std::iter::once(Line::styled(title, Style::default().fg(Color::Cyan)))
                    .chain(help.into_iter().map(Line::from))
                    .collect();
            frame.render_widget(
                Paragraph::new(lines).block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
        })?;

        if !event::poll(std::time::Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => break,
            KeyCode::Char(' ') => {
                index += 1;
            }
            KeyCode::Backspace => {
                index = index.saturating_sub(1);
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let digit = c.to_digit(10).unwrap() as i64;
                if is_valid_value(&variable, digit) {
                    conn.execute(
                        "INSERT INTO image_variable_values (image_id, variable_id, value) VALUES (?1, ?2, ?3)
                         ON CONFLICT(image_id, variable_id) DO UPDATE SET value = excluded.value",
                        params![image_id, variable.id, digit as f64],
                    )?;
                    tagged += 1;
                    index += 1;
                }
            }
            _ => {}
        }
    }
    let _ = queue.drain(..);

    Ok(json!({
        "variable": variable_name,
        "tagged_this_session": tagged,
        "remaining": pending_image_ids(conn, variable.id)?.len(),
    }))
}

fn is_valid_value(variable: &VariableDef, value: i64) -> bool {
    if variable.var_type == "nominal" {
        variable.categories.iter().any(|c| c.code == value)
    } else {
        match (variable.min_value, variable.max_value) {
            (Some(min), Some(max)) => (value as f64) >= min && (value as f64) <= max,
            _ => true,
        }
    }
}

fn help_text(variable: &VariableDef) -> Vec<String> {
    let mut lines = Vec::new();
    if variable.var_type == "nominal" {
        let options: Vec<String> = variable
            .categories
            .iter()
            .map(|c| format!("{}={}", c.code, c.label))
            .collect();
        lines.push(format!("Categorías: {}", options.join("  ")));
    } else if let (Some(min), Some(max)) = (variable.min_value, variable.max_value) {
        lines.push(format!("Rango ordinal: {min}-{max}"));
    }
    lines.push(
        "Número: asigna y avanza · Espacio: saltar · Backspace: volver · Q: salir".to_string(),
    );
    lines
}
