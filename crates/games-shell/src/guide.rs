//! **The guide (G6): one panel, two languages, in the game.**
//!
//! Both games here are sat down in front of by somebody who has never read a word about them —
//! the author's own browser play is what found this — and until now the only thing that said what
//! the world was and which keys did anything was a line of small text in the HUD and a `docs/`
//! file in another window. So: a panel, open the first time the game starts, `H` or `?` after
//! that, and a hint in the HUD saying so.
//!
//! **The frame is here and the words are the game's.** What a garden is and what a battle is have
//! nothing in common, but "a window with a paragraph and a key table, that `H` opens" is the same
//! thing twice, and the font is the same problem twice. So this module owns [`Guide`] — a title,
//! some paragraphs, some key rows, each with both languages — and each game fills one in.
//!
//! ## One language at a time (G6b)
//!
//! G6 drew both languages: the English paragraph and the Japanese under it, all the way down. The
//! author played it and said what anybody would — **half of what is on the screen is not for you,
//! whoever you are.** A reader of either language reads a page twice as long as the one they
//! needed, and the key table had three columns where two would do. So the panel now shows one
//! language and a pair of buttons at the top, `English | 日本語`, switches it.
//!
//! **Nothing in a game's `guide_text.rs` changed for this.** A [`GuideNote`] still carries both
//! languages and so does every [`GuideKey`]; [`GuideLang`] only decides which of the two is
//! drawn. That is what makes the author's "I will fix the Japanese myself" still a one-file job.
//!
//! Which language it starts in is [`GuideLang::pick`]: what the command line asked for, else what
//! the player chose last time ([`crate::settings::Settings`]), else the machine's own — `LANG` on
//! a PC, `navigator.language` in a browser. The HUD's one-line hint stays in both languages,
//! because it is the line that has to be understood *before* anybody has chosen anything.
//!
//! ## The font
//!
//! egui's default fonts are Ubuntu-Light and two Noto symbol faces: **no CJK at all**. Japanese in
//! a label comes out as the replacement box, which is what "tofu" means. There is no system font
//! to fall back on either — the browser build is a wasm module with no access to the machine's
//! fonts, and `fontdb`-style discovery would be a megabyte of code and a different answer on every
//! machine. So the font is *in the binary*: [`CJK`], a subset of Noto Sans JP cut down to the
//! characters this file and the two games' guides actually use (`crates/games-shell/assets/fonts/`,
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

/// Which language the panel is showing. The text is always there in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuideLang {
    #[default]
    En,
    Ja,
}

impl GuideLang {
    /// What is written to the settings store, and what `--lang` takes.
    pub fn tag(self) -> &'static str {
        match self {
            GuideLang::En => "en",
            GuideLang::Ja => "ja",
        }
    }

    /// `ja`, `ja_JP.UTF-8`, `ja-JP` — anything that starts with `ja` is Japanese, and anything
    /// else that names a language at all is English, because English is the only other one the
    /// panel has. `None` for a string that says nothing, so the next answer down is tried.
    pub fn of_tag(tag: &str) -> Option<GuideLang> {
        let tag = tag.trim();
        if tag.is_empty() {
            None
        } else if tag.to_ascii_lowercase().starts_with("ja") {
            Some(GuideLang::Ja)
        } else {
            Some(GuideLang::En)
        }
    }

    /// What the machine is set to: `LC_ALL`/`LANG`, or the browser's `navigator.language`.
    pub fn of_environment() -> GuideLang {
        crate::settings::environment_language()
            .and_then(|tag| GuideLang::of_tag(&tag))
            .unwrap_or(GuideLang::En)
    }

    /// The order the three answers come in: **what was asked for** on the command line, then
    /// **what was chosen** last time, then **where the machine is**. A player who has never
    /// touched the buttons gets their own language; one who has gets what they picked, in any
    /// locale; and `--lang ja` overrides both without remembering itself, which is what a
    /// screenshot wants.
    pub fn pick(asked: Option<&str>, chosen: Option<&str>) -> GuideLang {
        asked
            .and_then(GuideLang::of_tag)
            .or_else(|| chosen.and_then(GuideLang::of_tag))
            .unwrap_or_else(GuideLang::of_environment)
    }
}

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
    /// The window's title, in each language. G6 drew the English one as the title and the
    /// Japanese one as a line under it; now one of the two is the title and the other is waiting
    /// behind the button.
    pub title_en: String,
    pub title_ja: String,
    pub notes: Vec<GuideNote>,
    pub keys: Vec<GuideKey>,
    pub open: bool,
    /// Which language is being drawn. A game sets it from [`GuideLang::pick`] at startup; the
    /// buttons at the top of the panel change it after that.
    pub lang: GuideLang,
}

impl Default for Guide {
    fn default() -> Self {
        Guide {
            title_en: "Help".into(),
            title_ja: "説明".into(),
            notes: Vec::new(),
            keys: Vec::new(),
            open: true,
            lang: GuideLang::En,
        }
    }
}

impl Guide {
    /// A guide with a title in both languages. `.note(..)` and `.key(..)` fill it in.
    pub fn new(title_en: impl Into<String>, title_ja: impl Into<String>) -> Guide {
        Guide { title_en: title_en.into(), title_ja: title_ja.into(), ..Guide::default() }
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

    /// **The guide as the game starts it**: the words are the game's, the language is
    /// [`GuideLang::pick`]'s, and `open` is whether the panel is up on the first frame.
    ///
    /// A picture is asked for one thing and the panel sits over the middle of the window, so a
    /// `--shot` run starts with it shut unless `--guide` says otherwise. A player gets it open.
    pub fn opening(self, lang: GuideLang, open: bool) -> Guide {
        Guide { lang, open, ..self }
    }

    /// What a game's HUD draws to say the panel is there. Both languages, one line. It is here
    /// rather than in a game's `guide_text.rs` because both games show the same words; it is
    /// still guide text, and `tools/subset-font.sh` reads this file for that reason.
    pub const HINT: &'static str = "H: help / 操作説明";
}

// ---------------------------------------------------------------------------------------------
// The numbers (S5b-1). The `const`s name the defaults; the settings are `GuideStyle`.
// ---------------------------------------------------------------------------------------------

/// How big the panel stands before anybody drags it, and how tall it may grow.
///
/// **Cited, and not measured**: the first picture of G6 had the key table cut off half way down
/// on a 900-pixel window, so it was widened to this (`docs/plans/garden-plan.md`, "2 枚目では
/// キー表が下で切れていたので `default_size([640, 820])` に"). Nothing says why 860 is the
/// ceiling.
pub const SIZE: [f32; 2] = [640.0, 820.0];
pub const MAX_HEIGHT: f32 = 860.0;

/// The gap under each paragraph of the guide, so the notes read as notes rather than as one
/// block. **Source unknown** (S5b-2: it was written into `draw_guide`).
pub const NOTE_SPACING: f32 = 6.0;

/// The key table's column gap and row gap. **Source unknown** — like the editor's button padding,
/// the pair arrived with the panel and nothing says why 14 and 3.
pub const KEY_SPACING: [f32; 2] = [14.0, 3.0];

/// **Every number the guide has**, in one resource a game can hand [`GuidePlugin::styled`] or a
/// player can change through the `key=value` store ([`GuideStyle::read_from`]). The key colour is
/// here too, since it is the one colour this panel chooses for itself.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct GuideStyle {
    /// [`SIZE`]
    pub size: [f32; 2],
    /// [`MAX_HEIGHT`]
    pub max_height: f32,
    /// [`KEYCOL`]
    pub key_color: (u8, u8, u8),
    /// [`NOTE_SPACING`]
    pub note_spacing: f32,
    /// [`KEY_SPACING`]
    pub key_spacing: [f32; 2],
}

impl Default for GuideStyle {
    fn default() -> Self {
        GuideStyle {
            size: SIZE,
            max_height: MAX_HEIGHT,
            key_color: KEYCOL,
            note_spacing: NOTE_SPACING,
            key_spacing: KEY_SPACING,
        }
    }
}

impl GuideStyle {
    /// **What a player left in the `key=value` store** ([`crate::Settings`]).
    ///
    /// | key | field |
    /// |---|---|
    /// | `guide_width` | [`GuideStyle::size`]`[0]` |
    /// | `guide_height` | [`GuideStyle::size`]`[1]` |
    /// | `guide_max_height` | [`GuideStyle::max_height`] |
    /// | `guide_note_spacing` | [`GuideStyle::note_spacing`] |
    ///
    /// The colour is not among them, for the reason the camera's keys are not: a colour in a text
    /// file needs a parser, and a game may still set it. [`GuideStyle::key_spacing`] is left out
    /// for the same reason — it is a pair (S5b-2).
    pub fn read_from(&mut self, settings: &crate::Settings) {
        let take = |key: &str, slot: &mut f32| {
            if let Some(value) = settings.number(key) {
                *slot = value;
            }
        };
        take("guide_width", &mut self.size[0]);
        take("guide_height", &mut self.size[1]);
        take("guide_max_height", &mut self.max_height);
        take("guide_note_spacing", &mut self.note_spacing);
    }
}

#[derive(Default)]
pub struct GuidePlugin {
    pub style: GuideStyle,
}

impl GuidePlugin {
    /// The guide at a size of the game's choosing.
    pub fn styled(style: GuideStyle) -> GuidePlugin {
        GuidePlugin { style }
    }
}

impl Plugin for GuidePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.init_resource::<Guide>()
            .insert_resource(self.style.clone())
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

/// The colour of a key's name in the table, and of the language that is not being shown.
/// **Source unknown** (`docs/numbers.md` §1.4).
pub const KEYCOL: (u8, u8, u8) = (255, 226, 150);

/// The two buttons, in their own languages: nobody has to know the word "Japanese" in English or
/// the word "English" in Japanese to find the one they want. They are the first thing in the
/// panel because a player who cannot read the paragraph has to be able to see the way out of it
/// without reading anything.
const LANG_BUTTONS: [(GuideLang, &str); 2] = [(GuideLang::En, "English"), (GuideLang::Ja, "日本語")];

/// The line at the foot of the panel, which is the one thing it says about itself.
const FOOT_EN: &str = "H or ? closes this again";
const FOOT_JA: &str = "H か ? でこの説明を閉じます";

fn draw_guide(
    mut contexts: EguiContexts,
    mut guide: ResMut<Guide>,
    style: Res<GuideStyle>,
    mut settings: Option<ResMut<crate::settings::Settings>>,
) {
    if !guide.open {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let mut open = true;
    let lang = guide.lang;
    let (r, g, b) = style.key_color;
    let keycol = egui::Color32::from_rgb(r, g, b);
    let title = match lang {
        GuideLang::En => guide.title_en.clone(),
        GuideLang::Ja => guide.title_ja.clone(),
    };
    egui::Window::new(title)
        // the title is half of the text that switches, so the window's *name* changes with the
        // language — and a window egui knows by its name would be a different window after the
        // click, dropped back at the middle of the screen at its default size. The id is the
        // thing that is the same window in both languages.
        //
        // **The old crate name is in the id** (S4a, 2026-09-20: `rubevy-arena` became
        // `rubevy-egui` and `games-shell`). egui writes a window's position and size into its own
        // memory under this id, and a player's memory has the old string in it; renaming it here
        // would drop everybody's guide back at its default size and place for nothing.
        .id(egui::Id::new("rubevy-arena-guide"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        // tall enough for the paragraphs and the whole key table without scrolling on a
        // 900-pixel window: the first picture of this had the key list cut off half way down,
        // and a key list you have to find the scrollbar for is a key list nobody reads
        .default_size(style.size)
        .max_height(style.max_height)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        // in front of everything. Both games open the editor and the VM panel at startup, and
        // the first picture taken of this had the guide behind all three of them — an
        // explanation you have to find is not an explanation. `Order::Foreground` is above the
        // `Middle` every other window here uses, and the guide is still draggable and closable.
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let mut chose = None;
            ui.horizontal(|ui| {
                for (which, label) in LANG_BUTTONS {
                    if ui.selectable_label(lang == which, label).clicked() {
                        chose = Some(which);
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                for note in &guide.notes {
                    ui.label(match lang {
                        GuideLang::En => egui::RichText::new(&note.en),
                        GuideLang::Ja => egui::RichText::new(&note.ja),
                    });
                    ui.add_space(style.note_spacing);
                }
                if !guide.keys.is_empty() {
                    ui.separator();
                    // two columns now, not three: the key and what it does in the one language
                    egui::Grid::new("guide-keys")
                        .num_columns(2)
                        .spacing(style.key_spacing)
                        .striped(true)
                        .show(ui, |ui| {
                            for row in &guide.keys {
                                ui.label(
                                    egui::RichText::new(&row.keys).monospace().color(keycol).strong(),
                                );
                                ui.label(match lang {
                                    GuideLang::En => &row.en,
                                    GuideLang::Ja => &row.ja,
                                });
                                ui.end_row();
                            }
                        });
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(match lang {
                        GuideLang::En => FOOT_EN,
                        GuideLang::Ja => FOOT_JA,
                    })
                    .weak(),
                );
            });
            if let Some(which) = chose {
                guide.lang = which;
                // remembered where the save file is kept: the next run of this game, in this
                // browser or in this directory, opens in the language that was clicked
                if let Some(settings) = settings.as_mut() {
                    settings.set("lang", which.tag());
                }
            }
        });
    if !open {
        guide.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A number in the store changes the panel and leaves the rest alone** (S5b-1). The two
    /// halves of the size are separate keys because a panel that is too wide and a panel that is
    /// too tall are two different complaints.
    #[test]
    fn the_store_changes_the_size_and_leaves_the_rest() {
        let path = std::env::temp_dir().join("games-shell-guide-settings-test.txt");
        let _ = std::fs::remove_file(&path);
        fn read(path: &std::path::Path) -> Result<String, String> {
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
        fn write(path: &std::path::Path, text: &str) -> Result<(), String> {
            std::fs::write(path, text).map_err(|e| e.to_string())
        }
        let mut settings = crate::Settings::load(&path, "a test", read, write);
        settings.set("guide_height", "420");

        let mut style = GuideStyle::default();
        style.read_from(&settings);
        assert_eq!(style.size[1], 420.0);
        assert_eq!(style.size[0], SIZE[0], "a key nobody wrote leaves the default alone");
        assert_eq!(style.max_height, MAX_HEIGHT);
        assert_eq!(style.key_color, KEYCOL, "and the colour is not in the store at all");

        // S5b-2: the three gaps the panel was drawn with. The single one has a key; the pair does
        // not, for the reason the colour does not — a pair of numbers in a text file needs a
        // parser — and a game may still write it.
        settings.set("guide_note_spacing", "12");
        let mut spaced = GuideStyle::default();
        spaced.read_from(&settings);
        assert_eq!(spaced.note_spacing, 12.0);
        assert_eq!(spaced.key_spacing, KEY_SPACING, "the pair is not in the store");
        assert_eq!(GuideStyle { key_spacing: [1.0, 2.0], ..GuideStyle::default() }.key_spacing, [1.0, 2.0]);

        let _ = std::fs::remove_file(&path);
    }
}
