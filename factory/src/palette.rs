//! **What can be built, what is in hand, what to do next, and how far away the camera is** (F7).
//!
//! The author played the published game and said four things about it, and they are one thing:
//! *nothing on the screen said what the hand was holding or what could be held.*
//!
//! | what he said | what is here |
//! |---|---|
//! | "I cannot tell what I am allowed to build" | every row of [`draw_palette`], drawn from `data.rb` |
//! | "I want to pick one without the shortcut" | every row is a button |
//! | "I cannot tell which way round it will go" | the line under the rows, and the ghost on the grid |
//! | "the order to build things in is hard to work out" | [`NextStep`], one sentence, the next one only |
//! | "I would like to zoom the map" | the `[−] 3:1 [+]` row, and `+`/`-` in the shared camera |
//!
//! **The list is the digits' list.** [`crate::build::what_the_digits_hold`] is what `1`..`9` and
//! `0` mean, and it is worked out from the declarations, so a `data.rb` with a third machine in
//! it grows a row here and needs no line of this file. What each row shows is **the tile of the
//! sheet the thing is actually drawn with**, cut out of the tileset the map is drawn from
//! ([`Palette::icon`]) — not a second set of pictures to keep in step with the first.
//!
//! **One door for the hand.** A row clicked and a digit pressed both go through
//! [`crate::build::take_in_hand`], which is the only thing that writes [`Hand`]; the log says the
//! same sentence either way. egui is asked before the world hears a click at all
//! (`games_shell::camera::report_clicks`), so a click on this window never lays a belt behind it.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use games_shell::camera::{CameraControls, CameraHome, CameraView};
use games_shell::guide::GuideLang;

use crate::build::{take_in_hand, what_the_digits_hold, Hand};
use crate::data::Data;
use crate::draw::Chunks;
use crate::grid::{Grid, What};
use crate::guide_text as words;
use crate::{Map, TILE_PX};

// ---------------------------------------------------------------------------------------------
// The numbers
// ---------------------------------------------------------------------------------------------

/// **How much bigger than the art a palette icon is drawn**, as a whole number.
///
/// Derived twice over. Pixel art is drawn at a whole multiple or it shimmers, which is the same
/// rule the map is drawn by ([`crate::draw::snapped`]); and the multiple is the smallest one
/// taller than the row's own text, because a picture shorter than the word beside it reads as a
/// bullet point rather than as the thing. One tile is [`TILE_PX`] = 16 px and egui's body text in
/// these games is 13 px with a few pixels of line spacing, so 1× is too small and 2× is not.
pub const ICON_SCALE: f32 = 2.0;

/// How far the palette stands off the corner it starts in.
///
/// The same 8 px every other panel in these three games stands off an edge with (the HUD's
/// `default_pos([8, 8])`, the VM panel's, `EditorLayout::margin`, `GuideStyle::keys_margin`).
pub const MARGIN: f32 = 8.0;

/// **How solid the ghost on the grid is**, from 0 (not there) to 1 (a building).
///
/// **Source unknown**: a half. What it has to do is be read as *about to be* rather than as
/// *there*, and nobody has measured where that line is — so it is a setting (`ghost_alpha`) and
/// this is a starting point, not a finding.
pub const GHOST_ALPHA: f32 = 0.5;

/// **The colour a ghost is tinted where it would be refused** (`crate::build::would_refuse`).
///
/// Not in the store, for the reason no colour in these three games is: a colour in a `key=value`
/// file would need a parser. It is a field, so a game — or a later stage — may still set it.
/// Red at full, green and blue at the fraction that leaves the picture readable underneath.
pub const CANNOT: (u8, u8, u8) = (255, 90, 90);

/// The tint where it *would* be built: the picture's own colours, untouched, at
/// [`GHOST_ALPHA`].
pub const CAN: (u8, u8, u8) = (255, 255, 255);

/// **Every number F7 added to this game**, in one resource the store can write into
/// (`palette_icon_scale`, `palette_margin`, `ghost_alpha`) and a game can hand over whole.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct PaletteStyle {
    /// [`ICON_SCALE`]
    pub icon_scale: f32,
    /// [`MARGIN`]
    pub margin: f32,
    /// [`GHOST_ALPHA`]
    pub ghost_alpha: f32,
    /// [`CAN`]
    pub can: (u8, u8, u8),
    /// [`CANNOT`]
    pub cannot: (u8, u8, u8),
}

impl Default for PaletteStyle {
    fn default() -> Self {
        PaletteStyle {
            icon_scale: ICON_SCALE,
            margin: MARGIN,
            ghost_alpha: GHOST_ALPHA,
            can: CAN,
            cannot: CANNOT,
        }
    }
}

impl PaletteStyle {
    /// What a player left in `factory.settings.txt`. A number that is not more than zero is
    /// refused rather than rounded (S11's `positive`): an icon of no pixels and a ghost of no
    /// solidity are both "the thing is not drawn", which is not what anybody meant by a number.
    pub fn read_from(&mut self, settings: &games_shell::Settings) {
        self.icon_scale = settings.positive("palette_icon_scale", self.icon_scale);
        self.margin = settings.positive("palette_margin", self.margin);
        self.ghost_alpha = settings.positive("ghost_alpha", self.ghost_alpha);
    }

    /// The tint a ghost is drawn with, as a colour with the alpha in it.
    pub fn ghost_colour(&self, refused: bool) -> Color {
        let (r, g, b) = if refused { self.cannot } else { self.can };
        Color::srgba(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            self.ghost_alpha.clamp(0.0, 1.0),
        )
    }
}

// ---------------------------------------------------------------------------------------------
// The pictures, cut out of the tileset the map is drawn from
// ---------------------------------------------------------------------------------------------

/// **The tiles of the sheet, as egui textures**, cut one at a time and kept.
///
/// The map's tileset is an **array texture** — one layer per tile, which is what a
/// `TilemapChunk` wants (`crate::draw::start_drawing`) — and egui cannot draw one of those: its
/// own pipeline binds a plain 2D texture. So a row's picture is made from the image's own bytes
/// on the way past: layer *n* of the array is 16 rows of the strip, and those bytes are an
/// `egui::ColorImage` with nothing in between.
///
/// **It is a handful of 16 px squares and they are made once each**, the first frame a row asks
/// for one. Keeping the handle is what keeps the texture alive; dropping one frees it.
#[derive(Resource, Default)]
pub struct Palette {
    icons: std::collections::HashMap<u16, egui::TextureHandle>,
    /// said once, if the sheet turns out not to be something that can be cut up
    complained: bool,
}

impl Palette {
    /// Tile `index` of the sheet as something egui can draw, making it if this is the first ask.
    pub fn icon(
        &mut self,
        ctx: &egui::Context,
        sheet: Option<&Image>,
        index: u16,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.icons.get(&index) {
            return Some(handle.id());
        }
        let sheet = sheet?;
        let side = TILE_PX as usize;
        let bytes = sheet.data.as_ref()?;
        // the loader was asked for `Rgba8` and a PNG arrives as one; anything else is a sheet
        // this cannot cut, and saying so once is better than a panic or a blank row for ever
        let four_bytes = matches!(
            sheet.texture_descriptor.format,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
                | bevy::render::render_resource::TextureFormat::Rgba8Unorm
        );
        if !four_bytes || sheet.texture_descriptor.size.width as usize != side {
            if !self.complained {
                self.complained = true;
                warn!(
                    "the palette cannot cut icons out of a {:?} sheet {} px across",
                    sheet.texture_descriptor.format, sheet.texture_descriptor.size.width
                );
            }
            return None;
        }
        let layer = side * side * 4;
        let from = index as usize * layer;
        let slice = bytes.get(from..from + layer)?;
        let picture = egui::ColorImage::from_rgba_unmultiplied([side, side], slice);
        // **nearest, like everything else here**: this is the same art the map is drawn with and
        // a smoothed 16 px tile is a smear (`ImagePlugin::default_nearest` in `main`)
        let handle =
            ctx.load_texture(format!("factory-tile-{index}"), picture, egui::TextureOptions::NEAREST);
        let id = handle.id();
        self.icons.insert(index, handle);
        Some(id)
    }

    /// **Thrown away when the world is laid out again** (`data.rb` applied): the sheet is the
    /// same file, but a machine's `sprite:` may now be another tile and a cached picture would be
    /// the old one.
    pub fn forget(&mut self) {
        self.icons.clear();
    }
}

// ---------------------------------------------------------------------------------------------
// The one thing to do next
// ---------------------------------------------------------------------------------------------

/// **The next step, and only the next one** (F7, the author's fourth point).
///
/// A list of five things to do in order is a list nobody reads; one sentence that changes as the
/// factory grows is an instruction. The order is the one the guide's paragraph has always
/// described — a miner on ore, a belt away from it, a chest at the end, then a machine and an arm
/// — and what decides where in it the player is is **the factory itself**, not a tutorial's idea
/// of what has been done.
///
/// Once all five are standing there is nothing to tell anybody, so the line becomes the control
/// stage's own sentence (`@saying`, "gear 12 / 60"): what is left is the goal.
#[derive(Resource, Debug, Default)]
pub struct NextStep {
    /// What to say, in the language the guide is in. Empty while everything is built.
    pub say: Option<(String, String)>,
    /// Which [`Grid::changes`] this was worked out from.
    seen: u64,
}

impl NextStep {
    pub fn of(&self, lang: GuideLang) -> Option<&str> {
        self.say.as_ref().map(|(en, ja)| match lang {
            GuideLang::En => en.as_str(),
            GuideLang::Ja => ja.as_str(),
        })
    }
}

/// **Worked out from the grid, and only when the grid has changed.**
///
/// `Grid::changes` is the counter everything that follows the grid keeps its own place in — the
/// picture and `Flow` do the same (`crate::draw::draw_buildings`) — so counting five kinds of
/// building over every tile happens when something is built and not sixty times a second.
pub fn work_out_the_next_step(grid: Res<Grid>, data: Res<Data>, mut step: ResMut<NextStep>) {
    if step.seen == grid.changes && step.seen != 0 {
        return;
    }
    step.seen = grid.changes;
    let mut miner = false;
    let mut belt = false;
    let mut chest = false;
    let mut machine = false;
    let mut inserter = false;
    for &at in grid.built() {
        match grid.at(at as usize).map(|b| b.what) {
            Some(What::Miner) => miner = true,
            Some(What::Belt) => belt = true,
            Some(What::Chest) => chest = true,
            Some(What::Inserter) => inserter = true,
            Some(What::Machine(_)) => machine = true,
            _ => {}
        }
    }
    let digits = what_the_digits_hold(&data);
    // which key holds a thing, as the palette shows it — so that a `data.rb` with the machines in
    // another order tells the player the key that is really theirs
    let key_of = |wanted: fn(What) -> bool| -> String {
        digits
            .iter()
            .find(|(_, what)| what.is_some_and(wanted))
            .map(|(key, _)| crate::window::key_word(*key))
            .unwrap_or_else(|| "?".into())
    };
    let name_of = |wanted: fn(What) -> bool| -> (String, String) {
        let what = digits.iter().find(|(_, what)| what.is_some_and(wanted)).and_then(|(_, w)| *w);
        (
            words::word_in(GuideLang::En, what, &data),
            words::word_in(GuideLang::Ja, what, &data),
        )
    };
    let is_miner = |w: What| w == What::Miner;
    let is_belt = |w: What| w == What::Belt;
    let is_chest = |w: What| w == What::Chest;
    let is_arm = |w: What| w == What::Inserter;
    let is_machine = |w: What| matches!(w, What::Machine(_));
    let step_words = if !miner {
        Some((words::STEP_MINER, key_of(is_miner), name_of(is_miner)))
    } else if !belt {
        Some((words::STEP_BELT, key_of(is_belt), name_of(is_belt)))
    } else if !chest {
        Some((words::STEP_CHEST, key_of(is_chest), name_of(is_chest)))
    } else if !machine {
        Some((words::STEP_MACHINE, key_of(is_machine), name_of(is_machine)))
    } else if !inserter {
        Some((words::STEP_INSERTER, key_of(is_arm), name_of(is_arm)))
    } else {
        None
    };
    step.say = step_words.map(|(say, key, (en, ja))| {
        (
            say.en.replace("{key}", &key).replace("{name}", &en),
            say.ja.replace("{key}", &key).replace("{name}", &ja),
        )
    });
}

// ---------------------------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------------------------

/// **How much of the art one pixel of the screen is worth**, as the ratio a player reads: `3:1`
/// is three screen pixels to one of the art, `1:2` is half a pixel of art to a screen pixel.
///
/// It is the same measure [`crate::draw::snapped`] rounds to, which is why the zoom always shows
/// a whole number on one side of the colon: the two are one idea, and a readout in world units
/// would say nothing a player could check against what is on the screen.
pub fn how_near(window_height: f32, half_height: f32) -> String {
    let scale = window_height / (2.0 * half_height.max(f32::MIN_POSITIVE));
    if scale >= 1.0 {
        format!("{:.0}:1", scale)
    } else {
        format!("1:{:.0}", 1.0 / scale)
    }
}

/// **The whole map, as a half-view**: half of its longer side, which is what
/// `crate::point_the_camera_at_the_map` raises the zoom limit to reach.
///
/// **With the zoom rounded to whole pixels it is the rung that holds all of it**, not the exact
/// half-view: `crate::draw::snapped` rounds to the *nearest* whole zoom, and the nearest to "the
/// whole map" is as often the one that shows nine tenths of it. A button called "whole map" that
/// leaves a row of tiles off the bottom is a button that lies, so the rung is stepped out until
/// the map is inside it — which is why this takes the window's height and cannot be worked out
/// from the map alone.
pub fn the_whole_map(map: &Map, window_height: f32, snap: bool) -> f32 {
    let whole = (map.span().max_element() / 2.0).max(f32::MIN_POSITIVE);
    if !snap {
        return whole;
    }
    let mut rung = crate::draw::rung(window_height, whole);
    if crate::draw::half_height_of(window_height, rung) + 1e-3 < whole {
        rung = crate::draw::step_the_zoom(rung, -1);
    }
    crate::draw::half_height_of(window_height, rung)
}

/// The palette, the way round, the next step and the zoom — one small window, always up.
#[allow(clippy::too_many_arguments)]
pub fn draw_palette(
    mut contexts: EguiContexts,
    data: Res<Data>,
    guide: Res<games_shell::Guide>,
    style: Res<PaletteStyle>,
    mut palette: ResMut<Palette>,
    mut hand: ResMut<Hand>,
    step: Res<NextStep>,
    images: Res<Assets<Image>>,
    chunks: Option<Res<Chunks>>,
    map: Res<Map>,
    home: Res<CameraHome>,
    controls: Res<CameraControls>,
    mut view: ResMut<CameraView>,
    control: Res<crate::control::TheControl>,
    snap: Res<crate::draw::SnapZoom>,
    windows: Query<&Window>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let ctx = ctx.clone();
    let lang = guide.lang;
    // the sheet the map is drawn with, if it has arrived; a run whose tileset is still loading
    // draws the rows without their pictures for a frame or two
    let sheet: Option<&Image> = chunks.as_ref().and_then(|c| images.get(&c.tileset));
    let icon_px = (TILE_PX as f32 * style.icon_scale).round();
    let corner = egui::pos2(
        ctx.content_rect().min.x + style.margin,
        ctx.content_rect().max.y - style.margin,
    );
    egui::Window::new(words::PALETTE_TITLE.of(lang))
        .id(egui::Id::new("factory-palette"))
        .collapsible(true)
        .resizable(false)
        // **the bottom left**, which is where the VM panel sits when it is open and nothing sits
        // when it is not: the HUD has the top left, the editor the right, and the guide's keys
        // the bottom right. Both are dragged from there by whoever wants them elsewhere.
        .pivot(egui::Align2::LEFT_BOTTOM)
        .default_pos(corner)
        .show(&ctx, |ui| {
            ui.style_mut().interaction.selectable_labels = false;
            for (key, what) in what_the_digits_hold(&data) {
                let name = words::word_in(lang, what, &data);
                let label = format!("{}   {}", crate::window::key_word(key), name);
                let holding = hand.what == what;
                let picture = crate::draw::picture_of(what, &data)
                    .and_then(|tile| palette.icon(&ctx, sheet, tile));
                let clicked = match picture {
                    Some(id) => ui
                        .add(
                            egui::Button::image_and_text(
                                egui::Image::new(egui::load::SizedTexture::new(
                                    id,
                                    egui::vec2(icon_px, icon_px),
                                )),
                                label,
                            )
                            .selected(holding),
                        )
                        .clicked(),
                    // the wrecking ball has no picture of its own — it is the absence of one
                    None => ui
                        .add_sized(
                            [ui.available_width(), icon_px],
                            egui::Button::new(label).selected(holding),
                        )
                        .clicked(),
                };
                if clicked {
                    take_in_hand(&mut hand, what, &data);
                }
            }
            // ---- which way round ----------------------------------------------------------
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} {}",
                    words::FACING.of(lang),
                    words::way_round(lang, hand.dir)
                ));
                if ui.button("R").clicked() {
                    crate::build::turn_the_hand(&mut hand, &data);
                }
                ui.label(egui::RichText::new(words::TURN_IT.of(lang)).weak());
            });
            // ---- how near the camera is ---------------------------------------------------
            let height = windows.iter().next().map(|w| w.height()).unwrap_or(1.0).max(1.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(words::ZOOM.of(lang));
                // **every button here is notches through `zoom_by`** — the wheel's door and the
                // keys', limits and all (`games_shell::camera`)
                if ui.button("−").clicked() {
                    games_shell::camera::zoom_the_view(
                        &mut view,
                        -controls.notches_per_key,
                        &home,
                        &controls,
                    );
                }
                ui.label(
                    egui::RichText::new(how_near(height, view.half_height)).monospace().strong(),
                );
                if ui.button("+").clicked() {
                    games_shell::camera::zoom_the_view(
                        &mut view,
                        controls.notches_per_key,
                        &home,
                        &controls,
                    );
                }
                if ui.button(words::WHOLE_MAP.of(lang)).clicked() {
                    let notches = games_shell::camera::notches_to(
                        view.half_height,
                        the_whole_map(&map, height, snap.0),
                        &controls,
                    );
                    games_shell::camera::zoom_the_view(&mut view, notches, &home, &controls);
                }
                if ui.button(words::BACK_HOME.of(lang)).clicked() {
                    view.focus = home.focus;
                    view.half_height = home.half_height;
                }
            });
            // ---- the one thing to do next -------------------------------------------------
            ui.separator();
            match step.of(lang) {
                Some(say) => {
                    ui.label(
                        egui::RichText::new(format!("{}: {say}", words::NEXT_STEP.of(lang)))
                            .strong(),
                    );
                }
                // **everything the first line needs is standing**, so what is left to say is what
                // is left to do, which is the control stage's own sentence (`@saying`: "gear 12 /
                // 60"). It is the goal in the words `control.rb` chose, not a second wording of
                // it kept here.
                None => match &control.saying {
                    Some(saying) => {
                        ui.label(egui::RichText::new(saying).strong());
                    }
                    None => {
                        ui.label(egui::RichText::new(words::PALETTE_HINT.of(lang)).weak());
                    }
                },
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The readout is the same measure the zoom is rounded to, on both sides of 1:1.
    #[test]
    fn the_zoom_is_read_as_pixels_to_pixels() {
        assert_eq!(how_near(900.0, 150.0), "3:1");
        assert_eq!(how_near(900.0, 450.0), "1:1");
        assert_eq!(how_near(900.0, 900.0), "1:2");
        assert_eq!(how_near(720.0, 180.0), "2:1");
    }
}
