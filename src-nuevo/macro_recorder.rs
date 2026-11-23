//! MetryCAD macros: record the commands a person runs, keep them by name, play them back and
//! pin them as buttons in a small floating panel.
//!
//! A macro is a list of engine commands (the same JSON a script holds), so playing one never
//! runs arbitrary code. A list written by an assistant can be pasted in and runs the same way.
//! Macros are saved as `macros.json` in the app's data folder.
use crate::SolveApp;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Serialize, Deserialize)]
pub struct Step {
    pub command: String,
    pub params: Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Default)]
pub struct MacroState {
    pub open: bool,
    pub recording: bool,
    pub playing: bool,
    pub loaded: bool,
    pub steps: Vec<Step>,
    pub saved: Vec<Macro>,
    pub name: String,
    pub import_text: String,
    pub message: String,
}

enum Act {
    None,
    Record,
    Stop,
    Discard,
    SaveLast,
    PlayLast,
    Run(usize),
    Pin(usize),
    Copy(usize),
    Delete(usize),
    Import,
}

/// Commands that are never recorded: they change the view, the window or the files rather than
/// the design, or they would make a recording unsafe to play on another design.
fn recordable(id: &str) -> bool {
    const SKIP: [&str; 12] = ["view.", "plugin.", "tab.", "ui.", "window.", "render.", "help.", "doc.", "file.", "timeline.", "edit.undo", "edit.redo"];
    !SKIP.iter().any(|p| id.starts_with(p))
}

/// Commands a macro may not run, whoever wrote it (an assistant's list is pasted in as text).
fn allowed(id: &str) -> bool {
    const BLOCKED: [&str; 6] = ["plugin.", "file.trash", "file.move", "file.copy", "doc.close", "update."];
    solidcraft_engine::find_command(id).is_some() && !BLOCKED.iter().any(|p| id.starts_with(p))
}

fn file() -> Option<std::path::PathBuf> {
    solidcraft_engine::recovery::data_dir().map(|d| d.join("macros.json"))
}

fn load(app: &mut SolveApp) {
    app.macro_rec.loaded = true;
    let Some(path) = file() else { return };
    if let Some(list) = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Vec<Macro>>(&s).ok()) {
        app.macro_rec.saved = list;
    }
}

fn save(app: &mut SolveApp) {
    let Some(path) = file() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(&app.macro_rec.saved) {
        let _ = std::fs::write(path, text);
    }
}

/// Called after every command the window runs; adds the successful, recordable ones.
pub(crate) fn record(app: &mut SolveApp, id: &str, params: &Value, ok: bool) {
    if !ok || !app.macro_rec.recording || app.macro_rec.playing || !recordable(id) {
        return;
    }
    app.macro_rec.steps.push(Step { command: id.to_owned(), params: params.clone() });
}

fn script_json(steps: &[Step]) -> String {
    let commands: Vec<Value> = steps.iter().map(|s| json!({"command": s.command, "params": s.params})).collect();
    serde_json::to_string_pretty(&json!({"commands": commands})).unwrap_or_default()
}

/// A script as the CLI reads it (`{"commands": [...]}`) or a bare list of the same entries.
fn parse_script(text: &str) -> Result<Vec<Step>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let list = match &v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o.get("commands").and_then(Value::as_array).cloned().ok_or("no \"commands\" list")?,
        _ => return Err("expected a list of commands".into()),
    };
    let mut steps = Vec::new();
    for item in list {
        let command = item.get("command").and_then(Value::as_str).ok_or("an entry has no \"command\"")?.to_owned();
        if !allowed(&command) {
            return Err(format!("command not allowed or unknown: {command}"));
        }
        let params = item.get("params").cloned().unwrap_or_else(|| json!({}));
        steps.push(Step { command, params });
    }
    if steps.is_empty() {
        return Err("the list is empty".into());
    }
    Ok(steps)
}

fn play(app: &mut SolveApp, steps: &[Step], es: bool) {
    app.macro_rec.playing = true;
    let mut done = 0usize;
    let mut failed: Option<String> = None;
    for step in steps {
        if !allowed(&step.command) {
            failed = Some(format!("{}: not allowed", step.command));
            break;
        }
        match app.run(&step.command, step.params.clone()) {
            Ok(_) => done += 1,
            Err(e) => {
                failed = Some(format!("{}: {e}", step.command));
                break;
            }
        }
    }
    app.macro_rec.playing = false;
    app.macro_rec.message = match failed {
        Some(e) if es => format!("Se detuvo tras {done} paso(s). {e}"),
        Some(e) => format!("Stopped after {done} step(s). {e}"),
        None if es => format!("Listo: {done} paso(s) ejecutados."),
        None => format!("Done: {done} step(s) run."),
    };
}

/// The pinned buttons and the Macros button, bottom left; the panel opens from the button.
pub(crate) fn show(app: &mut SolveApp, ctx: &egui::Context) {
    if app.home.open {
        return;
    }
    if !app.macro_rec.loaded {
        load(app);
    }
    let es = solidcraft_ui_text::effective_language(&app.preferences.language).starts_with("es");
    let tx = |en: &'static str, sp: &'static str| if es { sp } else { en };
    let mut action = Act::None;

    egui::Area::new(egui::Id::new("metrycad_macro_pill")).anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -44.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        for i in 0..app.macro_rec.saved.len() {
            if app.macro_rec.saved[i].pinned {
                let label = format!("> {}", app.macro_rec.saved[i].name);
                if ui.button(label).clicked() {
                    action = Act::Run(i);
                }
            }
        }
        let label = if app.macro_rec.recording {
            format!("(REC {}) {}", app.macro_rec.steps.len(), tx("Macros", "Macros"))
        } else {
            tx("Macros", "Macros").to_owned()
        };
        if ui.button(label).clicked() {
            app.macro_rec.open = !app.macro_rec.open;
        }
    });

    let mut open = app.macro_rec.open;
    if open {
        egui::Window::new(tx("Macros and my tools", "Macros y mis herramientas")).open(&mut open).default_width(380.0).show(ctx, |ui| {
            if app.macro_rec.recording {
                ui.label(format!("{} {}", tx("Recording. Steps:", "Grabando. Pasos:"), app.macro_rec.steps.len()));
                if ui.button(tx("Stop recording", "Detener grabación")).clicked() {
                    action = Act::Stop;
                }
            } else if ui.button(tx("Record what I do", "Grabar lo que hago")).clicked() {
                action = Act::Record;
            }
            if !app.macro_rec.recording && !app.macro_rec.steps.is_empty() {
                ui.separator();
                ui.label(format!("{} {}", tx("Last recording, steps:", "Última grabación, pasos:"), app.macro_rec.steps.len()));
                ui.horizontal(|ui| {
                    ui.label(tx("Name", "Nombre"));
                    ui.text_edit_singleline(&mut app.macro_rec.name);
                });
                ui.horizontal(|ui| {
                    if ui.button(tx("Save", "Guardar")).clicked() {
                        action = Act::SaveLast;
                    }
                    if ui.button(tx("Play", "Reproducir")).clicked() {
                        action = Act::PlayLast;
                    }
                    if ui.button(tx("Discard", "Descartar")).clicked() {
                        action = Act::Discard;
                    }
                });
            }
            ui.separator();
            ui.label(tx("Saved macros", "Macros guardadas"));
            if app.macro_rec.saved.is_empty() {
                ui.label(tx("None yet. Record one, or paste a command list below.", "Ninguna aún. Graba una, o pega una lista de comandos abajo."));
            }
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for i in 0..app.macro_rec.saved.len() {
                    let name = app.macro_rec.saved[i].name.clone();
                    let count = app.macro_rec.saved[i].steps.len();
                    let mut pinned = app.macro_rec.saved[i].pinned;
                    ui.horizontal(|ui| {
                        ui.label(format!("{name} ({count})"));
                        if ui.button(tx("Run", "Ejecutar")).clicked() {
                            action = Act::Run(i);
                        }
                        if ui.checkbox(&mut pinned, tx("Pin", "Fijar")).changed() {
                            action = Act::Pin(i);
                        }
                        if ui.button(tx("Copy", "Copiar")).clicked() {
                            action = Act::Copy(i);
                        }
                        if ui.button(tx("Delete", "Borrar")).clicked() {
                            action = Act::Delete(i);
                        }
                    });
                }
            });
            ui.separator();
            ui.label(tx("Paste a command list (JSON) from an assistant:", "Pega una lista de comandos (JSON) de un asistente:"));
            ui.add(egui::TextEdit::multiline(&mut app.macro_rec.import_text).desired_rows(4).desired_width(f32::INFINITY));
            if ui.button(tx("Import as macro", "Importar como macro")).clicked() {
                action = Act::Import;
            }
            if !app.macro_rec.message.is_empty() {
                ui.separator();
                ui.label(app.macro_rec.message.clone());
            }
        });
    }
    app.macro_rec.open = open;

    match action {
        Act::None => {}
        Act::Record => {
            app.macro_rec.steps.clear();
            app.macro_rec.recording = true;
            app.macro_rec.message.clear();
        }
        Act::Stop => app.macro_rec.recording = false,
        Act::Discard => app.macro_rec.steps.clear(),
        Act::SaveLast => {
            let steps = std::mem::take(&mut app.macro_rec.steps);
            let typed = app.macro_rec.name.trim().to_owned();
            let name = if typed.is_empty() { format!("Macro {}", app.macro_rec.saved.len() + 1) } else { typed };
            app.macro_rec.saved.push(Macro { name, steps, pinned: false });
            app.macro_rec.name.clear();
            save(app);
            app.macro_rec.message = tx("Saved.", "Guardada.").to_owned();
        }
        Act::PlayLast => {
            let steps = app.macro_rec.steps.clone();
            play(app, &steps, es);
        }
        Act::Run(i) => {
            if let Some(steps) = app.macro_rec.saved.get(i).map(|m| m.steps.clone()) {
                play(app, &steps, es);
            }
        }
        Act::Pin(i) => {
            if let Some(m) = app.macro_rec.saved.get_mut(i) {
                m.pinned = !m.pinned;
            }
            save(app);
        }
        Act::Copy(i) => {
            if let Some(text) = app.macro_rec.saved.get(i).map(|m| script_json(&m.steps)) {
                ctx.copy_text(text);
                app.macro_rec.message = tx("Copied as a command list.", "Copiada como lista de comandos.").to_owned();
            }
        }
        Act::Delete(i) => {
            if i < app.macro_rec.saved.len() {
                app.macro_rec.saved.remove(i);
            }
            save(app);
        }
        Act::Import => match parse_script(&app.macro_rec.import_text) {
            Ok(steps) => {
                let name = format!("Macro {}", app.macro_rec.saved.len() + 1);
                app.macro_rec.saved.push(Macro { name, steps, pinned: false });
                app.macro_rec.import_text.clear();
                save(app);
                app.macro_rec.message = tx("Imported. Pin it to use it as a button.", "Importada. Fíjala para usarla como botón.").to_owned();
            }
            Err(e) => app.macro_rec.message = format!("{}: {e}", tx("Could not import", "No se pudo importar")),
        },
    }
}
