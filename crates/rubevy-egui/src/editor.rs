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
//!
//! **Where the numbers are** (S5b-1). How big the panel is and what size its letters are is
//! [`EditorLayout`], a resource a game can hand the plugin, write into while it runs, or let a
//! player change through the `key=value` store (`editor_*` keys,
//! [`EditorLayout::read_from`]). What it is *painted* with is [`EditorColors`], which sits in
//! [`Editor`] beside the panel's other words and marks, because a check that asks the panel
//! which colour a byte was drawn in ([`drawn_kind`]) already has the [`Editor`] in its hand and
//! cannot ask for a second resource beside it. The `const`s below are the names of the defaults
//! and nothing else; each says in a line where its value came from, and where the answer is
//! "nobody wrote it down", it says that.

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
    /// A second key that applies, beside `Ctrl+Enter`, which always does. `None` by default:
    /// which keys are free is the game's to know (SabiRuby Battle gives this one `F5`; the
    /// garden's `F5` writes the world to a file, so there it stays `None`).
    pub apply_key: Option<KeyCode>,
    /// What one of the things being edited is called, for the hover texts: `script` by default,
    /// `robot` or `creature` where the game says so.
    pub noun: String,
    /// **What the listing is painted with** ([`EditorColors`]). It is a field of the panel's
    /// contents rather than a resource of its own so that [`drawn_kind`] — which a game's checks
    /// call with the `Editor` they are already holding — needs nothing else.
    pub colors: EditorColors,

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
            // **Neutral, and deliberately so** (S4a). Until 2026-09-20 these three said
            // `▶ Apply (F5)`, `F5` and `robot` — SabiRuby Battle's words and SabiRuby Battle's
            // key, sitting in a crate that is not supposed to know what a robot is, and left
            // there because the first game that used it happened to want them. A crate that
            // names one game's unit teaches the next game to override a word rather than to say
            // its own. `script` is what this panel edits whatever the game calls it, the label
            // does not promise a key, and no key is claimed: a game that wants one says which,
            // because only it knows what its other keys already mean.
            apply_label: "▶ Apply".into(),
            apply_all_label: Some("Apply to all".into()),
            apply_key: None,
            noun: "script".into(),
            colors: EditorColors::default(),
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
/// name (`garden/src/platform.rs`). A plain `fn` pointer rather than a boxed closure, for the
/// same reason the `read` and `write` a settings store is given are: there is nothing for one to
/// capture, and a resource holding it stays `Copy`.
pub type Highlighter = fn(&str) -> Vec<u8>;

/// The game's [`Highlighter`], where it registered one ([`EditorPlugin::with_highlighter`]).
#[derive(Resource, Clone, Copy)]
pub struct Highlight(pub Highlighter);

/// The panel. [`EditorPlugin::with_highlighter`] is how a game adds colour to it; plain
/// `EditorPlugin` is the panel without any, which is what it was before H2.
///
/// How big it is is [`EditorPlugin::sized`]; what it is painted with is [`Editor::colors`],
/// which a game writes in a `Startup` system beside the panel's other words (`noun`,
/// `apply_label`), since that is where it says them already.
#[derive(Default)]
pub struct EditorPlugin {
    highlight: Option<Highlighter>,
    layout: EditorLayout,
}

impl EditorPlugin {
    /// The panel, with the game's lexer behind its colours.
    pub fn with_highlighter(highlight: Highlighter) -> EditorPlugin {
        EditorPlugin { highlight: Some(highlight), layout: EditorLayout::default() }
    }

    /// The panel at a size of the game's choosing ([`EditorLayout`]).
    pub fn sized(mut self, layout: EditorLayout) -> EditorPlugin {
        self.layout = layout;
        self
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
            .insert_resource(self.layout.clone())
            .init_resource::<crate::ViewInsets>()
            .add_systems(EguiPrimaryContextPass, draw_editor);
    }
}

fn draw_editor(
    mut contexts: EguiContexts,
    mut editor: ResMut<Editor>,
    layout: Res<EditorLayout>,
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
    let colors = editor.colors.clone();
    let amber = colors.amber;
    let font = layout.font;

    // egui 0.36 grows panels inside a Ui; a window takes the context, and a movable one suits an
    // editor that shares the screen with the game. It starts at the top right; drag its title bar
    // to put it anywhere else.
    let content = ctx.content_rect();
    let right = content.right();
    let panel = egui::Window::new("Ruby")
        .collapsible(false)
        .resizable(true)
        .default_width(layout.width)
        .default_height(layout.height)
        .default_pos([right - layout.margin - layout.width, layout.margin])
        .show(ctx, |ui| {
            // one button per thing the game offers; the one showing is marked
            if !choices.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    // big enough to hit without aiming
                    ui.spacing_mut().button_padding =
                        egui::vec2(layout.choice_padding[0], layout.choice_padding[1]);
                    ui.spacing_mut().item_spacing.x = layout.choice_spacing;
                    for choice in &choices {
                        let (r, g, b) = choice.color;
                        let mut color = egui::Color32::from_rgb(r, g, b);
                        if choice.dim {
                            color = color.gamma_multiply(0.45);
                        }
                        let text =
                            egui::RichText::new(&choice.label).color(color).strong().size(layout.choice_font);
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
                ui.spacing_mut().button_padding =
                    egui::vec2(layout.button_padding[0], layout.button_padding[1]);
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
                ui.fonts_mut(|fonts| {
                    fonts.layout_job(listing(text.as_str(), &heat, &kinds, &colors, font))
                })
            };
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let rows = editor.text.lines().count().max(1);
                    let gutter: String = (1..=rows).map(|n| format!("{n:>4}\n")).collect();
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(gutter)
                                .font(egui::FontId::monospace(font))
                                // the comment colour, by construction (`EditorColors::gutter`)
                                .color(colors.gutter()),
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
    // The band is measured from the panel's own rectangle rather than from [`EditorLayout`],
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

// ---------------------------------------------------------------------------------------------
// The numbers: how big it is (`EditorLayout`) and what it is painted with (`EditorColors`)
// ---------------------------------------------------------------------------------------------

/// The size of the letters in the listing and the gutter. **Source unknown**: it arrived with the
/// panel and nobody wrote down why 13 (`docs/numbers.md` §1.2).
pub const FONT: f32 = 13.0;

/// **Where the editor stands before anybody drags it**, in egui points from the top right.
///
/// They are named rather than written into the `Window` builder because a check has to be able to
/// put a pointer *inside* the panel without a screen to look at: the garden's window checks drive
/// the wheel over the editor to show that the camera does not take it
/// (`garden/src/window.rs`, `window_selftest`). A test that guessed the rectangle would be
/// testing its own guess — which is also why the check reads the *setting* and not this default
/// (S5b-1): a game that opened a wider panel would otherwise be checked against a rectangle it
/// never drew.
///
/// **The values themselves have no recorded source** (`docs/numbers.md` §1.2): the reason the
/// three are named is written down, the reason they are 8, 520 and 640 is not.
pub const MARGIN: f32 = 8.0;
pub const WIDTH: f32 = 520.0;
pub const HEIGHT: f32 = 640.0;

/// The choice buttons along the top: the padding round one, the gap between two, and the size of
/// the label. *Reason only*, and it is the reason for all three: "big enough to hit without
/// aiming" (`draw_editor`). No measurement.
pub const CHOICE_PADDING: [f32; 2] = [10.0, 5.0];
pub const CHOICE_SPACING: f32 = 6.0;
pub const CHOICE_FONT: f32 = 16.0;

/// The padding round the Apply / Save / Revert buttons. **Source unknown** beyond being smaller
/// than the choice buttons' above.
pub const BUTTON_PADDING: [f32; 2] = [8.0, 4.0];

/// **How big the panel is and what size its letters are**, in one resource a game can hand
/// [`EditorPlugin::sized`], write into while it runs, or let a player change through the
/// `key=value` store ([`EditorLayout::read_from`]).
///
/// The defaults are the `const`s above, each of which says where its value came from.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct EditorLayout {
    /// [`MARGIN`]
    pub margin: f32,
    /// [`WIDTH`]
    pub width: f32,
    /// [`HEIGHT`]
    pub height: f32,
    /// [`FONT`]
    pub font: f32,
    /// [`CHOICE_PADDING`]
    pub choice_padding: [f32; 2],
    /// [`CHOICE_SPACING`]
    pub choice_spacing: f32,
    /// [`CHOICE_FONT`]
    pub choice_font: f32,
    /// [`BUTTON_PADDING`]
    pub button_padding: [f32; 2],
}

impl Default for EditorLayout {
    fn default() -> Self {
        EditorLayout {
            margin: MARGIN,
            width: WIDTH,
            height: HEIGHT,
            font: FONT,
            choice_padding: CHOICE_PADDING,
            choice_spacing: CHOICE_SPACING,
            choice_font: CHOICE_FONT,
            button_padding: BUTTON_PADDING,
        }
    }
}

impl EditorLayout {
    /// **What a player left in a `key=value` store**, for the numbers a person can sensibly be
    /// asked about: how big the panel is and how big its letters are. The store itself is the
    /// game's (`games_shell::Settings`), which this crate does not depend on, so what comes in
    /// here is a function that answers a key.
    ///
    /// | key | field |
    /// |---|---|
    /// | `editor_margin` | [`EditorLayout::margin`] |
    /// | `editor_width` | [`EditorLayout::width`] |
    /// | `editor_height` | [`EditorLayout::height`] |
    /// | `editor_font` | [`EditorLayout::font`] |
    /// | `editor_choice_font` | [`EditorLayout::choice_font`] |
    /// | `editor_choice_spacing` | [`EditorLayout::choice_spacing`] |
    ///
    /// The two paddings are left out for the same reason the camera leaves its keys out: a pair
    /// of numbers in a text file needs a parser, and a button's padding is not a thing a player
    /// asks for by name. A game may still set them.
    ///
    /// A key that is not there leaves the field alone, which is what makes a store written by an
    /// older build safe to read.
    pub fn read_from(&mut self, number: impl Fn(&str) -> Option<f32>) {
        let take = |key: &str, slot: &mut f32| {
            if let Some(value) = number(key) {
                *slot = value;
            }
        };
        take("editor_margin", &mut self.margin);
        take("editor_width", &mut self.width);
        take("editor_height", &mut self.height);
        take("editor_font", &mut self.font);
        take("editor_choice_font", &mut self.choice_font);
        take("editor_choice_spacing", &mut self.choice_spacing);
    }
}

/// The colour of the heat band at its strongest, and how strong that is.
///
/// **Cited**: the band's colour is what the nine kinds below were measured against — composited
/// at this alpha over egui's dark panel it makes (103, 77, 17), which is the second of the two
/// backgrounds every colour was checked on. The alpha of 170 itself has **no recorded source**;
/// the colours are measured *given* it.
pub const HEAT_BAND: (u8, u8, u8) = (150, 110, 20);
pub const HEAT_ALPHA: u8 = 170;

/// Below this share of the hottest line, no band at all. *Reason only*: "a line passed through
/// once is not a place".
pub const HEAT_FLOOR: f32 = 0.1;

/// `* edited`, and the list of scripts with edits not applied. *Reason only*: the warm end of the
/// wheel is the heat's, and this mark means the same kind of thing.
pub const AMBER: egui::Color32 = egui::Color32::from_rgb(240, 190, 90);

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
///
/// **Changing them breaks what was measured** (S5b-1). These nine are the one table in this
/// crate with a measurement behind it, and the measurement is of the *set*: every colour at
/// least 4.96 against the panel and 3.09 against the hottest band, and no two closer than
/// ΔE 25.9 (`docs/worklog/2026-09-18-editor-highlight.md`). A game may put its own colours in
/// [`EditorColors::kinds`] — and then none of those three sentences is true of them any more,
/// and nothing here will notice. The same goes for changing [`HEAT_BAND`] or [`HEAT_ALPHA`],
/// which is the background half of the same measurement.
pub const KINDS: [egui::Color32; 9] = [
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

/// **What the listing is painted with**, in one value a game can put in [`Editor::colors`].
///
/// The defaults are the `const`s above. Read the note on [`KINDS`] before changing them: those
/// nine are the one table here with a measurement behind it, and it is a measurement of the set.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorColors {
    /// [`KINDS`]
    pub kinds: [egui::Color32; 9],
    /// [`HEAT_BAND`]
    pub heat_band: (u8, u8, u8),
    /// [`HEAT_ALPHA`]
    pub heat_alpha: u8,
    /// [`HEAT_FLOOR`]
    pub heat_floor: f32,
    /// [`AMBER`]
    pub amber: egui::Color32,
}

impl Default for EditorColors {
    fn default() -> Self {
        EditorColors {
            kinds: KINDS,
            heat_band: HEAT_BAND,
            heat_alpha: HEAT_ALPHA,
            heat_floor: HEAT_FLOOR,
            amber: AMBER,
        }
    }
}

impl EditorColors {
    /// The colour a kind is painted in, and the body colour for anything the table does not name.
    pub fn of(&self, kind: u8) -> egui::Color32 {
        self.kinds.get(kind as usize).copied().unwrap_or(self.kinds[0])
    }

    /// **The line numbers down the left.** It is the comment colour and not a fourth grey of its
    /// own: a line number is the same kind of thing as a comment — there, weak, not in the way
    /// (`editor-highlight-plan.md`). Written as this lookup rather than as a second copy of the
    /// value so that a game that changes the comment colour does not leave the gutter behind.
    pub fn gutter(&self) -> egui::Color32 {
        self.of(3)
    }
}

/// **Which kind the byte at `at` of the panel's text is painted with**, going the whole way
/// through [`listing`] and reading the colour back out of the `LayoutJob` — the window's checks
/// say "`def` is drawn in the keyword colour" with this, without a pixel to look at. `None`
/// where that byte is not in the listing at all.
pub fn drawn_kind(editor: &Editor, at: usize) -> Option<u8> {
    // the size of the letters does not decide which run a byte falls in, only how wide it is
    // drawn, so the default is as good as the game's layout here
    let job = listing(&editor.text, &editor.heat, &editor.kinds, &editor.colors, FONT);
    // `LayoutSection::byte_range` is a range of egui's `ByteIndex`, which is a byte offset
    let at = egui::text::ByteIndex(at);
    let section = job.sections.iter().find(|s| s.byte_range.contains(&at))?;
    editor.colors.kinds.iter().position(|c| *c == section.format.color).map(|k| k as u8)
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
pub fn listing(
    text: &str,
    heat: &[f32],
    kinds: &[u8],
    colors: &EditorColors,
    font: f32,
) -> egui::text::LayoutJob {
    use egui::text::LayoutJob;
    let hottest = heat.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let (r, g, b) = colors.heat_band;
    let mut at = 0; // where this line starts in `text`, which is where it starts in `kinds`
    for (i, line) in text.split_inclusive('\n').enumerate() {
        let share = heat.get(i).copied().unwrap_or(0.0) / hottest;
        // below a tenth of the hottest line, nothing: a line passed through once is not a place
        let share = if share < colors.heat_floor { 0.0 } else { share };
        let alpha = (share * colors.heat_alpha as f32) as u8;
        let band = egui::Color32::from_rgba_unmultiplied(r, g, b, alpha);
        let kind_at = |o: usize| kinds.get(at + o).copied().unwrap_or(0);
        let mut run = 0; // the byte in `line` the run being gathered started at
        let mut kind = kind_at(0);
        for (o, _) in line.char_indices() {
            let here = kind_at(o);
            if here != kind {
                job.append(&line[run..o], 0.0, format(kind, band, colors, font));
                run = o;
                kind = here;
            }
        }
        job.append(&line[run..], 0.0, format(kind, band, colors, font));
        at += line.len();
    }
    job
}

/// One run of one kind, over whatever the heat put behind this line.
fn format(kind: u8, band: egui::Color32, colors: &EditorColors, font: f32) -> egui::text::TextFormat {
    egui::text::TextFormat {
        font_id: egui::FontId::monospace(font),
        color: colors.of(kind),
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
        let colors = EditorColors::default();
        let job = listing(text, &[], kinds, &colors, FONT);
        job.sections
            .iter()
            .map(|s| {
                let kind = colors.kinds.iter().position(|c| *c == s.format.color).unwrap() as u8;
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

    /// **A number in the store changes the panel and leaves the rest alone** (S5b-1). The store
    /// is the game's — `games_shell::Settings`, which this crate does not depend on — so what is
    /// handed over is a function that answers a key, and this is one written by hand.
    #[test]
    fn the_store_changes_a_size_and_leaves_the_rest() {
        let mut layout = EditorLayout::default();
        layout.read_from(|key| match key {
            "editor_width" => Some(720.0),
            "editor_font" => Some(16.0),
            _ => None,
        });
        assert_eq!(layout.width, 720.0);
        assert_eq!(layout.font, 16.0);
        assert_eq!(layout.height, HEIGHT, "a key nobody wrote leaves the default alone");
        assert_eq!(layout.margin, MARGIN);
        // and nothing at all in the store is the panel exactly as it was
        let mut untouched = EditorLayout::default();
        untouched.read_from(|_| None);
        assert_eq!(untouched, EditorLayout::default());
    }

    /// **Changed colours are what the listing is painted with** — the whole of what making the
    /// nine settable means, and the reason the note on [`KINDS`] says what it says: nothing here
    /// measures the new ones.
    #[test]
    fn the_listing_is_painted_with_the_colours_it_was_given() {
        let mut editor = Editor::default();
        editor.text = "def a".into();
        editor.kinds = vec![1, 1, 1, 0, 8];
        assert_eq!(drawn_kind(&editor, 0), Some(1), "the default palette, as it always was");

        let mine = egui::Color32::from_rgb(1, 2, 3);
        editor.colors.kinds[1] = mine;
        let job = listing(&editor.text, &editor.heat, &editor.kinds, &editor.colors, FONT);
        assert_eq!(job.sections[0].format.color, mine, "`def` is drawn in the game's colour");
        // and the kind is still read back out of it, because the lookup is the same table
        assert_eq!(drawn_kind(&editor, 0), Some(1));
        // the gutter follows the comment colour rather than keeping a copy of it
        editor.colors.kinds[3] = mine;
        assert_eq!(editor.colors.gutter(), mine);
    }

    /// The heat band is the other half of the measured pair, and it is settable too: at alpha 0
    /// the hottest line has no band at all.
    #[test]
    fn the_heat_band_is_the_colour_and_the_strength_it_was_given() {
        let text = "a\nb\n";
        let heat = vec![0.0, 1.0];
        let mut colors = EditorColors::default();
        let banded = listing(text, &heat, &[], &colors, FONT);
        let hot = banded.sections.last().unwrap().format.background;
        assert_eq!(hot, egui::Color32::from_rgba_unmultiplied(150, 110, 20, HEAT_ALPHA));

        colors.heat_alpha = 0;
        let flat = listing(text, &heat, &[], &colors, FONT);
        assert_eq!(flat.sections.last().unwrap().format.background.a(), 0, "no band at all");
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
