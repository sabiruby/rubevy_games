//! The code panel, editable: the script the game is showing, the line it stands on, and a way to
//! change it without leaving the game.
//!
//! The editor does no file I/O. What is typed lives in memory, and the buttons turn into an
//! [`EditorAction`] the game carries out: apply the text to the running script, apply it to every
//! script with the same brain, write it to the file, or throw it away. Trying something in a
//! running game should not rewrite the project on disk — and a game whose files cannot be written
//! (a packaged build, a browser) gets the same editor.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

/// One of the things the editor can be switched to — a robot, in SabiRuby Battle. The game fills
/// [`Editor::choices`]; the editor draws a button for each.
#[derive(Debug, Clone)]
pub struct EditorChoice {
    /// The game's own id for it, handed back in [`Editor::picked`].
    pub id: u64,
    pub label: String,
    pub color: (u8, u8, u8),
    /// Drawn faded (a robot that is down, say).
    pub dim: bool,
}

/// What the user asked the editor to do. The game takes it from [`Editor::action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    /// Run the edited text in the script being shown (F5 or Ctrl+Enter).
    Apply,
    /// Run it in every script that has the same brain.
    ApplyAll,
    /// Write it to the brain's file (Ctrl+S).
    Save,
    /// Throw the edits away and go back to what the file says.
    Revert,
}

/// What the panel shows and edits.
#[derive(Resource, Default)]
pub struct Editor {
    /// The buttons along the top: what the editor can be switched to.
    pub choices: Vec<EditorChoice>,
    /// Which of them is showing, to mark its button.
    pub selected: Option<u64>,
    /// Set when a button is clicked; the game takes it and switches.
    pub picked: Option<u64>,
    /// Set when an action button or its key is pressed; the game takes it and does it.
    pub action: Option<EditorAction>,

    /// Whose text this is: the game's id for the script being shown.
    pub key: Option<u64>,
    /// Shown in the title, e.g. `3 blue/scout`.
    pub label: String,
    /// The brain's file name, for the title and the Save button.
    pub file: String,
    /// Whether the script is running a brain that exists only in memory (applied, not saved).
    pub in_memory: bool,
    /// The text as edited.
    pub text: String,
    /// What the script is running now, so the panel knows whether the text has been changed.
    base: String,
    /// Edits not applied yet, per script, so switching away and back does not lose them.
    drafts: HashMap<u64, (String, String)>,

    /// 1-based: the line the script is standing on, marked in the gutter.
    pub current: Option<u32>,
    /// Where the script is when that is not in its own source (inside the DSL, say).
    pub elsewhere: Option<String>,
    /// The last thing that happened, for the panel.
    pub message: String,
    pub open: bool,
}

impl Editor {
    /// Shows the script `key`, whose running source is `running`. Switching away keeps what was
    /// typed and not applied; coming back brings it back. Showing the same script again does
    /// nothing, so a game can call this every frame.
    pub fn show(&mut self, key: u64, running: impl FnOnce() -> String) {
        if self.key == Some(key) {
            return;
        }
        if let Some(old) = self.key.take() {
            if self.text != self.base {
                self.drafts.insert(old, (std::mem::take(&mut self.text), std::mem::take(&mut self.base)));
            }
        }
        self.key = Some(key);
        self.open = true;
        if let Some((text, base)) = self.drafts.remove(&key) {
            self.text = text;
            self.base = base;
            self.message = "edits not applied yet".into();
        } else {
            self.base = running();
            self.text = self.base.clone();
            self.message.clear();
        }
    }

    /// Whether the text differs from what the script is running.
    pub fn changed(&self) -> bool {
        self.text != self.base
    }

    /// The game ran the text: it is what the script is running now.
    pub fn applied(&mut self, message: impl Into<String>) {
        self.base = self.text.clone();
        self.message = message.into();
    }

    /// The game went back to `source` (a revert, or a file that changed underneath).
    pub fn reset_to(&mut self, source: String, message: impl Into<String>) {
        self.base = source.clone();
        self.text = source;
        self.message = message.into();
    }

    /// Scripts other than the one shown that have edits not applied yet.
    pub fn drafts(&self) -> impl Iterator<Item = &u64> {
        self.drafts.keys()
    }
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.init_resource::<Editor>()
            .add_systems(EguiPrimaryContextPass, draw_editor);
    }
}

fn draw_editor(mut contexts: EguiContexts, mut editor: ResMut<Editor>, keys: Res<ButtonInput<KeyCode>>) {
    if !editor.open {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if keys.just_pressed(KeyCode::F5) || (ctrl && keys.just_pressed(KeyCode::Enter)) {
        editor.action = Some(EditorAction::Apply);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        editor.action = Some(EditorAction::Save);
    }

    let star = if editor.in_memory { "*" } else { "" };
    let title = match &editor.elsewhere {
        Some(at) => format!("{}  {}{star}  (in {at})", editor.label, editor.file),
        None => format!("{}  {}{star}", editor.label, editor.file),
    };
    let changed = editor.changed();
    let current = editor.current;
    let message = editor.message.clone();
    let choices = editor.choices.clone();
    let selected = editor.selected;
    let pending: Vec<String> = editor
        .drafts()
        .filter_map(|id| choices.iter().find(|c| c.id == *id).map(|c| c.label.clone()))
        .collect();
    let file = editor.file.clone();
    let amber = egui::Color32::from_rgb(240, 190, 90);

    // egui 0.36 grows panels inside a Ui; a window takes the context, and a movable one suits an
    // editor that shares the screen with the game
    egui::Window::new("code")
        .title_bar(false)
        .resizable(true)
        .default_width(520.0)
        .default_height(640.0)
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .show(ctx, |ui| {
            // one button per thing the game offers; the one showing is marked
            if !choices.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    // big enough to hit without aiming
                    ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for choice in &choices {
                        let (r, g, b) = choice.color;
                        let mut color = egui::Color32::from_rgb(r, g, b);
                        if choice.dim {
                            color = color.gamma_multiply(0.45);
                        }
                        let text = egui::RichText::new(&choice.label).color(color).strong().size(16.0);
                        if ui.add(egui::Button::selectable(selected == Some(choice.id), text)).clicked() {
                            editor.picked = Some(choice.id);
                        }
                    }
                });
                ui.separator();
            }
            if !pending.is_empty() {
                ui.label(egui::RichText::new(format!("not applied yet: {}", pending.join(", "))).color(amber));
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&title).strong());
                if changed {
                    ui.label(egui::RichText::new("● edited").color(amber));
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
                if ui.button("▶ Apply (F5)").on_hover_text("run this in this robot only").clicked() {
                    editor.action = Some(EditorAction::Apply);
                }
                if ui.button(format!("Apply to all {file}")).on_hover_text("run this in every robot with this brain").clicked() {
                    editor.action = Some(EditorAction::ApplyAll);
                }
                if ui.button("Save to file (Ctrl+S)").on_hover_text("write it to disk, for keeping").clicked() {
                    editor.action = Some(EditorAction::Save);
                }
                if ui.button("Revert").on_hover_text("forget the edits, back to the file").clicked() {
                    editor.action = Some(EditorAction::Revert);
                }
            });
            if !message.is_empty() {
                ui.label(egui::RichText::new(&message).weak());
            }
            ui.separator();

            // a listing does not wrap: one row of the gutter is one line of the file, and the line
            // the script is standing on has a band behind it
            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
                ui.fonts_mut(|fonts| fonts.layout_job(listing(text.as_str(), current)))
            };
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let rows = editor.text.lines().count().max(1);
                    let gutter: String = (1..=rows).map(|n| format!("{n:>4}\n")).collect();
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(gutter)
                                .font(egui::FontId::monospace(FONT))
                                .color(egui::Color32::from_rgb(120, 128, 140)),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut editor.text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(rows)
                            .layouter(&mut layouter),
                    );
                });
            });
        });
}

const FONT: f32 = 13.0;

/// The source as a layout: monospace, no wrapping, and the current line on a band.
fn listing(text: &str, current: Option<u32>) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let normal = TextFormat {
        font_id: egui::FontId::monospace(FONT),
        color: egui::Color32::from_rgb(210, 214, 222),
        ..Default::default()
    };
    let marked = TextFormat {
        font_id: egui::FontId::monospace(FONT),
        color: egui::Color32::from_rgb(255, 236, 150),
        background: egui::Color32::from_rgba_unmultiplied(120, 95, 20, 150),
        ..Default::default()
    };
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        let format = if current == Some(i as u32 + 1) { marked.clone() } else { normal.clone() };
        job.append(line, 0.0, format);
    }
    job
}

