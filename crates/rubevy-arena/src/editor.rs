//! The code panel, editable: the script the game is showing, the line it stands on, and a way to
//! change it without leaving the game.
//!
//! The editor does no file I/O. What is typed lives in memory, and the buttons turn into an
//! [`EditorAction`] the game carries out: apply the text to the running script, apply it to every
//! script with the same brain, write it to the file, or throw it away. Trying something in a
//! running game should not rewrite the project on disk — and a game whose files cannot be written
//! (a packaged build, a browser) gets the same editor.
//!
//! **Colour** (H2 of `docs/plans/editor-highlight-plan.md`): the listing is painted token by
//! token, from a table of one byte per source byte that the game's [`Highlighter`] hands over.
//! Nothing here reads Ruby — the classification is Prism's, through
//! `sabiruby_compiler::highlight` on a PC and through the page's bridge in a browser, and which
//! of the two is the game's `platform.rs` to know. A game that registers no highlighter, or a
//! page too old to have the bridge, gets a table of zeroes and the listing it always had.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

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
///
/// The four fields at the bottom are what a second game needed. SabiRuby Battle's unit is a
/// robot, which has a file of its own, so "apply to this one" and "apply to everything on this
/// file" are two different things and `F5` is free to mean the first. The garden's unit is a
/// *species* — every beetle runs `beetle.rb` — so there is only one of those two, and `F5`
/// there already means "write the garden down". Rather than teach the editor about either game,
/// it takes the two button labels (`None` hides the second button) and the key, and asks the
/// game what to call the thing it is editing.
#[derive(Resource)]
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

    /// 1-based: the line the script is standing on right now.
    pub current: Option<u32>,
    /// Where the script has been spending its time, per line (index 0 is line 1), smoothed over
    /// the last second or so. The listing shades each line by it: a brain jumps between lines
    /// far faster than an eye can follow, but where it *keeps* coming back to is readable.
    pub heat: Vec<f32>,
    /// Where the script is when that is not in its own source (inside the DSL, say).
    pub elsewhere: Option<String>,
    /// The Save button's text, where the game's save is not a file ("Save in browser").
    pub save_label: Option<String>,
    /// The last thing that happened, for the panel.
    pub message: String,
    pub open: bool,

    /// What the first button says. Its action is always [`EditorAction::Apply`].
    pub apply_label: String,
    /// What the second button says, and whether there is one: `None` draws no second button,
    /// for a game where "all of them" and "this one" are the same set.
    pub apply_all_label: Option<String>,
    /// The key that applies, beside `Ctrl+Enter`. `None` where the game wants that key
    /// (the garden's `F5` writes the world to a file).
    pub apply_key: Option<KeyCode>,
    /// What one of the things being edited is called, for the hover texts: `robot`, `creature`.
    pub noun: String,

    /// **What kind each byte of [`Editor::text`] is**, 0..=8, as
    /// `sabiruby_compiler::highlight` writes it (`docs/plans/editor-highlight-plan.md` §1 has
    /// the nine). Empty where the game registered no [`Highlighter`]; where the browser's bridge
    /// is missing it is all zeroes, which is the default kind and so the listing the panel had
    /// before there was any colour. [`listing`] reads it; nothing else may write it.
    pub kinds: Vec<u8>,
    /// The hash of the text [`Editor::kinds`] was made from, so the lexer runs when the text
    /// changes and not once a frame. `None` before the first pass.
    ///
    /// The layouter is called every frame the panel is drawn, and Prism takes 147 µs over the
    /// largest file this editor shows (`world.rb`, 16 KB — the measurements in sabiruby's
    /// `docs/worklog/2026-09-18-highlight.md`), which is 0.9% of a 60 Hz frame to pay for
    /// nothing. Hashing the same file takes 2.3 µs (measured, best of 200, this machine — see
    /// this repository's `docs/worklog/2026-09-18-editor-highlight.md`), so the comparison
    /// that saves the 147 µs costs 1.6% of it.
    highlighted_for: Option<u64>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor {
            choices: Vec::new(),
            selected: None,
            picked: None,
            action: None,
            key: None,
            label: String::new(),
            file: String::new(),
            in_memory: false,
            text: String::new(),
            base: String::new(),
            drafts: HashMap::new(),
            current: None,
            heat: Vec::new(),
            elsewhere: None,
            save_label: None,
            message: String::new(),
            open: false,
            apply_label: "▶ Apply (F5)".into(),
            apply_all_label: Some("Apply to all".into()),
            apply_key: Some(KeyCode::F5),
            noun: "robot".into(),
            kinds: Vec::new(),
            highlighted_for: None,
        }
    }
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

    /// Forgets everything shown and every draft: the scripts it was about are gone (a restart).
    pub fn clear(&mut self) {
        self.key = None;
        self.selected = None;
        self.picked = None;
        self.drafts.clear();
        self.text.clear();
        self.base.clear();
        self.kinds.clear();
        self.highlighted_for = None;
        self.heat.clear();
        self.current = None;
        self.elsewhere = None;
    }

    /// Scripts other than the one shown that have edits not applied yet.
    pub fn drafts(&self) -> impl Iterator<Item = &u64> {
        self.drafts.keys()
    }

    /// Classifies the text again if it has changed since the last time, and nothing otherwise.
    ///
    /// One hash of the text against one comparison: every way the text can change goes through
    /// [`Editor::text`] — a keystroke, `show`, `applied`, `reset_to`, a game writing into it —
    /// and none of them has to remember to say so.
    fn reclassify(&mut self, highlight: Highlighter) {
        let mut hasher = DefaultHasher::new();
        self.text.hash(&mut hasher);
        let hash = hasher.finish();
        if self.highlighted_for == Some(hash) {
            return;
        }
        self.kinds = highlight(&self.text);
        self.highlighted_for = Some(hash);
    }
}

/// **What the game hands the panel to get colour**: Ruby source in, one kind per byte out, the
/// nine of `docs/plans/editor-highlight-plan.md` §1. The table must be as long as the source;
/// a byte past its end is drawn in the default colour.
///
/// It is the game's and not this crate's because the two ends of it are the game's: a PC links
/// the Ruby compiler in and a browser calls a function the page defines under the game's own
/// name (`garden/src/platform.rs`). A plain `fn` pointer rather than a boxed closure, which is
/// what [`crate::Settings::load`] already takes for `read` and `write` for the same reason —
/// there is nothing for it to capture.
pub type Highlighter = fn(&str) -> Vec<u8>;

/// The game's [`Highlighter`], where it registered one ([`EditorPlugin::with_highlighter`]).
#[derive(Resource, Clone, Copy)]
pub struct Highlight(pub Highlighter);

/// The panel. [`EditorPlugin::with_highlighter`] is how a game adds colour to it; plain
/// `EditorPlugin` is the panel without any, which is what it was before H2.
#[derive(Default)]
pub struct EditorPlugin {
    highlight: Option<Highlighter>,
}

impl EditorPlugin {
    /// The panel, with the game's lexer behind its colours.
    pub fn with_highlighter(highlight: Highlighter) -> EditorPlugin {
        EditorPlugin { highlight: Some(highlight) }
    }
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        if let Some(highlight) = self.highlight {
            app.insert_resource(Highlight(highlight));
        }
        app.init_resource::<Editor>()
            .init_resource::<crate::ViewInsets>()
            .add_systems(EguiPrimaryContextPass, draw_editor);
    }
}

fn draw_editor(
    mut contexts: EguiContexts,
    mut editor: ResMut<Editor>,
    highlight: Option<Res<Highlight>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut insets: ResMut<crate::ViewInsets>,
) {
    if !editor.open {
        // S3: a camera reads this to keep what matters out from under the panel. Written only
        // when it changes, so a closed editor does not wake the camera sixty times a second.
        if insets.left != 0.0 || insets.right != 0.0 {
            insets.left = 0.0;
            insets.right = 0.0;
        }
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    // only while the panel is open, and only when what it holds has changed
    if let Some(highlight) = highlight.as_deref().copied() {
        editor.reclassify(highlight.0);
    }

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let apply_key = editor.apply_key.is_some_and(|k| keys.just_pressed(k));
    if apply_key || (ctrl && keys.just_pressed(KeyCode::Enter)) {
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
    let heat = editor.heat.clone();
    // the layouter cannot borrow the resource it is called from: `TextEdit::multiline` already
    // holds `editor.text`
    let kinds = editor.kinds.clone();
    let message = editor.message.clone();
    let choices = editor.choices.clone();
    let selected = editor.selected;
    let pending: Vec<String> = editor
        .drafts()
        .filter_map(|id| choices.iter().find(|c| c.id == *id).map(|c| c.label.clone()))
        .collect();
    let save_label = editor.save_label.clone().unwrap_or_else(|| "Save to file (Ctrl+S)".into());
    let apply_label = editor.apply_label.clone();
    let apply_all_label = editor.apply_all_label.clone();
    let noun = editor.noun.clone();
    let amber = egui::Color32::from_rgb(240, 190, 90);

    // egui 0.36 grows panels inside a Ui; a window takes the context, and a movable one suits an
    // editor that shares the screen with the game. It starts at the top right; drag its title bar
    // to put it anywhere else.
    let content = ctx.content_rect();
    let right = content.right();
    let panel = egui::Window::new("Ruby")
        .collapsible(false)
        .resizable(true)
        .default_width(WIDTH)
        .default_height(HEIGHT)
        .default_pos([right - MARGIN - WIDTH, MARGIN])
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
                    ui.label(egui::RichText::new("* edited").color(amber));
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
                if ui.add(egui::Button::new(&apply_label)).on_hover_text(format!("run this in the {noun} shown")).clicked() {
                    editor.action = Some(EditorAction::Apply);
                }
                if let Some(label) = &apply_all_label
                    && ui.button(label).on_hover_text(format!("run this in every {noun} with this behaviour")).clicked()
                {
                    editor.action = Some(EditorAction::ApplyAll);
                }
                if ui.button(save_label).on_hover_text(format!("keep it: {noun}s on the file start with it from now on")).clicked() {
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
                ui.fonts_mut(|fonts| fonts.layout_job(listing(text.as_str(), &heat, &kinds)))
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

    // **What the panel covers, for whatever camera is behind it** (S3, [`crate::ViewInsets`]).
    // The band is measured from the panel's own rectangle rather than from `WIDTH` below,
    // because egui may have grown it to fit its buttons and because the player may have dragged
    // or resized it; it is claimed on whichever side of the window the panel is nearer to, since
    // a panel that has been dragged to the left of the screen is covering the left.
    let covered = panel.map(|p| p.response.rect);
    let (left, right_side) = match covered {
        Some(rect) if rect.center().x >= content.center().x => {
            (0.0, (content.right() - rect.left()).max(0.0))
        }
        Some(rect) => ((rect.right() - content.left()).max(0.0), 0.0),
        None => (0.0, 0.0),
    };
    insets.left = left;
    insets.right = right_side;
}

const FONT: f32 = 13.0;

/// **Where the editor stands before anybody drags it**, in egui points from the top right.
///
/// They are named rather than written into the `Window` builder because a check has to be able to
/// put a pointer *inside* the panel without a screen to look at: the garden's window checks drive
/// the wheel over the editor to show that the camera does not take it
/// (`garden/src/window.rs`, `window_selftest`). A test that guessed the rectangle would be
/// testing its own guess.
pub const MARGIN: f32 = 8.0;
pub const WIDTH: f32 = 520.0;
pub const HEIGHT: f32 = 640.0;

/// **The nine kinds, as colours.** The index is the kind `sabiruby_compiler::highlight` writes:
/// 0 anything else, 1 keyword, 2 string, 3 comment, 4 number, 5 symbol, 6 constant, 7 variable,
/// 8 method name (`docs/plans/editor-highlight-plan.md` §1).
///
/// **Two of them were given.** 0 is the body colour the listing always had, so text that the
/// lexer says nothing special about does not move; 3 is the gutter's own grey, which is what a
/// comment is — there, weak, not in the way.
///
/// **The rest are chosen against two backgrounds and against each other.** The panel is egui's
/// dark theme, so a letter sits on `Visuals::dark().extreme_bg_color`, grey 10; and on the lines
/// the brain keeps coming back to it sits on the heat band, which at its full alpha of 170 makes
/// (103, 77, 17) over that grey. Measured (WCAG contrast, and CIE76 ΔE between every pair — the
/// figures are in `docs/worklog/2026-09-18-editor-highlight.md`): every colour here is at least
/// 4.9 against the panel and 3.0 against the hottest band, and no two are closer than ΔE 25.
///
/// **The warm end of the wheel is the heat's**, which is why nothing here is orange or amber: the
/// band is orange and so is the `* edited` mark above the listing, and a colour that means
/// "this line is where the time goes" must not also mean "this word is a number".
///
/// **8 is deliberately the quietest.** In Ruby an operator is a method call and so is `[]`, so
/// `x =~ /re/` paints `=~` and `me[:Hunger]` paints both brackets with it (sabiruby's
/// `docs/worklog/2026-09-18-highlight.md`, "演算子は 8 になる"). A loud colour there would
/// scribble over every line of the garden's scripts; a soft blue a shade off the body colour
/// (ΔE 28 from 0) says "a name that is called" without shouting it.
const KINDS: [egui::Color32; 9] = [
    egui::Color32::from_rgb(210, 214, 222), // 0 anything else — the body colour, unchanged
    egui::Color32::from_rgb(185, 145, 235), // 1 keyword — violet
    egui::Color32::from_rgb(150, 200, 130), // 2 string — green
    egui::Color32::from_rgb(120, 128, 140), // 3 comment — the gutter's grey
    egui::Color32::from_rgb(120, 200, 205), // 4 number — cyan
    egui::Color32::from_rgb(235, 150, 200), // 5 symbol — pink
    egui::Color32::from_rgb(230, 205, 140), // 6 constant — sand
    egui::Color32::from_rgb(240, 130, 120), // 7 variable — coral
    egui::Color32::from_rgb(140, 180, 230), // 8 method name — soft blue, the quiet one
];

/// The colour a kind is painted in, and the body colour for anything the table does not name.
pub fn kind_color(kind: u8) -> egui::Color32 {
    KINDS.get(kind as usize).copied().unwrap_or(KINDS[0])
}

/// **Which kind the byte at `at` of the panel's text is painted with**, going the whole way
/// through [`listing`] and reading the colour back out of the `LayoutJob` — the window's checks
/// say "`def` is drawn in the keyword colour" with this, without a pixel to look at. `None`
/// where that byte is not in the listing at all.
pub fn drawn_kind(editor: &Editor, at: usize) -> Option<u8> {
    let job = listing(&editor.text, &editor.heat, &editor.kinds);
    // `LayoutSection::byte_range` is a range of egui's `ByteIndex`, which is a byte offset
    let at = egui::text::ByteIndex(at);
    let section = job.sections.iter().find(|s| s.byte_range.contains(&at))?;
    KINDS.iter().position(|c| *c == section.format.color).map(|k| k as u8)
}

/// The source as a layout: monospace, no wrapping, each line shaded by how much of the brain's
/// recent time it took — the hottest line fully, the rest in proportion — and each token in the
/// colour of its kind.
///
/// The two are a background and a foreground and they do not fight: the band says where the
/// brain is spending its life, the letters say what they are. (Before H2 the foreground said
/// both, and the hottest lines were drawn in amber; there is nothing left for that to say that
/// the band does not, and the letters are wanted for Ruby.)
///
/// `kinds` is one byte per byte of `text`, or empty, or short — a byte it does not reach is the
/// default kind. **The runs are cut at character boundaries**, not at bytes: `append` takes a
/// `&str` and a Japanese comment is three bytes a character, so the loop walks `char_indices`
/// and a kind is the kind of a character's first byte (`editor-highlight-plan.md` §5.4).
pub fn listing(text: &str, heat: &[f32], kinds: &[u8]) -> egui::text::LayoutJob {
    use egui::text::LayoutJob;
    let hottest = heat.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let mut at = 0; // where this line starts in `text`, which is where it starts in `kinds`
    for (i, line) in text.split_inclusive('\n').enumerate() {
        let share = heat.get(i).copied().unwrap_or(0.0) / hottest;
        // below a tenth of the hottest line, nothing: a line passed through once is not a place
        let share = if share < 0.1 { 0.0 } else { share };
        let alpha = (share * 170.0) as u8;
        let band = egui::Color32::from_rgba_unmultiplied(150, 110, 20, alpha);
        let kind_at = |o: usize| kinds.get(at + o).copied().unwrap_or(0);
        let mut run = 0; // the byte in `line` the run being gathered started at
        let mut kind = kind_at(0);
        for (o, _) in line.char_indices() {
            let here = kind_at(o);
            if here != kind {
                job.append(&line[run..o], 0.0, format(kind, band));
                run = o;
                kind = here;
            }
        }
        job.append(&line[run..], 0.0, format(kind, band));
        at += line.len();
    }
    job
}

/// One run of one kind, over whatever the heat put behind this line.
fn format(kind: u8, band: egui::Color32) -> egui::text::TextFormat {
    egui::text::TextFormat {
        font_id: egui::FontId::monospace(FONT),
        color: kind_color(kind),
        background: band,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the panel would draw, run by run: the text of each section and the kind its colour
    /// stands for.
    fn runs(text: &str, kinds: &[u8]) -> Vec<(String, u8)> {
        let job = listing(text, &[], kinds);
        job.sections
            .iter()
            .map(|s| {
                let kind = KINDS.iter().position(|c| *c == s.format.color).unwrap() as u8;
                (job.text[s.byte_range.start.0..s.byte_range.end.0].to_string(), kind)
            })
            .collect()
    }

    /// The whole text is drawn, once, in order — whatever the table says. A run that was dropped
    /// or drawn twice would be a hole in the listing.
    fn is_whole(text: &str, kinds: &[u8]) {
        let drawn: String = runs(text, kinds).into_iter().map(|(s, _)| s).collect();
        assert_eq!(drawn, text);
    }

    #[test]
    fn a_run_per_kind() {
        //                      d  e  f     n  a  m  e
        let kinds: &[u8] = &[1, 1, 1, 0, 8, 8, 8, 8];
        assert_eq!(runs("def name", kinds), vec![("def".into(), 1), (" ".into(), 0), ("name".into(), 8)]);
    }

    /// **The runs are cut at character boundaries** (`editor-highlight-plan.md` §5.4). `append`
    /// takes a `&str`, and slicing a `String` through the middle of a character is a panic — in
    /// the layouter, which runs sixty times a second with the panel open.
    ///
    /// The table here is deliberately a liar: it changes kind in the middle of every one of the
    /// three-byte characters. A table from Prism never does that (sabiruby's
    /// `docs/worklog/2026-09-18-highlight.md` checks it), but the panel is not the place to find
    /// out that one did.
    #[test]
    fn a_kind_that_changes_inside_a_character_does_not_cut_it() {
        let text = "# 甲虫は歩く — 3 bytes a character\n";
        let mut kinds: Vec<u8> = vec![3; text.len()];
        for (i, k) in kinds.iter_mut().enumerate() {
            *k = (i % 9) as u8;
        }
        is_whole(text, &kinds);
        // and every run that was drawn is a whole number of characters
        for (run, _) in runs(text, &kinds) {
            assert!(text.contains(&run), "{run:?} is not a piece of the text");
        }
    }

    /// A shorter table, or none at all, is the listing as it was before there were colours: one
    /// run, in the body colour, for the whole of it. (One and not two, because `LayoutJob::append`
    /// merges a run into the one before it when the format is the same — which is also why a file
    /// with no colour in it costs no more sections than it used to.)
    #[test]
    fn a_short_table_is_the_default_kind() {
        let text = "def a\n@b = 1\n";
        assert_eq!(runs(text, &[]), vec![(text.to_string(), 0)]);
        is_whole(text, &[]);
        is_whole(text, &[1, 1, 1]);
    }

    /// Line by line: the table is indexed from the start of the text, not from the start of a
    /// line. An off-by-one here would paint the second line with the first line's kinds.
    #[test]
    fn the_table_is_indexed_from_the_start_of_the_text() {
        //              d  e  f  \n  @  b
        let kinds = [1, 1, 1, 0, 7, 7];
        assert_eq!(
            runs("def\n@b", &kinds),
            vec![("def".into(), 1), ("\n".into(), 0), ("@b".into(), 7)]
        );
    }

    /// The text a game shows can change under the panel; the classification follows it and is
    /// not redone while it has not.
    #[test]
    fn it_classifies_when_the_text_changes_and_not_otherwise() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn count(src: &str) -> Vec<u8> {
            CALLS.fetch_add(1, Ordering::Relaxed);
            vec![7; src.len()]
        }
        let mut editor = Editor::default();
        editor.text = "def a".into();
        editor.reclassify(count);
        editor.reclassify(count);
        editor.reclassify(count);
        assert_eq!(CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(editor.kinds, vec![7; 5]);
        editor.text.push('b');
        editor.reclassify(count);
        assert_eq!(CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(editor.kinds.len(), 6);
    }
}
