//! **The guide (G6): one panel, two languages, in the game.**
//!
//! Both games here are sat down in front of by somebody who has never read a word about them —
//! the author's own browser play is what found this — and until now the only thing that said what
//! the world was and which keys did anything was a line of small text in the HUD and a `docs/`
//! file in another window. So: a panel, open the first time the game starts, `H` or `?` after
//! that, and a hint in the HUD saying so.
//!
//! **The frame is here and the words are the game's.** What a garden is and what a battle is have
//! nothing in common, but "a window with a paragraph and a key table, in English with the Japanese
//! under it, that `H` opens" is the same thing twice, and the font is the same problem twice. So
//! this module owns [`Guide`] — a title, some paragraphs, some key rows, each with both languages
//! — and each game fills one in.
//!
//! ## The font
//!
//! egui's default fonts are Ubuntu-Light and two Noto symbol faces: **no CJK at all**. Japanese in
//! a label comes out as the replacement box, which is what "tofu" means. There is no system font
//! to fall back on either — the browser build is a wasm module with no access to the machine's
//! fonts, and `fontdb`-style discovery would be a megabyte of code and a different answer on every
//! machine. So the font is *in the binary*: [`CJK`], a subset of Noto Sans JP cut down to the
//! characters this file and the two games' guides actually use (`crates/rubevy-arena/assets/fonts/`,
//! with its licence beside it; `CREDITS.md` and `docs/web.md` have the size).
//!
//! It goes in as a **fallback**, appended to both of egui's families rather than replacing them:
//! egui walks the list per character, so every character the default fonts have is still drawn by
//! them and the Latin text in the editor and the panels is pixel-for-pixel what it was. Only the
//! characters they do not have reach Noto — which is exactly the set the subset contains.
//!
//! Because the subset is cut to the text, **the text and the font have to be cut together**:
//! `tools/subset-font.sh` reads the two games' `src/guide_text.rs` and this file and writes the
//! `.ttf` again. A Japanese word edited into a guide without that step is drawn as blank boxes.
//! The strings a *game* shows all live in its own `guide_text.rs`; the only ones here are
//! [`Guide::HINT`] and the line at the foot of the panel.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

/// Noto Sans JP, subset to the characters the guides use. See the module note.
pub const CJK: &[u8] = include_bytes!("../assets/fonts/NotoSansJP-Guide.subset.ttf");

/// The name egui knows the font by. Anything unique will do; it is a key in a map.
const CJK_NAME: &str = "noto_sans_jp_subset";

/// A paragraph of the guide, in both languages.
#[derive(Debug, Clone)]
pub struct GuideNote {
    pub en: String,
    pub ja: String,
}

/// One row of the key list: what to press, and what it does in both languages.
#[derive(Debug, Clone)]
pub struct GuideKey {
    pub keys: String,
    pub en: String,
    pub ja: String,
}

/// What the panel says. A game builds one at startup and inserts it; the panel is this module's.
///
/// `open` starts `true`, which is the "shown once at the start" the plan asks for: the first
/// thing a player sees is the explanation, and closing it is `H`, `?`, `Esc` or the button.
#[derive(Resource, Debug, Clone)]
pub struct Guide {
    /// The window's title, and the line above the paragraphs.
    pub title: String,
    pub subtitle: String,
    pub notes: Vec<GuideNote>,
    pub keys: Vec<GuideKey>,
    pub open: bool,
}

impl Default for Guide {
    fn default() -> Self {
        Guide {
            title: "Help".into(),
            subtitle: String::new(),
            notes: Vec::new(),
            keys: Vec::new(),
            open: true,
        }
    }
}

impl Guide {
    /// A guide with a title. `.note(..)` and `.key(..)` fill it in.
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>) -> Guide {
        Guide { title: title.into(), subtitle: subtitle.into(), ..Guide::default() }
    }

    /// A paragraph: the English, then the Japanese under it.
    pub fn note(mut self, en: impl Into<String>, ja: impl Into<String>) -> Guide {
        self.notes.push(GuideNote { en: en.into(), ja: ja.into() });
        self
    }

    /// A row of the key list.
    pub fn key(mut self, keys: impl Into<String>, en: impl Into<String>, ja: impl Into<String>) -> Guide {
        self.keys.push(GuideKey { keys: keys.into(), en: en.into(), ja: ja.into() });
        self
    }

    /// What a game's HUD draws to say the panel is there. Both languages, one line. It is here
    /// rather than in a game's `guide_text.rs` because both games show the same words; it is
    /// still guide text, and `tools/subset-font.sh` reads this file for that reason.
    pub const HINT: &'static str = "H: help / 操作説明";
}

pub struct GuidePlugin;

impl Plugin for GuidePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.init_resource::<Guide>()
            .add_systems(Update, guide_keys)
            // the font has to be in the context before anything draws with it, and `install_font`
            // does nothing after the first frame
            .add_systems(EguiPrimaryContextPass, (install_font, draw_guide).chain());
    }
}

/// `H` and `?` open and close it; `Esc` closes it. Not while the editor has the keyboard — an `h`
/// typed into a creature's brain is a letter, not a key.
fn guide_keys(
    keys: Res<ButtonInput<KeyCode>>,
    typing: Option<Res<bevy_egui::input::EguiWantsInput>>,
    mut guide: ResMut<Guide>,
) {
    if typing.is_some_and(|t| t.wants_keyboard_input()) {
        return;
    }
    // `?` is Shift and the slash key on the layouts this is played on, and winit reports the
    // physical key either way, so both spellings of it are the same `KeyCode`
    if keys.just_pressed(KeyCode::KeyH) || keys.just_pressed(KeyCode::Slash) {
        guide.open = !guide.open;
    }
    if guide.open && keys.just_pressed(KeyCode::Escape) {
        guide.open = false;
    }
}

/// Puts the subset in front of egui as a fallback for both families, once.
///
/// It runs every frame and returns on the second one. The alternative — a `Startup` system — does
/// not work: `EguiContexts` has no context before the primary window's has been made, and
/// `ctx_mut()` there answers `Err`. Doing it in the draw pass and remembering is four lines and
/// has no such ordering to get wrong.
fn install_font(mut contexts: EguiContexts, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(CJK_NAME.to_owned(), std::sync::Arc::new(egui::FontData::from_static(CJK)));
    // *appended*, not put first: egui tries the fonts of a family in order and the defaults keep
    // every character they have, so nothing that was drawn before changes shape
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push(CJK_NAME.to_owned());
    }
    ctx.set_fonts(fonts);
    *done = true;
    info!("the guide's font is in: {} bytes of Noto Sans JP", CJK.len());
}

/// The English in the ordinary colour, the Japanese under it in a quieter one: two languages in
/// one column read as one text, and two columns read as a choice the player has to make.
const JA: egui::Color32 = egui::Color32::from_rgb(150, 178, 210);
const KEYCOL: egui::Color32 = egui::Color32::from_rgb(255, 226, 150);

fn draw_guide(mut contexts: EguiContexts, mut guide: ResMut<Guide>) {
    if !guide.open {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let mut open = true;
    egui::Window::new(guide.title.clone())
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        // tall enough for the paragraphs and the whole key table without scrolling on a
        // 900-pixel window: the first picture of this had the key list cut off half way down,
        // and a key list you have to find the scrollbar for is a key list nobody reads
        .default_size([640.0, 820.0])
        .max_height(860.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        // in front of everything. Both games open the editor and the VM panel at startup, and
        // the first picture taken of this had the guide behind all three of them — an
        // explanation you have to find is not an explanation. `Order::Foreground` is above the
        // `Middle` every other window here uses, and the guide is still draggable and closable.
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            if !guide.subtitle.is_empty() {
                ui.label(egui::RichText::new(&guide.subtitle).color(JA));
                ui.separator();
            }
            egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                for note in &guide.notes {
                    ui.label(egui::RichText::new(&note.en).strong());
                    ui.label(egui::RichText::new(&note.ja).color(JA));
                    ui.add_space(6.0);
                }
                if !guide.keys.is_empty() {
                    ui.separator();
                    egui::Grid::new("guide-keys")
                        .num_columns(3)
                        .spacing([14.0, 3.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for row in &guide.keys {
                                ui.label(
                                    egui::RichText::new(&row.keys).monospace().color(KEYCOL).strong(),
                                );
                                ui.label(&row.en);
                                ui.label(egui::RichText::new(&row.ja).color(JA));
                                ui.end_row();
                            }
                        });
                }
                ui.separator();
                ui.label(
                    egui::RichText::new("H or ? closes this again  ·  H か ? でこの説明を閉じます")
                        .weak(),
                );
            });
        });
    if !open {
        guide.open = false;
    }
}
