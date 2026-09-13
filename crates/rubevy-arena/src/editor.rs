//! The code panel, editable: the script the game is showing, the line it stands on, and a way to
//! change it without leaving the game.
//!
//! A game fills [`Editor`] with the file to show and where the script is; the panel draws it,
//! lets it be edited, and writes it back on Ctrl+S. The game does not have to notice the write:
//! the same [`crate::Watch`] that picks up an edit made in a text editor picks this one up.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

/// What the panel shows and edits.
#[derive(Resource, Default)]
pub struct Editor {
    /// The file being shown. Setting it to a different path loads that file.
    pub path: Option<PathBuf>,
    /// Whose file it is, for the title (several robots can share a file).
    pub label: String,
    /// Its text, as edited.
    pub text: String,
    /// What is on disk, so the panel knows whether the text has been changed.
    saved: String,
    /// 1-based: the line the script is standing on, marked in the gutter.
    pub current: Option<u32>,
    /// Where the script is when that is not in this file (inside the DSL, say).
    pub elsewhere: Option<String>,
    /// The last thing that happened to the file, for the panel's footer.
    pub message: String,
    pub open: bool,
}

impl Editor {
    /// Shows `path`, reading it where it is not the file already shown.
    pub fn show(&mut self, path: &std::path::Path) {
        if self.path.as_deref() == Some(path) {
            return;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.saved = text.clone();
                self.text = text;
                self.path = Some(path.to_path_buf());
                self.message.clear();
                self.open = true;
            }
            Err(e) => self.message = format!("{path:?}: {e}"),
        }
    }

    /// Whether the text differs from what is on disk.
    pub fn changed(&self) -> bool {
        self.text != self.saved
    }

    /// Writes it back. The file watcher does the rest: the script starts again.
    pub fn save(&mut self) {
        let Some(path) = self.path.clone() else { return };
        match std::fs::write(&path, &self.text) {
            Ok(()) => {
                self.saved = self.text.clone();
                self.message = "saved".into();
            }
            Err(e) => self.message = format!("could not save: {e}"),
        }
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
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        editor.save();
    }

    let file = editor.path.as_deref().map(name_of).unwrap_or_else(|| "no file".into());
    let title = match &editor.elsewhere {
        Some(at) => format!("{}  {file}  (in {at})", editor.label),
        None => format!("{}  {file}", editor.label),
    };
    let changed = editor.changed();
    let current = editor.current;
    let message = editor.message.clone();

    // egui 0.36 grows panels inside a Ui; a window takes the context, and a movable one suits an
    // editor that shares the screen with the game
    egui::Window::new("code")
        .title_bar(false)
        .resizable(true)
        .default_width(520.0)
        .default_height(640.0)
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&title).strong());
                if changed {
                    ui.label(egui::RichText::new("● unsaved").color(egui::Color32::from_rgb(240, 190, 90)));
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Save (Ctrl+S)").clicked() {
                    editor.save();
                }
                ui.label(egui::RichText::new(&message).weak());
            });
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

fn name_of(path: &std::path::Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
