//! Garden — a small world, in 3D, whose rules are Rust and whose minds are Ruby.
//!
//! This is stages G0, G0a and G1 of `docs/plans/garden-plan.md`. Grass grows, creatures walk
//! about, get hungry, eat, starve, and the sun goes round once a minute — and **what walks is a
//! Ruby task per creature** (`ruby/creatures/*.rb`), reading and writing its own ECS components
//! by name:
//!
//!     me[:Hunger]                       # → [42.0]
//!     me[:Velocity] = [[vx, vz]]
//!     plant[:Transform][:translation]   # → [x, y, z]
//!
//! There is no per-component glue anywhere in this file, and there is not meant to be: rubevy
//! walks Bevy's type registry (`docs/host-api.md`, "Components by name"), and so does the one
//! answering system here — `answer_garden` resolves `:Plant` through `ReflectComponent` rather
//! than matching on a name. The table in `docs/garden.md` is that registry, written out.
//!
//! The two questions a script asks that are *not* a component read are `garden.nearest(kind)`
//! and `garden.count(kind)`. That is the whole of the game's `ask` surface; SabiRuby Battle's
//! is six kinds, and the difference is the point of the two samples standing side by side.
//!
//!     cargo run -p garden                     # a window
//!     cargo run -p garden -- --headless 90    # no window, 90 seconds, the result on stdout
//!     GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90
//!     cargo run -p garden -- --headless 30 --save garden.save.json   # and write it down
//!     cargo run -p garden -- --load garden.save.json                 # and pick it up again
//!     cargo run -p garden -- --shot n.png 12 --at midnight           # a picture of the night
//!     cargo run -p garden -- --shot c.png 12 --eye 18                # …from 18 units back
//!     cargo run -p garden -- --shot g.png 10 --guide --lang ja        # …of the guide, in Japanese
//!
//! Mouse: drag to orbit, wheel to zoom. F5 saves the garden, F9 brings it back.

mod genome;
/// G6: the words of the `H` panel, and the only file to edit to change them.
mod guide_text;
mod platform;
mod window;

use std::path::{Path, PathBuf};

use bevy::asset::AssetId;
use bevy::gltf::GltfAssetLabel;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
// G8: the event the glTF loader triggers when a model's entities actually exist, which is the
// only moment a loaded material can be replaced (`tint_species`).
use bevy::world_serialization::WorldInstanceReady;
use rubevy::{
    in_the_authors_lines, replace_script, Answer, Arg, MrbAsset, Program, RubevyPlugin, RubevySet,
    Script, ScriptTask, ScriptWorld,
};
use games_shell::GuidePlugin;
use rubevy_egui::{EditorPlugin, VmInspector, VmInspectorPlugin, Watch};
use sabiruby::value::ObjId;
use sabiruby::{IntoRuby, Value, Vm};
use serde::{Deserialize, Serialize};

use crate::genome::{Birth, CreatureSpec, Genome};

// ---------------------------------------------------------------------------------------------
// The field. It lies on XZ with y up, which is the only thing 3D costs the Ruby side: a position
// is `[x, y, z]` and `act` takes `(vx, vz)`.
// ---------------------------------------------------------------------------------------------

/// 40 × 30, as the plan says, measured in world units (one unit is about a rabbit).
///
/// **Quoted**: `docs/plans/garden-plan.md`, "40×30 マスの草地". Why *those* two numbers rather
/// than any other pair is not recorded anywhere, so the plan is the source and the size is the
/// default ([`Place`]), not a fact about the game.
const FIELD_W: f32 = 40.0;
const FIELD_D: f32 = 30.0;
/// How far inside the wall a creature is stopped, and where [`separate`] puts one that ended up
/// outside. **Source unknown** — the same half unit has been written in `move_creatures` and
/// `inside_the_walls` since G0.
const WALL_MARGIN: f32 = 0.5;

/// **How big the garden is, and what its walls do** (S5b-3).
///
/// The field used to be two `const`s and two more derived from them, which meant the one number
/// the whole world is measured in could only be changed by rebuilding. It is a setting now:
/// `field_width=60` in `garden.settings.txt` is a wider garden, with wider walls, a bigger lawn,
/// the fixtures further apart and the camera allowed further out — because every one of those
/// reads this resource rather than a constant of its own.
///
/// **Nothing about the *rules* is here.** How fast grass grows and how far a creature sees are
/// `ruby/world.rb`'s; this is the shape of the room they happen in.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// [`FIELD_W`]
    pub width: f32,
    /// [`FIELD_D`]
    pub depth: f32,
    /// [`WALL_MARGIN`]
    pub wall_margin: f32,
    /// [`SEPARATE_PASSES`]
    pub separate_passes: usize,
}

impl Default for Place {
    fn default() -> Self {
        Place {
            width: FIELD_W,
            depth: FIELD_D,
            wall_margin: WALL_MARGIN,
            separate_passes: SEPARATE_PASSES,
        }
    }
}

impl Place {
    /// Half the field, which is what almost everything actually wants: the walls stand at ±this.
    pub fn half_w(&self) -> f32 {
        self.width / 2.0
    }
    pub fn half_d(&self) -> f32 {
        self.depth / 2.0
    }

    /// `field_width` / `field_depth` / `wall_margin` / `separate_passes` in
    /// `garden.settings.txt`. A key nobody wrote leaves its field alone, which is what makes a
    /// store written by an older build safe to read.
    fn read_from(&mut self, settings: &games_shell::Settings) {
        take(settings, "field_width", &mut self.width);
        take(settings, "field_depth", &mut self.depth);
        take(settings, "wall_margin", &mut self.wall_margin);
        if let Some(value) = settings.number("separate_passes") {
            self.separate_passes = value.max(0.0) as usize;
        }
    }
}

/// One `key=value` into one `f32`, for the seven resources below. A key that is not in the store
/// leaves the field as it was; a key that is not a number is not one either.
fn take(settings: &games_shell::Settings, key: &str, slot: &mut f32) {
    if let Some(value) = settings.number(key) {
        *slot = value;
    }
}

/// One turn of the sun. Night is the half of it the sun spends under the ground.
///
/// **It is the default now, not the rule** (W1): the rule is `day_length` in `ruby/world.rb`, and
/// what it says arrives in [`Sky::day_length`] through `garden.rules`. This is what the sun turns
/// at before that file has said anything, and what it goes on turning at where that file will not
/// compile. `MIDNIGHT` and `--at midnight` are worked out from it because they are about the
/// picture a `--shot` takes, which is taken in the first seconds of a run.
const DAY_LENGTH: f32 = 60.0;
/// Where in that turn the world starts: a little after sunrise, so the first thing a run sees is
/// daylight and the first `"night"` is something that arrives rather than something that was.
/// *Reason only* — the sentence above is the record and 0.08 itself is **unknown**.
const DAWN_OFFSET: f32 = 0.08;

/// Midnight, on the world's clock, in the first turn of the sun: the phase where the sun is
/// furthest under the ground is 0.75, and `phase = (now / DAY_LENGTH + dawn_offset).fract()`
/// makes that `now = (0.75 - dawn_offset) * DAY_LENGTH`. `--at midnight` is the darkest picture
/// the garden has.
///
/// **Derived, so it is a function and not a setting** (S5b-3): it moves with `light_dawn_offset`
/// rather than being a second number that can disagree with it.
fn midnight(light: &Light) -> f32 {
    (0.75 - light.dawn_offset) * DAY_LENGTH
}

/// **How dark the night is (G6, and again in G6b).** The author played the browser build and
/// could not see the creatures or the trees at night at all; G6 raised these to 400 and 55 and
/// the author played it again and said it was *still* too dark. These are the third set, chosen
/// against a number this time: the mean luminance of the ground at midnight, measured off a
/// `--shot --at midnight` in the strip the panels do not cover, is **44 of 255** where G6's was
/// 26 and an afternoon is 81.
///
/// `MOON_LUX` is the `DirectionalLight` that stands in for the moon. It is what draws the *edges*:
/// a creature has a lit side and a shadowed side, and the tree trunks have a direction. The real
/// moon is about 0.1 lux and this is ten thousand times that, which is honest — a night in a game
/// has to be read, and a photograph of a real moonlit field needs a long exposure to look like
/// this.
///
/// It is close to the *day's* floor now (1,200 lux, the sun on the horizon), which G6 kept a wide
/// gap under, and measuring either side of sunset says that gap is not what the difference was
/// made of: dusk reads 39 and the first minute of night reads 24, because **the moon is aimed at
/// `-up`** — at sunset it lies along the horizon and lights nothing, and only by midnight is it
/// overhead. The night has a curve of its own, dark at both ends, and the number chosen here is
/// the number at the top of it.
///
/// `NIGHT_AMBIENT` is what fills the shadowed side, and it is the number that decides whether a
/// beetle under a tree exists. It is the weakest of the three by far in what it does to the
/// measurement — 110 to 190 moved the mean by 1.9 — and it is in the picture for what it does to
/// the *creatures*, which are small, round and mostly in their own shadow.
///
/// All three are multiplied by [`NightDial`], which is the author's own slider.
///
/// **These three are the measurement, and changing them throws it away** (S5b-3). Since
/// `light_moon_lux`, `light_night_ambient` and `light_night_sky_r` / `_g` / `_b` can be written
/// in `garden.settings.txt`, it is worth saying plainly what is lost: the mean ground luminance
/// of **44 of 255 at midnight** was measured off a `--shot --at midnight` of *this* trio on
/// 2026-09-18 (G6b), in the strip the panels do not cover, against a target of 40–50. It is a
/// measurement of the three together — the moon draws the edges, the ambient fills the shadows
/// and the sky is most of the picture — so moving any one of them makes 44 a number about a
/// garden that no longer exists, and nothing in the code re-measures it.
const MOON_LUX: f32 = 950.0;
const NIGHT_AMBIENT: f32 = 190.0;
/// The colour of the sky at night. It is not what the ground is lit by, but it is most of what a
/// picture of a dark garden *is*, and the blue is where the night's colour comes from.
const NIGHT_SKY: [f32; 3] = [0.14, 0.18, 0.36];

/// The day's floor and how much the sun adds by noon, in lux, and the same pair for the ambient
/// light that fills the shadows. *Reason only* for the first of the four — `main.rs` has called
/// 1,200 "the day's floor, the sun on the horizon" since G6 — and **unknown** for the other
/// three.
const DAY_LUX: [f32; 2] = [1_200.0, 9_000.0];
const DAY_AMBIENT: [f32; 2] = [120.0, 260.0];

/// What the sun is built with before `day_night` has had a frame, and how much ground its shadows
/// cover. The illuminance is overwritten on the first frame and is only ever seen if `day_night`
/// is not running; the cascades are not touched again. **Source unknown**, all four.
const SUN_LUX: f32 = 8_000.0;
const SHADOW_CASCADES: u32 = 2;
const SHADOW_NEAR: f32 = 24.0;
const SHADOW_FAR: f32 = 70.0;

/// **The light, as a setting** (S5b-3): the night the author was asked about, the day it turns
/// into, and where in the turn a run begins.
///
/// The one number in here that was already a setting is the *dial* ([`NightDial`], the `night`
/// key) — a multiplier over the three night quantities, put there in G6b so that the author
/// could answer "how dark is your screen?" with one number. This resource is the rest of the
/// same question: the quantities themselves, the range the dial may travel, and the day.
///
/// **Read before the first frame** (`main`), because a `--shot --at midnight` is taken at the
/// brightness the store asked for and there is no second chance at it.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Light {
    /// [`DAWN_OFFSET`]
    pub dawn_offset: f32,
    /// [`MOON_LUX`] — **measured; see the note there before moving it**
    pub moon_lux: f32,
    /// [`NIGHT_AMBIENT`] — the same measurement
    pub night_ambient: f32,
    /// [`NIGHT_SKY`] — the same measurement
    pub night_sky: [f32; 3],
    /// [`NIGHT_ZENITH`]
    pub night_zenith: f32,
    /// [`NIGHT_DIAL_MIN`] / [`NIGHT_DIAL_MAX`]
    pub dial_min: f32,
    pub dial_max: f32,
    /// [`DAY_LUX`]
    pub day_lux: [f32; 2],
    /// [`DAY_AMBIENT`]
    pub day_ambient: [f32; 2],
    /// [`SUN_LUX`]
    pub sun_lux: f32,
    /// [`SHADOW_CASCADES`] / [`SHADOW_NEAR`] / [`SHADOW_FAR`]
    pub shadow_cascades: u32,
    pub shadow_near: f32,
    pub shadow_far: f32,
}

impl Default for Light {
    fn default() -> Self {
        Light {
            dawn_offset: DAWN_OFFSET,
            moon_lux: MOON_LUX,
            night_ambient: NIGHT_AMBIENT,
            night_sky: NIGHT_SKY,
            night_zenith: NIGHT_ZENITH,
            dial_min: NIGHT_DIAL_MIN,
            dial_max: NIGHT_DIAL_MAX,
            day_lux: DAY_LUX,
            day_ambient: DAY_AMBIENT,
            sun_lux: SUN_LUX,
            shadow_cascades: SHADOW_CASCADES,
            shadow_near: SHADOW_NEAR,
            shadow_far: SHADOW_FAR,
        }
    }
}

impl Light {
    /// | key | field |
    /// |---|---|
    /// | `light_dawn_offset` | where in the sun's turn a run starts |
    /// | `light_moon_lux` / `light_night_ambient` / `light_night_sky_r` / `_g` / `_b` | **the measured night** |
    /// | `light_night_zenith` | how much darker the top of the night sky is than its rim |
    /// | `light_dial_min` / `light_dial_max` | how far the `night` slider goes |
    /// | `light_day_lux` / `light_day_lux_span` | the day's floor and what noon adds |
    /// | `light_day_ambient` / `light_day_ambient_span` | the same for the fill light |
    /// | `light_sun_lux` | what the sun is built with |
    /// | `light_shadow_cascades` / `light_shadow_near` / `light_shadow_far` | the shadow cascades |
    ///
    /// The night's colour is three keys rather than one string: S5b-1 kept colours out of the
    /// store because reading one needs a parser, and three numbers need none.
    fn read_from(&mut self, settings: &games_shell::Settings) {
        take(settings, "light_dawn_offset", &mut self.dawn_offset);
        take(settings, "light_moon_lux", &mut self.moon_lux);
        take(settings, "light_night_ambient", &mut self.night_ambient);
        take(settings, "light_night_sky_r", &mut self.night_sky[0]);
        take(settings, "light_night_sky_g", &mut self.night_sky[1]);
        take(settings, "light_night_sky_b", &mut self.night_sky[2]);
        take(settings, "light_night_zenith", &mut self.night_zenith);
        take(settings, "light_dial_min", &mut self.dial_min);
        take(settings, "light_dial_max", &mut self.dial_max);
        take(settings, "light_day_lux", &mut self.day_lux[0]);
        take(settings, "light_day_lux_span", &mut self.day_lux[1]);
        take(settings, "light_day_ambient", &mut self.day_ambient[0]);
        take(settings, "light_day_ambient_span", &mut self.day_ambient[1]);
        take(settings, "light_sun_lux", &mut self.sun_lux);
        if let Some(value) = settings.number("light_shadow_cascades") {
            self.shadow_cascades = value.max(1.0) as u32;
        }
        take(settings, "light_shadow_near", &mut self.shadow_near);
        take(settings, "light_shadow_far", &mut self.shadow_far);
    }
}

/// **The dial (G6b): everything above, multiplied.**
///
/// The author played the browser build again and the night was still too dark — which is the
/// second time a number chosen here has been wrong on the machine it is actually looked at, and
/// the reason is not that the numbers were badly chosen but that *we cannot see the author's
/// screen*. A brightness is not a fact about the code; it is a fact about a monitor in a room. So
/// the slider in the Garden panel multiplies all three night quantities together — moonlight,
/// ambient and sky — and the number it is left at is written to the log and remembered
/// (`games_shell::Settings`), so that the author can turn it until the night reads and tell us
/// one number to bake in here. Multiplying all three by one number is what makes that possible:
/// two dials would be a design decision handed to somebody who asked a question.
///
/// It is not in the headless build. There is nothing to light there.
#[derive(Resource, Debug, Clone, Copy)]
pub struct NightDial(pub f32);

impl Default for NightDial {
    fn default() -> Self {
        NightDial(1.0)
    }
}

/// How far the dial goes. Half the night is still a night; twice it is the author saying the
/// screen is darker than ours, and past that the moon is the sun.
pub const NIGHT_DIAL_MIN: f32 = 0.5;
pub const NIGHT_DIAL_MAX: f32 = 2.0;

// ---------------------------------------------------------------------------------------------
// The horizon (G8). The author played the browser build a third time and said the field was a
// green rectangle floating on a flat colour — which it was: the world ended at the wall, and past
// it was `ClearColor` and nothing else. What is here is the cheapest thing that makes it a place:
// **more ground, fog, a graded sky and a line of trees.** No cubemap, no texture, no second
// camera pass. All of it is the window's; a headless run has no `Look` and none of this exists in
// it, the way it has no `GlobalAmbientLight`.
// ---------------------------------------------------------------------------------------------

/// Half the side of the ground that lies past the field. It is the same material as the field —
/// one more plane, two triangles — and it **follows the camera on XZ**, so its edge is always
/// exactly this far away and always inside the fog. A plane pinned to the origin would have to be
/// four times as big to promise the same thing, and its corners would still be nearer than its
/// sides.
const HORIZON_HALF: f32 = 300.0;
/// and how far under the field it lies, so the two planes do not fight over the pixels they share
const HORIZON_DROP: f32 = -0.02;

/// The sky: a dome of [`SKY_SIDES`] × [`SKY_RINGS`] seen from the inside, centred on the camera,
/// **84 triangles and no texture at all**. The colour is in the vertices — Bevy's
/// `StandardMaterial` *replaces* `base_color` with the vertex colour where a mesh has one
/// (`pbr_fragment.wgsl`, `#ifdef VERTEX_COLORS`), so a gradient is 49 `[f32; 4]`s and a rewrite
/// when the hour changes.
const SKY_RADIUS: f32 = 500.0;
const SKY_SIDES: usize = 12;
const SKY_RINGS: usize = 4;
/// How far below the eye the dome's rim hangs. It only has to be below the horizon the ground
/// draws; the ground is opaque and hides the rest.
const SKY_FLOOR: f32 = -0.30;
/// How much darker the top of the night sky is than its rim. The rim keeps the number G6b
/// measured the night against ([`NIGHT_SKY`]); the gradient is above it.
const NIGHT_ZENITH: f32 = 0.55;

/// Where the fog begins, past the camera's own distance from what it is looking at, and how deep
/// it is. Tying it to the zoom is what keeps the *field* clear at every zoom: the far corner of a
/// 40 × 30 field is about 14 units further from the eye than the middle is, whatever the eye is,
/// so a fog that starts at `distance + 14` never touches the garden and always eats what is past
/// it. The cap is what keeps the end inside [`HORIZON_HALF`]: the ground has to be fog and
/// nothing else where it stops, or its edge would be a line.
///
/// The depth is the number that was measured rather than reasoned about. The default camera
/// stands 42 units out and looks down at 49°, and **the whole frame is ground**: the true horizon
/// is 26° above the top of it, and the furthest ground in the picture is only 72 units away
/// against the field's far corner at 57. Fifteen units of range is all there is at that angle, so
/// a fog deep enough to look like distance from the side (`end` at 300, say) does nothing at all
/// from above. Forty-five puts a visible haze across the top of the default picture and still
/// dissolves the whole plain when the camera is tilted down to look along it.
const FOG_NEAR: f32 = 14.0;
const FOG_DEPTH: f32 = 45.0;
const FOG_NEAR_MAX: f32 = 120.0;

/// A few trees past the wall, so that the edge of the field reads as the edge of the *scenery*
/// rather than as a fence. They carry no component at all — no `Tree`, no `Collider` — because
/// nothing can reach them: they are outside the wall `move_creatures` clamps to, and
/// `garden.nearest(:Tree)` must go on meaning the seven trees that are in the garden.
const EDGE_TREES: usize = 16;
/// How the treeline is scattered: how far along its own step a tree may slide, how far out past
/// the wall it stands, and how big it is. **Source unknown**, all three pairs.
const EDGE_JITTER: f32 = 0.4;
const EDGE_OUT: [f32; 2] = [4.0, 15.0];
const EDGE_SCALE: [f32; 2] = [1.7, 3.1];

/// What the fog is built with before `horizon_look` has had a frame — colour and the two
/// distances. Both are overwritten on the first frame and are only ever seen if `horizon_look` is
/// not running. **Source unknown**.
const FOG_COLOR: [f32; 3] = [0.35, 0.5, 0.7];
const FOG_AT_FIRST: [f32; 2] = [60.0, 220.0];

/// **The scenery past the wall, as a setting** (S5b-3): the far ground, the dome, the fog and
/// the treeline. All of it is the window's — a headless run builds none of it — and all of it is
/// about the *picture* rather than about the garden, which is why it is (c) and not (b).
///
/// Three numbers of the horizon are **not** here and stay `const`, because changing them breaks
/// something rather than changing it: [`HORIZON_DROP`] is the gap that stops two planes fighting
/// over the same pixels, [`SKY_FLOOR`] is the rim being below the horizon the ground draws, and
/// [`FOG_NEAR_MAX`] is the fog ending inside [`Scenery::horizon_half`] rather than past it.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Scenery {
    /// [`HORIZON_HALF`]
    pub horizon_half: f32,
    /// [`SKY_RADIUS`] / [`SKY_SIDES`] / [`SKY_RINGS`]
    pub sky_radius: f32,
    pub sky_sides: usize,
    pub sky_rings: usize,
    /// [`FOG_NEAR`] — **derived**; see the note there
    pub fog_near: f32,
    /// [`FOG_DEPTH`] — **measured**; see the note there
    pub fog_depth: f32,
    /// [`FOG_COLOR`] / [`FOG_AT_FIRST`]
    pub fog_color: [f32; 3],
    pub fog_at_first: [f32; 2],
    /// [`EDGE_TREES`] / [`EDGE_JITTER`] / [`EDGE_OUT`] / [`EDGE_SCALE`]
    pub edge_trees: usize,
    pub edge_jitter: f32,
    pub edge_out: [f32; 2],
    pub edge_scale: [f32; 2],
}

impl Default for Scenery {
    fn default() -> Self {
        Scenery {
            horizon_half: HORIZON_HALF,
            sky_radius: SKY_RADIUS,
            sky_sides: SKY_SIDES,
            sky_rings: SKY_RINGS,
            fog_near: FOG_NEAR,
            fog_depth: FOG_DEPTH,
            fog_color: FOG_COLOR,
            fog_at_first: FOG_AT_FIRST,
            edge_trees: EDGE_TREES,
            edge_jitter: EDGE_JITTER,
            edge_out: EDGE_OUT,
            edge_scale: EDGE_SCALE,
        }
    }
}

impl Scenery {
    /// `horizon_half`, `sky_radius` / `sky_sides` / `sky_rings`, `fog_near` / `fog_depth` /
    /// `fog_color_r` / `_g` / `_b` / `fog_start` / `fog_end`, `edge_trees` / `edge_jitter` /
    /// `edge_out_min` / `edge_out_max` / `edge_scale_min` / `edge_scale_max`.
    ///
    /// The dome wants at least three sides and two rings to be a dome at all, and a count below
    /// that is raised rather than refused — the store is a text file and this is the same
    /// clamping `look_floor_pattern` does in Battle.
    fn read_from(&mut self, settings: &games_shell::Settings) {
        take(settings, "horizon_half", &mut self.horizon_half);
        take(settings, "sky_radius", &mut self.sky_radius);
        if let Some(value) = settings.number("sky_sides") {
            self.sky_sides = (value.max(3.0)) as usize;
        }
        if let Some(value) = settings.number("sky_rings") {
            self.sky_rings = (value.max(2.0)) as usize;
        }
        take(settings, "fog_near", &mut self.fog_near);
        take(settings, "fog_depth", &mut self.fog_depth);
        take(settings, "fog_color_r", &mut self.fog_color[0]);
        take(settings, "fog_color_g", &mut self.fog_color[1]);
        take(settings, "fog_color_b", &mut self.fog_color[2]);
        take(settings, "fog_start", &mut self.fog_at_first[0]);
        take(settings, "fog_end", &mut self.fog_at_first[1]);
        if let Some(value) = settings.number("edge_trees") {
            self.edge_trees = value.max(0.0) as usize;
        }
        take(settings, "edge_jitter", &mut self.edge_jitter);
        take(settings, "edge_out_min", &mut self.edge_out[0]);
        take(settings, "edge_out_max", &mut self.edge_out[1]);
        take(settings, "edge_scale_min", &mut self.edge_scale[0]);
        take(settings, "edge_scale_max", &mut self.edge_scale[1]);
    }
}

/// **The colour of a species (G8).** The author's other complaint was that a rabbit and a beetle
/// are hard to tell apart, which they were: Kenney's Cube Pets share one palette and both animals
/// come out of it brown-pink. These are one number each, used twice — the model is washed with it
/// (`tint_species`) and the creature's name in the HUD is written in it (`window::species_color`)
/// — so that the colour in the list and the colour on the grass cannot drift apart.
pub const RABBIT_TINT: (f32, f32, f32) = (0.93, 0.90, 0.78);
pub const BEETLE_TINT: (f32, f32, f32) = (0.14, 0.48, 0.56);

pub fn species_tint(species: Species) -> (f32, f32, f32) {
    match species {
        Species::Beetle => BEETLE_TINT,
        Species::Rabbit => RABBIT_TINT,
    }
}

/// **Which model a beetle wears.** Cube Pets has no beetle (G0a); the crab is the nearest thing
/// in the pack, and G8 asked the author to choose between it and two others. Changing this line —
/// to `"models/animal-caterpillar.glb"` or `"models/animal-bee.glb"` — is the whole of the swap:
/// the clips are in the same order in every file of the pack, and the tint above is what makes
/// the animal a beetle's colour whatever it is underneath. The other two files are **not in this
/// repository**; they come from the same CC0 pack (`CREDITS.md`) and go beside this one.
const BEETLE_MODEL: &str = "models/animal-crab.glb";

/// The window the game opens. **Quoted, indirectly**: `--eye`'s note says "at the default 42 a
/// beetle is thirty pixels across in a 1600-wide window", so the one number the camera's default
/// distance was argued from is this one. 900 is **unknown**.
const WINDOW: [f32; 2] = [1600.0, 900.0];

/// The lawn. **Source unknown.**
const GROUND_COLOR: [f32; 3] = [0.36, 0.46, 0.25];

/// Where the hunger bar changes colour — red below the first, amber below the second, green
/// above — and how big the bar is in the HUD. **Source unknown**, all four. 55.0 is the same
/// number as `hungry_below` in `ruby/creatures/beetle.rb`, and the two are **not** connected:
/// there is no record of that being meant, and S5a decided to treat it as a coincidence rather
/// than invent the intention (`docs/numbers.md` §7-6).
const HUNGER_LOW: f32 = 20.0;
const HUNGER_WARN: f32 = 55.0;
const HUNGER_BAR: [f32; 2] = [64.0, 11.0];

/// How much a model is scaled up inside its child entity. Kenney's grass is about a quarter of a
/// unit high and the animals are about one; a plant here is a tuft a beetle can hide in. The
/// rock's three are one number squashed two ways. **Source unknown**, all of them.
const TUFT_SCALE: f32 = 2.2;
const BUSH_SCALE: f32 = 2.6;
const TREE_SCALE: f32 = 2.2;
const ROCK_SCALE: [f32; 2] = [3.4, 3.0];
const BEETLE_SCALE: f32 = 0.55;
const RABBIT_SCALE: f32 = 0.75;

/// How fast a creature has to be going before it is walking rather than idling, and how long the
/// blend from one clip to the other takes. **Source unknown**, both.
const WALKING_AT: f32 = 0.2;
const GAIT_BLEND_MS: u64 = 180;

/// **How the garden is drawn, as against how it works** (S5b-3) — the shape of Battle's `Look`,
/// and for the same reason: none of it changes what happens in the world, and all of it was
/// written into the middle of a function where nobody could reach it.
///
/// The model scales are handed on to [`Look`] when `make_look` builds it, so that the four
/// `spawn_*` helpers keep taking one `Option<&Look>` and a headless run — which has no `Look` at
/// all — goes on never seeing them.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Picture {
    /// [`WINDOW`] — read before the window is opened
    pub window: [f32; 2],
    /// [`GROUND_COLOR`]
    pub ground_color: [f32; 3],
    /// [`HUNGER_LOW`] / [`HUNGER_WARN`] / [`HUNGER_BAR`]
    pub hunger_low: f32,
    pub hunger_warn: f32,
    pub hunger_bar: [f32; 2],
    /// [`TUFT_SCALE`] / [`BUSH_SCALE`] / [`TREE_SCALE`] / [`ROCK_SCALE`] / [`BEETLE_SCALE`] /
    /// [`RABBIT_SCALE`]
    pub tuft_scale: f32,
    pub bush_scale: f32,
    pub tree_scale: f32,
    pub rock_scale: [f32; 2],
    pub beetle_scale: f32,
    pub rabbit_scale: f32,
    /// [`WALKING_AT`] / [`GAIT_BLEND_MS`]
    pub walking_at: f32,
    pub gait_blend_ms: u64,
    /// [`BEETLE_MODEL`] — the one entry here that is not a number. It is in the store for the
    /// reason its own note gives: swapping it is the whole of "put a different animal in the
    /// garden", and `look_beetle_model=models/animal-bee.glb` spares a rebuild for it.
    pub beetle_model: String,
}

impl Default for Picture {
    fn default() -> Self {
        Picture {
            window: WINDOW,
            ground_color: GROUND_COLOR,
            hunger_low: HUNGER_LOW,
            hunger_warn: HUNGER_WARN,
            hunger_bar: HUNGER_BAR,
            tuft_scale: TUFT_SCALE,
            bush_scale: BUSH_SCALE,
            tree_scale: TREE_SCALE,
            rock_scale: ROCK_SCALE,
            beetle_scale: BEETLE_SCALE,
            rabbit_scale: RABBIT_SCALE,
            walking_at: WALKING_AT,
            gait_blend_ms: GAIT_BLEND_MS,
            beetle_model: BEETLE_MODEL.to_string(),
        }
    }
}

impl Picture {
    /// | key | field |
    /// |---|---|
    /// | `window_width` / `window_height` | [`Picture::window`] — the same two keys Battle uses |
    /// | `look_ground_r` / `_g` / `_b` | the lawn |
    /// | `look_hunger_low` / `look_hunger_warn` | where the meter changes colour |
    /// | `look_hunger_bar_width` / `look_hunger_bar_height` | how big the meter is |
    /// | `look_tuft_scale` / `look_bush_scale` / `look_tree_scale` | the plants and the trees |
    /// | `look_rock_scale` / `look_rock_squash` | the boulders |
    /// | `look_beetle_scale` / `look_rabbit_scale` | the animals |
    /// | `look_walking_at` / `look_gait_blend_ms` | when a creature walks, and the blend |
    /// | `look_beetle_model` | which `.glb` a beetle wears |
    fn read_from(&mut self, settings: &games_shell::Settings) {
        take(settings, "window_width", &mut self.window[0]);
        take(settings, "window_height", &mut self.window[1]);
        take(settings, "look_ground_r", &mut self.ground_color[0]);
        take(settings, "look_ground_g", &mut self.ground_color[1]);
        take(settings, "look_ground_b", &mut self.ground_color[2]);
        take(settings, "look_hunger_low", &mut self.hunger_low);
        take(settings, "look_hunger_warn", &mut self.hunger_warn);
        take(settings, "look_hunger_bar_width", &mut self.hunger_bar[0]);
        take(settings, "look_hunger_bar_height", &mut self.hunger_bar[1]);
        take(settings, "look_tuft_scale", &mut self.tuft_scale);
        take(settings, "look_bush_scale", &mut self.bush_scale);
        take(settings, "look_tree_scale", &mut self.tree_scale);
        take(settings, "look_rock_scale", &mut self.rock_scale[0]);
        take(settings, "look_rock_squash", &mut self.rock_scale[1]);
        take(settings, "look_beetle_scale", &mut self.beetle_scale);
        take(settings, "look_rabbit_scale", &mut self.rabbit_scale);
        take(settings, "look_walking_at", &mut self.walking_at);
        if let Some(value) = settings.number("look_gait_blend_ms") {
            self.gait_blend_ms = value.max(0.0) as u64;
        }
        if let Some(name) = settings.get("look_beetle_model") {
            self.beetle_model = name.to_string();
        }
    }
}

/// **Plants — what is left here of them** (W1).
///
/// How fast grass grows, how often a blade comes up and how many the field holds are rules, and
/// rules are `ruby/world.rb`'s: `PLANT_GROWTH`, `SPROUT_RATE` and `PLANTS_MAX` were deleted from
/// this block and written there, with the same numbers. What stayed is the garden's *furniture*
/// — how many blades a new world starts with, how big a seed is when it is put down, and how big
/// a fully grown one is, which the first plants and the checks' fixtures are made at.
const PLANTS_AT_START: usize = 55;
const PLANT_MIN: f32 = 0.18;
/// **How big a blade the game builds full-grown is** — the largest of the grass a new garden is
/// scattered with, and the size of the three blades the checks' fixtures are planted at.
///
/// `world.rb` has a `plant_max` too, and **it is not this one** (S5b-4, and §7-15 of
/// `docs/numbers.md`): there it is the ceiling a blade's *growth* stops at, a rule that runs
/// sixty times a second over grass that is already in the ground. This is a size the game hands
/// to `spawn_plant`. They are the same number today and nothing holds them so — a garden whose
/// rules stop growth at 0.5 and whose furniture is built at 1.4 opens with grass that will never
/// be that big again, and one the other way round opens with seedlings that grow.
///
/// It was called `PLANT_MAX` and its key in the store was `plant_max` until S5b-4, which is how
/// the two came to look like one number with two homes. **Source: unknown.**
const PLANT_GROWN: f32 = 1.4;

/// Creatures
const BEETLES: usize = 6;
const RABBITS: usize = 4;

/// How hungry a creature is when a new garden is built. **Source unknown.**
const START_HUNGER: [f32; 2] = [45.0, 90.0];
/// How many times a spot is rolled again before the thing being placed is given up on (trees and
/// rocks) or put down where it last landed (creatures). **Source unknown.**
const START_TRIES: usize = 40;
/// How often a blade of grass is a round bush rather than a tuft, and how much a boulder is
/// squashed. **Source unknown**, both.
const ROUND_CHANCE: f32 = 0.35;
const ROCK_SQUASH: [f32; 2] = [0.8, 1.25];
/// The five distances a new garden is laid out by: how far two immovable things stand apart, how
/// far a blade of grass keeps off one, and the three a creature keeps — off the grass, off the
/// solid things, and off its neighbours. **Quoted** for the first: `docs/plans/garden-plan.md`,
/// "岩どうしは押し戻さないので初期配置の側で 1.5 以上離している". The other four are **unknown**.
const SOLID_APART: f32 = 1.5;
const GRASS_OFF_SOLID: f32 = 1.2;
const CREATURE_OFF_GRASS: f32 = 2.5;
const CREATURE_OFF_SOLID: f32 = 0.6;
const CREATURES_APART: f32 = 2.0;

/// **What a new garden is built with** (S5b-3): the furniture, and the spacing it is laid out by.
///
/// It is (c) rather than (b) because none of it is a *rule* — the rules are `ruby/world.rb`'s and
/// they take over on the first frame. This is the state of the world before any of them has run,
/// which a person may want more or less of without having an opinion about how grass grows.
///
/// **A garden read back from a save uses none of it**: `load_world` builds the world out of the
/// file, and the only thing from here it can still reach is [`Furniture::plant_grown`], which
/// `plant_the_meadow` plants the checks' corner at.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Furniture {
    /// [`PLANTS_AT_START`] / [`PLANT_MIN`] / [`PLANT_GROWN`]
    pub plants: usize,
    pub plant_min: f32,
    /// how big a blade this builds when it builds a full-grown one — **not** `world.rb`'s
    /// `plant_max`, which is where growth stops ([`PLANT_GROWN`])
    pub plant_grown: f32,
    /// [`BEETLES`] / [`RABBITS`] / [`TREES`] / [`ROCKS`]
    pub beetles: usize,
    pub rabbits: usize,
    pub trees: usize,
    pub rocks: usize,
    /// [`START_HUNGER`] / [`START_TRIES`]
    pub hunger: [f32; 2],
    pub tries: usize,
    /// [`ROUND_CHANCE`] / [`ROCK_SQUASH`]
    pub round_chance: f32,
    pub rock_squash: [f32; 2],
    /// [`SOLID_APART`] / [`GRASS_OFF_SOLID`] / [`CREATURE_OFF_GRASS`] / [`CREATURE_OFF_SOLID`] /
    /// [`CREATURES_APART`]
    pub solid_apart: f32,
    pub grass_off_solid: f32,
    pub creature_off_grass: f32,
    pub creature_off_solid: f32,
    pub creatures_apart: f32,
    /// **What each species is built with** ([`Genome::of`]) and how far a rolled one may stray
    /// from it ([`genome::SPREAD`]).
    ///
    /// They are the garden's furniture and not its rules, which is a change of mind (S5b-5). S5b-4
    /// set out to put them in `ruby/world.rb` with the rest of the numbers of play and found that
    /// it could not: `spawn_world` is a `Startup` system and `garden.rules` does not reach the
    /// game until the first `Update`, so a `beetle_speed` written in the rules would be **read by
    /// nothing, in any run**. What decides where a number can live is who reads it and when
    /// (`docs/worklog/2026-09-21-numbers-garden-play.md` §6). A number read once, while the world
    /// is being built, belongs with the other numbers read once while the world is being built —
    /// the count of plants, the count of creatures, how hungry they start.
    ///
    /// It says nothing about what a creature's genome may become afterwards: `Genome#mix` and
    /// `Genome#mutate` are the species' own Ruby, and `garden.spawn` takes whatever genome a
    /// script hands it.
    pub beetle: Genome,
    pub rabbit: Genome,
    pub genome_spread: f32,
}

impl Default for Furniture {
    fn default() -> Self {
        Furniture {
            plants: PLANTS_AT_START,
            plant_min: PLANT_MIN,
            plant_grown: PLANT_GROWN,
            beetles: BEETLES,
            rabbits: RABBITS,
            trees: TREES,
            rocks: ROCKS,
            hunger: START_HUNGER,
            tries: START_TRIES,
            round_chance: ROUND_CHANCE,
            rock_squash: ROCK_SQUASH,
            solid_apart: SOLID_APART,
            grass_off_solid: GRASS_OFF_SOLID,
            creature_off_grass: CREATURE_OFF_GRASS,
            creature_off_solid: CREATURE_OFF_SOLID,
            creatures_apart: CREATURES_APART,
            beetle: genome::BEETLE,
            rabbit: genome::RABBIT,
            genome_spread: genome::SPREAD,
        }
    }
}

impl Furniture {
    /// `start_plants`, `plant_min`, `plant_grown`, `start_beetles`, `start_rabbits`, `start_trees`,
    /// `start_rocks`, `start_hunger_min` / `start_hunger_max`, `start_tries`,
    /// `start_round_chance`, `start_rock_squash_min` / `_max`, `start_solid_apart`,
    /// `start_grass_off_solid`, `start_creature_off_grass`, `start_creature_off_solid`,
    /// `start_creatures_apart`, `start_beetle_speed` / `_sight` / `_appetite`,
    /// `start_rabbit_speed` / `_sight` / `_appetite`, `start_genome_spread`.
    fn read_from(&mut self, settings: &games_shell::Settings) {
        let count = |key: &str, slot: &mut usize| {
            if let Some(value) = settings.number(key) {
                *slot = value.max(0.0) as usize;
            }
        };
        count("start_plants", &mut self.plants);
        take(settings, "plant_min", &mut self.plant_min);
        take(settings, "plant_grown", &mut self.plant_grown);
        count("start_beetles", &mut self.beetles);
        count("start_rabbits", &mut self.rabbits);
        count("start_trees", &mut self.trees);
        count("start_rocks", &mut self.rocks);
        take(settings, "start_hunger_min", &mut self.hunger[0]);
        take(settings, "start_hunger_max", &mut self.hunger[1]);
        // one try is still a try; zero would leave a creature wherever `Vec2::ZERO` is
        if let Some(value) = settings.number("start_tries") {
            self.tries = value.max(1.0) as usize;
        }
        take(settings, "start_round_chance", &mut self.round_chance);
        take(settings, "start_rock_squash_min", &mut self.rock_squash[0]);
        take(settings, "start_rock_squash_max", &mut self.rock_squash[1]);
        take(settings, "start_solid_apart", &mut self.solid_apart);
        take(settings, "start_grass_off_solid", &mut self.grass_off_solid);
        take(settings, "start_creature_off_grass", &mut self.creature_off_grass);
        take(settings, "start_creature_off_solid", &mut self.creature_off_solid);
        take(settings, "start_creatures_apart", &mut self.creatures_apart);
        take(settings, "start_beetle_speed", &mut self.beetle.speed);
        take(settings, "start_beetle_sight", &mut self.beetle.sight);
        take(settings, "start_beetle_appetite", &mut self.beetle.appetite);
        take(settings, "start_rabbit_speed", &mut self.rabbit.speed);
        take(settings, "start_rabbit_sight", &mut self.rabbit.sight);
        take(settings, "start_rabbit_appetite", &mut self.rabbit.appetite);
        take(settings, "start_genome_spread", &mut self.genome_spread);
    }

    /// The genome this garden builds one of that species with — [`Genome::of`] with the store's
    /// answer where there is one.
    pub fn genome(&self, species: Species) -> Genome {
        match species {
            Species::Beetle => self.beetle,
            Species::Rabbit => self.rabbit,
        }
    }
}

/// A full meter. The rule that fills it and the rule that empties it are `world.rb`'s
/// (`hunger_max`, `hunger_rate`, `eat_rate`, `food_value`); this is here because the HUD draws a
/// bar and a bar needs to know what full is.
const HUNGER_MAX: f32 = 100.0;
/// How close is touching, for a rabbit startling a beetle — and the distance the fifth check
/// measures the probe's walk against. Eating has a reach of its own and it is `world.rb`'s now,
/// with this same number in it: `startle` is a rule that stayed and `eat` is one that went.
///
/// It is also what [`Reaches`] holds until `world.rb` has said otherwise with
/// `garden.rules(reach:)` — the same arrangement [`CHILD_HUNGER`] has, and for the same reason:
/// the game has to have an answer before the rules have spoken.
const REACH: f32 = 1.1;
/// How close a rabbit has to come to a beetle for the beetle to be told about it — **the rules'
/// number, and the game only sweeps for it** (S5b-4). `world.rb` says it (`touch_reach`) and
/// hands it over with the other distances; `startle` walks every beetle against every rabbit,
/// because that walk is what Ruby should not be writing sixty times a second (`world.rb`'s own
/// first paragraph).
///
/// What is left here is the **stand-in**: what [`Reaches`] holds for the frames before the rules
/// have spoken, and for ever in a garden whose `world.rb` will not compile — which is a garden
/// this game goes on running, deliberately (see [`WorldTrouble`]). It is not a second opinion
/// about the rules: a test (`the_stand_ins_are_what_world_rb_says`) reads `ruby/world.rb` and
/// fails if the two ever drift apart. **Source: unknown** — 1.3 has no record anywhere.
const TOUCH_REACH: f32 = 1.3;

/// How close two well-fed creatures have to be before the rules tell them about each other, until
/// `world.rb` says otherwise with `garden.rules(mate_reach:)`. The rule itself is `world.rb`'s
/// (`mate_reach`), with this same number in it; the one thing the game builds with it is the
/// selftest's meadow corner ([`plant_the_meadow`]), which has to stand where the rules can reach
/// across it.
const MATE_REACH: f32 = 2.0;

/// How long a creature has been alive before the selftest expects its handlers to answer for it.
/// A newborn (G2) is spawned with a `Script`, which rubevy turns into a task on a later frame,
/// and the task's first act is to subscribe to its five or six events — so for the first moments
/// of a life there is nobody listening, and an event published then is dropped. It is not a
/// *rule*: a creature that hears nothing simply carries on wandering. It is only that "did the
/// handler turn it?" cannot be asked of a creature that had no handlers yet, and before G2 every
/// creature in the world was as old as the world.
const NEWBORN_GRACE: f32 = 2.0;

/// How many frames a newborn is deaf for, which is how long the game waits before repeating the
/// sky to it ([`tell_newborns_the_sky`]).
///
/// **Measured, not chosen.** `children_arrive` puts the `Script` on the child in frame N, after
/// that frame's tick; rubevy turns it into a task and runs it for the first time in frame N+1's
/// `RubevySet::Tick`, and the first thing that task does is `start_handlers`, which is where
/// `Rubevy.subscribe` is called; rubevy's `publish_value` only reaches queues that exist when it
/// is called. So the first publish a child can hear is one made in frame N+2. Creatures spawned
/// one frame apart around nightfall said exactly that: one born two frames before the `"night"`
/// heard it, one born one frame before did not — while being already counted among the
/// addressees (`docs/worklog/2026-09-17-selftest-flakes.md` §4.2).
const NEWBORN_DEAF_FRAMES: u32 = 2;

/// How long a beetle has to have been left alone before a `"touched"` sent to it is one the sixth
/// check can ask a question about: long enough for its handler task to have finished anything that
/// was already in its queue (a handler holds the wheel for half a second over each message).
const TOUCH_SETTLE: f32 = 1.5;

/// Breeding (W2). **Every number that was here is in `ruby/world.rb` now** — how full two
/// creatures have to be, how close, how often the rules may say so, what a child costs its
/// parents and how long they may not have another — on the lines named beside them there, with
/// the values they had here. What is left in this source is the two the *game* builds with, and
/// they are here because the game has to have an answer before `world.rb` has spoken (and when
/// it never does, because it will not compile): they are [`DAY_LENGTH`]'s neighbours, not the
/// rules'.
///
/// a newborn's meter, until `world.rb` says otherwise with `garden.rules(child_hunger:)`. Well
/// under the three quarters the rules breed at, so nothing is born breeding.
const CHILD_HUNGER: f32 = 50.0;
/// how many creatures the game will make, until `world.rb` says otherwise with
/// `garden.rules(pop_max:)`. The rules hold the population down long before this — a pair is
/// never told to breed when the garden is full — and this is the game refusing to *build* past
/// what it was told, which is a different job: a script may ask for a creature without anybody
/// having told it to.
const POP_MAX: usize = 24;

/// Solid things. Circles on XZ, pushed apart after the move; no physics crate, because the rule
/// is three lines and a physics crate is a megabyte of wasm and a second vocabulary.
///
/// **The four are `world.rb`'s since S5b-4** ([`Bodies`]): how wide a body is is a number the
/// garden is played with — a fatter beetle is a beetle that bumps into more — and the rules hand
/// it over with the distances. These four are the **stand-ins**, the same arrangement [`REACH`]
/// and [`CHILD_HUNGER`] have: what a body is until the rules have spoken, and for ever in a
/// garden whose `world.rb` will not compile. **Source: unknown** for all four.
const BEETLE_RADIUS: f32 = 0.40;
const RABBIT_RADIUS: f32 = 0.50;
const TREE_RADIUS: f32 = 0.70;
const ROCK_RADIUS: f32 = 0.60;
const TREES: usize = 7;
const ROCKS: usize = 9;
/// the side of one cell of the neighbour grid: at least twice the largest radius, so two circles
/// that touch are always in the same cell or in neighbouring ones.
///
/// **It is a floor now, not the answer** (S5b-4). The radii are the rules' and a `world.rb` that
/// makes a tree three units wide would be sorting its neighbours into cells too small to find
/// them in — so the grid is [`Bodies::cell`], which is this or twice the largest body, whichever
/// is more. With the rules as they ship the largest body is 0.70 and twice it is 1.4, so the
/// answer is this number and the grid is the grid it always was.
const CELL: f32 = 1.6;
/// how many times the push is repeated in a frame. One pass settles a pair; a huddle of three or
/// four wants a few, and four is enough that nothing is ever seen overlapping.
const SEPARATE_PASSES: usize = 4;

// ---------------------------------------------------------------------------------------------
// The components. This block is the Ruby API of the game, and the only reason it is an API is
// the two derives and the `register_type` calls in `main`.
// ---------------------------------------------------------------------------------------------

/// Grass. It grows by itself, is eaten down, and is gone when there is nothing left of it.
/// `size` is also the entity's `Transform.scale`, so `e[:Transform][:scale]` says the same thing
/// twice — on purpose: growth is something Ruby can see without the game telling it.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Plant {
    pub size: f32,
}

/// Which sort of creature. A field-less enum reflects as a Symbol (`:Beetle`), which is what a
/// script wants to compare against — and serde writes a unit variant as a Symbol too, while
/// reading takes a Symbol or a String either way round (sabiruby `docs/design/serde.md`). So
/// `species: name` in a script, where `name` is the String `"Beetle"`, fits `Species` without
/// anything being written for it, and the save file has `"species": "Beetle"`.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Species {
    #[default]
    Beetle,
    Rabbit,
}

impl Species {
    pub fn name(self) -> &'static str {
        match self {
            Species::Beetle => "Beetle",
            Species::Rabbit => "Rabbit",
        }
    }

    /// The file every creature of it runs — the editor's unit in this game (G4).
    pub fn file(self) -> &'static str {
        match self {
            Species::Beetle => "beetle.rb",
            Species::Rabbit => "rabbit.rb",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Species::Beetle => 0,
            Species::Rabbit => 1,
        }
    }

    pub const ALL: [Species; 2] = [Species::Beetle, Species::Rabbit];
}

/// A living thing, how long it has been one, and what it was born with.
///
/// `genome` is G2's: how fast it may go, how far it sees and how fast it uses itself up, all
/// three read by the Rust rules below. Because `Genome` derives `Reflect` like everything else
/// here, a script reads it with no extra ceremony —
///
///     me[:Creature][:genome]      # => {speed: 2.31, sight: 8.4, appetite: 0.97}
///
/// — and because it *also* derives `RubyClass`, the same three numbers can come back as an
/// object with methods on them (`Rubevy.ask("genome")`, answered in `answer_garden`). Two views
/// of one value, and neither cost a line of glue.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Creature {
    pub species: Species,
    pub age: f32,
    pub genome: Genome,
    /// **Who asked for it** (W2), where anybody did: the creature whose script called
    /// `garden.spawn`. The world's first ten and everything read back from a save have none.
    ///
    /// It is here because the rules are Ruby's and a rule has to be able to *find* a newborn.
    /// The cost of a child falls on its parents when the child is actually in the world, and the
    /// only frame in which `world.rb` can tell that a creature is new is the frame it first sees
    /// it — which is the frame whose `age` is still exactly zero, because the thing that ages a
    /// creature is that same rule. So "age is zero and somebody asked for me" is a newborn, said
    /// in two fields the rules already read.
    ///
    /// `Option<Entity>` rather than an `Entity` and a placeholder because Ruby has to tell the
    /// two apart: an Option reads as `:None` or as `{Some: [creature]}` (rubevy `src/reflect.rs`)
    /// and it is never written from there.
    pub parent: Option<Entity>,
}

/// How full it is: `HUNGER_MAX` is stuffed, 0 is dead. (The name is the plan's; read it as "the
/// hunger meter", which empties.)
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Hunger(pub f32);

/// Where it is going, on the ground plane: `x` is world X and `y` is world Z. Rust integrates it
/// into `Transform` and stops it at the walls; from G1 it is what a brain writes.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Velocity(pub Vec2);

/// How far it can see. G1's `garden.nearest(:Plant)` looks this far and no further.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Sight(pub f32);

/// That this creature remembers things. It stayed a marker in G3, and that is the finding rather
/// than an omission: what a creature remembers is a Ruby Hash (`@memory`) living in the VM, on the
/// object its script made, and the save file reads it *from there* (`save_world` below) instead of
/// copying it into a component every time it changes. A component would be a second copy of the
/// same Hash, kept in step by whom? So what is in the ECS is the fact that this entity has a
/// memory, which is what a rule could want to know; the memory itself is the script's.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component)]
pub struct Memory;

/// How much room a solid thing takes on the ground: a circle on XZ. Creatures, trees and rocks
/// have one; grass does not, because walking into grass is how it is eaten.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Collider {
    pub radius: f32,
}

/// A tree. Not `Plant`: it is not food, it is something to walk round.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component)]
pub struct Tree;

/// A rock. The same, lower down.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component)]
pub struct Rock;

// ---------------------------------------------------------------------------------------------
// Components Ruby is **not** meant to see: no `Reflect`, no `register_type`. Keeping them out of
// the registry is the whole of the access control there is, and it is enough.
// ---------------------------------------------------------------------------------------------

/// The one directional light.
#[derive(Component)]
struct Sun;

/// What a creature's script is called and what it has cost, for the HUD and for the log. The
/// numbers come from `ScriptWorld::stats`, which is rubevy's, not the VM's public surface.
#[derive(Component)]
pub struct Mind {
    pub name: String,
    /// which file it is running, which in this game is which sort of creature it is: every
    /// beetle runs `beetle.rb`. (SabiRuby Battle's unit is the other way round — each robot has
    /// a file of its own — which is why its editor has two Apply buttons and this one has one.)
    pub species: Species,
    /// instructions the script had run at the end of the last frame
    last_instructions: u64,
    /// and how many it spent on this one
    pub spent: u64,
    /// how many frames it has been looked at, so that the log can say what it costs on average —
    /// one frame's number is nearly always zero, because a creature spends nearly every frame
    /// parked on a `sleep` or on an answer
    pub frames: u64,
    /// the line it is standing on, in the creature's own file where it is in one
    pub at: String,
    /// how many lines the prelude put in front of the creature's file
    pub prelude_lines: u32,

    // --- G4 ----------------------------------------------------------------
    /// It is running a text applied in the editor rather than what the file says.
    pub in_memory: bool,
    /// **Which hand-over of its species' program it is wearing** (S7, [`Wearing`]). A creature
    /// whose number is behind its species' is one the hand-over could not see, and
    /// `window::catch_up_minds` gives it the program on the next frame.
    pub generation: u32,
    /// the line of the creature's *own* file it is standing on, 1-based, for the editor's band
    pub own_line: Option<u32>,
    /// and where it has been spending its time, per line, decayed every frame: the editor shades
    /// the listing with it
    pub heat: Vec<f32>,
    /// The frame this task last ran an instruction in.
    ran_frame: u32,
    /// The frame `answer_garden` last answered one of the game's own questions for it. A gap that
    /// starts on that frame is a `Rubevy.ask` round trip and is known to be one.
    asked_frame: Option<u32>,
    /// round trips the *game* answered, and the frames they took. Component reads used to be
    /// counted beside these and are not any more: since 2026-09-17 rubevy answers a read inside
    /// the tick that asked it, so a read leaves no gap to count (see `watch_minds`).
    pub ask_trips: u32,
    pub ask_frames: u32,
    /// passes of the behaviour's loop this creature has finished, and what they cost it
    pub decisions: u32,
    pub decision_instructions: u64,
    /// `ScriptStats::instructions` as the pass now under way began. `None` until the first `sleep`
    /// this has seen the end of: a pass that was already half done when the counting started
    /// would be counted short.
    pass_start: Option<u64>,
}

impl Mind {
    /// The script is being replaced: what a round trip cost the old one says nothing about the
    /// new one (G4's editor).
    pub fn restart(&mut self) {
        self.ran_frame = 0;
        self.asked_frame = None;
        self.ask_trips = 0;
        self.ask_frames = 0;
        self.decisions = 0;
        self.decision_instructions = 0;
        self.pass_start = None;
    }

    /// The HUD's **insn/decision**: the mean number of VM instructions this creature spends on
    /// one pass of its behaviour's loop. `None` until it has finished one.
    ///
    /// This stands where G4's **frames/decision** stood. That number was the frames a task waited
    /// between asking the world something and running again with the answer, and it worked
    /// because a component read *parked* the task: the gap it left was the thing being measured.
    /// From 2026-09-17 rubevy answers a read inside the tick that asked it (rubevy
    /// `docs/host-api.md`, "A read costs no frame"), so there is no gap, and the old number went
    /// from counting 2352 reads a run to counting three or four gaps that were never reads at all
    /// (`docs/worklog/2026-09-17-sync-reads.md`). What a read costs now is instructions, not
    /// frames, so that is what this counts.
    pub fn instructions_per_decision(&self) -> Option<f32> {
        (self.decisions > 0).then(|| self.decision_instructions as f32 / self.decisions as f32)
    }
}

/// Mid-meal, and for a moment after the last bite so that the animation does not flicker between
/// two blades of grass. Set by `eat`, read by `animate_creatures` — and by nothing else.
#[derive(Component)]
struct Eating {
    until: f32,
}

/// Which of the model's clips a creature is playing, and the entity of the `AnimationPlayer` the
/// scene brought with it. Only the windowed build has either.
#[derive(Component)]
struct Animated {
    player: Entity,
    playing: Gait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gait {
    Idle,
    Walk,
    Eat,
}

/// When a creature may court again, and who it was last told about.
///
/// **It is Ruby's now** (W2), and that is the whole of the change: it was a private component
/// because the rule that kept it was `court`, in this file, and the rule is `ruby/world.rb`'s.
/// A component is how a rule keeps something about a creature that has to outlive the rule's own
/// script — the editor takes `world.rb` away and gives it back while the garden runs, and a
/// cooldown held in an instance variable of the world object would be forgiven by every
/// keystroke — and it is how the same fact dies with the creature it is about, without anybody
/// sweeping a table of entities that are not there any more.
///
/// The alternative was a Hash in `world.rb` keyed by entity, and both of those are why it is not
/// that (`docs/worklog/2026-09-17-garden-world.md`, W2).
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
struct Breeding {
    /// the earliest the rules may tell this creature about a partner again — in the garden's own
    /// clock, which is what `garden.now` answers and what `P` holds still
    ready_at: f32,
    /// who it was last told about, so that the cost of a child can be charged to **both** parents
    /// when the child arrives, a frame or two later
    ///
    /// [`Entity::PLACEHOLDER`] where it has been told about nobody, and not an `Option<Entity>`,
    /// because this one **is written from Ruby**: rubevy applies a Hash field by field, and a
    /// `{Some: […]}` cannot turn a `None` into a `Some` (`src/reflect.rs`, `apply_enum` — the
    /// fields of a variant are only written while the value is already that variant). A rule
    /// that reads the placeholder gets a creature that does not exist, whose components all read
    /// `nil`, which is the answer it wanted anyway.
    partner: Entity,
}

/// **The one entity the world's script sits on** (W1).
///
/// rubevy hangs a task on an entity, so the rules need one — and having one is not a formality:
/// `Rubevy.entity` is what `garden.within(me, …)` and `garden.spawn` read to know who is asking,
/// and it is what the editor will restart in W3, exactly as `restart_species` restarts a species.
/// It has no `Transform` and no `Mind`: it is not in the world, it is the world's opinion of it.
#[derive(Component)]
struct WorldScript;

/// **How many lines stand in front of `world.rb` in the program the world's task runs** (W3+).
///
/// A creature keeps this in its `Mind` and the VM panel takes it off every frame it draws
/// (`VmInspector::fill`). The world has no `Mind` — its script is one task and there is nothing
/// to tell it apart from — so until the panel could be pointed at the world's VM nobody needed
/// the number anywhere but inside `compile_world_source`. It is a resource because there is one
/// set of rules, and it is written wherever they are put on, so that editing `world_prelude.rb`
/// and saving it moves the panel's line numbers with it.
#[derive(Resource, Default)]
pub struct WorldPrelude(pub u32);

/// The selftest's fasting beetle: no script, almost no hunger left.
#[derive(Component)]
struct Fasting;

/// The selftest's probe: a hungry beetle put down within sight of exactly one plant, to see
/// whether a script-driven creature actually walks to its food.
#[derive(Component)]
struct Probe {
    /// the plant it was given
    dinner: Entity,
}

/// Where `make_look` is, so that `spawn_world` can be after it in the build that has one.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct MakeLook;

/// **G8.** The two things that are not in the world but around it: the ground past the field and
/// the sky over it. Both ride with the camera (`horizon_look`), which is why they are marked
/// rather than found by their mesh.
#[derive(Component)]
struct Horizon;

#[derive(Component)]
struct SkyShell;

/// **G8.** "Whatever model hangs under me, wash it in this species' colour." It goes on the child
/// that carries the `WorldAssetRoot` — which is the entity `WorldInstanceReady` names — and
/// `tint_species` reads it when the model has finished arriving.
#[derive(Component, Clone, Copy)]
struct Tint(Species);

// ---------------------------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------------------------

/// The world's dice, SplitMix64 — the same one sabibots rolls, so a world can be replayed from a
/// seed once anything wants to.
#[derive(Resource)]
struct Dice(u64);

impl Dice {
    /// 0..1
    fn roll(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }
    fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.roll() * (hi - lo)
    }
}

/// Where the sun is, and whether the world thinks it is night. The flip is what gets published.
///
/// `shift` is G3's: the world's clock is `Time::elapsed_secs() + shift`, and a garden read back
/// from a file sets it so that the day goes on from where it was saved. It is the only clock that
/// is shifted — a cooldown or a contact is bookkeeping about *this* run and is not in the save —
/// and while a loaded world's minds are starting (`Restoring`) it is held so that the world's
/// clock stands perfectly still.
///
/// `day_length` is the one rule of the sun's that Ruby owns (W1). The sun is *drawn* here — the
/// light, the colour, the sky's gradient are all Rust — so what crosses the boundary is the one
/// number the drawing needs, handed over once by `world.rb`'s `day_length` through
/// `garden.rules`. Until it says otherwise it is [`DAY_LENGTH`], which is also what a garden whose
/// `world.rb` will not compile keeps running on.
#[derive(Resource)]
struct Sky {
    phase: f32,
    night: bool,
    shift: f32,
    day_length: f32,
}

impl Default for Sky {
    fn default() -> Self {
        Sky { phase: 0.0, night: false, shift: 0.0, day_length: DAY_LENGTH }
    }
}

/// **The second VM's name tag (W1): the world's own.**
///
/// The creatures' VM is the app's first and is spelled as it always was (`ScriptWorld`,
/// `RubevySet::Tick`, `Script::new`); this one is the same plugin added again under a name,
/// reading `ruby/` and running exactly one script — `ruby/world.rb`, the rules. The tag is a type
/// and nothing else: it implements nothing, holds nothing and costs nothing at run time (rubevy
/// `docs/host-api.md`, "Two VMs in one app").
///
/// Why a second VM rather than one more task in the creatures': the rules and the creatures are
/// two pieces of somebody else's writing and the editor swaps either without the other, so a
/// runaway `world.rb` must not be able to spend the beetles' frame, and a constant one of them
/// defines must not be able to reach the other. What it costs is that a message published to one
/// VM does not reach the other, which is why `tell` (W2) goes through Rust.
///
/// **It shadows Bevy's `World`**, which `bevy::prelude::*` brings in: an item written in a module
/// wins over a glob import. The three places in this file that mean Bevy's world therefore spell
/// it out (`bevy::ecs::world::World`), and the name is worth that, because every other mention of
/// `World` here — `ScriptWorld<World>`, `RubevySet::<World>::tick()`, `Script::<World>::for_vm` —
/// is the thing this game calls the world.
struct World;

/// Which rabbit is standing on which beetle right now, so `"touched"` is published when a contact
/// begins rather than sixty times a second while it lasts.
#[derive(Resource, Default)]
struct Contacts(Vec<(Entity, Entity)>);

/// The same for `"bumped"`: which pairs the separation pass was pushing apart last frame. The key
/// is the two entities' bits, smaller first, so it does not depend on the order the query happened
/// to walk them in.
#[derive(Resource, Default)]
struct Bumps(Vec<(u64, u64)>);

fn pair_key(a: Entity, b: Entity) -> (u64, u64) {
    if a.to_bits() <= b.to_bits() { (a.to_bits(), b.to_bits()) } else { (b.to_bits(), a.to_bits()) }
}

/// The models, loaded once (G0a). Every one of them is a CC0 `.glb` from Kenney in
/// `assets/models/`; what the game had before was Bevy's own primitives, and the swap changed no
/// component at all, because the look was already a **child** of the entity that carries them.
///
/// A headless run has no `Look`: the models are the renderer's business, the components are not,
/// and the checks are all about the components. Every spawn below takes `Option<&Look>` for
/// exactly that reason, the way `day_night` takes `Option<ResMut<GlobalAmbientLight>>`.
#[derive(Resource)]
struct Look {
    ground: Handle<Mesh>,
    turf: Handle<StandardMaterial>,
    tuft: Handle<WorldAsset>,
    bush: Handle<WorldAsset>,
    tree: Handle<WorldAsset>,
    rock: Handle<WorldAsset>,
    beetle: Handle<WorldAsset>,
    rabbit: Handle<WorldAsset>,
    /// the three clips of each animal, in one graph each
    beetle_gaits: Gaits,
    rabbit_gaits: Gaits,
    /// **How big each model is drawn, carried from [`Picture`]** (S5b-3). It rides here rather
    /// than being read where it is used because the four `spawn_*` helpers already take one
    /// `Option<&Look>` and a headless run has none — which is exactly the set of places a model
    /// scale is ever wanted. `make_look` copies the six in when it builds this.
    tuft_scale: f32,
    bush_scale: f32,
    tree_scale: f32,
    rock_scale: [f32; 2],
    beetle_scale: f32,
    rabbit_scale: f32,
    /// and the two the animation wants (`animate_creatures`)
    walking_at: f32,
    gait_blend_ms: u64,
}

/// **The sky's mesh, and what colour it is standing at (G8).**
///
/// The gradient lives in the mesh's `ATTRIBUTE_COLOR`, so changing the hour means rewriting 49
/// vertices. `up` is how far up the dome each of those vertices is, 0 at the rim and 1 overhead,
/// kept here rather than worked back out of the positions; `was` is what was written last, so a
/// frame in which the sky has not visibly moved writes nothing and the mesh is not sent to the
/// GPU again. In sixty seconds of a day the sky crosses about 1/255 of its range four times a
/// second, so this is a write every quarter of a second rather than every frame.
#[derive(Resource)]
struct SkyDome {
    mesh: Handle<Mesh>,
    up: Vec<f32>,
    was: Option<(LinearRgba, LinearRgba)>,
}

/// **The tinted copies of a model's materials (G8).**
///
/// A `.glb` loads once and every rabbit in the garden is another instance of that one
/// `WorldAsset`, so the `StandardMaterial` that arrives on a rabbit's mesh is **the asset the
/// loader made, shared by every rabbit and owned by the loader**. Colouring it in place would
/// work — every rabbit wants the same colour — but it would be writing into somebody else's
/// asset, and the moment two things want two colours out of one file it is wrong. So the material
/// is cloned, once, the first time a model of that species is ready, and every later instance is
/// handed the same clone. The key is the *source* material as well as the species, because a
/// model with two materials must end up with two clones and not one.
#[derive(Resource, Default)]
struct Tints(std::collections::HashMap<(usize, AssetId<StandardMaterial>), Handle<StandardMaterial>>);

/// One animal's walk, idle and eat, as nodes of an `AnimationGraph`. The Kenney "Cube Pets"
/// models carry eight clips each — `static`, `idle`, `walk`, `run`, `eat`, `dance` and two
/// gestures — and the garden uses three of them.
#[derive(Clone)]
struct Gaits {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    eat: AnimationNodeIndex,
}

impl Gaits {
    fn node(&self, gait: Gait) -> AnimationNodeIndex {
        match gait {
            Gait::Idle => self.idle,
            Gait::Walk => self.walk,
            Gait::Eat => self.eat,
        }
    }
}

/// Where the Ruby lives. `ruby/prelude.rb` goes in front of every creature's file.
#[derive(Resource)]
pub struct RubyDir(pub PathBuf);

/// **G4.** A creature file as the editor has it, per species: `None` is "whatever the file says".
///
/// This is where the garden and SabiRuby Battle come apart. There, a brain applied in the editor
/// is a field of the robot, because a robot is the thing that has a file. Here the file *is* the
/// species: every beetle in the world runs `beetle.rb`, so a text applied in the editor belongs
/// to the species, every beetle restarts on it, and a beetle born an hour later is born running
/// it. Nothing reaches the disk until Save (`platform::write`), so trying something on the
/// beetles does not rewrite the project.
///
/// **W3 gave it a third slot, and it is not a species.** `ruby/world.rb` is the rules, it is
/// edited in the same panel by the same three buttons, and what "applied, not saved" means about
/// it is word for word what it means about `beetle.rb` — so it is the same resource with one more
/// place in the array rather than a second one beside it. The two indices a species answers are
/// [`Species::index`]; the third is [`Brains::WORLD`], and nothing else may be there.
#[derive(Resource, Default)]
pub struct Brains {
    applied: [Option<String>; 3],
    /// **What every creature of a species is to be running now** (S7), per slot — the compiled
    /// program the last hand-over put there, and the generation number that says which hand-over
    /// it was. `None` in a slot is "nobody has been handed anything; whatever `give_mind`
    /// compiled when the creature was born is right".
    wearing: [Option<Wearing>; 3],
}

/// One hand-over: the program a species is to be wearing, and which hand-over it was.
///
/// **Why a generation number** (S7). `window::restart_species` goes round a `Query` and hands the
/// new program to every creature it can see — and it cannot see a creature whose `Mind` is still
/// in the `Commands` queue of the very frame Apply was pressed, which is what a creature born in
/// that frame is (`children_arrive` and `do_editor_actions` have no ordering edge between them,
/// so Bevy puts no sync point between them either). Such a creature was missed and went on
/// running the *old* program for the rest of the run, which is the bug S6 found and reproduced
/// (`docs/worklog/2026-09-20-window-check-flakes.md` §3).
///
/// A number the hand-over bumps and the creature carries turns "who was in the `Query`" into
/// "who is behind", which can be asked again on any later frame — so a creature that was in the
/// queue is caught on the next frame instead of never (`window::catch_up_minds`). It is a
/// counter and not a threshold: there is no number to choose here.
struct Wearing {
    handle: Handle<MrbAsset>,
    prelude_lines: u32,
    /// it came from the editor rather than from the file (`Mind::in_memory`)
    in_memory: bool,
    /// hand-overs so far, starting at 1. A `Mind` born before any of them carries 0.
    generation: u32,
}

impl Brains {
    /// Where the world's rules sit in `applied`: after the two species, whose slots are their own
    /// `index()`. It is also the id the editor's third button carries (`window::show_code`).
    pub const WORLD: usize = Species::ALL.len();

    /// Which hand-over this species is on. 0 until the first one.
    pub fn generation(&self, species: Species) -> u32 {
        self.wearing[species.index()].as_ref().map_or(0, |w| w.generation)
    }

    /// **A species has been handed a new program**: Apply, Revert, Save, or the file changing on
    /// disk. Returns the generation it is now on, which is what the creatures handed the program
    /// in this frame are stamped with.
    ///
    /// The world's slot has no hand-over of its own and none is missing: the rules are worn by
    /// one entity that is spawned at `Startup` and never again, so there is no `world.rb` script
    /// that can be in the command queue while the editor applies (`wear_the_rules`), and the
    /// timer tasks `every` made stop themselves in Ruby rather than being found by a `Query`
    /// (`ruby/world_prelude.rb`).
    fn hand_over(&mut self, species: Species, handle: Handle<MrbAsset>, prelude_lines: u32, in_memory: bool) -> u32 {
        let generation = self.generation(species) + 1;
        self.wearing[species.index()] = Some(Wearing { handle, prelude_lines, in_memory, generation });
        generation
    }

    /// The program this species is to be wearing, if it has been handed one.
    fn wearing(&self, species: Species) -> Option<&Wearing> {
        self.wearing[species.index()].as_ref()
    }

    /// **The first program of a species, compiled once** (S5b-4, from rubevy's R2).
    ///
    /// [`give_mind`] used to compile the species' file for **every creature it made** — six
    /// beetles at startup, and one more on every birth — and hand each of them its own
    /// `Handle<MrbAsset>`. rubevy's R2 made the *irep* one copy in the VM, but
    /// `Assets<MrbAsset>` still held the same bytes once per creature, and there is no reason
    /// for a program to be stored a dozen times because a dozen creatures are running it.
    ///
    /// So the first creature of a species puts what it compiled here, and every creature after
    /// it wears the same handle — which is what [`restart_species`](window::restart_species) has
    /// always done after an edit. It is **generation 0**: no hand-over has happened, nobody is
    /// behind, and `hand_over` still numbers the first edit 1.
    fn first_program(
        &mut self,
        species: Species,
        handle: Handle<MrbAsset>,
        prelude_lines: u32,
        in_memory: bool,
    ) {
        self.wearing[species.index()] = Some(Wearing { handle, prelude_lines, in_memory, generation: 0 });
    }

    pub fn text(&self, species: Species) -> Option<&String> {
        self.applied[species.index()].as_ref()
    }

    pub fn set(&mut self, species: Species, text: Option<String>) {
        self.applied[species.index()] = text;
    }

    /// Where that species' file lives.
    pub fn path(&self, ruby: &Path, species: Species) -> PathBuf {
        ruby.join("creatures").join(species.file())
    }

    /// The rules as the editor has them: `None` is "whatever `ruby/world.rb` says" (W3).
    pub fn world(&self) -> Option<&String> {
        self.applied[Brains::WORLD].as_ref()
    }

    pub fn set_world(&mut self, text: Option<String>) {
        self.applied[Brains::WORLD] = text;
    }

    /// Where the rules live. They are not under `creatures/`: a creature's file is one of a kind
    /// the game has two of, and this one is the garden's own.
    pub fn world_path(&self, ruby: &Path) -> PathBuf {
        ruby.join(WORLD_FILE)
    }
}

/// The rules' file name, in the one place both the loader and the editor read it from (W3).
pub const WORLD_FILE: &str = "world.rb";

/// What goes in front of a creature's file, and in front of the world's, in the one program each
/// is compiled as. They are named because the compiler's line numbers have to be told about them
/// (rubevy's [`in_the_authors_lines`]) as well as read from them.
pub const PRELUDE_FILE: &str = "prelude.rb";
pub const WORLD_PRELUDE_FILE: &str = "world_prelude.rb";

/// How many seeds `world.rb` asked for this frame (`garden.sprout`), read in `answer_world` and
/// put in the ground by `sprout_plants` a moment later.
///
/// The split is `Births`' below, and for the same reason: the answering system has the whole
/// world and no `Commands`, while putting a plant down wants `Look` and `Dice`, which are
/// ordinary system parameters. What the rule decided — *whether* there is room and *whether* the
/// dice came up — is `world.rb`'s; where the seed lands and what it is made of is the garden's
/// furniture and stayed here.
///
/// **And how far a new blade has to stand from the ones already up is the rules' too** (S5b-4).
/// It was the one number of the grass that W1 left behind in this source, because the thing it
/// is used for is a walk over every plant in the field and that walk is Rust's. The number
/// crosses with the others, once, in `garden.rules(sprout_gap:)`; the walk stays here. It is
/// kept in this resource rather than in one of its own because this is the resource
/// [`sprout_plants`] already reads — numbers live where they are used, which is [`Births`]'
/// arrangement.
#[derive(Resource)]
struct Sprouts {
    /// asked for this frame, not yet in the ground
    asked: u32,
    /// how close to another blade a new one may not be put (`world.rb`'s `sprout_gap`)
    gap: f32,
}

impl Default for Sprouts {
    /// The game's own, for the frames before `world.rb` has spoken and for a garden whose
    /// `world.rb` will not compile ([`SPROUT_GAP`]).
    fn default() -> Self {
        Sprouts { asked: 0, gap: SPROUT_GAP }
    }
}

/// How far a new blade of grass has to stand from every blade already up, until `world.rb` says
/// otherwise with `garden.rules(sprout_gap:)`.
///
/// It is the **stand-in** and nothing else — the number itself is `ruby/world.rb`'s, on the line
/// named beside it there, and a test holds the two together
/// (`the_stand_ins_are_what_world_rb_says`). **Source: unknown**: 1.5 was written into
/// `sprout_plants` when the grass was still Rust's and no record says why that far.
const SPROUT_GAP: f32 = 1.5;

/// **What one pass of `world.rb`'s `each_frame` costs** (W1), and the wall time this VM's tick
/// took — the two numbers `ScriptWorld<World>`'s `budget` and `frame_time` were chosen from.
///
/// The world's script is one task that waits on one `Rubevy.ask("frame")` and therefore runs
/// **exactly one pass per frame**, so the instructions it ran between two frames *are* a pass:
/// there is nothing to separate out and no gap to interpret, which is what makes this a simpler
/// measurement than the creatures' `insn/decision` (`watch_minds`).
#[derive(Resource, Default)]
struct WorldMeter {
    /// `ScriptStats::instructions` at the end of the last frame
    last_instructions: u64,
    /// **what the rules cost in each frame they ran in at all** — one entry per such frame, and
    /// the name says that rather than "passes" (S5b-5).
    ///
    /// The two are the same number for the rules as they are written: the script waits on
    /// `Rubevy.ask("frame")`, which is the only thing it asks that costs a frame, so one frame is
    /// one pass of `each_frame` and a run counts 5,372 of them in 5,373 frames (`docs/garden.md`).
    /// But that is a fact about *these* rules and not about the measurement: a `world.rb` that
    /// asks two questions costing a frame, or one whose pass does not finish inside a frame,
    /// would still make one entry here per frame it ran in. The old name claimed the fact the
    /// rules happen to have; this one claims what is counted.
    ran_in: Vec<u64>,
    /// the last of them, which is the figure the HUD draws (W3). The list above is for choosing a
    /// budget after the run; this is what the rules cost *now*, beside what the creatures cost
    /// now, which is the comparison the panel is for.
    last_pass: u64,
    /// the biggest of them, kept here rather than worked out twice
    most: u64,
    /// milliseconds `RubevySet::<World>::tick()` took, this frame and smoothed
    started: Option<bevy::platform::time::Instant>,
    spent_ms: f32,
    mean_ms: f32,
    most_ms: f32,
    /// and every frame's, so that a **budget** can be chosen from the shape of the thing rather
    /// than from its two ends: a mean says nothing about how often the worst happens, and the
    /// worst on its own is usually the first frame
    times: Vec<f32>,
}

impl WorldMeter {
    /// The middle frame's worth. `None` until one has been measured.
    fn median(&self) -> Option<u64> {
        if self.ran_in.is_empty() {
            return None;
        }
        let mut sorted = self.ran_in.clone();
        sorted.sort_unstable();
        Some(sorted[sorted.len() / 2])
    }

    /// The tick time at the middle and at the 99th frame in a hundred. The second is the number a
    /// `frame_time` has to clear: a budget that bites once in a hundred frames is a rule that runs
    /// at half speed twice a second.
    fn milliseconds(&self) -> (f32, f32) {
        if self.times.is_empty() {
            return (f32::NAN, f32::NAN);
        }
        let mut sorted = self.times.clone();
        sorted.sort_by(f32::total_cmp);
        let at = |q: f32| sorted[((sorted.len() as f32 * q) as usize).min(sorted.len() - 1)];
        (at(0.5), at(0.99))
    }
}

/// Why the world has no rules, where it has none: `ruby/world.rb` would not compile, or would not
/// be read.
///
/// A garden whose rules will not compile **runs anyway** — the grass stops growing and nobody gets
/// hungry, and everything that is still Rust (the sun, the walking, the pushing apart) carries on
/// — and this is the sentence that says so, on the HUD and in the log. Refusing to start would be
/// the wrong answer to a typo in a file the player is invited to edit.
#[derive(Resource, Default)]
pub struct WorldTrouble(pub Option<String>);

/// **The game's side of making a creature**: the children a script has asked for this frame
/// (`garden.spawn`), read out of the Ruby Hash in `answer_garden`, and the two numbers the
/// building of one needs.
///
/// The asking and the making are separate because the answering system has the whole `World` and
/// no `Commands`, while spawning a creature wants `Look`, `RubyDir` and `Assets<MrbAsset>` —
/// three ordinary system parameters. Reading the Hash needs the VM; making the creature does not.
///
/// `hunger` and `cap` are `world.rb`'s, handed over once by `garden.rules` (W2) and living here
/// rather than in a `Rules` resource of their own, **where they are used** — which is the same
/// arrangement `day_length` has in [`Sky`], for the same reason (`docs/plans/garden-world-plan.md`
/// §2, default 7). What the rules keep for themselves is who may breed with whom and what it
/// costs them; what crosses is what the *builder* cannot do without.
#[derive(Resource)]
struct Births {
    /// asked for, not yet made
    waiting: Vec<Birth>,
    /// what a newborn's meter says when it arrives (`world.rb`'s `child_hunger`)
    hunger: f32,
    /// how many creatures the game will make (`world.rb`'s `pop_max`)
    cap: usize,
}

impl Default for Births {
    /// The game's own answers, for the frames before `world.rb` has spoken — and for a garden
    /// whose `world.rb` will not compile, which runs on them for ever.
    fn default() -> Self {
        Births { waiting: Vec::new(), hunger: CHILD_HUNGER, cap: POP_MAX }
    }
}

/// **The two distances the rules keep, for the one thing the game builds out of them**
/// (2026-09-18): the selftest's meadow corner.
///
/// It is [`Births`]'s arrangement — `world.rb`'s numbers, handed over once by `garden.rules`, kept
/// where they are used — with one difference that matters to the schedule. `hunger` and `cap` are
/// wanted whenever a child arrives, which is minutes into a run; these two are wanted **once**,
/// while the corner is being built, and the corner used to be built in `spawn_world`, which is a
/// `Startup` system. The rules cannot have spoken by then: the world's script becomes a task in
/// the first `Update` and `run_world` asks `garden.rules` in that task's first line. So the corner
/// waits for this ([`plant_the_meadow`]) instead of being planted before the world's VM has drawn
/// breath.
#[derive(Resource)]
struct Reaches {
    /// how far from a blade a creature may stand and still eat it, before the blade's own size is
    /// added (`world.rb`'s `reach`)
    eat: f32,
    /// how close a rabbit has to come to a beetle for the beetle to be told (`world.rb`'s
    /// `touch_reach`). **Wanted every frame**, by [`startle`], which is why it is read out of a
    /// resource here rather than asked of the VM: the rules say the number once and the game
    /// does the sweeping (S5b-4).
    touch: f32,
    /// how close two well-fed creatures have to be to be told about each other (`world.rb`'s
    /// `mate_reach`)
    mate: f32,
    /// whether `world.rb` has said. Until it has, the three above are the game's own — and a
    /// `world.rb` that will not compile leaves them on the game's own for ever, which is the case
    /// [`plant_the_meadow`] has to notice so that the corner is planted at all.
    told: bool,
}

impl Default for Reaches {
    fn default() -> Self {
        Reaches { eat: REACH, touch: TOUCH_REACH, mate: MATE_REACH, told: false }
    }
}

/// **How wide the things in the garden are** — `world.rb`'s numbers, handed over once by
/// `garden.rules`, and the only place the game asks how big a body is (S5b-4).
///
/// It is [`Reaches`]' arrangement with one thing more to do. A reach is read where it is used and
/// that is the whole of it; a radius is also **written onto an entity** — `Collider` is what
/// `separate` walks and what the fourth check measures — so a handover has to reach the bodies
/// that are already standing about. [`bodies_wear_the_rules`] is that, and it is
/// `plants_wear_their_size` for creatures: one number, seen in two places, copied by the game
/// rather than kept in step by hand.
///
/// So editing `beetle_radius` in the editor and pressing Ctrl+Enter widens every beetle in the
/// garden on the next frame, which is what putting a number in a file a player can edit is for.
/// What it does **not** move is where the garden was laid out: `spawn_world` is a `Startup`
/// system and scatters trees and creatures with the stand-ins below, an hour of play before the
/// first blade is eaten in any case.
#[derive(Resource)]
struct Bodies {
    beetle: f32,
    rabbit: f32,
    tree: f32,
    rock: f32,
}

impl Default for Bodies {
    /// The game's own, for the frames before `world.rb` has spoken and for a garden whose
    /// `world.rb` will not compile.
    fn default() -> Self {
        Bodies { beetle: BEETLE_RADIUS, rabbit: RABBIT_RADIUS, tree: TREE_RADIUS, rock: ROCK_RADIUS }
    }
}

impl Bodies {
    fn of(&self, species: Species) -> f32 {
        match species {
            Species::Beetle => self.beetle,
            Species::Rabbit => self.rabbit,
        }
    }

    /// The side of one cell of `separate`'s neighbour grid: [`CELL`], or twice the largest body,
    /// whichever is more. **Derived, not chosen** — a cell narrower than a body's width is a pair
    /// that touches and is sorted into cells that never look at each other, which is what the
    /// comment on [`CELL`] has always said the number was for. With the rules as they ship this
    /// answers [`CELL`] exactly.
    fn cell(&self) -> f32 {
        let widest = self.beetle.max(self.rabbit).max(self.tree).max(self.rock);
        CELL.max(2.0 * widest)
    }
}

/// **The creatures that have been born and cannot hear yet** (2026-09-18).
///
/// A child's body exists from the frame `children_arrive` makes it, and its script cannot hear
/// anything for two more frames ([`NEWBORN_DEAF_FRAMES`]). Anything the world said in between is
/// lost to it — and the world says `"night"` and `"day"` once each, at the turn, so a creature
/// born into the night was awake until the next morning and one born in the frame of the publish
/// was counted among its addressees without having subscribed.
///
/// So each newborn waits here for its two frames and is then told what the sky is doing, on its
/// own. It is a list rather than a component because what is waited for is frames of the
/// creatures' VM, which is the thing the list is counted in.
///
/// **A creature read out of a save file is one of these too** (2026-09-18). `load_world` builds a
/// body and calls `give_mind`, exactly as `children_arrive` does, and the script it hangs there
/// is exactly as deaf — so a garden saved at night and opened again had every creature walking
/// about until morning. It goes on the same list and is told by the same system; it is not a
/// newborn in the world's terms (`Creature::age` is the age it was saved with, and the rules do
/// not treat it as a child) but it is a newborn in the only sense this list means: a script that
/// has not heard anything yet.
#[derive(Resource, Default)]
struct Newborns {
    /// the creature, and how many frames it has waited
    waiting: Vec<(Entity, u32)>,
}

/// **What the run has to say about its two VMs, as one system parameter.**
///
/// `stop_when_over` was at Bevy's limit of sixteen system parameters before W1 added a second VM
/// to report on, and a `SystemParam` of its own is the answer Bevy gives to that: one parameter
/// that expands to five, named for the reason the five travel together.
#[derive(bevy::ecs::system::SystemParam)]
struct VmReport<'w> {
    /// the creatures' VM: what its tick cost this frame, and the VM itself for the panel
    clock: Res<'w, window::VmClock>,
    scripts: Res<'w, ScriptWorld>,
    /// and the world's: what a pass of `each_frame` cost, why it has no rules where it has none,
    /// and the share of the frame it is allowed
    meter: Res<'w, WorldMeter>,
    trouble: Res<'w, WorldTrouble>,
    world: Res<'w, ScriptWorld<World>>,
    /// and what the world's own program has in front of it, so the panel's lines are `world.rb`'s
    prelude: Res<'w, WorldPrelude>,
}

/// `--headless N`: how long the world may run.
#[derive(Resource)]
struct Headless {
    until: f32,
}

/// Where a saved garden goes and comes from: `--save PATH`, or `platform::SAVE_FILE` for F5 in
/// the window. On the web the "path" is a `localStorage` key (`platform.rs`).
#[derive(Resource)]
struct SaveFile {
    path: String,
    /// `--save` was asked for on the command line, so a headless run writes one when it ends
    on_exit: bool,
}

/// Somebody asked for a save this frame: F5, or the end of a `--save` run. A flag rather than a
/// call, because the thing that asks (a key, the system that ends the run) has none of the twenty
/// queries the save itself wants.
#[derive(Resource, Default)]
struct SaveNow(bool);

/// The last thing a save or a load said, for the HUD to show (G5).
///
/// A headless run has the log and nothing else, and a page has a console nobody opens. The window
/// had neither: F5 wrote a line into stdout that the player never sees. A refused version is the
/// reason this exists now — "your garden was saved by another build" has to reach the person
/// whose garden it was — but the successes go through it too, because a line that only ever
/// appears when something is wrong is a line nobody reads in time.
#[derive(Resource, Default)]
struct SaveNote {
    text: String,
    /// the world clock when it was said, so the HUD can let it fade
    at: f32,
    /// nothing was written or read: the HUD says it in amber
    bad: bool,
}

impl SaveNote {
    fn say(&mut self, at: f32, bad: bool, text: String) {
        self.text = text;
        self.at = at;
        self.bad = bad;
    }
}

/// What the HUD's Save and Load buttons asked for (G5).
///
/// The buttons cannot do the work themselves: `draw_hud` runs in egui's own schedule, after the
/// frame's systems, and a load has to happen before rubevy looks at the world. So they leave a
/// flag that `save_load_keys` reads on the next frame, exactly as if the key had been pressed
/// then — a sixtieth of a second nobody can see, and one code path for a key and a button.
#[derive(Resource, Default)]
struct Asked {
    save: bool,
    load: bool,
}

/// A garden read out of a file and not yet built. `--load PATH` puts one here before the first
/// frame; F9 puts one here at any time, and `load_world` does the same thing in both cases —
/// which is why there is no second code path for "load while the world is running".
#[derive(Resource)]
struct Loading {
    save: GardenSave,
}

/// `GARDEN_RELOAD_AT=SECONDS` (checks only): the `--load` file goes in through F9's door.
///
/// `--load PATH` reads its file before the first frame, and F9 reads one into a garden that has
/// been running for minutes. They end in the same `load_world`, but they are not the same thing
/// to the *creatures*: the first builds minds that have never run, the second replaces minds that
/// have. G5 found that only the second one lost the memories it read (`11 creatures never
/// started; the garden is running anyway`), and a headless run could not reach it, because
/// `MinimalPlugins` has no keyboard to press F9 with. So this defers the `--load`: the garden is
/// built new at t=0, lives its own life for `at` seconds, and only then opens the file — which is
/// F9 exactly, in a run that a shell can drive.
///
/// `restored` is what lets `--headless N` with `GARDEN_RELOAD_AT=N` write a file that can be
/// compared byte for byte with the one it read: `stop_when_over` will not end the run until the
/// deferred load has put every memory back, and it is set in `restore_memory`, which runs just
/// before `stop_when_over` on that same frame — so the save is of the world as it was read, with
/// no frame of walking between.
#[derive(Resource)]
struct ReloadAt {
    at: f32,
    path: String,
    asked: bool,
    restored: bool,
}

/// A loaded world whose minds are still starting.
///
/// A creature's `@memory` cannot be put back until there is something to put it on: the Hash
/// belongs to the object the script makes in `run_creature`, which does not exist until rubevy
/// has turned the `Script` into a task and that task has run its first instructions. That is a
/// frame or two, and **the world does not age through them** — the rules do not run and the
/// clock does not move (`Sky::shift` is held), so a garden that is read back is the garden that
/// was written down, not that garden plus two frames of walking.
#[derive(Resource)]
struct Restoring {
    /// the creatures whose memory has not been handed over yet
    pending: Vec<(Entity, serde_json::Value)>,
    /// the world's clock, held while this lasts
    tick: f32,
    /// real time when the load happened, so that a script that never starts cannot freeze the
    /// world for ever
    since: f32,
    /// every memory is in place (or has been given up on); the world may move again at the end of
    /// this frame
    done: bool,
}

/// How long a load waits for a script that is not starting before it gives up and lets the world
/// run. A creature whose file does not compile has no task at all, and the garden should not stop
/// for it.
const RESTORE_PATIENCE: f32 = 5.0;

/// `--shot FILE [SECONDS]`: where the picture goes, and when.
#[derive(Resource)]
struct Shot {
    path: String,
    after: f32,
    taken: bool,
}

/// Where the camera stands: three numbers for the look from above, and — since G6 — the point on
/// the ground it is looking *at*, which is what panning moves.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
struct Orbit {
    yaw: f32,
    pitch: f32,
    distance: f32,
    /// The spot on the ground the camera turns around and points at, on XZ. `Home` puts it back.
    focus: Vec2,
}

impl Default for Orbit {
    fn default() -> Self {
        Orbit { yaw: 0.0, pitch: PITCH, distance: DISTANCE, focus: Vec2::ZERO }
    }
}

impl Orbit {
    /// Where `Home` puts the camera back to — the setting's default rather than the constant's,
    /// so that an eye moved in `garden.settings.txt` is the eye `Home` returns to.
    fn home(eye: &Eye) -> Orbit {
        Orbit { yaw: 0.0, pitch: eye.pitch, distance: eye.distance, focus: Vec2::ZERO }
    }
}

/// Where the camera stands when nobody has said otherwise. **Quoted** for the distance (not for
/// the number, for its effect): `--eye`'s note says "at the default 42 a beetle is thirty pixels
/// across in a 1600-wide window". The pitch is quoted the same way — [`FOG_DEPTH`] was measured
/// at "42 units out and 49° down", so the fog's measurement rests on this angle and moving it
/// makes that measurement one about a different picture.
const PITCH: f32 = 0.85;
const DISTANCE: f32 = 42.0;
/// How far a pixel of drag turns the camera, and how far up and down the pitch may go.
/// **Source unknown**, all three.
const TURN_PER_PIXEL: f32 = 0.005;
const PITCH_MIN: f32 = 0.12;
const PITCH_MAX: f32 = 1.45;

/// **The wheel (G6).** The author played the browser build and found the wheel had two steps in
/// it: all the way in, all the way out. The reason is in the unit a wheel message carries.
///
/// A mouse on a PC sends *lines* — winit hands Bevy `MouseScrollUnit::Line` with `y = ±1.0` per
/// notch — and a browser sends *pixels*: a `wheel` event's `deltaY` is how far the page would
/// scroll, and one notch of a real mouse in Chromium is 100 of them (Firefox sends 3 lines, and
/// winit turns that into `Line` again). The old code was `distance -= y * 2.0`, so one notch was
/// two units on a PC and a hundred on a page — and the range is 12 to 90 units wide, which a
/// hundred crosses in one turn of the finger. Two steps.
///
/// So a message is first turned into **notches** ([`notches_of`]), and the notches are then a
/// *ratio* rather than a subtraction ([`zoom_by`]): ten per cent nearer per notch, which is the
/// same felt step at 8 units as at 80 — the thing a subtraction cannot be. Thirty notches cross
/// the whole range either way, in both builds — about ten flicks of a finger, where it used to be
/// one on a page and forty on a PC.
const ZOOM_PER_NOTCH: f32 = 1.10;
/// One notch of a wheel, in the pixels a browser measures one in.
const PIXELS_PER_NOTCH: f32 = 100.0;
/// How close and how far the camera may get. Six units is a creature filling a third of the
/// window; a hundred and ten has the whole forty-by-thirty field and its walls in view.
const ZOOM_MIN: f32 = 6.0;
const ZOOM_MAX: f32 = 110.0;
/// A pixel of drag, in world units per unit of distance: panning feels the same however close in
/// the camera is, which it would not if the ground moved a fixed number of units per pixel.
const PAN_PER_PIXEL: f32 = 0.0016;
/// Arrow keys and WASD, in world units per second per unit of distance.
const PAN_PER_SECOND: f32 = 0.9;
/// How far past the wall the eye may wander before it is stopped.
const PAN_LIMIT: f32 = 8.0;

/// **The eye, as a setting** (S5b-3): where the camera starts, how far it may go, and how fast
/// the mouse and the keys move it.
///
/// This is the garden's 3D orbit and it has nothing to do with `games_shell::CameraControls`,
/// which is the shared crate's pan-and-zoom 2D camera and is what Battle uses. The two hold some
/// of the same numbers because S3 copied the reasoning out of here when it wrote that one; they
/// are not the same camera, and the keys are `eye_*` rather than `camera_*` so that a reader of
/// a `garden.settings.txt` is never in doubt about which of the two a line is addressed to.
///
/// [`PIXELS_PER_NOTCH`] is **not** here: it is not a preference but a fact about Chromium, and a
/// build that read it from a file would let somebody make the browser's wheel disagree with the
/// browser.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Eye {
    /// [`PITCH`] / [`DISTANCE`] — where the camera starts and what `Home` puts it back to
    pub pitch: f32,
    pub distance: f32,
    /// [`ZOOM_PER_NOTCH`] / [`ZOOM_MIN`] / [`ZOOM_MAX`]
    pub zoom_per_notch: f32,
    pub zoom_min: f32,
    pub zoom_max: f32,
    /// [`PAN_PER_PIXEL`] / [`PAN_PER_SECOND`] / [`PAN_LIMIT`]
    pub pan_per_pixel: f32,
    pub pan_per_second: f32,
    pub pan_limit: f32,
    /// [`TURN_PER_PIXEL`] / [`PITCH_MIN`] / [`PITCH_MAX`]
    pub turn_per_pixel: f32,
    pub pitch_min: f32,
    pub pitch_max: f32,
    /// [`window::CLICK_REACH`] / [`window::CLICK_SLOP`] — picking a creature, which is the mouse
    /// too
    pub click_reach: f32,
    pub click_slop: f32,
}

impl Default for Eye {
    fn default() -> Self {
        Eye {
            pitch: PITCH,
            distance: DISTANCE,
            zoom_per_notch: ZOOM_PER_NOTCH,
            zoom_min: ZOOM_MIN,
            zoom_max: ZOOM_MAX,
            pan_per_pixel: PAN_PER_PIXEL,
            pan_per_second: PAN_PER_SECOND,
            pan_limit: PAN_LIMIT,
            turn_per_pixel: TURN_PER_PIXEL,
            pitch_min: PITCH_MIN,
            pitch_max: PITCH_MAX,
            click_reach: window::CLICK_REACH,
            click_slop: window::CLICK_SLOP,
        }
    }
}

impl Eye {
    /// `eye_pitch`, `eye_distance`, `eye_zoom_per_notch`, `eye_zoom_min`, `eye_zoom_max`,
    /// `eye_pan_per_pixel`, `eye_pan_per_second`, `eye_pan_limit`, `eye_turn_per_pixel`,
    /// `eye_pitch_min`, `eye_pitch_max`, `eye_click_reach`, `eye_click_slop`.
    ///
    /// `--eye UNITS` still wins over `eye_distance` for the run it is given on, the way
    /// `--headless N` wins over `headless_seconds`.
    fn read_from(&mut self, settings: &games_shell::Settings) {
        take(settings, "eye_pitch", &mut self.pitch);
        take(settings, "eye_distance", &mut self.distance);
        take(settings, "eye_zoom_per_notch", &mut self.zoom_per_notch);
        take(settings, "eye_zoom_min", &mut self.zoom_min);
        take(settings, "eye_zoom_max", &mut self.zoom_max);
        take(settings, "eye_pan_per_pixel", &mut self.pan_per_pixel);
        take(settings, "eye_pan_per_second", &mut self.pan_per_second);
        take(settings, "eye_pan_limit", &mut self.pan_limit);
        take(settings, "eye_turn_per_pixel", &mut self.turn_per_pixel);
        take(settings, "eye_pitch_min", &mut self.pitch_min);
        take(settings, "eye_pitch_max", &mut self.pitch_max);
        take(settings, "eye_click_reach", &mut self.click_reach);
        take(settings, "eye_click_slop", &mut self.click_slop);
    }
}

/// One wheel message as a number of notches, whatever unit it arrived in.
fn notches_of(unit: bevy::input::mouse::MouseScrollUnit, y: f32) -> f32 {
    use bevy::input::mouse::MouseScrollUnit;
    match unit {
        MouseScrollUnit::Line => y,
        MouseScrollUnit::Pixel => y / PIXELS_PER_NOTCH,
    }
}

/// `notches` notches of wheel from `distance`, as a ratio, kept inside the range.
fn zoom_by(eye: &Eye, distance: f32, notches: f32) -> f32 {
    (distance * eye.zoom_per_notch.powf(-notches)).clamp(eye.zoom_min, eye.zoom_max)
}

/// One `"touched"` the sixth check is watching, and what the half second after it has shown.
///
/// The check used to need three fields and one look; it needs these because it now watches the
/// **whole window** rather than its far end (`watch_turning`).
#[derive(Debug, Clone, Copy)]
struct Touch {
    beetle: Entity,
    /// when the game published `"touched"` to it
    at: f32,
    /// the heading it had at that moment
    was: Vec2,
    /// whether any frame since has shown a heading a quarter turn or more off `was`
    turned: bool,
    /// the closest any frame came to that, and when — a miss has to be able to say how close it
    /// got, or "it never turned" is a claim with no size to it
    closest: f32,
    closest_at: f32,
    /// frames inside the window where the beetle was there and readable (not against a wall)
    looked: u32,
    /// what it was doing at the last readable frame, for the miss line
    last: Vec2,
}

/// `GARDEN_SELFTEST=1`: what the plan asks the world to prove about itself — G0's four things
/// about the rules, and G1's three about the minds.
#[derive(Resource)]
struct SelfTest {
    /// when somebody first ate
    ate_at: Option<f32>,
    /// when night first fell
    night_at: Option<f32>,
    /// the creature that starved, and when
    starved: Option<(Entity, f32)>,
    /// the closest two colliders ever came, as a fraction of the sum of their radii: 1.0 is
    /// touching, below 0.9 is the check failing
    closest: f32,
    /// how many frames had a pair below 0.9, and how many frames were looked at
    overlaps: u32,
    frames: u32,

    // --- G1: the minds -----------------------------------------------------
    /// how far the probe beetle started from the plant it was given, and how close it ever got
    probe_from: f32,
    probe_closest: f32,
    /// when it got there, if it did
    probe_reached: Option<f32>,
    /// when a rabbit first walked into the probe while it was still on its way, if one did. A
    /// run where that happened has not measured this check either way (`watch_probe`)
    probe_disturbed: Option<f32>,
    /// beetles that were touched by a rabbit while walking, and what has been seen of each since
    touched: Vec<Touch>,
    /// when each beetle was last put on that list, so that a beetle a rabbit keeps walking into
    /// is looked at once rather than five times: the handler takes half a second to run and the
    /// second message waits in the queue behind the first, so the answer to "did it turn?" for
    /// touch four is about touch one
    last_touch: Vec<(Entity, f32)>,
    /// how many of those were still there to look at half a second later, and how many had turned
    turn_checked: u32,
    turned: u32,
    /// the fastest anybody moved in the second after night fell, and how many creatures were
    /// looked at in that frame
    asleep_at: Option<f32>,
    awake_speed: f32,
    asleep_counted: u32,

    // --- G2: the genome ----------------------------------------------------
    /// every pairing a child was asked out of: who asked, its own genome, its partner's, when.
    ///
    /// W2 moved where this is written rather than what it says. `court` filled it, because
    /// `court` was the rule and knew both parents; the rule is `ruby/world.rb`'s now and reports
    /// nothing, so it is taken in `answer_spawn` out of the asker's `Creature` and the `Breeding`
    /// the rules wrote on it — the check watches the world instead of being told.
    /// **and, since S5b-4, the rate its file mutates at**, read out of the VM in that same
    /// frame ([`mutation_rate_of`]): the file may be edited between a child being asked for and
    /// the check printing its line, and what the child was made by is the file as it was.
    matings: Vec<(Entity, Genome, Genome, f32, Option<f32>)>,
    /// how many of the rules' `"mate"`s the check may be judged on, and how many children came
    /// back. The first is counted where the message is carried across to the creatures' VM
    /// (`answer_world`), and it is the only thing left in this source that knows that name — for
    /// the sake of the line the eighth check prints when no child was born at all, which would
    /// otherwise not be able to say whether the rules had been silent or the scripts had.
    ///
    /// **It is the pairings that could have produced a child**, not every pairing the rules made
    /// (2026-09-18). Three kinds never could, and none of them is a rule that is broken:
    ///
    ///  * a pairing of a species whose file has no `on(:mate)` — the rabbit's — where the
    ///    message reaches nobody. `world.rb` pairs by species and says so in as many words:
    ///    "whether a creature does anything at all with the message is its own script's
    ///    business". Such a pairing is never counted (see [`listens_for`]).
    ///  * a pairing where one of the two was gone before the one that was told could ask for a
    ///    child. Those are counted, and taken back out again when it happens
    ///    ([`close_courtings`]) — which is `courtings_lost`, kept only for the sentence the
    ///    check prints.
    ///  * a pairing put to a creature that was asleep when the rules spoke. `beetle.rb`'s
    ///    `on(:mate)` is `next if @asleep` before it is anything else, so a courtship in the
    ///    night ends where it starts. Never counted (see [`asleep_now`]), and added to
    ///    `courtings_lost` for the same sentence.
    courtings: u32,
    /// the pairings that were not measured — asleep when they were told ([`asleep_now`]), or one
    /// of the two gone before the handler could ask ([`close_courtings`]). It is in no
    /// denominator; it is there so the check can say *why* it measured nothing
    courtings_lost: u32,
    /// the pairings that have been counted and are still waiting to be measured: who was told,
    /// who it was told about, and when. An entry leaves when that creature asks the game for a
    /// child — the road the check is about was walked — or when either of the two is gone before
    /// it could ([`close_courtings`]).
    courtings_open: Vec<(Entity, Entity, f32)>,
    births: u32,
    /// the first child a script asked the game for: when it was asked for, and what the check
    /// below made of its three genes against its parents'
    born_at: Option<f32>,
    born_says: String,
    born_ok: bool,

    // --- G3: the save file -------------------------------------------------
    /// what the game told the tester script when it asked for a creature whose genome is missing
    /// a gene: serde's message, as the script heard it
    bad_spawn: Option<String>,

    // --- G5: the save's version ---------------------------------------------
    /// why the save from another version was not loaded, as `--load` said it. `None` means it was
    /// loaded — or that this run was given a `--load` of its own and the check did not run
    version_refused: Option<String>,

    // --- W1: the rules are Ruby's ---------------------------------------------
    /// the first moment a plant was seen to be **bigger** than it was the frame before, which is
    /// `world.rb`'s `each_frame` having run and having written what it worked out
    grew_at: Option<f32>,
    /// what every plant measured last frame, for the line above
    grass: Vec<(Entity, f32)>,
    /// what every creature's meter said last frame, so that a rise is a meal and a fall is
    /// hunger. It stands where `eat` and `starve` used to write `ate_at` and `starved` from the
    /// inside: the rules are somebody else's now, and a check of a rule that is somebody else's
    /// has to watch what it **did to the world**
    meters: Vec<(Entity, f32)>,
    /// every creature that was there last frame, for the same reason: `starve` used to say who it
    /// despawned and now nothing here decides that
    alive: Vec<(Entity, Species, f32)>,

    // --- W1: the rules can be swapped while the world runs --------------------
    /// the frozen rules (an `each_frame` that does nothing) have been asked for
    freeze_asked: bool,
    /// …and the world's task has actually restarted on them, at this moment
    frozen_at: Option<f32>,
    /// how many times a creature's meter fell while they were in force. It should be none: the
    /// rules that make a creature hungry are in the file that was taken away
    fell_while_frozen: u32,
    // --- W2: the world says something and the creatures hear it --------------
    /// the rules have said `"season"` at least once, which is when it is worth reading anybody's
    /// memory (a `read_memory` per creature is two `ivar_get`s and a conversion, and doing it on
    /// every frame of a ninety-second run to find out that nothing has been said yet would be
    /// the check costing more than the thing it checks)
    season_told: bool,
    /// the first creature whose `@memory` came back with a season in it, what it said, and when
    season_heard: Option<(String, String, f32)>,
    /// the real rules have been asked for again
    thaw_asked: bool,
    /// …and a meter has fallen since, which is the second half of the check: rules that stop when
    /// they are replaced and never start again are not rules that are alive
    fell_after_thaw: bool,
}

impl Default for SelfTest {
    fn default() -> Self {
        // the minima start where nothing can be worse; `awake_speed` is a maximum
        SelfTest {
            ate_at: None,
            night_at: None,
            starved: None,
            closest: f32::INFINITY,
            overlaps: 0,
            frames: 0,
            probe_from: 0.0,
            probe_closest: f32::INFINITY,
            probe_reached: None,
            probe_disturbed: None,
            touched: Vec::new(),
            last_touch: Vec::new(),
            turn_checked: 0,
            turned: 0,
            asleep_at: None,
            awake_speed: 0.0,
            asleep_counted: 0,
            matings: Vec::new(),
            courtings: 0,
            courtings_lost: 0,
            courtings_open: Vec::new(),
            births: 0,
            born_at: None,
            born_says: String::new(),
            born_ok: false,
            bad_spawn: None,
            version_refused: None,
            grew_at: None,
            grass: Vec::new(),
            meters: Vec::new(),
            alive: Vec::new(),
            freeze_asked: false,
            frozen_at: None,
            fell_while_frozen: 0,
            season_told: false,
            season_heard: None,
            thaw_asked: false,
            fell_after_thaw: false,
        }
    }
}

/// **Check 12's rules**: a `world.rb` that does nothing at all.
///
/// It is the smallest thing that is still a world — it compiles, it defines an `each_frame`, and
/// that `each_frame` is empty — so the garden it leaves behind is one where the grass does not
/// grow, nobody gets hungry, nobody eats, nobody breeds and nobody starves, while everything
/// that is still Rust (the sun, the walking, the pushing apart, a rabbit startling a beetle)
/// carries on exactly as before. That is what makes "no meter fell for five seconds" a statement
/// about *these* rules and not about the world having stopped.
///
/// W2 made that sentence simpler rather than harder. While breeding was Rust's, taking the rules
/// away made it happen *more* — nobody was getting hungry, so everybody stayed over
/// `MATE_HUNGER` — and every child charged its parents thirty points, which is a meter falling
/// for a reason the check had to be told to ignore (`SelfTest::paid`, now gone). Breeding is in
/// the file that is taken away, so there is nothing left in this build that can lower a meter
/// while it is away.
const FROZEN_WORLD: &str = r#"world do
  day_length 60.0
  each_frame do |dt|
  end
end
"#;

// ---------------------------------------------------------------------------------------------

/// How long `--headless` runs when it is given no number, and what `--shot` writes to and waits
/// for when it is given neither. **They are the values that were written into `main` here before
/// S1**, moved out only because the parsing they were in is `games_shell::Args`' now and the
/// Battle's `--shot` waits a different three seconds; no run's behaviour turns on them, since
/// every line in `docs/garden.md` passes its own number.
const HEADLESS_SECONDS: f32 = 10.0;
const SHOT_FILE: &str = "shot.png";
const SHOT_SECONDS: f32 = 6.0;

/// **What the two VMs are allowed in a frame, and the three other numbers about how a run keeps
/// time** (S5b-3).
///
/// The garden runs two VMs — `ruby/world.rb` in one and every creature's script in the other —
/// and until S5b-3 only one of them had a number anybody could point at. See [`WORLD_BUDGET`] and
/// [`CREATURE_BUDGET`] for where each came from; the short of it is that **both are measured at
/// the garden's own caps now** — the creatures' was rubevy's default until S5b-5 — and that both
/// are carried here under the game's own name so that they can be said out loud and so that
/// `window::scheduler_frames` can be worked out from one of them instead of writing the number a
/// second time.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Budgets {
    /// [`WORLD_BUDGET`] / [`WORLD_FRAME_TIME_MS`]
    pub world: u64,
    pub world_frame_time_ms: f32,
    /// [`CREATURE_BUDGET`] / [`CREATURE_FRAME_TIME_MS`]
    pub creature: u64,
    pub creature_frame_time_ms: f32,
    /// [`RESTORE_PATIENCE`]
    pub restore_patience: f32,
    /// [`SHORTEST_SLEEP`]
    pub shortest_sleep: f32,
    /// [`HEAT_DECAY`]
    pub heat_decay: f32,
}

/// **The world's share of a frame, measured** (W1/W3). The whole of the reckoning is in
/// [`install_world_answers`], where it is applied: 45,000 is 1.74 times the worst frame of 392
/// frames at the garden's own caps (90 blades, 24 creatures), measured 2026-09-17.
///
/// **Changing it throws that away**: the 1.74 is a margin over a measurement of *these* rules at
/// *these* caps, and a `world.rb` rewritten into more work, or a `pop_max` raised, is a different
/// measurement that nothing here re-takes.
const WORLD_BUDGET: u64 = 45_000;
/// rubevy's own 8 ms, left where it is because the instruction count bites first — at the rate
/// measured in W3 (about 9,300 instructions per millisecond) 45,000 is roughly 4.8 ms.
const WORLD_FRAME_TIME_MS: f32 = 8.0;

/// **The creatures' share of a frame, measured** (S5b-5, off S5b-3's measurement).
///
/// It was 200,000 until 2026-09-21: **rubevy's default, inherited** — the garden had never said
/// anything about it, which S6 noticed while working out where a check's wait should come from.
/// One VM's budget was measured and written down ([`WORLD_BUDGET`]) and the other, the one with a
/// dozen tasks in it, was whatever the library happened to ship, and rubevy's own source for the
/// number is **unknown** (rubevy `docs/numbers.md`).
///
/// **41,000 is `1.74 × 23,686`, rounded to the nearest thousand** (author's decision,
/// 2026-09-21). Both halves of that are borrowed rather than chosen:
///
/// * **23,686 instructions is the worst frame the creatures' VM was measured at.** S5b-3 sat in
///   the fullest garden the rules allow — `start_plants=130`, `start_beetles=14`,
///   `start_rabbits=10` in `garden.settings.txt`, which is `world.rb`'s own `pop_max` of 24 — and
///   ran `--headless 60` three times, 10,749 frames (`2026-09-21-numbers-garden-settings.md` §4).
///   The worst frame is the **first** one, where all 24 scripts run their opening pass at once,
///   and all three runs spent the same number of instructions in it to the instruction. The worst
///   frame after that is 4,955.
/// * **1.74 is the margin the world's VM was given**, which is 45,000 ÷ its own measured worst of
///   25,837 ([`WORLD_BUDGET`]). Reusing it says the two VMs are trusted to the same degree rather
///   than that a second margin was felt out.
///
/// `1.74 × 23,686` is 41,214, and the thousand it is rounded to is the rounding the world's
/// number already had (`1.74 × 25,837` = 44,956 → 45,000). Rounding down leaves the margin at
/// 41,000 ÷ 23,686 = **1.73**, which is the one place this number is not exactly the world's
/// reckoning.
///
/// **The seat was sat in again before this was written** (S5b-5, six runs of the capped garden,
/// 10,777 frames): the opening frame now costs **23,912**, the same number in all three runs as
/// before, and the 226 it gained is S5b-4's `mutation_rate 0.1` in `beetle.rb` — a line every
/// beetle reads in its first pass. So the margin the shipped number really carries is 41,000 ÷
/// 23,912 = **1.71**. The default is the author's 41,000 and not a third number worked out here;
/// what the re-measurement is for is that nothing in this paragraph is a quotation of a figure
/// nobody has seen since.
///
/// **What it buys**: a runaway script — a loop with no `sleep` in it — is stopped inside one
/// frame after a fifth of the instructions it used to be allowed, and no frame of an ordinary run
/// comes near it (the worst measured is 58% of it, and that frame happens once). **What it
/// costs**: a garden whose creatures are given a genuinely heavier brain than these can meet it,
/// and the answer to that is `script_budget` in `garden.settings.txt` — which is also why moving
/// it is safe to do at all.
///
/// [`window::scheduler_frames`] is worked out from this, so the checks' patience moved with it:
/// 2 + ceil(41,000 / 45,600) is **3** frames where it was 7.
const CREATURE_BUDGET: u64 = 41_000;
/// rubevy's own 8 ms, **left where it is**: at the caps the creatures' VM's worst tick was 1.72 ms
/// and the two VMs together took 30% of a frame at 60 Hz (S5b-3 §4), so there is a measurement
/// saying it is not tight and none saying what a tighter number should be.
const CREATURE_FRAME_TIME_MS: f32 = 8.0;

/// How fast the editor's heat fades, per frame. *Reason only* — the heat is there so that a line
/// a brain keeps coming back to stays lit; 0.985 itself is **unknown**.
const HEAT_DECAY: f32 = 0.985;

impl Default for Budgets {
    fn default() -> Self {
        Budgets {
            world: WORLD_BUDGET,
            world_frame_time_ms: WORLD_FRAME_TIME_MS,
            creature: CREATURE_BUDGET,
            creature_frame_time_ms: CREATURE_FRAME_TIME_MS,
            restore_patience: RESTORE_PATIENCE,
            shortest_sleep: SHORTEST_SLEEP,
            heat_decay: HEAT_DECAY,
        }
    }
}

impl Budgets {
    /// | key | field |
    /// |---|---|
    /// | `script_budget` / `script_frame_time_ms` | the creatures' VM — **the same two keys Battle uses** |
    /// | `world_script_budget` / `world_script_frame_time_ms` | `ruby/world.rb`'s VM |
    /// | `restore_patience` | how long a load waits for a script that is not starting |
    /// | `shortest_sleep` | the gap the HUD reads as "it slept" |
    /// | `heat_decay` | how fast the editor's heat fades |
    ///
    /// A `frame_time` of 0 or less means **no wall-clock guard at all**, which is what rubevy's
    /// `Option<Duration>` says with `None`; it is the same reading Battle gives the key.
    fn read_from(&mut self, settings: &games_shell::Settings) {
        let count = |key: &str, slot: &mut u64| {
            if let Some(value) = settings.number(key) {
                *slot = value.max(0.0) as u64;
            }
        };
        count("script_budget", &mut self.creature);
        take(settings, "script_frame_time_ms", &mut self.creature_frame_time_ms);
        count("world_script_budget", &mut self.world);
        take(settings, "world_script_frame_time_ms", &mut self.world_frame_time_ms);
        take(settings, "restore_patience", &mut self.restore_patience);
        take(settings, "shortest_sleep", &mut self.shortest_sleep);
        take(settings, "heat_decay", &mut self.heat_decay);
    }

    /// One of the two as rubevy wants it: `None` where the number says there is to be no
    /// wall-clock guard.
    fn frame_time(ms: f32) -> Option<std::time::Duration> {
        (ms > 0.0).then(|| std::time::Duration::from_secs_f32(ms / 1000.0))
    }
}

/// Where F5 writes and F9 reads: `--save PATH` if a run was given one, else `save_file` in the
/// store, else the game's own name (S5b-3). The flag wins over the store, as `--headless N` wins
/// over `headless_seconds`.
fn save_path(settings: &games_shell::Settings, save_to: Option<&str>) -> String {
    save_to
        .map(str::to_string)
        .unwrap_or_else(|| settings.get("save_file").unwrap_or(platform::SAVE_FILE).to_string())
}

fn main() {
    // The flags both games take (`--headless`, `--shot`, `--vm`, `--lang`) are read by
    // `games_shell::Args`; the garden's own four are read off the same words below.
    let args = games_shell::Args::from_env();
    // `--lang en|ja` (G6b): which language the guide opens in. It is *not* remembered — the
    // player's own click is (`games_shell::Settings`), and a picture asked for in Japanese on
    // the command line should not change what the next run shows a person.
    let lang_asked = args.value("--lang");
    // **The store, read before anything else** (S5b-3). G6b read it inside the windowed arm,
    // because the only two things in it were the guide's language and the night's dial and a
    // headless run has neither. Since S5b-3 the field's size, what a new garden is built with,
    // what the two VMs are allowed in a frame and the flags' own defaults come out of the same
    // file — and a headless run has all four — so it is read before the two arms rather than
    // inside one of them. It is Battle's arrangement since S5b-2, for the same reason.
    //
    // `Settings::load` only ever reads; a store that is not there is an empty one, so a headless
    // run that never had a `garden.settings.txt` is exactly what it was.
    let (settings, lang) = games_shell::remembered(
        platform::SETTINGS_FILE,
        "garden: what the panel remembers. Delete a line to go back to the default.",
        platform::read,
        platform::write,
        lang_asked.as_deref(),
    );
    let mut place = Place::default();
    place.read_from(&settings);
    let mut furniture = Furniture::default();
    furniture.read_from(&settings);
    let mut light = Light::default();
    light.read_from(&settings);
    let mut scenery = Scenery::default();
    scenery.read_from(&settings);
    let mut picture = Picture::default();
    picture.read_from(&settings);
    let mut eye_at = Eye::default();
    eye_at.read_from(&settings);
    let mut budgets = Budgets::default();
    budgets.read_from(&settings);
    // `--headless N`: no window, N seconds, the world reported on stdout. It runs exactly the
    // same systems as the windowed one; only the drawing is missing.
    let headless = args.headless(settings.number("headless_seconds").unwrap_or(HEADLESS_SECONDS));
    // `--shot FILE [SECONDS]`: a window, a picture of it, and out.
    let shot = args.shot(
        settings.get("shot_file").unwrap_or(SHOT_FILE),
        settings.number("shot_seconds").unwrap_or(SHOT_SECONDS),
    );
    // `--at SECONDS`: **where the garden's clock stands when the picture is taken** — or, with no
    // `--shot`, where it starts. G6 wanted a picture of midnight, and waiting forty seconds for
    // one on lavapipe (which draws a shadowed PBR frame in about a second) is not a way to
    // compare two sets of light numbers. It moves `Sky::shift`, which is G3's clock and nothing
    // else: the plants have grown as long as the run is old and the creatures are as hungry as
    // they have had time to get. Only the sun has moved. `--at MIDNIGHT` is the darkest one.
    // `--at midnight` is the one hour anybody asks for by name, so it has one.
    //
    // S5b-3: `at_seconds` in the store is the default of the flag — the hour a run starts at when
    // nobody said on the command line — and the flag wins where it is given, as
    // `headless_seconds` and `--headless N` do. The word `midnight` is worked out from
    // `light_dawn_offset` rather than from a constant, so an hour moved in the store moves the
    // name with it.
    let at = args
        .value("--at")
        .and_then(|s| if s == "midnight" { Some(midnight(&light)) } else { s.parse::<f32>().ok() })
        .or_else(|| settings.number("at_seconds"));
    // `--eye UNITS`: **how far back the camera stands when the picture is taken.** The wheel's
    // range, on the command line, and nothing else — the same zoom `Home` puts back. G8 added it
    // for one job: three pictures of the same garden wearing three different beetles, close
    // enough that the difference between a crab and a bee is something a person can see. At the
    // default 42 a beetle is thirty pixels across in a 1600-wide window, which is a picture of a
    // decision nobody can make. It is a `--shot` flag in the way `--at` is: a run with a window
    // and a player has a wheel.
    // S5b-3: its own default is `eye_distance` in the store — which is [`Eye::distance`], the one
    // the camera starts at and `Home` goes back to, rather than a second number beside it.
    let eye = args.number("--eye").map(|d| d.clamp(eye_at.zoom_min, eye_at.zoom_max));
    // `GARDEN_SELFTEST=1` on a PC, `?selftest` in the page's address (G5): a browser has no
    // environment, and the checks are what says from outside that the world is alive
    let selftest = platform::selftest_asked();
    // `--save PATH` / `--load PATH` (G3): the same two things F5 and F9 do in the window, for a
    // run that has no keyboard. A `--save` is written when the run ends, which is what makes
    // "save, load in a second process, save again, compare the two files" one shell line each.
    let save_to = args.value("--save");
    let load_from = args.value("--load");
    // `--vm` (G9): open the VM panel. The panel is closed unless somebody asks for it, and on a
    // command line this is the asking — `--shot docs/garden-vm.png 14 --vm` is how the picture in
    // `docs/garden.md` is taken. A player asks with `F2`.
    let wants_vm = args.has("--vm");

    let mut app = App::new();
    match headless {
        Some(seconds) => {
            app.add_plugins((
                MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    std::time::Duration::from_secs_f32(1.0 / 60.0),
                )),
                bevy::log::LogPlugin { filter: "info,bevy_asset=off".into(), ..default() },
                bevy::asset::AssetPlugin { file_path: platform::assets_dir(), ..default() },
                RubevyPlugin::default(),
                // W1: the world's own VM, the same plugin under a name tag. Its asset root is
                // the game's, because nothing in `ruby/` uses `require` — the prelude and the
                // file are compiled as one program, here as for a creature.
                RubevyPlugin::<World>::for_vm(platform::assets_dir()),
            ))
            // no renderer here, and from G0a no models either: the same startup builds the same
            // entities with the same components, and only the child that carries the look is
            // missing. What the headless run tests is the world, not the picture of it.
            .insert_resource(Headless { until: seconds })
            // between the restore and the save (G3): a `--save` run writes its file as the last
            // thing it does, and a `--load --save` run does both in the frame the world is whole
            .add_systems(Update, stop_when_over.after(restore_memory).before(save_world));
        }
        None => {
            // G6b. What the player chose last time: the guide's language and the night's dial.
            // Both are wanted *before the first frame* — the guide opens by itself at startup and
            // would show one language and then jump to the other, and a `--shot` of the night
            // would be taken at the wrong brightness — which is why the store is read at the top
            // of `main` and not in a system. On a PC it is a file beside the save; in a browser
            // it is a key in the same local storage the save uses (`platform.rs`).
            let night = settings
                .number("night")
                .map(|n| n.clamp(light.dial_min, light.dial_max))
                .unwrap_or(1.0);
            app.add_plugins((
                DefaultPlugins
                    .set(AssetPlugin {
                        file_path: platform::assets_dir(),
                        meta_check: bevy::asset::AssetMetaCheck::Never,
                        ..default()
                    })
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            title: "Garden".into(),
                            resolution: (
                                picture.window[0].max(1.0) as u32,
                                picture.window[1].max(1.0) as u32,
                            )
                                .into(),
                            canvas: Some("#garden".into()),
                            fit_canvas_to_parent: true,
                            ..default()
                        }),
                        ..default()
                    }),
                RubevyPlugin::default(),
                // W1: and the world's own, as in the headless arm above
                RubevyPlugin::<World>::for_vm(platform::assets_dir()),
                // G4: the editor and the VM panel, both `rubevy-egui`'s — the same two SabiRuby
                // Battle uses. They bring `bevy_egui` between them.
                //
                // H2: with the lexer behind the editor's colours. Which lexer is `platform.rs`'s
                // to know — the compiler linked in on a PC, `window.gardenHighlight` in a page —
                // and the panel only ever sees one kind per byte.
                EditorPlugin::with_highlighter(platform::highlight),
                VmInspectorPlugin,
                // G6: the `H` panel, and with it the Japanese font every egui panel in the game
                // now has as a fallback (`games_shell::guide`)
                GuidePlugin::default(),
                // S5b-1: whatever the player left in `garden.settings.txt` about the panels —
                // how big the editor is, how big its letters are, how deep the VM panel looks.
                // Each panel knows its own keys; this is the wiring.
                games_shell::PanelSettingsPlugin,
            ))
            // G6. A picture is asked for one thing, and the panel sits over the middle of the
            // window — which would be that thing. So a `--shot` run starts with it shut unless
            // `--guide` says otherwise, and `--shot p 8 --guide` is how the guide's own picture
            // (the one that says the Japanese is not tofu) is taken. A player gets it open.
            // G6b: one language at a time, and `opening` is the one it starts in
            .insert_resource(
                guide_text::guide().opening(lang, shot.is_none() || args.has("--guide")),
            )
            .insert_resource(NightDial(night))
            .insert_resource(Orbit { distance: eye.unwrap_or(eye_at.distance), ..Orbit::home(&eye_at) })
            .init_resource::<window::Watched>()
            .init_resource::<window::Paused>()
            // **Closed** (G9). It used to open with the game, which is a debugger thrown over the
            // middle of the window at somebody who came to look at a garden; `F2` opens it, and a
            // picture gets it only where `--vm` asks — `--shot docs/garden-vm.png 14 --vm`.
            .insert_resource(VmInspector::following().opened(wants_vm))
            .init_resource::<Tints>()
            // G8: the loaded model's materials, cloned and tinted per species the moment the
            // loader has built the model's entities
            .add_observer(tint_species)
            .add_systems(Startup, (make_look.in_set(MakeLook), spawn_camera))
            .add_systems(Update, (orbit_camera, dress_animations, animate_creatures))
            // G9: `P` stops the world, and this is the half of it that is not a run condition —
            // the garden's own clock, held where it stands for as long as the pause lasts. Only
            // a window can pause, so only a window has it.
            .add_systems(Update, hold_the_clock.run_if(is_paused).before(day_night))
            // G8: the fog, the sky's gradient and where the two of them stand. `after(day_night)`
            // because the hour it draws is the one that system has just worked out, and
            // `after(orbit_camera)` because the eye it hangs the sky on is the one that system has
            // just moved. Neither is in a headless app, and neither is this.
            .add_systems(Update, horizon_look.after(day_night).after(orbit_camera))
            // F5 and F9 (G3). Only the windowed build has a keyboard to read: `MinimalPlugins`
            // brings no input plugin at all, which is why the headless run is asked on the
            // command line instead.
            .add_systems(Update, save_load_keys.before(save_world))
            .add_systems(
                Update,
                (
                    window::choose_watched,
                    window::show_code,
                    window::inspect_keys,
                    window::show_vm,
                    window::do_editor_actions,
                    window::reload_changed,
                )
                    .chain()
                    .after(watch_minds),
            )
            // **S7, and on purpose not in the chain above.** It hands its species' program to
            // whoever the two systems above could not see — a creature whose `Mind` was still in
            // the command queue when the hand-over went round the `Query`. It wants no ordering:
            // it asks "is anybody behind?", which is true until it is answered and is as true on
            // the next frame as on this one, so being a frame late costs the creature a frame and
            // costs the reader nothing.
            //
            // **Chaining it cost something real.** Put at the end of that chain it is a system
            // with `Commands` ordered after another, so Bevy adds an `ApplyDeferred` there, and
            // that changes where the `Update` schedule is cut in two. The two wheel checks
            // (`window_selftest` steps 13-17) turn on whether `orbit_camera` reads the forged
            // `MouseWheel` in the frame it was written or the frame after — neither system is
            // ordered against the other — and with the extra sync point, eight of sixteen runs
            // on a machine running eight of them at once failed `the wheel over the editor
            // scrolls the editor and not the garden`, against none of sixteen without it.
            // Measured 2026-09-20; `docs/worklog/2026-09-20-window-check-fixes.md` §3.3.
            .add_systems(Update, window::catch_up_minds)
            .add_systems(bevy_egui::EguiPrimaryContextPass, window::draw_hud);
            register_scene_types(&mut app);
            // a creature's file saved from outside restarts that species, as Save does
            let dir = platform::ruby_dir();
            match Watch::new(&dir) {
                Some(watch) => {
                    app.insert_resource(watch);
                }
                None => warn!("could not watch {dir:?}: saving a creature's file will not reload it"),
            }
        }
    }

    // What Ruby will see, and the only thing it takes. `Transform` is Bevy's own and is
    // registered by `DefaultPlugins`; `MinimalPlugins` does not register it (rubevy's
    // `docs/host-api.md` says so), and a headless run is where the checks live, so it is named
    // here. Registering a type twice is not an error.
    app.register_type::<Transform>()
        .register_type::<Plant>()
        .register_type::<Creature>()
        .register_type::<Species>()
        // `Genome` is a *field* of `Creature`, not a component, and a nested field is reached
        // through its parent: taking this line out changes nothing a script can see (measured —
        // `me[:Creature][:genome]` still reads as a Hash, and the breeding check still passes).
        // It is here because the type is one the game means Ruby to have, and G3 will hand one
        // to `Serde<CreatureSpec>` by name.
        .register_type::<Genome>()
        .register_type::<Hunger>()
        // W2: the cooldown, because the rule that keeps it is `ruby/world.rb`'s now. This line
        // and the two derives on `Breeding` are the whole of "Ruby can see it"
        .register_type::<Breeding>()
        .register_type::<Velocity>()
        .register_type::<Sight>()
        .register_type::<Memory>()
        .register_type::<Collider>()
        .register_type::<Tree>()
        .register_type::<Rock>();

    // G3. The file a save goes to: `--save`'s path, or the game's own name for F5 (on the web
    // that name is a `localStorage` key rather than a file — `platform.rs`).
    // S5b-3: `save_file` in the store moves the name F5 writes to (and F9 reads back), the way
    // `shot_file` moves `--shot`'s. **In a browser that name is a `localStorage` key**, so
    // writing one is choosing a second garden to keep rather than renaming the first — the
    // prefix `garden:` is not touched and the old key is still there.
    app.insert_resource(SaveFile {
        path: save_path(&settings, save_to.as_deref()),
        on_exit: save_to.is_some(),
    })
    .init_resource::<SaveNow>()
    .init_resource::<SaveNote>()
    .init_resource::<Asked>();
    // G5's tenth check, and it is put here because here is what it is about. A save from another
    // version has to be refused by the thing that reads one, so the check writes such a file and
    // then hands it to `--load` — the arm below, unchanged — rather than to a function called
    // beside it. A run that was given a real `--load` keeps it; the check then says it did not run.
    let probe = (selftest && load_from.is_none()).then(write_a_save_from_another_version);
    let mut version_refused = None;
    // `GARDEN_RELOAD_AT=SECONDS` (checks only): the `--load` is not for the first frame. The
    // garden is built new, runs on its own, and the file goes in at `SECONDS` through the same
    // door F9 uses — which is the one path a headless run had no way to reach (`ReloadAt`).
    let reload_at = selftest.then(platform::reload_asked_at).flatten();
    let deferred = match (reload_at, load_from.as_ref()) {
        (Some(at), Some(path)) => {
            app.insert_resource(ReloadAt { at, path: path.clone(), asked: false, restored: false })
                .add_systems(Update, reload_while_running.before(load_world));
            true
        }
        (Some(_), None) => {
            error!("GARDEN_RELOAD_AT wants a --load PATH to open; loading nothing");
            false
        }
        _ => false,
    };
    if let Some(path) = (!deferred).then(|| load_from.as_ref().or(probe.as_ref())).flatten() {
        match read_save(path) {
            // the world is not built by `spawn_world` at all in this case: `load_world` does it on
            // the first frame, exactly as F9 does it on the four-hundredth
            Ok(save) => {
                app.insert_resource(Loading { save });
            }
            // nothing is loaded, and nothing else happens: `spawn_world` builds a new garden on
            // the first frame because no `Loading` is there to stop it
            Err(e) => {
                error!("{e}");
                version_refused = Some(e.clone());
                app.insert_resource(SaveNote { text: e, at: 0.0, bad: true });
            }
        }
    }

    // **The seven the store may have moved** (S5b-3). They go in for both arms, because the field
    // and the furniture are the headless run's world too, and the budgets are the headless run's
    // VMs; only `Picture` and `Scenery` are wholly the window's, and a resource nothing reads
    // costs a headless run nothing. The store itself goes in with them — the night dial writes
    // back to it, and `PanelSettingsPlugin` reads it in `PreStartup`.
    app.insert_resource(settings)
        .insert_resource(place)
        .insert_resource(furniture)
        .insert_resource(light)
        .insert_resource(scenery)
        .insert_resource(picture)
        .insert_resource(eye_at)
        .insert_resource(budgets)
        .insert_resource(Dice(platform::clock_seed()))
        .insert_resource(RubyDir(platform::ruby_dir()))
        .init_resource::<Brains>()
        // `--at` starts the sky ahead, and where a picture is asked for it counts back from the
        // moment of the picture, so `--shot p 8 --at 40` is a garden eight seconds old whose sun
        // is where it would be at forty
        .insert_resource(Sky {
            shift: at.map(|at| at - shot.as_ref().map(|(_, after)| *after).unwrap_or(0.0)).unwrap_or(0.0),
            ..default()
        })
        .init_resource::<Contacts>()
        .init_resource::<Bumps>()
        .init_resource::<Births>()
        .init_resource::<Newborns>()
        // W1: the seeds `world.rb` asked for, the cost of its pass, and why it has no rules where
        // it has none
        .init_resource::<Sprouts>()
        // and the three distances it keeps that the game sweeps or builds a place with
        // (2026-09-18, and `touch_reach` since S5b-4), and the four bodies (S5b-4)
        .init_resource::<Reaches>()
        .init_resource::<Bodies>()
        .init_resource::<WorldMeter>()
        .init_resource::<WorldTrouble>()
        .init_resource::<WorldPrelude>()
        // The one thing this game puts in the VM (G2): the `Genome` class and its seven methods.
        // `ScriptWorld::vm` is public and the resource exists as soon as `RubevyPlugin` is added,
        // while no script runs before the first `Update` — so `Startup` is the place and rubevy
        // needs no entry point for it (rubevy `docs/host-api.md`, "Adding to the VM").
        .add_systems(Startup, (install_host_api, install_world_answers))
        // **The two VMs' frames, ordered against each other** (W1). Nothing in rubevy orders them
        // — each plugin chains its own three sets and no more (rubevy `docs/host-api.md`, "Two
        // VMs in one app") — so this line is the whole of the arrangement, and it is the reason
        // the rules can be Ruby at all:
        //
        //   the Rust rules → the world's tick → **the creatures' tick** → the answers
        //
        // The world's writes are applied at the end of its own tick (`apply_component_writes`),
        // so a `Hunger` the rules worked out this frame is the `Hunger` a beetle reads in the
        // *same* frame's tick — a read is answered inside the tick, out of the world as it stands
        // there (rubevy, "A read costs no frame"). The other way round it would be a frame old,
        // and a creature deciding on a meter one frame behind the rule that wrote it is exactly
        // the kind of luck `.before(RubevySet::Tick)` was put in to end
        // (`docs/worklog/2026-09-17-sync-reads.md`, §1).
        //
        // `is_still` is the run condition the deleted rule chain carried, now where the rules
        // are: a garden that is being read back from a file does not age while its minds are
        // starting, and `P` stops the world. Skipping the set skips the tick, so the pass simply
        // does not happen — the world's task is parked on `Rubevy.ask("frame")` either way, and
        // the delta it wakes with is one frame's and not the pause's.
        //
        // `VmClockSet` is the pair of systems that time the creatures' tick (`rubevy-egui`). They
        // are ordered against `RubevySet::Tick` and would otherwise have had the world's tick
        // scheduled inside their window, which made the HUD's "VM 0.9 ms" read 2.1 — one VM's
        // figure against two VMs' work. Ordering the world's tick ahead of the pair puts each
        // number back on its own VM.
        .configure_sets(
            Update,
            RubevySet::<World>::tick()
                .before(RubevySet::Tick)
                .before(window::VmClockSet)
                .run_if(is_still),
        )
        // W2: and the world's *answers* before the creatures' tick too, which is what makes
        // `tell` arrive in the frame it was said. rubevy chains each VM's own three sets and no
        // more (`docs/host-api.md`, "Two VMs in one app"), so without this line the system that
        // carries a message from one VM to the other could be scheduled after the VM it carries
        // it to has already had its frame — and a beetle would hear about its lunch on the next
        // one. Nothing of the creatures' is read here that their tick has not already written:
        // the world's questions are about the world.
        .configure_sets(Update, RubevySet::<World>::answer().before(RubevySet::Tick))
        // `spawn_world` asks for `Option<Res<Look>>`, and a `None` there is how the headless
        // build says "no models". That makes the order load-bearing: without this the windowed
        // build's `spawn_world` may run before `make_look` and then it is `None` there too —
        // which is a garden with no ground, no trees and no creatures, and only the plants that
        // sprouted later (`sprout_plants` runs in `Update`, long after) with a model on them.
        // `MakeLook` is a set rather than `after(make_look)` because `make_look` is not in the
        // headless schedule at all.
        .add_systems(Startup, spawn_world.after(MakeLook))
        // W1: and the rules, on an entity of their own in the world's VM
        .add_systems(Startup, give_the_world_its_rules)
        .add_systems(
            Update,
            (
                day_night,
                // G0's `wander` stood here, writing `Velocity` so that the world was not a still
                // life. It is gone: the only thing that writes `Velocity` now is
                // `me[:Velocity] = [[vx, vz]]` in a Ruby task, and every system below reads it
                // without caring who wrote it.
                move_creatures,
                separate,
                startle,
            )
                .chain()
                // G3: a garden that is being read back does not age while its minds are starting
                .after(load_world)
                // **What is left of the rules moves the world before anybody looks at it.**
                //
                // These four are the mechanics rather than the rules (W1): where a creature ends
                // up when it walks and when it is pushed, who is standing on whom, and where the
                // sun is. The five that used to stand between them — `grow_plants`,
                // `sprout_plants`, `get_hungry`, `eat`, `starve` — are `ruby/world.rb` now.
                //
                // It is `.before(RubevySet::<World>::tick())` rather than `.before(RubevySet::
                // ::Tick)` because the world's VM is the first of the two to run and the rules
                // are what it reads: a `Transform` this chain has just written is the `Transform`
                // `garden.within` measures from, in the same frame. Since the world's tick is
                // itself before the creatures', ordering against it orders against both — the
                // chain kept everything the 2026-09-17 ordering bought it
                // (`docs/worklog/2026-09-17-sync-reads.md`, §1 and §8), and moved one set earlier.
                .before(RubevySet::<World>::tick())
                .run_if(is_still),
        )
        // G3: `--load` before the first frame, F9 at any time. It is before the scripts are dealt
        // with, so a creature that was despawned here has lost its `ScriptTask` — and with it its
        // task in the VM and its queues — before rubevy looks at the world again.
        .add_systems(Update, load_world.run_if(resource_exists::<Loading>).before(RubevySet::Deliver))
        // after the answers, because what it makes was asked for in this frame's `answer_garden`
        // and the request is answered there too
        .add_systems(Update, children_arrive.after(RubevySet::Answer).run_if(is_still))
        // and the word the newborn of two frames ago is owed: after `day_night`, so that a child
        // told the sky in the frame the sky turns over is told the new one, and before the
        // creatures' tick, so that the word is in its queue on the first frame it can read one
        .add_systems(
            Update,
            tell_newborns_the_sky.after(day_night).before(RubevySet::Tick).run_if(is_still),
        )
        // G3, in this order and after the scripts have had their frame: put the memories back
        // (which needs the object a script makes in its first frame), then write the file if
        // anybody asked for one, then — last — let a restored world start moving. A run that
        // loads and saves in one go therefore writes the world it read, not that world plus a
        // frame, which is what makes the round-trip check a plain `diff`.
        .add_systems(
            Update,
            (
                restore_memory.run_if(resource_exists::<Restoring>),
                save_world,
                finish_restore.run_if(resource_exists::<Restoring>),
            )
                .chain()
                .after(RubevySet::Answer),
        )
        // The one system that answers a script. It goes in `RubevySet::Answer`, which is the
        // only placement where a question is answered on the frame it was asked — anywhere else
        // costs a second frame per round trip, and where it landed used to be luck (rubevy
        // `docs/host-api.md`, "Where the game's systems go in the frame").
        .add_systems(Update, answer_garden.in_set(RubevySet::Answer))
        // W1: and the one that answers the *world's* script, in the world VM's own set. It is a
        // different system reading a different resource, which is how the answering side knows
        // which VM asked (rubevy `docs/host-api.md`, "Two VMs in one app").
        .add_systems(Update, answer_world.in_set(RubevySet::<World>::answer()))
        // W1: what the rules did, read from the outside.
        //
        // `plants_wear_their_size` is the look: `Plant.size` is also the entity's
        // `Transform.scale` — one number seen two ways, which `grow_plants` used to write both
        // halves of — and `world.rb` writes the number, not the picture of it.
        // `note_the_rules` is the other direction: what a rule *did* to the world, for the things
        // that used to be noted from inside the rule itself (the chewing animation, and three of
        // the checks). `sprout_plants` puts down the seeds the rules asked for.
        //
        // All three are after the world's tick, which is where its writes land.
        .add_systems(
            Update,
            (plants_wear_their_size, note_the_rules, sprout_plants)
                .after(RubevySet::<World>::tick())
                .after(RubevySet::Answer),
        )
        // `bodies_wear_the_rules` is `plants_wear_their_size`'s shape for the other number the
        // rules hand over that is also written onto an entity: how wide a body is (S5b-4). It is
        // ordered after the set the handover is *read* in rather than with the three above,
        // because that is the edge it actually needs and because giving the three of them a new
        // edge is giving Bevy a new sync point to put somewhere (S7's lesson, and the reason the
        // window checks were flaky).
        .add_systems(
            Update,
            bodies_wear_the_rules.after(RubevySet::<World>::answer()),
        )
        .add_systems(Update, watch_minds.after(RubevySet::Answer))
        // W1: what one pass of `world.rb` costs, round the set that runs it
        .add_systems(Update, world_clock_start.before(RubevySet::<World>::tick()))
        .add_systems(Update, world_clock_end.after(RubevySet::<World>::tick()).before(RubevySet::Tick))
        // (G4's other HUD number, the wall time the frame's scripts took, is added below: in a
        // window `VmInspectorPlugin` measures it, and only the headless build adds it itself.)
        ;
    if headless.is_some() {
        // G4's other HUD number, measured round the set that runs the scripts. In a window it is
        // `VmInspectorPlugin`'s, because the VM panel is what shows it (G9); here there is no
        // plugin and no panel, and the figure is printed at the end of the run.
        app.init_resource::<window::VmClock>()
            .add_systems(Update, window::vm_clock_start.before(RubevySet::Tick).in_set(window::VmClockSet))
            .add_systems(
                Update,
                window::vm_clock_end
                    .after(RubevySet::Tick)
                    .before(RubevySet::Answer)
                    .in_set(window::VmClockSet),
            );
    }
    if selftest && headless.is_none() {
        // G6: and the camera, which is the one thing a browser check has no other way to read
        app.insert_resource(CameraLog);
        // the editor's buttons and the two keys, which no headless run can press
        // before both of the systems whose keys it presses: `inspect_keys` reads `F2` and `P`, and
        // `choose_watched` reads `F3` and `Tab`. A key pressed into `ButtonInput` after the system
        // that reads it has run in that frame is a key nobody ever sees — `just_pressed` is
        // cleared in the next frame's `PreUpdate`.
        app.insert_resource(window::WindowTest::after(3.0, &budgets)).add_systems(
            Update,
            window::window_selftest.before(window::inspect_keys).before(window::choose_watched),
        );
        // S7: the one beetle the checks want born in the frame Apply is pressed. `.after` so it
        // sees the step the check has just moved to, `.before(children_arrive)` so the birth is
        // made in this frame — and no edge at all to `do_editor_actions`, because the whole
        // point is that the new `Mind` is still in the command queue when the hand-over goes
        // round (`window::birth_in_the_apply_frame`).
        app.add_systems(
            Update,
            window::birth_in_the_apply_frame.after(window::window_selftest).before(children_arrive),
        );
    }
    if selftest {
        // after `separate`, so what it measures is the world as the frame leaves it
        app.insert_resource(SelfTest { version_refused, ..default() })
            .add_systems(Update, watch_overlap.after(separate))
            .add_systems(
                Update,
                (watch_probe, watch_turning, watch_sleep, watch_the_rules, watch_the_season)
                    .after(RubevySet::Answer),
            )
            // W1's twelfth check: the rules taken away and given back while the world runs. It is
            // after `note_the_rules`, because what it looks at is the falling meters that system
            // counted on this frame.
            .add_systems(Update, swap_the_rules.after(note_the_rules))
            // G2's pair, on the frame the rules say where it goes (2026-09-18). After the world's
            // answers, so that frame is this one and not the next.
            .add_systems(
                Update,
                plant_the_meadow
                    .after(RubevySet::<World>::answer())
                    .run_if(resource_exists::<Meadow>),
            );
    }
    if let Some((path, after)) = shot {
        app.insert_resource(Shot { path, after, taken: false }).add_systems(Update, take_shot);
    }
    // **What the creatures' VM is allowed in a frame** (S5b-3), written after `RubevyPlugin` has
    // made the `ScriptWorld` and before the first frame runs. The world's VM is set in
    // `install_world_answers`, which is a `Startup` system because it is where the rest of that
    // VM's arrangement lives; this one has no such system, and a plugin's resource can only be
    // written once the plugin has been added.
    //
    // The default is the garden's own measured number since S5b-5 — see [`CREATURE_BUDGET`].
    // Saying it here rather than leaving it to the library is what lets
    // `window::scheduler_frames` divide by it instead of writing the number a second time.
    {
        let mut world = app.world_mut().resource_mut::<ScriptWorld>();
        world.budget = budgets.creature;
        world.frame_time = Budgets::frame_time(budgets.creature_frame_time_ms);
    }
    app.run();
}

/// What a `.glb` needs in the type registry, in the windowed build only.
///
/// bevy 0.19 spawns a loaded glTF through `bevy_world_serialization`, and that spawner **panics
/// on any type in the loaded world the app has not registered** — there is no "skip what you do
/// not know". Nothing registers Bevy's own types automatically here: that is the
/// `reflect_auto_register` feature, which registers every type in the binary that derives
/// `Reflect`. Turning it on would be one line, and it would put a few hundred of Bevy's types in
/// front of Ruby — the registry would stop being a thing this game decides, which is most of what
/// the garden is for. So the plumbing a model brings with it is named here, one line each, the
/// way `Transform` already was. (The game's private components would survive either way: they
/// derive no `Reflect`, so nothing can register them.)
///
/// The cost is that a script running in the window can read these too — `e[:GlobalTransform]`,
/// `e[:Name]`. They are Bevy's, not the game's, and the component table in `docs/garden.md` is
/// still the whole of what the *game* offers. The headless build, where the checks live, does
/// not load a model and does not register them, so what the checks see is exactly the table.
fn register_scene_types(app: &mut App) {
    app.register_type::<GlobalTransform>()
        .register_type::<bevy::transform::components::TransformTreeChanged>()
        .register_type::<Visibility>()
        .register_type::<bevy::camera::visibility::VisibilityClass>()
        .register_type::<bevy::camera::primitives::Aabb>()
        .register_type::<InheritedVisibility>()
        .register_type::<ViewVisibility>()
        .register_type::<Name>()
        .register_type::<ChildOf>()
        .register_type::<Children>()
        .register_type::<Mesh3d>()
        .register_type::<MeshMaterial3d<StandardMaterial>>()
        .register_type::<AnimationPlayer>()
        .register_type::<bevy::animation::AnimationTargetId>()
        .register_type::<bevy::animation::AnimatedBy>()
        .register_type::<AnimationGraphHandle>()
        .register_type::<bevy::gltf::GltfExtras>()
        .register_type::<bevy::gltf::GltfSceneExtras>()
        .register_type::<bevy::gltf::GltfMeshExtras>()
        .register_type::<bevy::gltf::GltfMaterialExtras>()
        .register_type::<bevy::gltf::GltfMaterialName>()
        .register_type::<bevy::gltf::GltfMeshName>()
        .register_type::<bevy::gltf::GltfSceneName>();
}

// ---------------------------------------------------------------------------------------------
// Building the world
// ---------------------------------------------------------------------------------------------

/// The models (G0a), loaded once, and the ground they stand on. Only the windowed build runs
/// this: a headless world has no `Look`, and every spawn below simply leaves the child out.
///
/// The grass, the tree and the rock are Kenney's Nature Kit; the rabbit and the beetle are
/// Kenney's Cube Pets, which are node-animated (no skin) and carry `idle`, `walk` and `eat`
/// among their eight clips. `assets/models/` has the two packs' own licence texts beside them,
/// and `CREDITS.md` the sizes and the sources.
#[allow(clippy::too_many_arguments)]
fn make_look(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    place: Res<Place>,
    picture: Res<Picture>,
    scenery: Res<Scenery>,
) {
    // the clips of one glb, by their index in it: 0 static, 1 idle, 2 walk, 3 run, 4 eat, and
    // three the garden has no use for
    let mut gaits = |file: &str| {
        let clip = |i: u32| server.load(GltfAssetLabel::Animation(i as usize).from_asset(file.to_string()));
        let (graph, nodes) = AnimationGraph::from_clips([clip(1), clip(2), clip(4)]);
        Gaits { graph: graphs.add(graph), idle: nodes[0], walk: nodes[1], eat: nodes[2] }
    };
    let scene = |file: &str| server.load(GltfAssetLabel::Scene(0).from_asset(file.to_string()));

    let [r, g, b] = picture.ground_color;
    let turf = materials.add(StandardMaterial {
        base_color: Color::srgb(r, g, b),
        perceptual_roughness: 1.0,
        ..default()
    });
    let tree = scene("models/tree_default.glb");

    commands.insert_resource(Look {
        ground: meshes.add(Plane3d::new(Vec3::Y, Vec2::new(place.half_w(), place.half_d()))),
        turf: turf.clone(),
        tuft: scene("models/grass.glb"),
        bush: scene("models/plant_bush.glb"),
        tree: tree.clone(),
        rock: scene("models/rock_smallA.glb"),
        beetle: scene(&picture.beetle_model),
        rabbit: scene("models/animal-bunny.glb"),
        beetle_gaits: gaits(&picture.beetle_model),
        rabbit_gaits: gaits("models/animal-bunny.glb"),
        tuft_scale: picture.tuft_scale,
        bush_scale: picture.bush_scale,
        tree_scale: picture.tree_scale,
        rock_scale: picture.rock_scale,
        beetle_scale: picture.beetle_scale,
        rabbit_scale: picture.rabbit_scale,
        walking_at: picture.walking_at,
        gait_blend_ms: picture.gait_blend_ms,
    });

    // G8: the sky's mesh, made here because `make_look` is the windowed build's and nothing else
    // is. Its colours are written on the first frame by `horizon_look`, so the handle goes in
    // with none: a mesh with no `ATTRIBUTE_COLOR` is drawn with the material's own `base_color`,
    // which is the sky's mid blue, and one frame of that is what the first frame of a run is.
    let (mesh, up) = sky_dome(&scenery);
    let mesh = meshes.add(mesh);
    commands.insert_resource(SkyDome { mesh: mesh.clone(), up, was: None });

    // …and the three things that hang on it. They are made here rather than in `spawn_world`
    // because `spawn_world` is the world's, and the world is the same with a window and without:
    // it takes `Option<&Look>` and no `Assets<_>` at all, and a headless app has neither of the
    // two asset collections this wants. The scenery belongs to the look, so it is built where the
    // look is.
    let sky = materials.add(StandardMaterial {
        // what one frame looks like before `horizon_look` writes the vertices, and what the
        // vertices then stand in for: a mesh that has an `ATTRIBUTE_COLOR` uses it *instead* of
        // this, so this is only ever the very first frame
        base_color: Color::srgb(0.42, 0.62, 0.86),
        unlit: true,
        // seen from the inside, and not fogged: the dome is five hundred units away, and fog
        // would paint the whole of it the one colour the fog is — which is the colour of its rim
        cull_mode: None,
        fog_enabled: false,
        ..default()
    });
    commands.spawn((
        SkyShell,
        Mesh3d(mesh),
        MeshMaterial3d(sky),
        Transform::default(),
        NotShadowCaster,
        NotShadowReceiver,
    ));
    // the ground past the wall: the field's own material, so the two are one lawn
    commands.spawn((
        Horizon,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(scenery.horizon_half)))),
        MeshMaterial3d(turf),
        Transform::from_xyz(0.0, HORIZON_DROP, 0.0),
        NotShadowCaster,
    ));
    // and the treeline. Its dice are its own — `Dice` the resource is the world's, and drawing
    // from it here would move every tree, rock, plant and creature in the garden by a number of
    // draws that depends on whether there is a window.
    let mut dice = Dice(0x600D_5EED_0000_0008);
    for i in 0..scenery.edge_trees {
        // once round the field's rim, one tree per step, each pushed out by a random amount and
        // slid along by up to half a step so the row is not a fence
        let jitter = scenery.edge_jitter;
        let step = (i as f32 + dice.between(-jitter, jitter)) / scenery.edge_trees as f32;
        let out = dice.between(scenery.edge_out[0], scenery.edge_out[1]);
        let (x, z) = rim_at(&place, step, out);
        let scale = dice.between(scenery.edge_scale[0], scenery.edge_scale[1]);
        commands.spawn((
            WorldAssetRoot(tree.clone()),
            Transform::from_xyz(x, 0.0, z)
                .with_scale(Vec3::splat(scale))
                .with_rotation(Quat::from_rotation_y(dice.between(0.0, std::f32::consts::TAU))),
        ));
    }
}

/// A point `out` units outside the field's wall, `t` of the way round it (0 is the middle of the
/// `+Z` side). The rim is a rectangle, not a circle, because the field is.
fn rim_at(place: &Place, t: f32, out: f32) -> (f32, f32) {
    let (w, d) = (place.half_w() + out, place.half_d() + out);
    let side = t.rem_euclid(1.0) * 4.0;
    match side as u32 {
        0 => (w * (2.0 * side - 1.0), d),
        1 => (w, d * (3.0 - 2.0 * side)),
        2 => (w * (5.0 - 2.0 * side), -d),
        _ => (-w, d * (2.0 * side - 7.0)),
    }
}

/// The dome, and how far up it each of its vertices is.
///
/// [`SKY_SIDES`] columns and [`SKY_RINGS`] rows from the rim to a single point overhead:
/// `(SKY_RINGS - 1) * SKY_SIDES * 2 + SKY_SIDES` = **84 triangles** on 49 vertices. The winding
/// is not thought about and the normals are not looked at, because the material is `unlit` with
/// `cull_mode: None` — a sky is the one surface in a game that is only ever a colour.
fn sky_dome(scenery: &Scenery) -> (Mesh, Vec<f32>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    use std::f32::consts::{FRAC_PI_2, TAU};

    let (sides, rings, radius) = (scenery.sky_sides, scenery.sky_rings, scenery.sky_radius);
    let mut position: Vec<[f32; 3]> = Vec::new();
    let mut normal: Vec<[f32; 3]> = Vec::new();
    let mut up: Vec<f32> = Vec::new();
    for ring in 0..rings {
        let t = ring as f32 / rings as f32;
        let e = SKY_FLOOR + (FRAC_PI_2 - SKY_FLOOR) * t;
        for side in 0..sides {
            let a = side as f32 / sides as f32 * TAU;
            let p = Vec3::new(e.cos() * a.cos(), e.sin(), e.cos() * a.sin());
            position.push((p * radius).to_array());
            normal.push((-p).to_array());
            up.push(t);
        }
    }
    let apex = position.len() as u32;
    position.push([0.0, radius, 0.0]);
    normal.push([0.0, -1.0, 0.0]);
    up.push(1.0);

    let mut index: Vec<u32> = Vec::new();
    for ring in 0..rings - 1 {
        for side in 0..sides {
            let next = (side + 1) % sides;
            let a = (ring * sides + side) as u32;
            let b = (ring * sides + next) as u32;
            let (c, d) = (a + sides as u32, b + sides as u32);
            index.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    for side in 0..sides {
        let next = (side + 1) % sides;
        let rim = ((rings - 1) * sides) as u32;
        index.extend_from_slice(&[rim + side as u32, apex, rim + next as u32]);
    }

    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, position)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normal)
        .with_inserted_indices(Indices::U32(index));
    (mesh, up)
}

#[allow(clippy::too_many_arguments)]
fn spawn_world(
    mut commands: Commands,
    look: Option<Res<Look>>,
    ruby: Res<RubyDir>,
    mut brains: ResMut<Brains>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut dice: ResMut<Dice>,
    selftest: Option<Res<SelfTest>>,
    loading: Option<Res<Loading>>,
    place: Res<Place>,
    built: Res<Furniture>,
    bodies: Res<Bodies>,
    light: Res<Light>,
) {
    let look = look.as_deref();
    let (half_w, half_d) = (place.half_w(), place.half_d());

    // the ground: one plane, 40 × 30. It is the only mesh left that is not a model, because a
    // lawn made of Kenney's 1 × 1 grass tiles would be twelve hundred entities for a flat green.
    if let Some(look) = look {
        commands.spawn((
            Mesh3d(look.ground.clone()),
            MeshMaterial3d(look.turf.clone()),
            Transform::default(),
        ));
    }

    // the sun. Its rotation, colour and brightness are `day_night`'s from the first frame; what
    // is set here is what does not change — that it casts shadows, and over how much ground.
    commands.spawn((
        Sun,
        DirectionalLight { illuminance: light.sun_lux, shadow_maps_enabled: true, ..default() },
        CascadeShadowConfigBuilder {
            num_cascades: light.shadow_cascades as usize,
            first_cascade_far_bound: light.shadow_near,
            maximum_distance: light.shadow_far,
            ..default()
        }
        .build(),
        Transform::default(),
    ));

    // `--load PATH` (G3): the ground and the sun are this world's furniture and are made either
    // way; everything that lives in it comes out of the file, on the first frame, in `load_world`
    // — the same system F9 uses, so there is one way to build a garden from a save and not two.
    if loading.is_some() {
        info!("the world will come from the save file");
        return;
    }

    // the selftest wants a creature that certainly starves, which means one that certainly does
    // not eat: it is spawned in the far corner **with no script at all**, so it never moves, and
    // the grass is kept away from it. Everything else about it is an ordinary beetle — what it
    // lacks is a brain. (In G0 the same beetle was the one without the `Wander` component.)
    let fasting_at = Vec2::new(-half_w + 3.0, -half_d + 3.0);
    // and one that certainly has something to walk to: a hungry beetle in the other far corner
    // with one plant five units away, which is inside a beetle's `Sight` of eight and outside
    // everything else.
    let probe_at = Vec2::new(half_w - 3.0, half_d - 3.0);
    let dinner_at = probe_at + Vec2::new(-4.0, -3.0);
    // and, for G2, a third corner: grass with a hungry beetle behind it, twice over. Nothing about
    // the rules is bent for it — the two walk to the grass because they are hungry, eat because
    // they are near it, and are told about each other because they are well fed and close. What is
    // arranged is only that it happens, and **where** is [`plant_the_meadow`]'s to work out from
    // the rules' own two distances, a frame or two from now (2026-09-18).
    let keep_clear = selftest.is_some();
    let clear_of_fixtures = |at: Vec2| {
        !keep_clear
            || (at.distance(fasting_at) > 6.0
                && at.distance(probe_at) > 7.0
                && at.distance(meadow_at(&place)) > MEADOW_CLEAR)
    };

    // trees and rocks first: they never move, so everything else is placed around them. They are
    // kept apart from each other at the start, because the separation pass moves creatures only
    // and two rocks left inside one another would overlap for the whole run.
    let mut solid: Vec<(Vec2, f32)> = Vec::new();
    for i in 0..(built.trees + built.rocks) {
        let tree = i < built.trees;
        let radius = if tree { TREE_RADIUS } else { ROCK_RADIUS };
        let mut at = Vec2::ZERO;
        let mut room = false;
        for _ in 0..built.tries {
            at = Vec2::new(dice.between(-half_w + 2.0, half_w - 2.0), dice.between(-half_d + 2.0, half_d - 2.0));
            if clear_of_fixtures(at)
                && solid.iter().all(|(p, r)| p.distance(at) > r + radius + built.solid_apart)
            {
                room = true;
                break;
            }
        }
        if !room {
            continue;
        }
        if tree {
            spawn_tree(&mut commands, look, &bodies, at);
        } else {
            spawn_rock(&mut commands, look, &bodies, at, dice.between(built.rock_squash[0], built.rock_squash[1]));
        }
        solid.push((at, radius));
    }

    let mut grass: Vec<Vec2> = Vec::new();
    for _ in 0..built.plants {
        let at = Vec2::new(dice.between(-half_w + 1.0, half_w - 1.0), dice.between(-half_d + 1.0, half_d - 1.0));
        if !clear_of_fixtures(at) {
            continue;
        }
        // grass under a tree cannot be reached, so it is not put there
        if solid.iter().any(|(p, r)| p.distance(at) < r + built.grass_off_solid) {
            continue;
        }
        // **0.3 is not in `docs/numbers.md`** and is left alone (S5b-3): the inventory has
        // `plant_min` 0.18 — which is what a *sprout* starts at — and missed the different
        // number a garden's first grass is scattered between. Moving a number the list does not
        // have is how a stage widens itself, so it is reported instead (§7 of the list).
        let size = dice.between(0.3, built.plant_grown);
        let round = dice.roll() < built.round_chance;
        spawn_plant(&mut commands, look, at, size, round);
        grass.push(at);
    }

    let mut taken: Vec<Vec2> = Vec::new();
    for i in 0..(built.beetles + built.rabbits) {
        let species = if i < built.beetles { Species::Beetle } else { Species::Rabbit };
        // not standing on its dinner: a creature that starts inside a plant has eaten before it
        // has moved, and then "somebody ate within 10 s" says nothing about walking or about the
        // contact test. The positions are kept in hand because the plants above are still
        // commands and are not in the world to be queried yet.
        let radius = bodies.of(species);
        let mut at = Vec2::ZERO;
        for _ in 0..built.tries {
            at = Vec2::new(dice.between(-half_w + 2.0, half_w - 2.0), dice.between(-half_d + 2.0, half_d - 2.0));
            let clear_of_grass = grass.iter().all(|g| g.distance(at) > built.creature_off_grass);
            // nothing starts inside anything: the separation pass would otherwise have a pileup
            // to undo on the first frame, and the overlap check looks at that frame too
            let clear_of_solid =
                solid.iter().all(|(p, r)| p.distance(at) > r + radius + built.creature_off_solid);
            let clear_of_kin = taken.iter().all(|p: &Vec2| p.distance(at) > built.creatures_apart);
            if clear_of_fixtures(at) && clear_of_grass && clear_of_solid && clear_of_kin {
                break;
            }
        }
        taken.push(at);
        let hunger = dice.between(built.hunger[0], built.hunger[1]);
        // no two creatures alike, so that `Genome#mix` has something to average
        let genome = Genome::roll(built.genome(species), built.genome_spread, |lo, hi| dice.between(lo, hi));
        let entity = spawn_creature(&mut commands, look, &bodies, species, at, hunger, genome, None);
        give_mind(&mut commands, &ruby.0, &mut brains, &mut mrb, entity, species);
    }

    if keep_clear {
        let entity =
            spawn_creature(&mut commands, look, &bodies, Species::Beetle, fasting_at, 3.0, Genome::of(Species::Beetle), None);
        commands.entity(entity).insert(Fasting);
        info!("selftest: a beetle with no behaviour and nothing to eat stands at ({:.1}, {:.1})", fasting_at.x, fasting_at.y);

        let dinner = spawn_plant(&mut commands, look, dinner_at, built.plant_grown, false);
        // hungry enough that its script goes looking rather than wandering (the beetle's own
        // threshold is 55), and far enough that getting there has to be walking
        // the species' own genome, not a rolled one: the check below is written for a beetle
        // that sees exactly eight units, and a rolled `Sight` of 6.6 would be testing the dice
        let probe =
            spawn_creature(&mut commands, look, &bodies, Species::Beetle, probe_at, 40.0, Genome::of(Species::Beetle), None);
        commands.entity(probe).insert(Probe { dinner });
        give_mind(&mut commands, &ruby.0, &mut brains, &mut mrb, probe, Species::Beetle);
        info!(
            "selftest: a hungry beetle at ({:.1}, {:.1}) with one plant {:.1} away",
            probe_at.x,
            probe_at.y,
            probe_at.distance(dinner_at)
        );

        // G2's pair is not planted here (2026-09-18): where it stands is worked out from two
        // numbers `world.rb` has not handed over yet, so the corner is left to
        // [`plant_the_meadow`] and this is only the note that it is coming.
        commands.insert_resource(Meadow);

        // G3's ninth check, and the only script in the game that is not a creature: it asks the
        // game for a creature whose genome has no `sight` and reports what it is told. The point
        // is the message — a Hash of the wrong shape has to say what is wrong with it, in the
        // script's own terms, and with serde doing the reading that message is written by nobody.
        if let Ok((handle, _)) = compile_source(&ruby.0, "tester.rb", TESTER, &mut mrb) {
            commands.spawn(Script::new(handle).with_name("Tester").with_priority(120));
            info!("selftest: a tester script will ask for a creature with a gene missing");
        }
    }
}

/// Where the selftest's meadow corner is, which is the one thing about it that is the garden's
/// own: a spot far enough from the other two fixtures that `clear_of_fixtures` can keep eight
/// units of field clear round it. Everything else about the corner — how far apart the two blades
/// stand and how far behind them the two beetles start — comes out of the rules
/// ([`plant_the_meadow`]).
fn meadow_at(place: &Place) -> Vec2 {
    Vec2::new(-place.half_w() + 6.0, place.half_d() - 6.0)
}

/// How much ground round [`meadow_at`] `spawn_world` keeps empty of everything it scatters — grass,
/// trees, rocks and the garden's own creatures. It is the number `clear_of_fixtures` has had since
/// G2, named here rather than changed, because [`plant_the_meadow`] has to build inside it: a blade
/// of the field's own that stands nearer to one of the corner's beetles than the blade the corner
/// put in front of it is a beetle that walks the other way, and that is what it was doing.
const MEADOW_CLEAR: f32 = 8.0;

/// **The meadow corner has been asked for and not yet planted.** Inserted by [`spawn_world`] when
/// the run is a selftest of a garden it built itself (a garden read back from a save has no
/// fixtures at all), and taken away by [`plant_the_meadow`] on the frame it plants it.
#[derive(Resource)]
struct Meadow;

/// **G2's pair, placed out of the rules' own two distances** (2026-09-18).
///
/// The corner exists so that two beetles certainly meet: the eighth check is about what a script
/// does with a `"mate"`, and nothing can be asked of it in a run where the rules never said one.
/// Until today the corner was a tight clump of grass with a beetle four and a half units away on
/// **either side** of it, and that arrangement cannot work, for a reason that is geometry and not
/// luck (`docs/worklog/2026-09-18-garden-leftovers.md` §5.2): a creature eats from a distance,
/// and it stops walking towards the grass the moment it is no longer hungry — so two that came
/// from opposite sides stand still on opposite sides, `2 × reach + a blade` apart, and the rules
/// pair nobody. In sixty-four twenty-second runs the corner made a child inside two seconds
/// thirty-five times and left the check waiting for a chance pairing somewhere else in the garden
/// the other twenty-nine.
///
/// **So they come from the same side, each to a blade of its own.** A creature that is eating
/// stands somewhere between `reach` and `reach + half a blade` from its blade, on the line it
/// walked in on, and never past it. Put the two blades side by side, `apart` from each other, and
/// each beetle straight behind its own: they walk in parallel, and when they stop they are
///
///     √(apart² + (how much further one stopped short than the other)²)
///
/// from each other, with the second term at most half a blade. The rules pair them when that is
/// `mate_reach` or less, which is the whole of the placement:
///
///     apart² + (PLANT_MAX/2)² ≤ mate_reach²
///
/// The other wall is their own bodies — below `2 × BEETLE_RADIUS` they start inside each other
/// and the first thing that happens in the corner is the separation pass pushing them apart — so
/// `apart` is the middle of the window those two leave. With the rules as `world.rb` has them
/// (`reach` 1.1, `mate_reach` 2.0) that window is 0.80 to 1.87 and the corner is 1.34 wide.
///
/// How far **behind** its blade each beetle starts is the same shape of answer, with three walls
/// instead of two. It has to be outside `reach + half a blade`, or the beetle eats where it
/// stands and never walks; inside the smaller of the two genomes' `sight`, or `garden.nearest`
/// does not find the blade to walk to; and inside the ground the corner owns, which is the one
/// that was doing the damage. `spawn_world` keeps [`MEADOW_CLEAR`] of field empty round
/// [`meadow_at`], so the nearest blade of the field's own can be `MEADOW_CLEAR − (how far the
/// beetle stands from the middle)` away — and if that is less than `start`, the beetle's own
/// blade is *not* the nearest plant and `garden.nearest` sends it walking the other way:
///
///     start + apart/2 + start ≤ MEADOW_CLEAR
///
/// Seven of sixteen traced runs had that happening with a start of 4.4: in one, both beetles
/// walked the wrong way to a blade of 0.46 that two other creatures were already eating, and were
/// still on 42 of their meters five seconds later; in others, one of the pair did. The middle of
/// the window the three walls leave is 2.73 with today's numbers, where the old corner said 4.5 —
/// and at 2.73 the nearest blade the field can put down is 5.19 away, which is nearly twice the
/// beetle's whole walk.
///
/// **Nothing here is a copy of a number in `world.rb`.** `reach` and `mate_reach` arrive through
/// `garden.rules` like `day_length` does ([`RuleBook`]), which is why this is not part of
/// [`spawn_world`]: `spawn_world` is a `Startup` system and the world's script has not run a line
/// by then. It waits for [`Reaches::told`] — or for [`WorldTrouble`], which is the game finding
/// out at startup that the rules will never speak at all, and the corner then stands on the
/// game's own [`REACH`] and [`MATE_REACH`].
///
/// The size of a blade is the *garden's* number and not the rules': [`Furniture::plant_grown`] is
/// what `spawn_plant` is told to make these two, and `world.rb`'s `plant_max` is the cap its
/// growth stops at. **S5b-4 gave them different names**, because they were never the same
/// sentence: they are the same number today and nothing in either file holds them together.
#[allow(clippy::too_many_arguments)]
fn plant_the_meadow(
    mut commands: Commands,
    look: Option<Res<Look>>,
    ruby: Res<RubyDir>,
    mut brains: ResMut<Brains>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    reaches: Res<Reaches>,
    trouble: Res<WorldTrouble>,
    place: Res<Place>,
    built: Res<Furniture>,
    bodies: Res<Bodies>,
) {
    // the rules have spoken, or the game knows they never will
    if !reaches.told && trouble.0.is_none() {
        return;
    }
    commands.remove_resource::<Meadow>();
    let look = look.as_deref();

    // and they are not alike: `mix` averaging two copies of one thing would say nothing
    let lovers =
        [Genome { speed: 2.0, sight: 7.0, appetite: 0.9 }, Genome { speed: 2.4, sight: 9.0, appetite: 1.1 }];

    let corner = meadow_at(&place);
    let half_blade = built.plant_grown * 0.5;
    // how far from its blade a creature may be and still eat it (`world.rb`: `arm = reach + cap * 0.5`)
    let arm = reaches.eat + half_blade;
    // √(mate_reach² − (half a blade)²), and the two beetles' own bodies under it
    let touching = 2.0 * bodies.beetle;
    let widest = (reaches.mate * reaches.mate - half_blade * half_blade).max(0.0).sqrt();
    let apart = (touching + widest.max(touching)) * 0.5;
    // outside the arm, inside the shorter sight, and inside the ground the corner owns; in the
    // middle of what those leave
    let sight = lovers.iter().fold(f32::INFINITY, |least, g| least.min(g.sight));
    let own_ground = (MEADOW_CLEAR - apart * 0.5) * 0.5;
    let start = (arm + sight.min(own_ground)) * 0.5;

    // the corner is the field's near left one, so "into the field" is +x and the two blades
    // stand one behind the other along z
    let toward = Vec2::X;
    let along = Vec2::Y;
    for (i, genome) in lovers.into_iter().enumerate() {
        let side = if i == 0 { 0.5 } else { -0.5 };
        let blade = corner + along * apart * side;
        spawn_plant(&mut commands, look, blade, built.plant_grown, i == 0);
        let lover =
            spawn_creature(&mut commands, look, &bodies, Species::Beetle, blade + toward * start, 45.0, genome, None);
        give_mind(&mut commands, &ruby.0, &mut brains, &mut mrb, lover, Species::Beetle);
    }
    info!(
        "selftest: two hungry beetles {apart:.2} apart, each {start:.2} behind a blade of its own at ({:.1}, {:.1}) — from the rules' reach {:.2} and mate_reach {:.2}{}",
        corner.x,
        corner.y,
        reaches.eat,
        reaches.mate,
        if reaches.told { "" } else { " (the game's own: the rules never spoke)" },
    );
}


/// The selftest's tester (G3). It is a creature by the prelude's reckoning — it has to be, because
/// `garden` is a `Creature` method — and it never moves.
const TESTER: &str = r#"creature "Tester" do
  def run
    answer = garden.spawn(species: "Beetle", genome: {speed: 2.0, appetite: 1.0}, at: [0.0, 0.0])
    log "a genome with no sight: #{answer}"
    sleep 1000
  end
end
"#;

/// A plant: the entity carries the component and the scale, the model hangs under it. Splitting
/// them is what lets `Transform.scale` be the plant's size without the model having to know.
fn spawn_plant(commands: &mut Commands, look: Option<&Look>, at: Vec2, size: f32, round: bool) -> Entity {
    let mut plant = commands.spawn((
        Plant { size },
        Transform::from_xyz(at.x, 0.0, at.y).with_scale(Vec3::splat(size)),
        Visibility::default(),
    ));
    if let Some(look) = look {
        // Kenney's grass is about a quarter of a unit high, and a plant here is a tuft a beetle
        // can hide in: the model is scaled up inside the child, where nothing Ruby reads is
        let (model, scale) =
            if round { (look.bush.clone(), look.bush_scale) } else { (look.tuft.clone(), look.tuft_scale) };
        plant.with_children(|plant| {
            plant.spawn((WorldAssetRoot(model), Transform::from_scale(Vec3::splat(scale))));
        });
    }
    plant.id()
}

/// A tree, and a `Collider` that does not move. `Tree`, not `Plant` — walking into grass is
/// eating, walking into a tree is not.
fn spawn_tree(commands: &mut Commands, look: Option<&Look>, bodies: &Bodies, at: Vec2) {
    let mut tree = commands.spawn((
        Tree,
        Collider { radius: bodies.tree },
        Transform::from_xyz(at.x, 0.0, at.y),
        Visibility::default(),
    ));
    if let Some(look) = look {
        tree.with_children(|tree| {
            tree.spawn((
                WorldAssetRoot(look.tree.clone()),
                Transform::from_scale(Vec3::splat(look.tree_scale)),
            ));
        });
    }
}

/// A rock: the same immovable circle, and a boulder squashed a little so that nine of them do
/// not look like nine of one thing.
fn spawn_rock(commands: &mut Commands, look: Option<&Look>, bodies: &Bodies, at: Vec2, squash: f32) {
    let mut rock = commands.spawn((
        Rock,
        Collider { radius: bodies.rock },
        Transform::from_xyz(at.x, 0.0, at.y),
        Visibility::default(),
    ));
    if let Some(look) = look {
        rock.with_children(|rock| {
            rock.spawn((
                WorldAssetRoot(look.rock.clone()),
                Transform::from_scale(Vec3::new(
                    look.rock_scale[0] * squash,
                    look.rock_scale[1],
                    look.rock_scale[0] / squash,
                )),
            ));
        });
    }
}

/// A creature: the same split. The entity's `Transform` is position and facing and nothing else,
/// which is what a brain reads and writes; the model is a child — which is exactly why G0a swapped
/// six primitives for six `.glb` files without a single component changing.
fn spawn_creature(
    commands: &mut Commands,
    look: Option<&Look>,
    bodies: &Bodies,
    species: Species,
    at: Vec2,
    hunger: f32,
    genome: Genome,
    parent: Option<Entity>,
) -> Entity {
    let mut entity = commands.spawn((
        Creature { species, age: 0.0, genome, parent },
        Hunger(hunger),
        Velocity(Vec2::ZERO),
        // G2: how far it sees is its genome's, not its species'. `Sight` stays a component of
        // its own because that is what `answer_garden` reads to decide how far `garden.nearest`
        // may look, and what a script reads with `me[:Sight]`.
        Sight(genome.sight),
        Collider { radius: bodies.of(species) },
        Breeding { ready_at: 0.0, partner: Entity::PLACEHOLDER },
        Memory,
        Transform::from_xyz(at.x, 0.0, at.y),
        Visibility::default(),
    ));
    if let Some(look) = look {
        // the models face +Z and the world's heading does too (`move_creatures` turns the parent),
        // so the child only sets the size
        let (model, scale) = match species {
            Species::Beetle => (look.beetle.clone(), look.beetle_scale),
            Species::Rabbit => (look.rabbit.clone(), look.rabbit_scale),
        };
        entity.with_children(|body| {
            // G8: `Tint` is what `tint_species` reads when the model has finished arriving. It is
            // on the child rather than on the creature because the child is the entity a
            // `WorldInstanceReady` names, and because a plant, a tree and a rock hang the same
            // way and must not be touched.
            body.spawn((
                WorldAssetRoot(model),
                Transform::from_scale(Vec3::splat(scale)),
                Tint(species),
            ));
        });
    }
    entity.id()
}

/// **Washing a model in its species' colour (G8).**
///
/// The author could not tell a rabbit from a beetle. The reason is in the pack: Kenney's Cube Pets
/// index one 512 × 512 `colormap.png` which is a *palette* — flat squares of colour, no shading at
/// all — and the two animals pick neighbouring browns out of it. A tint that multiplied the
/// palette would give a dark brown and a darker brown, so the clone drops the texture and the
/// colour is the material's own. What is lost is the eyes; what is gained is that the two animals
/// are a cream one and a blue-green one from across the field, which is the thing that was asked
/// for.
///
/// It runs on `WorldInstanceReady`, which is triggered on the entity that carries the
/// `WorldAssetRoot` once the loader has actually built the model's entities — there is no earlier
/// moment, because before it there are no meshes to re-dress. A model with no [`Tint`] (a tree, a
/// rock, a tuft of grass) is left alone on the first line.
fn tint_species(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    tinted: Query<&Tint>,
    worn: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tints: ResMut<Tints>,
) {
    let Ok(tint) = tinted.get(ready.entity) else {
        return;
    };
    let (r, g, b) = species_tint(tint.0);
    for part in children.iter_descendants(ready.entity) {
        let Ok(worn) = worn.get(part) else {
            continue;
        };
        let key = (tint.0.index(), worn.id());
        let copy = match tints.0.get(&key) {
            Some(handle) => handle.clone(),
            None => {
                let Some(source) = materials.get(worn.id()) else {
                    continue;
                };
                let mut copy = source.clone();
                copy.base_color = Color::srgb(r, g, b);
                copy.base_color_texture = None;
                let handle = materials.add(copy);
                tints.0.insert(key, handle.clone());
                handle
            }
        };
        commands.entity(part).insert(MeshMaterial3d(copy));
    }
}

/// The mind: `ruby/prelude.rb` and one creature file, compiled together and hung on the entity as
/// a `Script`. rubevy starts it on the next frame, and from then on the creature moves because a
/// Ruby task says so.
fn give_mind(
    commands: &mut Commands,
    ruby: &Path,
    brains: &mut Brains,
    mrb: &mut Assets<MrbAsset>,
    entity: Entity,
    species: Species,
) {
    // **Compiled once per species, not once per creature** (S5b-4, rubevy's R2). The first
    // creature of a species compiles its file and leaves what it made in `Brains`
    // ([`Brains::first_program`]); every creature after it — and every child born for the rest
    // of the run — is handed that same `Handle<MrbAsset>`, which is what the editor's Apply has
    // always handed the whole species.
    //
    // G4: a species whose file has been rewritten in the editor and not saved runs the text that
    // was applied, and so does anything born into it afterwards — the brain belongs to the
    // species, not to the creature, because the file does. That is why there is nothing to
    // invalidate here: every path that changes what a species is running goes through
    // `Brains::hand_over`, which replaces this.
    if brains.wearing(species).is_none() {
        let applied = brains.text(species).cloned();
        let compiled = match applied.as_deref() {
            Some(text) => compile_source(ruby, species.file(), text, mrb),
            None => compile(ruby, &brains.path(ruby, species), mrb),
        };
        match compiled {
            Ok((handle, prelude_lines)) => {
                brains.first_program(species, handle, prelude_lines, applied.is_some())
            }
            Err(why) => {
                error!("{why}");
                return;
            }
        }
    }
    let Some(worn) = brains.wearing(species) else { return };
    let (handle, prelude_lines, in_memory, generation) =
        (worn.handle.clone(), worn.prelude_lines, worn.in_memory, worn.generation);
    let name = format!("{} {}", species.name(), entity);
    commands.entity(entity).insert((
        Script::new(handle).with_name(&name).with_priority(100),
        Mind {
            name,
            species,
            last_instructions: 0,
            spent: 0,
            frames: 0,
            at: String::new(),
            prelude_lines,
            in_memory,
            // S7: what the species is wearing *now*. A creature born in a frame where the editor
            // has already applied is born on the new program and is not behind; one born earlier
            // in a frame where the editor applies later carries the old number and is caught up
            // on the next frame.
            generation,
            own_line: None,
            heat: Vec::new(),
            ran_frame: 0,
            asked_frame: None,
            ask_trips: 0,
            ask_frames: 0,
            decisions: 0,
            decision_instructions: 0,
            pass_start: None,
        },
    ));
}

/// The prelude and one creature's file, compiled to bytecode in process (the reference compiler),
/// so the game reads `.rb` and nothing has to be built ahead of time. The two are compiled as one
/// program, which is why neither needs a `require`; the answer says how many lines the prelude
/// added, so a line can be reported in the author's own terms.
///
/// The error is the compiler's, already put back into those terms ([`in_the_authors_lines`]) —
/// the caller logs it and shows it, and there is nothing left for either of them to work out.
fn compile(
    ruby: &Path,
    creature: &Path,
    mrb: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    let body = platform::read(creature)?;
    let name = creature.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    compile_source(ruby, &name, &body, mrb)
}

/// The same, for a creature whose file is not a file: the selftest's tester (G3), which is four
/// lines of Ruby in this source and wants the prelude in front of it like any other creature.
///
/// The putting-together and the line numbers are rubevy's since S2 ([`Program`],
/// [`in_the_authors_lines`], R6): what this file had were the two functions every game that lets
/// a player write Ruby has to write, and they are the same in sabibots. `Program::new` counts
/// the lines off the text it really put in front instead of the `prelude.lines().count() + 2`
/// that stood here — the same number for a prelude read from a file, which is the only kind the
/// garden has, and the right one for a prelude that does not end with a newline.
fn compile_source(
    ruby: &Path,
    name: &str,
    body: &str,
    mrb: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    let prelude = platform::read(&ruby.join(PRELUDE_FILE))?;
    let program = Program::new(&prelude, name, body, "run_creature");
    match platform::compile(&program.source, name) {
        Ok(bytes) => Ok((mrb.add(MrbAsset { bytes }), program.prelude_lines)),
        Err(e) => Err(in_the_authors_lines(&e, program.prelude_lines, PRELUDE_FILE)),
    }
}

/// **The world's rules, compiled and hung on an entity of their own** (W1).
///
/// It is `give_mind`'s shape with everything a creature needs taken out. There is one of these
/// and there is no `Mind`: the HUD's columns are about creatures, and what the world's script
/// costs is measured by `WorldMeter` instead, because it is one task that runs one pass a frame
/// and does not have to be told apart from anything.
///
/// **A `world.rb` that will not compile is not a reason to refuse to start.** The file is one the
/// player is invited to edit, and a typo in it should leave a garden that runs — the sun turns,
/// the creatures walk, nothing grows and nobody gets hungry — with a sentence on the HUD saying
/// so. That is [`WorldTrouble`], and it is the whole of the error handling here.
fn give_the_world_its_rules(
    mut commands: Commands,
    ruby: Res<RubyDir>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut trouble: ResMut<WorldTrouble>,
) {
    let entity = commands.spawn(WorldScript).id();
    match compile_world(&ruby.0, &mut mrb) {
        Ok((handle, prelude_lines)) => {
            commands.insert_resource(WorldPrelude(prelude_lines));
            // the priority rubevy gives a script by default. A creature's is set because a
            // creature has handlers that must be looked at before its behaviour (`give_mind`);
            // the world has one task and nothing to be ahead of.
            commands.entity(entity).insert(Script::<World>::for_vm(handle).with_name("world"));
        }
        Err(why) => {
            error!("the world has no rules: {why}");
            trouble.0 = Some(why);
        }
    }
}

/// `ruby/world.rb`, as the file says it.
fn compile_world(
    ruby: &Path,
    mrb: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    let body = platform::read(&ruby.join(WORLD_FILE))?;
    compile_world_source(ruby, &body, mrb)
}

/// **Put a compiled `world.rb` on the world's entity in place of the one it is wearing.**
///
/// It is `window::restart_species` for a garden that has one of the thing rather than a dozen,
/// and it is the road W1's twelfth check already drove (`swap_the_rules`); W3 made it a function
/// because the editor's `F3` drives the same one. Dropping the `ScriptTask<World>` is what ends
/// the old rules: rubevy terminates the task and closes the queues it was subscribed to, and the
/// timer tasks `every` made — which are asleep and have no subscription to lose — stop themselves
/// the next time they wake, because `run_world` wrote a new `$world_being` over theirs
/// (`ruby/world_prelude.rb`). The new `Script` becomes a task on the next frame.
///
/// **What does not come back is the world's state.** `@season` is an instance variable of the
/// object the old script made, and the new script makes a new one — so a garden whose rules are
/// replaced opens in the wet season again, exactly as a beetle handed a new behaviour has
/// forgotten what it ate. What *is* kept is everything the rules wrote into components: a
/// `Breeding` cooldown outlives the rule that set it, which is most of why it is a component
/// (W2).
fn wear_the_rules(
    commands: &mut Commands,
    entity: Entity,
    handle: Handle<MrbAsset>,
    prelude_lines: u32,
) {
    // S3: the same three lines `restart_species` gave to rubevy in S2, on the world's VM this
    // time — `replace_script` is generic in the VM, so the `World` marker rides along
    replace_script(commands, entity, Script::<World>::for_vm(handle).with_name("world"));
    // the new program may have a prelude of a different length (`world_prelude.rb` saved), and
    // the panel's line numbers are worked out from it every frame
    commands.insert_resource(WorldPrelude(prelude_lines));
}

/// `world_prelude.rb` and one world, compiled as one program — which is why neither needs a
/// `require` — with `run_world` on the end, exactly as a creature's file gets `run_creature`.
///
/// It answers the same pair `compile_source` does. The line offset used to be left out here,
/// because the only readers of it were a creature's editor band and the HUD's line column; a
/// compiler's error is the third reader and it belongs to both files
/// ([`in_the_authors_lines`]).
fn compile_world_source(
    ruby: &Path,
    body: &str,
    mrb: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    let prelude = platform::read(&ruby.join(WORLD_PRELUDE_FILE))?;
    let program = Program::new(&prelude, WORLD_FILE, body, "run_world");
    let bytes = platform::compile(&program.source, WORLD_FILE)
        .map_err(|e| in_the_authors_lines(&e, program.prelude_lines, WORLD_PRELUDE_FILE))?;
    Ok((mrb.add(MrbAsset { bytes }), program.prelude_lines))
}

fn spawn_camera(mut commands: Commands, orbit: Res<Orbit>, scenery: Res<Scenery>) {
    // G8: the fog. Its colour and its two distances are `horizon_look`'s from the first frame, the
    // way the sun's are `day_night`'s; what is set here is that there is one at all.
    commands.spawn((
        Camera3d::default(),
        camera_at(&orbit),
        DistanceFog {
            color: Color::srgb(scenery.fog_color[0], scenery.fog_color[1], scenery.fog_color[2]),
            falloff: FogFalloff::Linear {
                start: scenery.fog_at_first[0],
                end: scenery.fog_at_first[1],
            },
            ..default()
        },
    ));
}

fn camera_at(orbit: &Orbit) -> Transform {
    let focus = Vec3::new(orbit.focus.x, 0.0, orbit.focus.y);
    let eye = focus
        + Vec3::new(
            orbit.distance * orbit.pitch.cos() * orbit.yaw.sin(),
            orbit.distance * orbit.pitch.sin(),
            orbit.distance * orbit.pitch.cos() * orbit.yaw.cos(),
        );
    Transform::from_translation(eye).looking_at(focus, Vec3::Y)
}

/// The two directions a pan can go, on the ground, as the window sees them: to the right of the
/// screen, and away from the viewer. They are the camera's own axes flattened onto XZ, so panning
/// after turning the camera goes where the eye expects rather than where the world's X is.
fn ground_axes(yaw: f32) -> (Vec2, Vec2) {
    let (s, c) = yaw.sin_cos();
    // the camera stands at +Z when the yaw is 0 and looks towards -Z; its right is +X
    (Vec2::new(c, -s), Vec2::new(-s, -c))
}

/// Drag to turn round the garden, drag with the other button to slide it, wheel to come closer,
/// `Home` to put it all back. The whole camera, because a world seen from one fixed angle does
/// not look like a world — and, since G6, one seen from one fixed *place* does not either: the
/// field is forty by thirty and a beetle in a corner was something you could turn towards but
/// never go to.
///
/// **What the panels keep** (2026-09-18). The author scrolled the editor's listing and the garden
/// zoomed out under it: the wheel was the one gesture that was never asked whether egui wanted it.
/// The drag was (G4) and the keyboard was, so the hole was in one loop and not in the idea — a
/// wheel message is read here whatever the pointer is over, and egui reads the same message for
/// its `ScrollArea`, so both happened at once.
///
/// The question asked is `wants_pointer_input() || is_pointer_over_area()`, which is wider than
/// the `wants_pointer_input()` the drag used alone. egui's own `wants_pointer_input` is
/// `is_using_pointer() || (is_pointer_over_area() && no button is down)` — so a pointer resting on
/// a panel with a button held is *not* wanted by egui, and a wheel turned there would have come
/// back to the camera. Over an area is over an area, whatever the buttons are doing.
///
/// Widening it would have cost the drags something, though: a turn that begins on the grass and
/// sweeps across the editor would stop dead half way. So a drag is decided **when the button goes
/// down** ([`orbit_camera`]'s `grabbed`): one that began on the garden stays the camera's wherever
/// the pointer goes, and one that began on a panel never becomes the camera's however far it is
/// dragged out. That is what the old code did by accident, through the `any_down` in egui's own
/// definition, and it is said here on purpose.
#[allow(clippy::too_many_arguments)]
fn orbit_camera(
    time: Res<Time>,
    mut orbit: ResMut<Orbit>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Option<Res<bevy_egui::input::EguiWantsInput>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
    watch: Option<Res<CameraLog>>,
    eye: Res<Eye>,
    place: Res<Place>,
    mut said_at: Local<f32>,
    // whether the drag under way is the camera's: decided on the press, held until the buttons
    // are all up again
    mut grabbed: Local<bool>,
) {
    let was = *orbit;
    // G4: a drag inside a panel is the panel's, not the camera's; nor is a key typed into the
    // editor the camera's, or `w` in a creature's brain would slide the garden about. The wheel
    // joined the first of those in 2026-09-18 (see above).
    let (egui_pointer, mine_keys) = match pointer {
        Some(p) => (p.wants_pointer_input() || p.is_pointer_over_area(), !p.wants_keyboard_input()),
        None => (false, true),
    };
    let dragging = [MouseButton::Left, MouseButton::Right];
    if !buttons.any_pressed(dragging) {
        *grabbed = false;
    } else if buttons.any_just_pressed(dragging) {
        *grabbed = !egui_pointer;
    }
    let mine = *grabbed;
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    // left drag turns; right drag — or Shift and left, for a trackpad and for a browser that
    // keeps the right button for its own menu — slides
    let turning = mine && buttons.pressed(MouseButton::Left) && !shift;
    let sliding = mine
        && (buttons.pressed(MouseButton::Right) || (buttons.pressed(MouseButton::Left) && shift));
    let (right, away) = ground_axes(orbit.yaw);
    for m in motion.read() {
        if turning {
            orbit.yaw -= m.delta.x * eye.turn_per_pixel;
            orbit.pitch =
                (orbit.pitch + m.delta.y * eye.turn_per_pixel).clamp(eye.pitch_min, eye.pitch_max);
        } else if sliding {
            // the ground is grabbed and pulled: the mouse moves right, the garden moves right,
            // so the point the camera looks at moves left
            let step = eye.pan_per_pixel * orbit.distance;
            let d = right * -m.delta.x * step + away * m.delta.y * step;
            orbit.focus = clamp_focus(&eye, &place, orbit.focus + d);
        }
    }
    for w in wheel.read() {
        // read either way, so that a wheel turned over a panel is not still sitting in the reader
        // when the pointer comes back off it
        if egui_pointer {
            continue;
        }
        let notches = notches_of(w.unit, w.y);
        let was_at = orbit.distance;
        orbit.distance = zoom_by(&eye, orbit.distance, notches);
        if watch.is_some() {
            // one line per wheel *message*, because the message is what the browser and the
            // window disagreed about: the unit and the raw number are in it, so the log says
            // what arrived as well as what was done with it
            info!(
                "camera: wheel {:?} y={} -> {notches:.3} notches, distance {was_at:.2} -> {:.2}",
                w.unit, w.y, orbit.distance
            );
        }
    }
    if mine_keys {
        let mut d = Vec2::ZERO;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            d += away;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            d -= away;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            d += right;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            d -= right;
        }
        if d != Vec2::ZERO {
            let step = eye.pan_per_second * orbit.distance * time.delta_secs();
            orbit.focus = clamp_focus(&eye, &place, orbit.focus + d.normalize_or_zero() * step);
        }
        if keys.just_pressed(KeyCode::Home) {
            *orbit = Orbit::home(&eye);
        }
    }
    if *orbit != was {
        let at = camera_at(&orbit);
        for mut transform in &mut cameras {
            *transform = at;
        }
        // a drag is sixty messages a second and the wheel has already said its piece, so the
        // rest of the camera's life is sampled rather than logged
        let now = time.elapsed_secs();
        if watch.is_some() && now - *said_at > 0.25 {
            *said_at = now;
            info!(
                "camera: yaw {:.3} pitch {:.3} distance {:.2} focus ({:.2}, {:.2})",
                orbit.yaw, orbit.pitch, orbit.distance, orbit.focus.x, orbit.focus.y
            );
        }
    }
}

/// `GARDEN_SELFTEST=1`, or `?selftest` in a page's address: the camera says what it is doing.
///
/// G6 had to answer "how many steps does one turn of the wheel have *in a browser*", and there is
/// nothing to read from outside a wasm canvas — no window title, no `Transform` to query, and the
/// camera is not in the HUD. A console line is the one thing playwright can collect, so the
/// checks make the camera write one. Off in an ordinary run: this is a game, not a log.
#[derive(Resource)]
struct CameraLog;

/// The eye may go a little past the wall, and no further: a garden you can lose is not a garden.
fn clamp_focus(eye: &Eye, place: &Place, focus: Vec2) -> Vec2 {
    let (w, d) = (place.half_w() + eye.pan_limit, place.half_d() + eye.pan_limit);
    Vec2::new(focus.x.clamp(-w, w), focus.y.clamp(-d, d))
}

// ---------------------------------------------------------------------------------------------
// The rules. Every one of them is a Rust system; none of them knows about Ruby except through
// `ScriptWorld::publish`, which is a no-op while nobody has subscribed.
// ---------------------------------------------------------------------------------------------

/// The sun goes round once a minute: where it is decides the light's direction, its colour and
/// its strength, the ambient light and the colour of the sky. The moment it goes under, `"night"`
/// is published to every script that asked for it; at sunrise, `"day"`.
/// Where the sun stands at a given point in the day: sunrise at phase 0, overhead at 0.25, gone
/// at 0.5. G8 gave it a name of its own because the horizon wants the same number `day_night`
/// does, a system later in the same frame.
fn sun_up(phase: f32) -> Vec3 {
    let angle = phase * std::f32::consts::TAU;
    Vec3::new(angle.cos() * 0.8, angle.sin(), 0.35).normalize()
}

/// **The sky's two colours at a given moment (G8):** what it is along the horizon, and what it is
/// straight overhead.
///
/// The first of them is exactly the number `ClearColor` was before G8 — the day's blue and G6b's
/// [`NIGHT_SKY`] — because that is the one the night was *measured* against, and a horizon that
/// changed would make the two sets of measurements in `docs/garden.md` mean different things. The
/// gradient is all above it: the sky gets deeper towards the top, which is what a sky does and
/// what makes a dome read as a dome rather than as a wall.
fn sky_colors(light: &Light, height: f32, night: bool, dial: f32) -> (Color, Color) {
    if night {
        let [r, g, b] = light.night_sky;
        // the same blue, turned up with the rest of it: a sky that stayed put while the ground
        // brightened would read as fog rather than as a lighter night
        let lit = |k: f32| {
            Color::srgb((r * k * dial).min(1.0), (g * k * dial).min(1.0), (b * k * dial).min(1.0))
        };
        (lit(1.0), lit(light.night_zenith))
    } else {
        let noon = height.clamp(0.0, 1.0);
        (
            Color::srgb(0.35 + 0.15 * noon, 0.5 + 0.22 * noon, 0.7 + 0.22 * noon),
            Color::srgb(0.15 + 0.11 * noon, 0.33 + 0.21 * noon, 0.64 + 0.24 * noon),
        )
    }
}

fn day_night(
    time: Res<Time>,
    mut sky: ResMut<Sky>,
    mut world: ResMut<ScriptWorld>,
    mut test: Option<ResMut<SelfTest>>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    ambient: Option<ResMut<GlobalAmbientLight>>,
    clear: Option<ResMut<ClearColor>>,
    // G6b. `Option` for the same reason the two above it are: the headless run has no light to
    // turn up and no panel to turn it up with
    dial: Option<Res<NightDial>>,
    light: Res<Light>,
) {
    let dial = dial.map(|d| d.0).unwrap_or(1.0);
    // the world's clock, not the process's: a garden read back from a file goes on from the hour
    // it was saved at (`Sky::shift`), and in a run that loaded nothing the two are the same number
    let now = world_now(&time, &sky);
    // **the length of the day is `world.rb`'s** (W1). `day_length 60.0` in that file reaches this
    // line through `garden.rules`; until it does — and for ever, if that file will not compile —
    // it is [`DAY_LENGTH`]. A `day_length` of zero would divide by it, so a rule that asks for
    // nonsense is ignored rather than obeyed (`answer_world`).
    sky.phase = (now / sky.day_length + light.dawn_offset).fract();
    let up = sun_up(sky.phase);
    let height = up.y;
    let night = height <= 0.0;

    // at night the light comes from where the sun is not: a moon, blue and much weaker than the
    // sun, still casting the shadows that say the world is 3D
    let from = if night { -up } else { up };
    let (color, illuminance) = if night {
        (Color::srgb(0.62, 0.70, 1.0), light.moon_lux * dial)
    } else {
        // low sun is orange, high sun is white
        let noon = height.clamp(0.0, 1.0);
        (
            Color::srgb(1.0, 0.72 + 0.24 * noon, 0.45 + 0.5 * noon),
            light.day_lux[0] + light.day_lux[1] * noon,
        )
    };
    for (mut transform, mut light) in &mut sun {
        *transform = Transform::from_translation(from * 60.0).looking_at(Vec3::ZERO, Vec3::Y);
        light.color = color;
        light.illuminance = illuminance;
    }
    // `GlobalAmbientLight` and `ClearColor` come with the render plugins; a headless run has
    // neither, and everything above it still runs
    if let Some(mut ambient) = ambient {
        if night {
            ambient.color = Color::srgb(0.45, 0.54, 0.85);
            ambient.brightness = light.night_ambient * dial;
        } else {
            ambient.color = Color::srgb(0.7, 0.8, 1.0);
            ambient.brightness = light.day_ambient[0] + light.day_ambient[1] * height.clamp(0.0, 1.0);
        }
    }
    if let Some(mut clear) = clear {
        // G8: the sky the window actually shows is the dome (`horizon_look`), and this is what is
        // behind it — the same colour the dome's rim is, so that nothing the dome fails to cover
        // is a different blue. In a `--headless` run there is no `ClearColor` and this is skipped
        // whole, as it was before.
        clear.0 = sky_colors(&light, height, night, dial).0;
    }

    if night != sky.night {
        sky.night = night;
        let name = if night { "night" } else { "day" };
        world.publish(None, name, Answer::Num(now as f64));
        info!("{name} at {now:.1} s");
        if night && let Some(test) = test.as_mut() && test.night_at.is_none() {
            test.night_at = Some(now);
        }
    }
}

/// **The horizon (G8), once a frame, in the window only.**
///
/// Three small things, and each of them is a decision:
///
/// *The sky and the far ground ride with the camera.* The dome is centred on the eye and the
/// plane is slid under it on XZ, so the rim of one and the edge of the other are always exactly
/// as far away as they were last frame. That is what lets both be small — five hundred units and
/// three hundred — instead of being sized for the worst case of a camera at the far corner of its
/// range, and it is why the default far plane (1,000) is enough and the projection is untouched.
/// The plane keeps its own height, because sliding it up and down would show.
///
/// *The fog's colour is the sky's rim, and its distance is the zoom's.* Those two together are
/// the whole trick: the ground fades into exactly the colour of the sky it meets, so the line
/// where they meet is not a line, and because the fog begins [`FOG_NEAR`] past whatever the
/// camera's own distance is, no amount of zooming out puts haze on the garden.
///
/// *The gradient is rewritten, not recomputed in a shader.* 49 vertices, and only when the colour
/// has moved by more than a 255th — which in a sixty-second day is about four times a second,
/// against sixty frames.
fn horizon_look(
    sky: Res<Sky>,
    orbit: Res<Orbit>,
    dial: Res<NightDial>,
    light: Res<Light>,
    scenery: Res<Scenery>,
    mut dome: ResMut<SkyDome>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    mut fog: Query<&mut DistanceFog>,
    mut shell: Query<&mut Transform, (With<SkyShell>, Without<Horizon>)>,
    mut ground: Query<&mut Transform, (With<Horizon>, Without<SkyShell>)>,
) {
    let up = sun_up(sky.phase);
    let (horizon, zenith) = sky_colors(&light, up.y, up.y <= 0.0, dial.0);
    let (horizon, zenith) = (horizon.to_linear(), zenith.to_linear());

    if let Ok(eye) = camera.single() {
        let at = eye.translation();
        for mut transform in &mut shell {
            transform.translation = at;
        }
        for mut transform in &mut ground {
            transform.translation = Vec3::new(at.x, HORIZON_DROP, at.z);
        }
    }

    for mut fog in &mut fog {
        fog.color = Color::LinearRgba(horizon);
        let start = (orbit.distance + scenery.fog_near).min(FOG_NEAR_MAX);
        fog.falloff = FogFalloff::Linear { start, end: start + scenery.fog_depth };
    }

    let moved = |a: LinearRgba, b: LinearRgba| {
        (a.red - b.red).abs().max((a.green - b.green).abs()).max((a.blue - b.blue).abs()) > 1.0 / 255.0
    };
    if dome.was.is_some_and(|(h, z)| !moved(h, horizon) && !moved(z, zenith)) {
        return;
    }
    let Some(mut mesh) = meshes.get_mut(&dome.mesh) else {
        return;
    };
    let colors: Vec<[f32; 4]> = dome
        .up
        .iter()
        .map(|t| {
            // the rim's colour is held a little way up the dome before the climb starts, so that
            // the band the eye actually sees along the horizon is the colour the fog is
            let t = (t * 1.25 - 0.1).clamp(0.0, 1.0);
            let c = horizon.mix(&zenith, t);
            [c.red, c.green, c.blue, 1.0]
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    dome.was = Some((horizon, zenith));
}

/// `Velocity` into `Transform`, on XZ, and the walls stop it. A creature also turns to face the
/// way it is going, which is the only thing in the game that writes `Transform.rotation`.
fn move_creatures(
    time: Res<Time>,
    place: Res<Place>,
    mut creatures: Query<(&Creature, &mut Velocity, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let (w, d) = (place.half_w() - place.wall_margin, place.half_d() - place.wall_margin);
    for (creature, mut velocity, mut transform) in &mut creatures {
        // a brain may ask for more than the body can give; the rule is the body's, and from G2
        // the body's number comes off its own genome rather than off its species
        let limit = creature.genome.speed;
        if velocity.0.length() > limit {
            velocity.0 = velocity.0.normalize_or_zero() * limit;
        }
        let mut x = transform.translation.x + velocity.0.x * dt;
        let mut z = transform.translation.z + velocity.0.y * dt;
        if x < -w || x > w {
            x = x.clamp(-w, w);
            velocity.0.x = 0.0;
        }
        if z < -d || z > d {
            z = z.clamp(-d, d);
            velocity.0.y = 0.0;
        }
        transform.translation.x = x;
        transform.translation.z = z;
        if velocity.0.length_squared() > 0.04 {
            transform.rotation = Quat::from_rotation_y((-velocity.0.x).atan2(-velocity.0.y));
        }
    }
}

/// Where the walls let a creature stand: the same half unit off the edge that `move_creatures`
/// keeps, said once so that [`separate`] and the wall can be talked about in the same terms.
fn inside_the_walls(place: &Place, at: Vec2) -> Vec2 {
    let (w, d) = (place.half_w() - place.wall_margin, place.half_d() - place.wall_margin);
    Vec2::new(at.x.clamp(-w, w), at.y.clamp(-d, d))
}

/// The part of a step that the walls will not allow — zero in the middle of the garden, and what
/// is left over at the edge of it. It is what an immovable thing refuses to give, measured, so
/// that [`separate`] can hand it to whoever else is in the pair.
fn refused_by_the_walls(place: &Place, from: Vec2, step: Vec2) -> Vec2 {
    let want = from + step;
    want - inside_the_walls(place, want)
}

/// Nothing walks through anything solid. Circles on XZ, pushed apart until they only touch: two
/// creatures give half each, a tree or a rock gives nothing — and so does a wall, which is the
/// same rule said of the edge of the world: what the wall will not let one of a pair have, the
/// other takes. This is the whole of the physics in the game, and it is deliberately not a
/// physics crate — avian or rapier would be a megabyte of
/// wasm and a second vocabulary for a rule that fits on a screen.
///
/// Neighbours come from a grid of `CELL`-sided squares (`HashMap<(i32, i32), Vec<usize>>`), so the
/// cost is the number of pairs that are actually near each other rather than n². At a dozen
/// creatures it makes no difference; it is here because the world is meant to grow.
///
/// A pair that was not touching last frame and is now gets `"bumped"` published to the creature,
/// with the other thing as a `Rubevy::Entity`.
fn separate(
    mut world: ResMut<ScriptWorld>,
    mut bumps: ResMut<Bumps>,
    place: Res<Place>,
    bodies: Res<Bodies>,
    mut movers: Query<(Entity, &Collider, &mut Transform), With<Creature>>,
    fixed: Query<(Entity, &Collider, &Transform), Without<Creature>>,
) {
    // one list of everything solid: the movers first, so an index below `mover_count` is one
    let mut at: Vec<Vec2> = Vec::new();
    let mut radius: Vec<f32> = Vec::new();
    let mut who: Vec<Entity> = Vec::new();
    for (entity, collider, transform) in &movers {
        // inside the walls before anything is pushed, so that "everything that can move is inside
        // the walls" holds over the whole of this function and the write-back has nothing to
        // correct. It is where the write-back's clamp went (below); a creature that arrived from
        // outside — a hand-edited save is the only way — is still brought in, as it was before.
        at.push(inside_the_walls(&place, Vec2::new(transform.translation.x, transform.translation.z)));
        radius.push(collider.radius);
        who.push(entity);
    }
    let mover_count = at.len();
    for (entity, collider, transform) in &fixed {
        at.push(Vec2::new(transform.translation.x, transform.translation.z));
        radius.push(collider.radius);
        who.push(entity);
    }
    if at.len() < 2 {
        return;
    }

    let mut touching: Vec<(Entity, Entity)> = Vec::new();
    // the grid the pairs are found in, wide enough for the widest body the rules have named
    // ([`Bodies::cell`])
    let cell_side = bodies.cell();
    for pass in 0..place.separate_passes {
        let mut grid: bevy::platform::collections::HashMap<(i32, i32), Vec<usize>> = default();
        for (i, p) in at.iter().enumerate() {
            grid.entry(((p.x / cell_side).floor() as i32, (p.y / cell_side).floor() as i32)).or_default().push(i);
        }
        for i in 0..at.len() {
            let cell = ((at[i].x / cell_side).floor() as i32, (at[i].y / cell_side).floor() as i32);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let Some(near) = grid.get(&(cell.0 + dx, cell.1 + dz)) else { continue };
                    for &j in near {
                        // each pair once, and two immovable things have nothing to settle
                        if j <= i || (i >= mover_count && j >= mover_count) {
                            continue;
                        }
                        let sum = radius[i] + radius[j];
                        let gap = at[j] - at[i];
                        let distance = gap.length();
                        if distance >= sum {
                            continue;
                        }
                        if pass == 0 {
                            touching.push((who[i], who[j]));
                        }
                        // exactly on top of each other: any direction will do, and it must be the
                        // same one every frame or the pair jitters
                        let dir = if distance > 1e-4 {
                            gap / distance
                        } else {
                            Vec2::new(1.0, 0.0)
                        };
                        let push = sum - distance;
                        match (i < mover_count, j < mover_count) {
                            (true, true) => {
                                // Half each — **unless the wall is in the way** (2026-09-18).
                                //
                                // A wall is an immovable thing like a tree or a rock, and the
                                // rule for those is the one written at the top of this function:
                                // an immovable thing gives nothing, and the creature takes the
                                // whole of the push. Said of the wall, that is this: what the
                                // wall refuses one of the pair is handed to the other, so the
                                // pair still settles the whole of `push` between them.
                                //
                                // Before, the halves were taken without asking the wall, and the
                                // write-back's `clamp` pulled the one that had gone through it
                                // back — straight into the other creature. That is the whole of
                                // the fourth check's flakiness: 23 events out of 23 were a pair
                                // the passes had pushed exactly apart (`preclamp` 1.000) and the
                                // clamp had put back together
                                // (`docs/worklog/2026-09-17-selftest-flakes.md` §2).
                                let half = dir * push * 0.5;
                                let refused_i = refused_by_the_walls(&place, at[i], -half);
                                let refused_j = refused_by_the_walls(&place, at[j], half);
                                at[i] = inside_the_walls(&place, at[i] - half - refused_j);
                                at[j] = inside_the_walls(&place, at[j] + half - refused_i);
                            }
                            // and the same for a push off something fixed: it may press a
                            // creature against a wall, and a creature pressed against a wall
                            // stays where the wall is — there is nobody to hand the rest to
                            (true, false) => at[i] = inside_the_walls(&place, at[i] - dir * push),
                            (false, true) => at[j] = inside_the_walls(&place, at[j] + dir * push),
                            (false, false) => {}
                        }
                    }
                }
            }
        }
    }

    // Back into the world. The walls are the pushing's business now (above) and every write into
    // `at` went through `inside_the_walls`, so there is nothing left here for a clamp to move:
    // what comes out is what the passes agreed on, and the check that runs after this sees it.
    let mut i = 0;
    for (_, _, mut transform) in &mut movers {
        transform.translation.x = at[i].x;
        transform.translation.z = at[i].y;
        i += 1;
    }

    // `"bumped"` is published when a contact begins, not on every frame of it: a creature leaning
    // on a tree is pushed a hundred times in the second and a half its heading lasts, and a
    // hundred messages would only fill the queue (rubevy keeps 64 and drops the oldest) and fire
    // G1's `on(:bumped)` over and over for one event. The plan says "the frame it is pushed";
    // this is the first of them.
    let mut now: Vec<(u64, u64)> = Vec::with_capacity(touching.len());
    for (a, b) in &touching {
        let key = pair_key(*a, *b);
        now.push(key);
        if bumps.0.contains(&key) {
            continue;
        }
        // the creature is always the first of the pair, by how the list was built
        world.publish(Some(*a), "bumped", Answer::Entity(*b));
        if movers.get(*b).is_ok() {
            world.publish(Some(*b), "bumped", Answer::Entity(*a));
        }
    }
    bumps.0 = now;
}

/// `GARDEN_SELFTEST=1`: nothing ever gets inside anything. Run after `separate`, so what it sees
/// is the world as the frame leaves it; a dozen creatures and sixteen obstacles is few enough to
/// look at every pair, and a check that walks the same grid as the thing it is checking would be
/// checking the grid against itself.
fn watch_overlap(mut test: ResMut<SelfTest>, solids: Query<(&Collider, &Transform, Option<&Creature>)>) {
    let all: Vec<(Vec2, f32, bool)> = solids
        .iter()
        .map(|(c, t, creature)| (Vec2::new(t.translation.x, t.translation.z), c.radius, creature.is_some()))
        .collect();
    let mut worst = f32::INFINITY;
    for (i, (p, r, creature)) in all.iter().enumerate() {
        for (q, s, other_creature) in all.iter().skip(i + 1) {
            // the rule pushes creatures; two rocks are placed apart and are nobody's business
            if !creature && !other_creature {
                continue;
            }
            let ratio = p.distance(*q) / (r + s);
            worst = worst.min(ratio);
        }
    }
    test.frames += 1;
    if worst < f32::INFINITY {
        test.closest = test.closest.min(worst);
        if worst < 0.9 {
            test.overlaps += 1;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// **What is left of the grass, hunger and eating** (W1).
//
// The rules themselves are `ruby/world.rb`. Five systems stood here — `grow_plants`,
// `sprout_plants`, `get_hungry`, `eat`, `starve` — and what is in their place is three systems
// that know nothing about any rule: one that shows a plant's size, one that puts a plant down
// where the rules asked for one, and one that watches what the rules did and tells the parts of
// the game that were being told from the inside.
// ---------------------------------------------------------------------------------------------

/// A plant's size is also its `Transform.scale` — one number seen two ways, which is what lets a
/// script watch grass grow without the game telling it anything (`docs/garden.md`, the component
/// table).
///
/// `grow_plants` wrote both halves because it was the thing that grew the plant. The rule is
/// Ruby's now and writes the number; the *picture* of the number stays here, where the rest of
/// the look is, and it is the one line of that swap that is not a deletion. It is a plain copy,
/// so it is right whatever changed the size — growth, a bite, or a person editing `world.rb`.
fn plants_wear_their_size(mut plants: Query<(&Plant, &mut Transform), Changed<Plant>>) {
    for (plant, mut transform) in &mut plants {
        transform.scale = Vec3::splat(plant.size);
    }
}

/// **Every body in the garden is the width the rules said it is** (S5b-4).
///
/// The four radii are `world.rb`'s and arrive once through `garden.rules` ([`Bodies`]), but a
/// radius is not only read where it is used: it is written onto the entity as a [`Collider`],
/// because that is what [`separate`] walks and what the fourth check measures. So the handover
/// has to reach what is already standing about — the ten creatures `spawn_world` put down before
/// the world's script had run a line, and everything alive when somebody presses `Ctrl+Enter` on
/// a `world.rb` that says a beetle is wider than it was.
///
/// It is [`plants_wear_their_size`]'s job in the other direction, and it costs what that costs:
/// the `Res<Bodies>::is_changed` is one comparison in a frame where nothing was handed over,
/// which is every frame but the first and the ones an edit lands in. What it writes then is one
/// float per creature, per tree and per rock.
///
/// A creature born after the handover already has the right body ([`spawn_creature`] reads the
/// same resource), so this is only ever catching up the ones that were built first.
fn bodies_wear_the_rules(
    bodies: Res<Bodies>,
    mut creatures: Query<(&Creature, &mut Collider)>,
    mut trees: Query<&mut Collider, (With<Tree>, Without<Creature>)>,
    mut rocks: Query<&mut Collider, (With<Rock>, Without<Creature>, Without<Tree>)>,
) {
    if !bodies.is_changed() {
        return;
    }
    // `Mut::set_if_neq` rather than a plain write, so that a handover that restates the numbers
    // — which is what every `Ctrl+Enter` on an unchanged `world.rb` is — marks nothing changed
    let wear = |collider: &mut Mut<Collider>, radius: f32| {
        if collider.radius != radius {
            collider.radius = radius;
        }
    };
    for (creature, mut collider) in &mut creatures {
        wear(&mut collider, bodies.of(creature.species));
    }
    for mut collider in &mut trees {
        wear(&mut collider, bodies.tree);
    }
    for mut collider in &mut rocks {
        wear(&mut collider, bodies.rock);
    }
}

/// The seeds `world.rb` asked for this frame, put in the ground.
///
/// **Where a seed may land is the garden's furniture; whether one lands is the rule.** So the
/// dice for *when* are in Ruby (`rand < sprout_rate * dt`, and the cap on how many blades the
/// field holds), and what is here is the spot, the refusal to put one on top of another, the size
/// a new blade starts at and whether it is a tuft or a bush — none of which a rule has an opinion
/// about and all of which want `Look`, `Dice` and `Commands`.
///
/// It is the same split as `garden.spawn` and `children_arrive`, and it has the same reason: the system that
/// answers a script has the whole world and no `Commands`.
fn sprout_plants(
    mut commands: Commands,
    look: Option<Res<Look>>,
    mut dice: ResMut<Dice>,
    mut asked: ResMut<Sprouts>,
    place: Res<Place>,
    built: Res<Furniture>,
    plants: Query<&Transform, With<Plant>>,
) {
    let gap = asked.gap;
    for _ in 0..std::mem::take(&mut asked.asked) {
        let at = Vec2::new(
            dice.between(-place.half_w() + 1.0, place.half_w() - 1.0),
            dice.between(-place.half_d() + 1.0, place.half_d() - 1.0),
        );
        // not on top of another one. **How close is on top of is the rules'** (S5b-4,
        // `world.rb`'s `sprout_gap`); the walk over every blade in the field is this system's,
        // for the reason `world.rb`'s own first paragraph gives.
        if plants.iter().any(|t| Vec2::new(t.translation.x, t.translation.z).distance(at) < gap) {
            continue;
        }
        let round = dice.roll() < built.round_chance;
        spawn_plant(&mut commands, look.as_deref(), at, built.plant_min, round);
    }
}

/// **What the rules did, read from the outside** (W1).
///
/// Three things in this game used to be noted by the rule that caused them, because the rule was
/// here: the chewing animation (`eat` inserted `Eating`), the first meal and the first death (the
/// checks' `ate_at` and `starved`). The rules are somebody else's file now, and they say nothing
/// to the game beyond writing components and despawning entities — which is the point, and which
/// means the way to know a creature ate is that **its meter went up**, and the way to know one
/// starved is that **it is not there any more**.
///
/// That is a better test than the one it replaces. `eat` recording its own success proved that
/// `eat` ran; a meter that is higher than it was proves the world changed, whoever changed it, and
/// it would still be true of a `world.rb` that fed creatures in some quite different way.
///
/// It runs after the world's tick, where that VM's writes and despawns land.
fn note_the_rules(
    time: Res<Time>,
    mut commands: Commands,
    mut test: Option<ResMut<SelfTest>>,
    creatures: Query<(Entity, &Creature, &Hunger)>,
) {
    let now = time.elapsed_secs();
    let mut meters: Vec<(Entity, f32)> = Vec::with_capacity(creatures.iter().len());
    let mut alive: Vec<(Entity, Species, f32)> = Vec::with_capacity(creatures.iter().len());
    // what the meters said last frame, and who was here to have one
    //
    // **W2 took a list away from here.** While breeding was Rust's, `hatch` charged a parent
    // thirty points for a child and told this system which parents it had charged, because a
    // meter that fell for *that* reason was not the rules getting anybody hungry — and the
    // twelfth check, which is exactly "no meter fell while the rules were away", counted two of
    // them. Breeding is a rule now and the rule is in the file the check takes away, so a fall
    // while the rules are gone is once again impossible rather than accounted for.
    let mut was: Vec<(Entity, f32)> = Vec::new();
    let mut were: Vec<(Entity, Species, f32)> = Vec::new();
    if let Some(test) = test.as_mut() {
        was = std::mem::take(&mut test.meters);
        were = std::mem::take(&mut test.alive);
    }
    let mut first_meal: Option<f32> = None;
    let mut fell = 0u32;
    for (entity, creature, hunger) in &creatures {
        meters.push((entity, hunger.0));
        alive.push((entity, creature.species, creature.age));
        let Some((_, before)) = was.iter().find(|(e, _)| *e == entity) else { continue };
        if hunger.0 > *before {
            // a mouthful. The model chews for a moment longer than the last bite, so that walking
            // from one blade of grass to the next does not flicker between two clips
            commands.entity(entity).insert(Eating { until: now + 0.35 });
            first_meal.get_or_insert(now);
        } else if hunger.0 < *before {
            fell += 1;
        }
    }
    // and who went. The only thing that despawns a creature is `world.rb`'s own `Rubevy.despawn`
    // on an empty meter — W2 added a way to be *born* and none to go — so a creature that was
    // here and is not is one that starved.
    let gone: Vec<(Entity, Species, f32)> =
        were.into_iter().filter(|(e, ..)| !meters.iter().any(|(a, _)| a == e)).collect();
    for (entity, species, age) in &gone {
        info!("{} {} starved at {now:.1} s (age {:.1} s)", species.name(), entity, age);
    }

    let Some(mut test) = test else { return };
    test.meters = meters;
    test.alive = alive;
    if let Some(at) = first_meal
        && test.ate_at.is_none()
    {
        test.ate_at = Some(at);
        info!("selftest: first meal at {at:.2} s");
    }
    if let Some((entity, _, _)) = gone.first()
        && test.starved.is_none()
    {
        test.starved = Some((*entity, now));
    }
    // the twelfth check's evidence: while the frozen rules are in force nothing may make a
    // creature hungrier, and once the real ones are back something must
    if test.frozen_at.is_some_and(|at| now - at < FREEZE_WINDOW) {
        test.fell_while_frozen += fell;
    } else if test.thaw_asked && fell > 0 {
        test.fell_after_thaw = true;
    }
}

/// **The eleventh check: the rules really are running, and they really are Ruby's.**
///
/// A plant that is bigger than it was is `world.rb`'s `each_frame` having run *and* having written
/// what it worked out — nothing else in this build makes a plant grow, and the growth is not
/// something the game could be doing by accident. Measured at the moment it is first seen, so the
/// check can say how long the world took to come to life.
fn watch_the_rules(mut test: ResMut<SelfTest>, time: Res<Time>, plants: Query<(Entity, &Plant)>) {
    if test.grew_at.is_some() {
        return;
    }
    let now = time.elapsed_secs();
    let mut sizes: Vec<(Entity, f32)> = Vec::with_capacity(plants.iter().len());
    for (entity, plant) in &plants {
        sizes.push((entity, plant.size));
        if test.grew_at.is_none()
            && let Some((_, before)) = test.grass.iter().find(|(e, _)| *e == entity)
            && plant.size > *before
        {
            test.grew_at = Some(now);
            info!("selftest: the grass grew at {now:.2} s — `world.rb` is running");
        }
    }
    test.grass = sizes;
}

/// **The thirteenth check: what the world says reaches the creatures** (W2).
///
/// The rules declare a season (`tell :all, "season", …` in `world.rb`), a creature's
/// `on(:season)` writes it into its own `@memory`, and this reads that memory back **out of the
/// VM** — the same two `ivar_get`s the save file is made of (`read_memory`). So what it proves is
/// the whole road and not a step of it: the world's VM said something, the game carried it across
/// to the creatures' VM, a handler in a script nobody here has read ran, and the thing it
/// remembered is in the heap where the save would find it.
///
/// It reads nothing until the rules have actually said it (`season_told`), and nothing after the
/// first creature is found to have heard it.
fn watch_the_season(
    time: Res<Time>,
    mut test: ResMut<SelfTest>,
    mut scripts: ResMut<ScriptWorld>,
    creatures: Query<(&Mind, &ScriptTask)>,
) {
    if !test.season_told || test.season_heard.is_some() {
        return;
    }
    let now = time.elapsed_secs();
    for (mind, task) in &creatures {
        let memory = read_memory(&mut scripts.vm, task.task());
        let Some(season) = memory.get("season").and_then(|v| v.as_str()) else { continue };
        info!("selftest: {} knows it is the {season} season at {now:.2} s", mind.name);
        test.season_heard = Some((mind.name.clone(), season.to_string(), now));
        return;
    }
}

/// How long the frozen rules are left in force before the twelfth check asks its question. Five
/// seconds is the plan's (`docs/plans/garden-world-plan.md` §3.1) and it is a long time in a world
/// where a creature loses `1.6 * appetite` of its meter every second: every creature in the garden
/// would have lost eight points of it, and the check is that **none** lost any.
const FREEZE_WINDOW: f32 = 5.0;
/// When the rules are taken away. It is after everything the first ten checks are about has
/// happened — the first meal (under a second), the first death (1.9 s), the probe's walk (1.8 s)
/// — and well before the night the seventh check watches for (25 s).
const FREEZE_AT: f32 = 20.0;

/// **The twelfth check: the rules can be replaced while the world runs.**
///
/// At [`FREEZE_AT`] the world's script is swapped for [`FROZEN_WORLD`], whose `each_frame` does
/// nothing, by exactly the road the editor will use in W3 and `restart_species` already uses for a
/// creature: take the `ScriptTask` away, which ends the task and closes its queues, and put a new
/// `Script` on. For [`FREEZE_WINDOW`] after the new task has actually started, no creature's meter
/// may fall. Then the real rules go back and one has to fall again.
///
/// The window starts when the task restarts and not when the swap was asked for: compiling and
/// starting a script takes a frame or two, and a check that began counting before the new rules
/// were in force would be counting the old ones.
fn swap_the_rules(
    mut commands: Commands,
    time: Res<Time>,
    ruby: Res<RubyDir>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut test: ResMut<SelfTest>,
    script: Query<(Entity, Option<&ScriptTask<World>>), With<WorldScript>>,
) {
    let now = time.elapsed_secs();
    let Ok((entity, task)) = script.single() else { return };
    let mut wear = |body: Option<&str>, commands: &mut Commands| {
        let compiled = match body {
            Some(text) => compile_world_source(&ruby.0, text, &mut mrb),
            None => compile_world(&ruby.0, &mut mrb),
        };
        match compiled {
            Ok((handle, prelude_lines)) => {
                wear_the_rules(commands, entity, handle, prelude_lines);
                Ok(())
            }
            Err(why) => Err(why),
        }
    };
    if !test.freeze_asked && now >= FREEZE_AT {
        test.freeze_asked = true;
        if let Err(why) = wear(Some(FROZEN_WORLD), &mut commands) {
            error!("selftest: the frozen rules would not compile: {why}");
        }
        return;
    }
    // the swap is in force from the frame the new task exists
    if test.freeze_asked && test.frozen_at.is_none() {
        if task.is_some() {
            test.frozen_at = Some(now);
            info!("selftest: the rules were taken away at {now:.2} s");
        }
        return;
    }
    if !test.thaw_asked && test.frozen_at.is_some_and(|at| now - at >= FREEZE_WINDOW) {
        test.thaw_asked = true;
        info!(
            "selftest: the rules go back at {now:.2} s ({} meters fell while they were away)",
            test.fell_while_frozen
        );
        if let Err(why) = wear(None, &mut commands) {
            error!("selftest: the real rules would not compile the second time: {why}");
        }
    }
}

/// A rabbit walking into a beetle is news to the beetle — the material for G1's `on(:touched)`
/// — and it is published once per contact, not once per frame.
fn startle(
    time: Res<Time>,
    mut world: ResMut<ScriptWorld>,
    mut contacts: ResMut<Contacts>,
    mut test: Option<ResMut<SelfTest>>,
    field: Res<Place>,
    reaches: Res<Reaches>,
    creatures: Query<(Entity, &Creature, &Transform, &Velocity)>,
) {
    let now = time.elapsed_secs();
    let mut touching: Vec<(Entity, Entity)> = Vec::new();
    let rabbits: Vec<(Entity, Vec2)> = creatures
        .iter()
        .filter(|(_, c, _, _)| c.species == Species::Rabbit)
        .map(|(e, _, t, _)| (e, Vec2::new(t.translation.x, t.translation.z)))
        .collect();
    for (beetle, creature, at, velocity) in &creatures {
        if creature.species != Species::Beetle {
            continue;
        }
        let here = Vec2::new(at.translation.x, at.translation.z);
        for (rabbit, there) in &rabbits {
            if here.distance(*there) <= reaches.touch {
                touching.push((*rabbit, beetle));
                if !contacts.0.contains(&(*rabbit, beetle)) {
                    world.publish(Some(beetle), "touched", Answer::Entity(*rabbit));
                    // and, for the selftest, which way it was going when it was told.
                    //
                    // **The clock is reset by every message, and the guards come after it.** A
                    // rabbit that keeps walking into a beetle publishes again every time the
                    // contact is remade; the handler takes half a second over each one and the
                    // rest wait in its queue, so "did it turn?" asked half a second after the
                    // third message is really asking about the first. G1 wrote that down and
                    // then reset the clock *inside* the three guards below — so a message sent
                    // to a beetle that happened to be standing still, or in the corner, or a
                    // second old did not count as having been sent at all, and the next message
                    // looked like the first thing that had happened to it in a long while.
                    // Measured over six ninety-second runs: **41 of 211 counted touches** had a
                    // message in the 1.5 s before them that the clock had not seen, and that is
                    // where the sixth check's remaining flakiness lived
                    // (`docs/worklog/2026-09-17-garden-G4.md`).
                    if let Some(test) = test.as_mut() {
                        let fresh = match test.last_touch.iter_mut().find(|(e, _)| *e == beetle) {
                            Some(seen) => {
                                let fresh = now - seen.1 > TOUCH_SETTLE;
                                seen.1 = now;
                                fresh
                            }
                            None => {
                                test.last_touch.push((beetle, now));
                                true
                            }
                        };
                        // and then: a beetle that was actually walking, in the open, old enough
                        // to have subscribed to anything (`NEWBORN_GRACE`)
                        if fresh
                            && velocity.0.length() > 0.5
                            && !by_a_wall(&field, at)
                            && creature.age > NEWBORN_GRACE
                        {
                            test.touched.push(Touch {
                                beetle,
                                at: now,
                                was: velocity.0,
                                turned: false,
                                closest: f32::INFINITY,
                                closest_at: now,
                                looked: 0,
                                last: velocity.0,
                            });
                        }
                    }
                }
            }
        }
    }
    contacts.0 = touching;
}

/// **The children a script asked for, made** (G2, and W2's leftover of `hatch`).
///
/// There were two systems here. `court` looked for a pair, published `"mate"` and kept the
/// cooldown; `hatch` made the child and charged its parents. Both are `ruby/world.rb` now —
/// which two of them mattered is what W2 is about — and what is left is the part that was never
/// a rule: a creature's **body**, which wants `Commands`, `Look`, the models, the compiler and
/// the species' file, and which no script can build.
///
/// It is `sprout_plants` for creatures, and the same sentence divides them: whether a child
/// arrives is the rules', what a child is made of is the game's.
///
/// The two numbers it builds with are the rules' as well, handed over once by `garden.rules`
/// ([`Births`]): how full a newborn is, and how many creatures the game will make at all.
#[allow(clippy::too_many_arguments)]
fn children_arrive(
    time: Res<Time>,
    sky: Res<Sky>,
    place: Res<Place>,
    mut commands: Commands,
    mut births: ResMut<Births>,
    mut newborns: ResMut<Newborns>,
    look: Option<Res<Look>>,
    ruby: Res<RubyDir>,
    mut brains: ResMut<Brains>,
    bodies: Res<Bodies>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut test: Option<ResMut<SelfTest>>,
) {
    // the garden's own clock, which is the one the rules write their cooldowns in (G9)
    let now = world_now(&time, &sky);
    let hunger = births.hunger;
    for birth in std::mem::take(&mut births.waiting) {
        let at = Vec2::new(
            birth.at.x.clamp(-place.half_w() + 1.0, place.half_w() - 1.0),
            birth.at.y.clamp(-place.half_d() + 1.0, place.half_d() - 1.0),
        );
        let child =
            spawn_creature(&mut commands, look.as_deref(), &bodies, birth.species, at, hunger, birth.genome, birth.parent);
        give_mind(&mut commands, &ruby.0, &mut brains, &mut mrb, child, birth.species);
        info!(
            "a {} was born at {now:.1} s ({}) — {}",
            birth.species.name(),
            child,
            birth.genome.describe()
        );
        // W2: what it costs its parents is `world.rb`'s, charged on the pass that first sees the
        // child — there is nothing to do here but let it into the world.

        // …except to tell it, in two frames' time, what the sky is doing: it cannot hear
        // anything until then, and what was said before it was born was said once
        // ([`tell_newborns_the_sky`]).
        newborns.waiting.push((child, 0));

        if let Some(test) = test.as_mut() {
            test.births += 1;
            if test.born_at.is_none() {
                let mating = birth
                    .parent
                    .and_then(|m| test.matings.iter().rev().find(|(e, ..)| *e == m).copied());
                let (says, ok) = match mating {
                    Some((_, one, two, _, rate)) => judge_child(&birth.genome, &one, &two, rate),
                    None => ("its parents' pairing was not recorded".into(), false),
                };
                test.born_at = Some(now);
                test.born_says = says;
                test.born_ok = ok;
            }
        }
    }
}

/// **A creature born into the night is told that it is night** (2026-09-18).
///
/// `"night"` and `"day"` are published once each, at the turn (`day_night`), to whoever is
/// subscribed at that moment. A creature born afterwards has never heard either — it walks
/// through the dark until morning, because `@asleep` is only ever set by the handler — and a
/// creature born in the frame of the publish, or the frame before it, is already counted among
/// the addressees and has not subscribed yet. That second one is the seventh check's flakiness,
/// and the reason it almost never fires is luck: the twelfth check's thaw pins a burst of births
/// five frames clear of the night, and five frames is all the room there is
/// (`docs/worklog/2026-09-17-selftest-flakes.md` §4).
///
/// The same is true of a creature built out of a save file, which has a body and a fresh script
/// and has heard nothing either — see [`Newborns`] and `load_world`.
///
/// The cure is not a wider window in the check but the missing letter: the sky is said again, to
/// each newborn on its own, as soon as it can hear it. There is no way for the game to ask rubevy
/// whether a task has subscribed yet — `subscriptions` lives inside its `HostState` — so what it
/// waits is frames, and the number of them is measured ([`NEWBORN_DEAF_FRAMES`]).
///
/// It runs before `RubevySet::Tick`, so the word is in the queue on the frame the creature is
/// first able to read it, and under `is_still` for the same reason `children_arrive` is: a
/// paused world runs no tasks, and a frame in which nobody subscribed to anything is not one of
/// the two frames being counted.
fn tell_newborns_the_sky(
    time: Res<Time>,
    sky: Res<Sky>,
    mut world: ResMut<ScriptWorld>,
    mut newborns: ResMut<Newborns>,
    minds: Query<(), With<Mind>>,
) {
    if newborns.waiting.is_empty() {
        return;
    }
    // the same payload `day_night` publishes: the world's hour, which is what `on(:night) { |at| }`
    // is handed
    let now = world_now(&time, &sky);
    let name = if sky.night { "night" } else { "day" };
    let mut still_waiting = Vec::with_capacity(newborns.waiting.len());
    for (child, waited) in std::mem::take(&mut newborns.waiting) {
        // starved or never given a behaviour (a species whose file will not compile): there is
        // nothing to tell
        if !minds.contains(child) {
            continue;
        }
        let waited = waited + 1;
        if waited < NEWBORN_DEAF_FRAMES {
            still_waiting.push((child, waited));
            continue;
        }
        // If the sky happens to turn over on this very frame, the creature is subscribed in time
        // to hear `day_night`'s publish as well and gets the word twice. A handler that sets
        // `@asleep` twice has done what a handler that sets it once did, and the alternative —
        // asking whether the sky moved this frame — would be a second way of saying the same
        // thing, kept in step by hand.
        world.publish(Some(child), name, Answer::Num(now as f64));
    }
    newborns.waiting = still_waiting;
}

/// Is this child the mutated average of those two parents? Every gene has to lie within the
/// mutation rate of the parents' mean, and at least one has to have actually moved — a child
/// exactly on the mean would mean `mutate` did nothing, and a child on a parent would mean `mix`
/// did nothing.
///
/// **The rate is read out of the VM** ([`mutation_rate_of`]), in the frame the child was asked
/// for, off the class of the creature that asked. It used to be a `const 0.1` here with a
/// comment saying it was a copy of `beetle.rb`'s — which meant that raising the rate in the
/// editor, which is a thing this game invites, turned the check into a FAIL. `rate` is `None`
/// for a file that does not name its rate, and then the stand-in is what it always was.
fn judge_child(child: &Genome, one: &Genome, two: &Genome, rate: Option<f32>) -> (String, bool) {
    let rate = rate.unwrap_or(MUTATION_RATE);
    let genes = [
        ("speed", child.speed, one.speed, two.speed),
        ("sight", child.sight, one.sight, two.sight),
        ("appetite", child.appetite, one.appetite, two.appetite),
    ];
    let mut moved = false;
    let mut said = Vec::new();
    let mut ok = true;
    for (name, got, a, b) in genes {
        let mean = (a + b) * 0.5;
        let drift = (got - mean).abs();
        // the mutation is a multiplication by 1 ± rate, so this is how far from the mean it may
        // be; the epsilon is the f32 round trip through the Ruby Float and back
        if drift > rate * mean.abs() + 1e-3 {
            ok = false;
        }
        if got != a && got != b {
            moved = true;
        }
        said.push(format!("{name} {got:.3} vs {a:.3}/{b:.3}, mean {mean:.3}"));
    }
    // the sentence is left exactly as it was: `docs/verification/selftest-lines.md` is a list of
    // the checks' sentences and a stage that means to change nothing has to diff empty against it
    (format!("{} ({})", said.join("; "), if moved { "mutated off both parents" } else { "identical to a parent" }), ok && moved)
}

// ---------------------------------------------------------------------------------------------
// The two questions the game answers (G1)
// ---------------------------------------------------------------------------------------------

/// What this game puts in the VM, once: the `Genome` class (G2) and `JSON` (G3).
///
/// `ScriptWorld::vm` is the VM the scheduler runs, and it exists from the moment `RubevyPlugin`
/// is added, while the first script does not start until the first `Update` — so a `Startup`
/// system is the whole of what "install a class" takes, and rubevy needed no entry point for it
/// (rubevy `docs/host-api.md`, "Adding to the VM"). What `register` adds is the methods: the
/// class and its store would appear by themselves on the first `into_ruby`.
///
/// `JSON` is one line and it is the game's line, not the engine's. The VM has no JSON of its own
/// and never links `serde_json` — a host that does not want a parser does not pay for one — so a
/// game that wants `JSON.generate(memory)` in its scripts says so here, and that is also what
/// says whose decision it was (sabiruby `docs/design/serde.md`, "Why JSON lives here").
fn install_host_api(mut world: ResMut<ScriptWorld>) {
    if let Err(e) = Genome::register(&mut world.vm) {
        error!("the Genome class would not register: {e:?}");
    }
    sabiruby_serde::install_json(&mut world.vm);
}

/// `garden.nearest(:Plant)` and `garden.count(:Plant)` — the whole of this game's `ask` surface.
/// SabiRuby Battle answers six kinds (`status`, `radar`, `incoming`, `act`, `seed`, `handler`);
/// the garden answers two, because everything else a creature wants to know is a component it
/// can read for itself.
///
/// **There is no per-component code here either.** The kind of thing to look for arrives as a
/// string, and it is resolved the way rubevy resolves `e[:Hunger]`: through the type registry to
/// a `ReflectComponent`, whose `contains` says whether an entity has one. So `garden.count(:Rock)`
/// works, and so would `garden.nearest(:Whatever)` the day something registers a `Whatever` —
/// without this function being touched. What is typed here is the game's *rule*: how far a
/// creature may see (`Sight`) and that it never finds itself.
///
/// It runs in `RubevySet::Answer`, so a question asked on this frame is answered on this frame
/// and the script wakes with it on the next one.
fn answer_garden(world: &mut bevy::ecs::world::World) {
    let Some(registry) = world.get_resource::<AppTypeRegistry>().cloned() else { return };
    let registry = registry.read();
    // what `garden.spawn` was asked for, filled in below and handed to `Births` at the end: the
    // answering system has the whole `World` but no `Commands`, and reading the Hash is the part
    // that needs the VM
    let mut newborn: Vec<Birth> = Vec::new();
    // and the first `garden.spawn` this frame that was refused, for the selftest's malformed Hash
    let mut refused: Option<String> = None;
    // W2: the two parents of each child asked for, for the eighth check (`answer_spawn`)
    let mut pairings: Vec<(Entity, Genome, Genome, Option<f32>)> = Vec::new();
    // and who asked for one at all, for the same check's other half (`close_courtings`)
    let mut asked_for_a_child: Vec<Entity> = Vec::new();
    // who asked, for the HUD's frames-per-decision (G4): a gap in a task's instruction count that
    // begins on this frame is a round trip and not a nap (`watch_minds`)
    let mut askers: Vec<Entity> = Vec::new();
    world.resource_scope(|world: &mut bevy::ecs::world::World, mut scripts: Mut<ScriptWorld>| {
        let world = &*world;
        for request in scripts.take_requests() {
            let of_kind = request
                .text(0)
                .and_then(|name| {
                    registry.get_with_short_type_path(name).or_else(|| registry.get_with_type_path(name))
                })
                .and_then(|r| r.data::<bevy::ecs::reflect::ReflectComponent>());
            let asker = request.entity;
            if let Some(asker) = asker
                && !askers.contains(&asker)
            {
                askers.push(asker);
            }
            match request.kind.as_str() {
                "garden.nearest" => {
                    let found = of_kind.zip(asker).and_then(|(rc, me)| {
                        let at = world.get::<Transform>(me)?.translation;
                        let here = Vec2::new(at.x, at.z);
                        // how far it may look is its own business, and its own component
                        let reach = world.get::<Sight>(me).map(|s| s.0).unwrap_or(0.0);
                        let mut best: Option<(Entity, f32)> = None;
                        for other in world.iter_entities() {
                            let entity = other.id();
                            if entity == me || !rc.contains(other) {
                                continue;
                            }
                            let Some(there) = other.get::<Transform>() else { continue };
                            let span = here.distance(Vec2::new(there.translation.x, there.translation.z));
                            if span > reach {
                                continue;
                            }
                            if best.is_none_or(|(_, b)| span < b) {
                                best = Some((entity, span));
                            }
                        }
                        best.map(|(e, _)| e)
                    });
                    match found {
                        Some(entity) => scripts.answer(&request, Answer::Entity(entity)),
                        None => scripts.answer(&request, Answer::Nil),
                    }
                }
                "garden.count" => {
                    scripts.answer(&request, Answer::Num(count_of(world, of_kind) as f64));
                }
                // G2. The asker's own genome, as **the Rust value**: `answer_value` hands the
                // host the `&mut Vm`, and `into_ruby` (written by `#[derive(RubyClass)]`) puts
                // the `Genome` in the VM's host store and answers with the `Data` object naming
                // it. The script gets something with methods on it, not a copy — and the same
                // three numbers are readable as a plain Hash through `me[:Creature][:genome]`,
                // by reflection, without this or any other question being asked.
                "genome" => {
                    let genome = asker.and_then(|me| world.get::<Creature>(me)).map(|c| c.genome);
                    match genome {
                        Some(genome) => scripts.answer_value(&request, move |vm| genome.into_ruby(vm)),
                        None => scripts.answer(&request, Answer::Nil),
                    }
                }
                // G2, rewritten in G3. `garden.spawn(species:, genome:, at:)` — the one question
                // whose argument has a shape. It arrives as `Arg::Value`, which is the Ruby Hash
                // itself rather than a copy of it (rubevy `docs/host-api.md`, "What a question may
                // carry"), and **one line reads it**: `from_value::<CreatureSpec>` walks the Hash
                // and fills a Rust struct, nested `Genome` and `Species` and all. G2 did the same
                // work by hand, key by key, in 72 lines of `genome.rs` (`docs/garden.md`, "The
                // spawn Hash, by hand and by serde"); what is left of them is the clamp in
                // `CreatureSpec::into_birth`, which was never about reading.
                //
                // The error is worth as much as the reading. A Hash with a gene missing raises a
                // `TypeError` naming it — "missing field `sight`" — and the script hears that
                // sentence as the answer to its question, because a creature that asked for a
                // child and got silence has no way of finding out why.
                //
                // It stays an `ask` rather than becoming a native taking `Serde<CreatureSpec>`:
                // a native is handed the `&mut Vm` and nothing else, so it could not see the
                // population, could not spawn anything, and could not answer `true` or the reason
                // — it would have to leave a note for a system to read, which is what a `Request`
                // already is. The population cap is checked here because the script answered a
                // frame after the rule spoke, and a frame is long enough for the garden to fill.
                "garden.spawn" => {
                    answer_spawn(world, &mut scripts, &request, &mut newborn, &mut refused, &mut pairings, &mut asked_for_a_child)
                }
                other => {
                    warn!("garden: nobody answers {other:?}");
                    scripts.answer(&request, Answer::Nil);
                }
            }
        }
    });
    if !newborn.is_empty() {
        world.resource_mut::<Births>().waiting.extend(newborn);
    }
    if !pairings.is_empty()
        && let Some(sky) = world.get_resource::<Sky>().map(|sky| sky.shift)
    {
        let now = world.resource::<Time>().elapsed_secs() + sky;
        let mut test = world.resource_mut::<SelfTest>();
        test.matings.extend(pairings.into_iter().map(|(who, one, two, rate)| (who, one, two, now, rate)));
    }
    // and the eighth check's pairings, settled once a frame: this is the system the creatures'
    // `garden.spawn` arrives at, so it is the one that knows whether a pairing was walked
    close_courtings(world, &asked_for_a_child);
    let frame = world.resource::<bevy::diagnostic::FrameCount>().0;
    for asker in askers {
        if let Some(mut mind) = world.get_mut::<Mind>(asker) {
            mind.asked_frame = Some(frame);
        }
    }
    // the selftest's tester asks for a creature whose genome has no `sight`; what it is told is
    // serde's own message, and the check is that the message names the gene
    if let Some(why) = refused
        && let Some(mut test) = world.get_resource_mut::<SelfTest>()
        && test.bad_spawn.is_none()
    {
        test.bad_spawn = Some(why);
    }
}

/// `garden.count(:Plant)`, for whichever VM asked. Both of them may, and neither needed a line of
/// per-component code: the kind was resolved through the type registry by the caller.
fn count_of(
    world: &bevy::ecs::world::World,
    of_kind: Option<&bevy::ecs::reflect::ReflectComponent>,
) -> usize {
    match of_kind {
        Some(rc) => world.iter_entities().filter(|e| rc.contains(*e)).count(),
        None => 0,
    }
}

/// `garden.spawn(species:, genome:, at:)`, for whichever VM asked (W1).
///
/// It was written into `answer_garden` and is a function now because the world's VM answers it
/// too: in W2 the rule that makes a child is `world.rb`'s, and a rule that may not spawn what it
/// decided on would not be the rule. The generic parameter is the name tag, so the one body serves
/// `ScriptWorld` and `ScriptWorld<World>` — what it needs of either is the `Vm` (to read the Hash)
/// and `answer`.
///
/// Everything it used to say is still true. The Hash arrives as `Arg::Value`, the Ruby value
/// itself rather than a copy of it, and one line reads it: `from_value::<CreatureSpec>` walks it
/// and fills a Rust struct, nested `Genome` and `Species` and all. The error is worth as much as
/// the reading — a Hash with a gene missing raises a `TypeError` naming it, and the script hears
/// that sentence as the answer to its question. The population cap is checked here because the
/// script answers a frame after the rule spoke, and a frame is long enough for the garden to fill
/// — it is the rules' own cap, handed over by `garden.rules(pop_max:)` (W2).
///
/// **`pairings` is the eighth check's, and it is taken here because there is nowhere else left**
/// (W2). It used to be written by `court`, which knew both parents because it had just put them
/// together; the rule is Ruby's now and says nothing to the game about it. What a spawn request
/// still carries is the creature that asked — and that creature is carrying who it was told
/// about, in the `Breeding` the rules wrote (`partner`). So the pair is still knowable at the
/// moment a child is asked for, out of two components, without the rules having to report
/// anything: the check watches the world rather than being told about it, exactly as
/// `note_the_rules` came to watch for a meal.
fn answer_spawn<M: 'static>(
    world: &bevy::ecs::world::World,
    scripts: &mut ScriptWorld<M>,
    request: &rubevy::Request,
    newborn: &mut Vec<Birth>,
    refused: &mut Option<String>,
    pairings: &mut Vec<(Entity, Genome, Genome, Option<f32>)>,
    asked_for_a_child: &mut Vec<Entity>,
) {
    // Whoever asked has walked the road the eighth check is about, whatever the answer turns out
    // to be: a refusal ("the garden is full", a gene missing) is the game's answer to a question
    // that *was* put, and the pairing behind it has been measured ([`close_courtings`]).
    if let Some(asker) = request.entity
        && world.get_resource::<SelfTest>().is_some()
    {
        asked_for_a_child.push(asker);
    }
    let population =
        world.iter_entities().filter(|e| e.contains::<Creature>()).count() + newborn.len();
    let cap = world.get_resource::<Births>().map(|b| b.cap).unwrap_or(POP_MAX);
    let outcome = match request.value(0) {
        _ if population >= cap => Err(format!("the garden is full ({population} creatures)")),
        Some(asked) => sabiruby_serde::from_value::<CreatureSpec>(&mut scripts.vm, asked)
            .map_err(|e| scripts.vm.describe_error(&e)),
        None => Err("spawn wants a Hash: species:, genome:, at:".to_string()),
    };
    match outcome {
        Ok(spec) => {
            if world.get_resource::<SelfTest>().is_some()
                && let Some(asker) = request.entity
                && let Some(one) = world.get::<Creature>(asker).map(|c| c.genome)
                && let Some(other) = world.get::<Breeding>(asker).map(|b| b.partner)
                && let Some(two) = world.get::<Creature>(other).map(|c| c.genome)
            {
                // and what its file mutates at, asked of the VM here because here is where the
                // task that asked is still in hand (S5b-4)
                let rate = world
                    .get::<ScriptTask<M>>(asker)
                    .and_then(|task| mutation_rate_of(&scripts.vm, task.task()));
                pairings.push((asker, one, two, rate));
            }
            newborn.push(spec.into_birth(request.entity));
            scripts.answer(request, Answer::Bool(true));
        }
        Err(why) => {
            // "missing field `sight`" and its like; "the garden is full" is a rule, not a shape,
            // and is not what the ninth check is about
            if refused.is_none() && why.contains("field") {
                *refused = Some(why.clone());
            }
            scripts.answer(request, Answer::Text(why));
        }
    }
}

/// What `world.rb` hands the game once, at the start: **the numbers a rule keeps but the game
/// has to build, draw or sweep with**. The test for whether one belongs here is whether the
/// thing that needs it is Rust: the sun is drawn here, a creature's body is made here, the
/// selftest's meadow corner is *placed* here (2026-09-18), and two of the garden's sweeps are
/// walks over every pair of things in the field, which is the one kind of loop `world.rb`'s own
/// first paragraph says Ruby should not be writing sixty times a second.
///
/// **S5b-4 added six**: `touch_reach` (what `startle` sweeps for), `sprout_gap` (what
/// `sprout_plants` sweeps for) and the four bodies. The rule for all six is the one the older
/// five follow — **the number is the rules', the walk is the game's** — and it is why they are a
/// handover rather than a question asked per creature or per seed: `garden.rules` crosses the
/// boundary once, in the world's script's first line.
///
/// Every field is an `Option` and `#[serde(default)]`, so a `world.rb` that says nothing about
/// the sun, or about children, or about how wide a beetle is, is not an error — it leaves the
/// game on its own numbers ([`DAY_LENGTH`], [`CHILD_HUNGER`], [`POP_MAX`], [`REACH`],
/// [`TOUCH_REACH`], [`MATE_REACH`], [`SPROUT_GAP`], [`Bodies::default`]), which is also what a
/// `world.rb` that will not compile leaves it on, and what an **older `world.rb`** does: a file
/// saved out of the editor before this stage names none of the six, and `run_world` sends only
/// the keys the world it is running actually has (`world_prelude.rb`).
///
/// `deny_unknown_fields` is not set here, and that is the older decision of the two: this Hash is
/// built by `run_world` out of the world's own methods rather than written out by the player, so
/// there is no key for a player to misspell — what a player misspells is a *method name*, and
/// then the world simply does not have that number and the game keeps its own.
#[derive(Deserialize, Debug, Default)]
struct RuleBook {
    #[serde(default)]
    day_length: Option<f32>,
    #[serde(default)]
    child_hunger: Option<f32>,
    #[serde(default)]
    pop_max: Option<f32>,
    #[serde(default)]
    reach: Option<f32>,
    #[serde(default)]
    touch_reach: Option<f32>,
    #[serde(default)]
    mate_reach: Option<f32>,
    #[serde(default)]
    sprout_gap: Option<f32>,
    #[serde(default)]
    beetle_radius: Option<f32>,
    #[serde(default)]
    rabbit_radius: Option<f32>,
    #[serde(default)]
    tree_radius: Option<f32>,
    #[serde(default)]
    rock_radius: Option<f32>,
}

/// **The question the world's script asks that costs no frame at all** (W1): `garden.within`.
///
/// A rule that looks at every creature every frame cannot afford a round trip per creature, and it
/// cannot do the walking itself either — 24 creatures against 90 plants is 2,160 distance tests,
/// and in Ruby each candidate would be a component read on top. `ScriptWorld::answer_in_tick`
/// registers a closure the tick calls between two runs of the VM, so the rows are there in the
/// line that asked for them (rubevy `docs/host-api.md`, "Answering inside the tick").
///
/// **There is no per-component code here either.** The kind arrives as a string and is resolved
/// through the type registry to a `ReflectComponent`, exactly as `garden.nearest` resolves it and
/// as rubevy resolves `e[:Hunger]` — so `garden.within(c, 3.0, :Rock)` works, and so would a kind
/// nothing has registered yet.
///
/// What it answers is an `Answer::Rows`: one row per thing, `[the entity's bits, how far away]`,
/// **nearest first**. An entity inside a table of numbers is a number, which is what a table of
/// them wants; `world.rb` turns a row back into something it can write to through the list it
/// already took with `Rubevy.find` that frame.
///
/// The closure is handed `&World` and cannot change anything, which is the compiler saying what
/// this is for: the world it sees is the world as it stands in `RubevySet::<World>::tick()` —
/// after the Rust rules, which are ordered before it, and before anything the scripts write.
fn install_world_answers(
    mut scripts: ResMut<ScriptWorld<World>>,
    dice: Res<Dice>,
    budgets: Res<Budgets>,
) {
    // **The world's share of the frame, measured** (W1, `docs/worklog/2026-09-17-garden-world.md`).
    //
    // The budgets are per VM and nothing caps them together: with two VMs at rubevy's defaults a
    // frame's worst case is 16 ms, which is the whole of one at 60 Hz. So the second VM gets a
    // number of its own, and the number is a measurement rather than a feeling.
    //
    // **What one pass of `each_frame` costs was measured at the garden's own caps** (W3). The
    // rules cap themselves: `world.rb` holds the grass at 90 blades (`plant_cap`) and [`POP_MAX`]
    // holds the creatures at 24, so there is a worst case and it can be sat in. A build rigged to
    // start with more grass than the rules allow and a full population was run for a minute three
    // times, and the 392 frames in which the field stood at 90 blades with 24 creatures on it say:
    //
    //     instructions   median 24,670   99th 25,782   worst 25,837
    //     the tick       median 2.65 ms  99th 4.60 ms  worst 4.86 ms
    //
    // `45_000` is **1.74 times the worst of those**: the rules can be rewritten into half as much
    // work again before the budget is anything a player meets, and a rule that has run away — a
    // loop with no `sleep` in it — is stopped inside one frame either way.
    //
    // **It used to be a little under a quarter of the creatures' 200,000**, which was read here as
    // saying in one number which of the two VMs was the guest. That reading is gone: S5b-5 gave
    // the creatures' VM the same treatment — its own worst frame times this same 1.74 — and the
    // two numbers came out within a tenth of each other, 45,000 and 41,000 ([`CREATURE_BUDGET`]).
    // The rules and two dozen creatures cost about the same, and neither is the guest; what the
    // old comparison was measuring was the size of an inherited default.
    //
    // W1 chose the same number a different way, and W3 had to take the reasoning back: it fitted
    // `262 + 159 × plants + 280 × creatures` over a run (residual 0.9%) and read the law off at a
    // garden of *twice* the caps. W2's rules are not that law any more — fitted the same way they
    // give a residual of 5% and, read off at the caps, they are 22% under what the caps actually
    // measure. Pairing is a search over the creatures that are full enough rather than a pass over
    // all of them, and a cost that is not linear in the population cannot be extrapolated past
    // one. **The number did not move; what it rests on did** — from a law read off the end of its
    // range to a measurement taken where the rules stop.
    //
    // **`frame_time` is left at rubevy's 8 ms.** At the caps the rules take 2.65 ms at the median
    // and 4.60 ms at the 99th frame in a hundred on the machine this was measured on, so 8 ms is a
    // guard with room rather than a reservation. The two guards no longer name the same limit as
    // they did in W1 — at the rate measured here (about 9,300 instructions per millisecond) 45,000
    // is roughly 4.8 ms — which means **the instruction count is what bites first**, and that is
    // the right way round: instructions are a fact about the rules and are the same number in a
    // browser several times slower (`docs/web.md`), while milliseconds are a fact about whichever
    // machine is running them.
    // **S5b-3: the number is [`WORLD_BUDGET`] and it is a setting now** (`world_script_budget`).
    // Everything above is why 45,000 is the *default*; a run that writes another number in
    // `garden.settings.txt` is a run the paragraph above is not about, which is the whole of
    // what "measured" buys and the whole of what changing it costs. `frame_time` is set for the
    // same reason it was left alone: so that the game says what it is rather than inheriting it.
    scripts.budget = budgets.world;
    scripts.frame_time = Budgets::frame_time(budgets.world_frame_time_ms);

    // **The world's dice, so that two runs are not the same run** (W2).
    //
    // sabiruby's generator starts from a constant and the VM has nothing of its own to mix in —
    // `srand` with no argument would take `gc_clock`, which rubevy does not set — so every roll
    // `world.rb` makes was the same roll on every run of W1: the grass came up at the same
    // moments in every ninety seconds anybody ever watched. **Where a roll has to differ between
    // runs it is the game's `Dice` that differs**, because that one is seeded from the clock
    // (`platform::clock_seed`, `Dice` in `main`), so the seed handed over here is one of its
    // numbers and the world's luck is the garden's luck.
    //
    // It is rolled from a **copy** of `Dice` rather than from the resource itself: taking
    // `ResMut` here would spend one of the game's numbers and move every roll `spawn_world`
    // makes along by one, which would be this line quietly rearranging the garden it is only
    // supposed to be reading a seed out of. The copy is the same generator from the same state,
    // so what comes out is a number nobody else will get, and nobody else's number moves.
    //
    // A million is sabibots' scale for the same handover (`sabibots/src/main.rs`, `"seed"`): a
    // whole number a Float carries exactly, and twenty bits of it, which is a different sequence
    // every run and not a different *quality* of sequence.
    let seed = (Dice(dice.0).roll() * 1_000_000.0).floor() as f64;
    scripts.answer_in_tick("garden.seed", Box::new(move |_, _| Answer::Num(seed)));

    // **The garden's own clock** (W2), which is not the process's: `Sky::shift` carries a loaded
    // world's age and `hold_the_clock` freezes it while the game is paused (G9). `court` and
    // `hatch` read it through `world_now` for exactly the reason `world.rb` reads it now — a
    // cooldown is a time in the garden's life, and a minute spent paused is not a minute of it.
    scripts.answer_in_tick(
        "garden.now",
        Box::new(|world: &bevy::ecs::world::World, _: &rubevy::Request| {
            match (world.get_resource::<Time>(), world.get_resource::<Sky>()) {
                (Some(time), Some(sky)) => Answer::Num(world_now(time, sky) as f64),
                _ => Answer::Nil,
            }
        }),
    );

    scripts.answer_in_tick(
        "garden.within",
        Box::new(|world: &bevy::ecs::world::World, request: &rubevy::Request| {
            let Some(me) = request.entity_arg(0) else { return Answer::Nil };
            let reach = request.num_or(1, 0.0) as f32;
            let Some(at) = world.get::<Transform>(me) else { return Answer::Nil };
            let here = Vec2::new(at.translation.x, at.translation.z);
            let Some(registry) = world.get_resource::<AppTypeRegistry>() else { return Answer::Nil };
            let registry = registry.read();
            let of_kind = request
                .text(2)
                .and_then(|name| {
                    registry.get_with_short_type_path(name).or_else(|| registry.get_with_type_path(name))
                })
                .and_then(|r| r.data::<bevy::ecs::reflect::ReflectComponent>());
            let Some(rc) = of_kind else { return Answer::Rows(Vec::new()) };
            let mut found: Vec<Vec<f64>> = Vec::new();
            for other in world.iter_entities() {
                let entity = other.id();
                if entity == me || !rc.contains(other) {
                    continue;
                }
                let Some(there) = other.get::<Transform>() else { continue };
                let span = here.distance(Vec2::new(there.translation.x, there.translation.z));
                if span > reach {
                    continue;
                }
                found.push(vec![entity.to_bits() as f64, span as f64]);
            }
            // nearest first, so that a rule may take the first row it can use and stop
            found.sort_by(|a, b| a[1].total_cmp(&b[1]));
            Answer::Rows(found)
        }),
    );
}

/// **The questions the world's script asks that a system answers** (W1).
///
/// It is `answer_garden`'s twin, in the other VM's `RubevySet::answer()`, and the reason there are
/// two of them rather than one generic one is the reason the second VM exists: the two VMs are
/// asked different things by different people, and which VM asked is which resource the system
/// reads. What they do share — `garden.spawn`, `garden.count` — is shared as a function.
///
/// `"frame"` is the smallest of them and the one the whole design rests on. It answers the frame
/// number, and `run_world` waits on it once a pass: a question a *system* answers costs exactly one
/// frame (rubevy `docs/host-api.md`, "Where the game's systems go in the frame"), so waiting on it
/// once is what makes one pass of `each_frame` one frame — no `sleep` to keep in step and nothing
/// to drift.
fn answer_world(world: &mut bevy::ecs::world::World) {
    let Some(registry) = world.get_resource::<AppTypeRegistry>().cloned() else { return };
    let registry = registry.read();
    let frame = world.resource::<bevy::diagnostic::FrameCount>().0;
    let mut newborn: Vec<Birth> = Vec::new();
    let mut refused: Option<String> = None;
    let mut pairings: Vec<(Entity, Genome, Genome, Option<f32>)> = Vec::new();
    // nothing in `world.rb` asks for a child — the rules pair, the creatures spawn — but
    // `answer_spawn` is shared with the creatures' VM and takes it either way
    let mut asked_for_a_child: Vec<Entity> = Vec::new();
    let mut seeds = 0u32;
    let mut day_length: Option<f32> = None;
    let mut child_hunger: Option<f32> = None;
    let mut pop_max: Option<usize> = None;
    let mut reaches: Option<(Option<f32>, Option<f32>, Option<f32>)> = None;
    let mut sprout_gap: Option<f32> = None;
    // the four bodies in the order `Bodies` keeps them: beetle, rabbit, tree, rock
    let mut bodies: Option<[Option<f32>; 4]> = None;
    // W2: what the rules said this frame, carried out of the world's VM and into the creatures'
    // below. It is collected rather than published on the spot because publishing needs the
    // *other* `ScriptWorld`, and this scope is holding the world's.
    let mut said: Vec<(Option<Entity>, String, Answer)> = Vec::new();
    world.resource_scope(|world: &mut bevy::ecs::world::World, mut scripts: Mut<ScriptWorld<World>>| {
        let world = &*world;
        for request in scripts.take_requests() {
            let of_kind = request
                .text(0)
                .and_then(|name| {
                    registry.get_with_short_type_path(name).or_else(|| registry.get_with_type_path(name))
                })
                .and_then(|r| r.data::<bevy::ecs::reflect::ReflectComponent>());
            match request.kind.as_str() {
                "frame" => scripts.answer(&request, Answer::Num(frame as f64)),
                // the numbers a rule keeps but the game has to build or draw with. Read with
                // serde, like the spawn Hash and for the same reason: the message a bad one gets
                // back is worth as much as the reading
                "garden.rules" => {
                    let asked = request
                        .value(0)
                        .ok_or_else(|| {
                            "rules wants a Hash: day_length:, child_hunger:, pop_max:, reach:, \
                             touch_reach:, mate_reach:, sprout_gap:, beetle_radius:, \
                             rabbit_radius:, tree_radius:, rock_radius:"
                                .to_string()
                        })
                        .and_then(|v| {
                            sabiruby_serde::from_value::<RuleBook>(&mut scripts.vm, v)
                                .map_err(|e| scripts.vm.describe_error(&e))
                        });
                    match asked {
                        Ok(book) => {
                            // a rule that asks for nonsense is refused rather than obeyed: a day
                            // of no seconds is a division by it in `day_night`, a newborn with no
                            // meter is one the rules would starve on the frame it arrived, and a
                            // garden that holds nobody is one where nothing can be born at all
                            let mut wrong = None;
                            match book.day_length {
                                Some(n) if n <= 0.0 => {
                                    wrong = Some(format!("a day of {n} seconds is not a day"))
                                }
                                Some(n) => day_length = Some(n),
                                None => {}
                            }
                            match book.child_hunger {
                                Some(n) if n <= 0.0 => {
                                    wrong = Some(format!("a newborn with a meter of {n} is stillborn"))
                                }
                                Some(n) => child_hunger = Some(n),
                                None => {}
                            }
                            match book.pop_max {
                                Some(n) if n < 1.0 => {
                                    wrong = Some(format!("a garden that holds {n} creatures holds none"))
                                }
                                Some(n) => pop_max = Some(n as usize),
                                None => {}
                            }
                            // the two distances (2026-09-18). A reach of nothing is a mouth that
                            // can never touch a blade, and a `mate_reach` of nothing is a garden
                            // where two creatures would have to stand in the same spot: both are
                            // rules the garden could be given, but neither is a place the meadow
                            // corner can be built out of, so they are refused where the other
                            // three are. **Both are taken together**, even where one is `nil`:
                            // the pair is what says "the rules have spoken" ([`Reaches::told`]),
                            // and a `world.rb` that names neither still says that much.
                            //
                            // **S5b-4 puts `touch_reach` in with them**, and it is refused on the
                            // same line for the same shape of reason: a touch of no distance is a
                            // rabbit that can never startle a beetle, which is the sixth check's
                            // whole subject.
                            for (n, what) in [
                                (book.reach, "reach"),
                                (book.touch_reach, "touch_reach"),
                                (book.mate_reach, "mate_reach"),
                            ] {
                                if let Some(n) = n
                                    && n <= 0.0
                                {
                                    wrong = Some(format!("a {what} of {n} is no distance at all"));
                                }
                            }
                            // **A body of no width is not a body** (S5b-4): `separate` divides by
                            // the distance between two circles and a circle of nothing is one
                            // nothing is ever inside, so a garden of them is one where creatures
                            // walk through trees. A negative one is worse — the push would be
                            // away from contact. The gap between blades may be nothing (grass
                            // that may come up anywhere, which is a garden somebody might want)
                            // and may not be less.
                            for (n, what) in [
                                (book.beetle_radius, "beetle"),
                                (book.rabbit_radius, "rabbit"),
                                (book.tree_radius, "tree"),
                                (book.rock_radius, "rock"),
                            ] {
                                if let Some(n) = n
                                    && n <= 0.0
                                {
                                    wrong = Some(format!("a {what} of {n} across is not a body"));
                                }
                            }
                            if let Some(n) = book.sprout_gap
                                && n < 0.0
                            {
                                wrong = Some(format!("a sprout_gap of {n} is not a distance"));
                            }
                            if wrong.is_none() {
                                reaches = Some((book.reach, book.touch_reach, book.mate_reach));
                                sprout_gap = book.sprout_gap;
                                bodies = Some([
                                    book.beetle_radius,
                                    book.rabbit_radius,
                                    book.tree_radius,
                                    book.rock_radius,
                                ]);
                            }
                            match wrong {
                                Some(why) => scripts.answer(&request, Answer::Text(why)),
                                None => scripts.answer(&request, Answer::Bool(true)),
                            }
                        }
                        Err(why) => scripts.answer(&request, Answer::Text(why)),
                    }
                }
                // a seed. The script does not wait for this one (`world_prelude.rb`, `sprout`), so
                // what the answer says reaches nobody; it is answered all the same, because a
                // request that is never answered is a queue the collector may not have.
                "garden.sprout" => {
                    seeds += 1;
                    scripts.answer(&request, Answer::Bool(true));
                }
                "garden.count" => {
                    scripts.answer(&request, Answer::Num(count_of(world, of_kind) as f64));
                }
                "garden.spawn" => {
                    answer_spawn(world, &mut scripts, &request, &mut newborn, &mut refused, &mut pairings, &mut asked_for_a_child)
                }
                // **What the world says to the creatures** (W2): `tell(who, name, payload)`.
                //
                // A script cannot publish — publishing is delivering a message into *another*
                // VM's queues, which is a thing only the host holds both ends of — so the rule
                // asks and the game carries. `who` is `:all` or one creature; `name` is what its
                // `on(…)` is written against; `payload` is the one value the handler is given.
                //
                // Like `garden.sprout`, it is **a command and not a question**: `world.rb` does
                // not `pop`, so a rule that speaks does not park its pass for a frame. It is
                // answered all the same, because a request nobody answers is a queue the
                // collector may not have.
                "garden.tell" => {
                    let name = request.text(1).unwrap_or_default().to_string();
                    // nil, a number, a string, or a creature — which is every shape a handler's
                    // one parameter takes (`prelude.rb`, `run_handler`)
                    let payload = match request.args.get(2) {
                        Some(Arg::Num(n)) => Answer::Num(*n),
                        Some(Arg::Text(t)) => Answer::Text(t.clone()),
                        Some(Arg::Entity(e)) => Answer::Entity(*e),
                        _ => Answer::Nil,
                    };
                    let to = match (request.entity_arg(0), request.text(0)) {
                        (Some(entity), _) => Ok(Some(entity)),
                        (None, Some("all")) => Ok(None),
                        _ => Err("tell wants :all or a creature, a name, and a payload"),
                    };
                    match to {
                        Ok(_) if name.is_empty() => {
                            scripts.answer(&request, Answer::Text("tell wants a name".into()))
                        }
                        Ok(to) => {
                            said.push((to, name, payload));
                            scripts.answer(&request, Answer::Bool(true));
                        }
                        Err(why) => scripts.answer(&request, Answer::Text(why.into())),
                    }
                }
                other => {
                    warn!("garden: nobody answers {other:?} for the world");
                    scripts.answer(&request, Answer::Nil);
                }
            }
        }
    });
    if !newborn.is_empty() {
        world.resource_mut::<Births>().waiting.extend(newborn);
    }
    if seeds > 0 {
        world.resource_mut::<Sprouts>().asked += seeds;
    }
    // W2. **The one place the two VMs touch.** What the rules said goes into the creatures'
    // queues here — the same `publish` `startle` and `day_night` use, from the same side of the
    // wall — and this set is ordered before the creatures' tick, so a creature hears about its
    // meal in the frame the meal began.
    if !said.is_empty() {
        // the check counts what the rules said, which is the only thing this source still knows
        // about the *names* of the messages: a `"mate"` is a pairing, and a `"season"` is what
        // the thirteenth check waits for before it starts reading memories
        if world.get_resource::<SelfTest>().is_some() {
            let season = said.iter().any(|(_, name, _)| name == "season");
            // **A `"mate"` is only a pairing the check can be judged on if somebody is listening
            // for it** (2026-09-18). `world.rb` pairs two of a sort that are well fed and close
            // enough, whatever sort that is, and says outright that what a creature does with the
            // message is its own script's business — the rabbit's file has no `on(:mate)` at all,
            // so a pair of rabbits is a pairing out of which no child was ever going to come.
            // Counting it put a run of the garden one unlucky meadow away from a FAIL that meant
            // nothing (`docs/worklog/2026-09-18-corner-and-selftest.md` §1.6, `final64/run52`:
            // two pairings, no children, both of them rabbits).
            let told: Vec<(Entity, Entity)> = said
                .iter()
                .filter(|(_, name, _)| name == "mate")
                .filter_map(|(to, _, payload)| match (to, payload) {
                    // `tell one, "mate", two` — the addressee and the partner it was told about
                    (Some(who), Answer::Entity(partner)) => Some((*who, *partner)),
                    _ => None,
                })
                .collect();
            // read before the pairings are looked at, because it is the stamp on both: when a
            // counted pairing started waiting, and when a sleeping one was asked and answered
            // nothing
            let now = world_clock(world);
            let mut counted: Vec<(Entity, Entity)> = Vec::with_capacity(told.len());
            // pairings put to somebody who was asleep — out of the denominator, and kept only as
            // a number for the sentence the check prints when no child was born at all
            let mut slept_through = 0u32;
            if !told.is_empty() {
                world.resource_scope(
                    |world: &mut bevy::ecs::world::World, mut creatures: Mut<ScriptWorld>| {
                        for (who, partner) in told {
                            let task = world.get::<ScriptTask>(who).map(|t| t.task());
                            let listens =
                                task.and_then(|task| listens_for(&mut creatures.vm, task, "mate"));
                            // **and asleep is the other way a file that does listen answers
                            // nothing** (2026-09-18). It is read here, in the frame the message is
                            // carried across, because that is the only moment this side can be
                            // sure of: the handler wakes a frame later, and by then the answer may
                            // have changed either way. See [`asleep_now`] for why it is asked of
                            // the VM rather than worked out from the sky.
                            let asleep = task.and_then(|task| asleep_now(&creatures.vm, task));
                            match (listens, asleep) {
                                (Some(false), _) => info!(
                                    "selftest: --   the rules paired two of a species whose file has no on(:mate): \
                                     nothing was ever going to come of it, and it is not counted"
                                ),
                                (_, Some(true)) => {
                                    slept_through += 1;
                                    info!(
                                        "selftest: --   the pairing at {now:.2} s measured nothing: \
                                         the one that was told was asleep"
                                    );
                                }
                                // listening and awake, or too young to be asked (its script has
                                // not reached `run_creature`): the check stays strict where it
                                // cannot tell
                                _ => counted.push((who, partner)),
                            }
                        }
                    },
                );
            }
            let mut test = world.resource_mut::<SelfTest>();
            test.season_told = test.season_told || season;
            test.courtings += counted.len() as u32;
            test.courtings_lost += slept_through;
            test.courtings_open.extend(counted.into_iter().map(|(who, partner)| (who, partner, now)));
        }
        let mut creatures = world.resource_mut::<ScriptWorld>();
        for (to, name, payload) in said {
            creatures.publish(to, &name, payload);
        }
    }
    if !pairings.is_empty()
        && let Some(sky) = world.get_resource::<Sky>().map(|sky| sky.shift)
    {
        let now = world.resource::<Time>().elapsed_secs() + sky;
        let mut test = world.resource_mut::<SelfTest>();
        test.matings.extend(pairings.into_iter().map(|(who, one, two, rate)| (who, one, two, now, rate)));
    }
    if let Some(seconds) = day_length {
        let mut sky = world.resource_mut::<Sky>();
        if sky.day_length != seconds {
            info!("the world says a day is {seconds} seconds long");
            sky.day_length = seconds;
        }
    }
    if child_hunger.is_some() || pop_max.is_some() {
        let mut births = world.resource_mut::<Births>();
        if let Some(hunger) = child_hunger {
            births.hunger = hunger;
        }
        if let Some(cap) = pop_max {
            births.cap = cap;
        }
    }
    if let Some((eat, touch, mate)) = reaches {
        let mut reaches = world.resource_mut::<Reaches>();
        if let Some(eat) = eat {
            reaches.eat = eat;
        }
        if let Some(touch) = touch {
            reaches.touch = touch;
        }
        if let Some(mate) = mate {
            reaches.mate = mate;
        }
        reaches.told = true;
    }
    if let Some(gap) = sprout_gap {
        world.resource_mut::<Sprouts>().gap = gap;
    }
    // **and the bodies** (S5b-4). Written only where the rules named one, so that a `world.rb`
    // that says what a beetle is and nothing about trees leaves the trees as they were. Taking
    // the resource mutably is what [`bodies_wear_the_rules`] watches for, so a handover that
    // changes nothing — the rules restated, which is every `Ctrl+Enter` — still costs one pass
    // over the bodies and no write to any of them.
    if let Some([beetle, rabbit, tree, rock]) = bodies
        && [beetle, rabbit, tree, rock].iter().any(Option::is_some)
    {
        let mut bodies = world.resource_mut::<Bodies>();
        if let Some(n) = beetle {
            bodies.beetle = n;
        }
        if let Some(n) = rabbit {
            bodies.rabbit = n;
        }
        if let Some(n) = tree {
            bodies.tree = n;
        }
        if let Some(n) = rock {
            bodies.rock = n;
        }
    }
    if let Some(why) = refused
        && let Some(mut test) = world.get_resource_mut::<SelfTest>()
        && test.bad_spawn.is_none()
    {
        test.bad_spawn = Some(why);
    }
}

/// What one pass of `world.rb` cost, measured round the set that runs it (W1).
///
/// The two halves are `VmClock`'s (`rubevy-egui`), which is the creatures' VM's and takes
/// `ScriptWorld` by name; this one is the world's. Keeping them apart is the point of the
/// measurement: the budgets are per VM and nothing caps them together, so what a frame can cost is
/// the sum, and the only way to choose the second VM's share is to know what it spends.
fn world_clock_start(mut meter: ResMut<WorldMeter>) {
    meter.started = Some(bevy::platform::time::Instant::now());
}

fn world_clock_end(
    mut meter: ResMut<WorldMeter>,
    scripts: Res<ScriptWorld<World>>,
    task: Query<&ScriptTask<World>>,
) {
    if let Some(started) = meter.started.take() {
        let spent = started.elapsed().as_secs_f32() * 1000.0;
        meter.spent_ms = spent;
        // a fifth of the new reading, as the other clock smooths its own
        meter.mean_ms = meter.mean_ms * 0.8 + spent * 0.2;
        meter.most_ms = meter.most_ms.max(spent);
        meter.times.push(spent);
    }
    let Ok(task) = task.single() else { return };
    let ran = scripts.stats(task).instructions;
    // `saturating_sub` is what a **restarted** script needs: the twelfth check swaps `world.rb`
    // while the world runs, and the new task's count starts again at zero. That frame is not a
    // pass and is not counted; the frames after it are.
    let pass = ran.saturating_sub(meter.last_instructions);
    meter.last_instructions = ran;
    if pass > 0 {
        meter.most = meter.most.max(pass);
        meter.last_pass = pass;
        meter.ran_in.push(pass);
    }
}

/// The shortest `sleep` any script in `ruby/` takes: one line of `run_creature`, before a
/// creature's first thought. Everything else sleeps for 0.1 s or more.
///
/// It is the only number the measurement below rests on, and it is a fact about the scripts in
/// this repository rather than a tolerance: a gap between two bursts of a task that is at least
/// this long cannot be anything but a `sleep`.
///
/// The line it names is not going anywhere, and it is worth saying why, because everything else
/// about waiting moved on 2026-09-17. That `sleep 0.05` is not there to space out reads: it is
/// there because a creature loaded from a save is handed its `@memory` by `restore_memory` at the
/// *end* of its first frame, and a `run` that started reading `memory` on the line below would
/// read the empty Hash (`ruby/prelude.rb`, `run_creature`). Writes still land at the end of the
/// frame, so that reason is untouched by reads becoming synchronous.
///
/// What did change is which half of this is usable. "At least this long, so it slept" is still
/// true. Its converse — "shorter than this, so it was waiting for the host" — used to identify a
/// component read and now identifies nothing: a read parks for no frames at all, so the short
/// gaps that are left are timeslices and scheduling, not questions.
const SHORTEST_SLEEP: f32 = 0.05;

/// What a script has cost, where it is standing, and **what a decision costs it** — the three
/// numbers the HUD draws and the headless run prints.
///
/// The last of them is what G4 added, and it is worth being exact about, because the whole shape
/// of the scripts follows from it. A creature's task runs in short bursts and is parked between
/// them. G4 measured the *gaps*: a burst ended because the script had asked the world something,
/// and **frames/decision** was how many frames went by before the answer arrived. Two kinds of
/// question left a gap and they were told apart by which — a gap starting on the frame
/// `answer_garden` answered something for this creature was a `Rubevy.ask` round trip
/// ([`Mind::asked_frame`]), and a shorter one was a component read.
///
/// **Since 2026-09-17 a component read leaves no gap at all**: rubevy answers it inside the tick
/// that asked it, between two runs of the scheduler, so the task never reaches the next frame
/// waiting (rubevy `docs/host-api.md`, "A read costs no frame"). Measured here before anything
/// else was changed, the old rule went from counting 2352 reads in a ninety-second run to
/// counting three or four — and those three or four are not reads, they are gaps that were
/// never reads and used to be lost among the real ones. A measurement that cannot be zero when
/// the thing it counts is gone is not measuring it (`docs/worklog/2026-09-17-sync-reads.md`).
///
/// So the gap this now measures is the one the scripts still make on purpose — the `sleep` at the
/// foot of every behaviour's loop — and what it measures across it is **instructions**:
///
/// * a gap of at least [`SHORTEST_SLEEP`]'s worth of frames is a `sleep`, and a `sleep` ends one
///   pass of the loop and begins the next. Everything between two of them is **one decision**:
///   the reads, the question the game answers, the arithmetic and the `act`;
/// * `ScriptStats::instructions` is a running total, so a pass costs the difference between the
///   totals at its two ends ([`Mind::instructions_per_decision`]);
/// * the round trips the *game* answers are still counted as they were, and still cost one frame
///   each. That number did not break, and it is now the only one of the two kinds of question
///   that costs a frame — which is the whole of what synchronous reads changed, in one line of
///   the log.
fn watch_minds(
    time: Res<Time>,
    frame: Res<bevy::diagnostic::FrameCount>,
    world: Res<ScriptWorld>,
    budgets: Res<Budgets>,
    mut minds: Query<(&mut Mind, Option<&ScriptTask>)>,
) {
    let now = frame.0;
    // how many frames a gap has to be before it is too long to be anything but a nap
    let dt = time.delta_secs().max(1.0 / 1000.0);
    let sleep_floor = (budgets.shortest_sleep / dt).ceil().max(2.0) as u32;
    for (mut mind, script) in &mut minds {
        let Some(script) = script else { continue };
        let stats = world.stats(script);
        // what the task had run when it last stopped: the total at the end of the pass that is
        // finishing, and the total at the start of the one beginning, are the same number
        let before = mind.last_instructions;
        let ran = stats.instructions > before;
        mind.spent = stats.instructions.saturating_sub(before);
        mind.last_instructions = stats.instructions;
        mind.frames += 1;
        if ran {
            let was = mind.ran_frame;
            let gap = now.saturating_sub(was);
            if was > 0 && gap >= 1 {
                if mind.asked_frame == Some(was) {
                    mind.ask_trips += 1;
                    mind.ask_frames += gap;
                }
                if gap >= sleep_floor {
                    // it has woken from a `sleep`: close the pass that was under way and open
                    // the next. The first `sleep` only opens one — a pass that was already
                    // half run when this creature was first looked at would be counted short.
                    if let Some(start) = mind.pass_start {
                        mind.decisions += 1;
                        mind.decision_instructions += before.saturating_sub(start);
                    }
                    mind.pass_start = Some(before);
                }
            }
            mind.ran_frame = now;
        }
        // the prelude sits in front of the creature's file in the compiled program, so a line
        // past its end is a line of the author's own file, counted from its own first line
        let lines = mind.prelude_lines;
        let own = stats.frames.iter().find(|(_, line)| *line > lines).map(|(f, l)| (f.clone(), l - lines));
        mind.own_line = own.as_ref().map(|(_, l)| *l);
        mind.at = match &own {
            Some((file, line)) => format!("{file}:{line}"),
            None => match stats.location {
                Some((_, line)) => format!("prelude.rb:{line}"),
                None => "-".into(),
            },
        };
        // where it keeps coming back to, for the editor's shading (G4). A brain jumps between
        // lines far faster than an eye can follow; what is readable is where it *stays*.
        if let Some(line) = mind.own_line {
            let i = line.saturating_sub(1) as usize;
            if mind.heat.len() <= i {
                mind.heat.resize(i + 1, 0.0);
            }
            for h in mind.heat.iter_mut() {
                *h *= budgets.heat_decay;
            }
            mind.heat[i] += 1.0;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Saving and loading (G3)
//
// The world as JSON, and the creatures' own memories with it. Two directions cross the boundary
// here and they are not symmetrical:
//
//   * **out** — `@memory` is a Ruby Hash on the object a script made, and the game reads it out
//     of the VM with two `ivar_get`s and `sabiruby_serde::from_value::<serde_json::Value>`. No
//     script is asked for anything and no script can refuse: the save button does not wake a
//     creature up.
//   * **in** — `sabiruby_serde::to_value` builds the Hash again and `ivar_set` hangs it back on
//     the object, once that object exists.
//
// Everything else in the file — the plants, where the trees are, how hungry a beetle is — is
// ordinary Rust data with `#[derive(Serialize, Deserialize)]` on it, and that is the comparison
// the stage is for: the same crate does both halves, and the Ruby half is three lines longer.
// ---------------------------------------------------------------------------------------------

/// What this build writes into a save, and the only number it will read back (G5).
///
/// It is here rather than derived from the struct because the struct is not a version: a field
/// added to `CreatureSave` that serde can default, and a field taken away, both leave a file that
/// parses and means something else. The number is a promise about the *meaning*, and only a
/// person can make it. The rule for changing it is the rule for changing the format: if a garden
/// written by the old build would come back wrong rather than not at all, this goes up.
const SAVE_VERSION: u32 = 1;

/// A garden, written down. This struct **is** the file format; there is no schema anywhere else.
///
/// What is not in it is as much of a decision as what is. A rock's squash and whether a plant is
/// a bush or a tuft are the model's business and are rolled again on the way back in; a breeding
/// cooldown, a contact, who bumped into whom last frame are bookkeeping about a run rather than
/// about a world. What is here is what a creature could tell you about itself.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct GardenSave {
    /// which build wrote this (G5). First, so that it is the first thing a person opening the
    /// file sees, and refused by number rather than by whichever field happened to change:
    /// `read_save` reads this alone before it reads the rest. A browser keeps a save in
    /// `localStorage` until it is cleared, so the first garden a new build meets is very often
    /// one the old build wrote.
    version: u32,
    /// seconds the world has lived, on the world's own clock (`Sky::shift`)
    tick: f32,
    /// where the sun is: 0 sunrise, 0.25 noon, 0.5 sunset
    day_phase: f32,
    night: bool,
    plants: Vec<PlantSave>,
    /// trees and rocks are positions and nothing else — they have no state to have
    trees: Vec<[f32; 2]>,
    rocks: Vec<[f32; 2]>,
    creatures: Vec<CreatureSave>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PlantSave {
    at: [f32; 2],
    size: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CreatureSave {
    species: Species,
    at: [f32; 2],
    hunger: f32,
    age: f32,
    /// the same `Genome` the component holds and the same one a script mixes: one type, and now
    /// three derives on it — `Reflect` for the component read, `RubyClass` for the object, and
    /// serde for this
    genome: Genome,
    /// the script's own `@memory`, exactly as it was in the VM. `serde_json::Value` rather than a
    /// struct of the game's: what a creature remembers is the *script's* shape, and a game that
    /// insisted on knowing it would be telling the Ruby what it may think about.
    memory: serde_json::Value,
}

/// The instance variable the prelude hangs the creature object on, in the task rubevy made
/// (`run_creature`: `Task.current.instance_variable_set(:@being, being)`).
///
/// This is the one line of arrangement the memory needed, and it is worth saying why. A script's
/// `@memory` is not on the task — a task's `self` is the VM's `main` object, shared by every task
/// in the VM, so a top-level `@ivar` in one creature's file would be the *same* variable in every
/// other creature's. That is why the prelude puts a creature in an object of its own in the first
/// place (`Creature.new`, one per script), and it is why the host cannot find that object without
/// being shown it. rubevy hangs the entity on the task the same way (`@rubevy_entity`), for the
/// same reason.
const BEING_IVAR: &str = "@being";

/// The world's own clock: `Time` plus whatever a load shifted it by.
fn world_now(time: &Time, sky: &Sky) -> f32 {
    time.elapsed_secs() + sky.shift
}

/// The same clock read out of the `World` itself, for the exclusive systems that answer the two
/// VMs and have no `Res` parameters to read it from. It answers zero where either resource is
/// missing, which is the frames before `main` has finished inserting them.
fn world_clock(world: &bevy::ecs::world::World) -> f32 {
    let shift = world.get_resource::<Sky>().map(|sky| sky.shift).unwrap_or(0.0);
    world.get_resource::<Time>().map(|time| time.elapsed_secs()).unwrap_or(0.0) + shift
}

/// The class-level `@handlers` the prelude's `Creature.on` fills: `[[:touched, 0], [:mate, 1], …]`.
const HANDLERS_IVAR: &str = "@handlers";

/// **Does this creature's script listen for an event at all?** (2026-09-18)
///
/// Which species breeds is a fact about `ruby/creatures/*.rb` — a file the player may edit, and a
/// file this source is not allowed to know the contents of — so the eighth check reads it out of
/// the VM rather than keeping a table of its own. The road is the one `read_memory` takes, one
/// step further: the task's `@being` is the creature object (`run_creature` put it there), its
/// class is the anonymous subclass `creature "Beetle" do … end` made, and the prelude's
/// `Creature.on` has been pushing `[event, slot]` onto that class's `@handlers` since the file was
/// loaded. Two `ivar_get`s, a `real_class_of`, and serde reading an Array of pairs — the same
/// three tools the save file is read with, and no new question for the script to answer.
///
/// It is asked rather than answered by rubevy because rubevy cannot answer it: a subscription
/// lives in its `HostState` and only its count comes back out (`ScriptWorld::subscriptions`),
/// which is the same wall `tell_newborns_the_sky` is written against.
///
/// `None` where it cannot be told — the task has not reached `run_creature` yet, so there is no
/// `@being` to ask. A caller that must decide something takes `None` for the strict answer.
fn listens_for(vm: &mut Vm, task: ObjId, event: &str) -> Option<bool> {
    let being = vm.ivar_get(task, BEING_IVAR);
    being.obj()?;
    let class = vm.real_class_of(being);
    let handlers = vm.ivar_get(class, HANDLERS_IVAR);
    // nil rather than an Array: a file with no `on` in it at all, which listens for nothing
    if handlers.obj().is_none() {
        return Some(false);
    }
    let handlers = sabiruby_serde::from_value::<Vec<(String, u32)>>(vm, handlers).ok()?;
    Some(handlers.iter().any(|(name, _)| name == event))
}

/// The class-level instance variable the prelude's `Creature.mutation_rate` fills: the number a
/// species' file passes to `Genome#mutate`, named rather than written into the call.
const MUTATION_RATE_IVAR: &str = "@mutation_rate";

/// **How far a child of this creature's species may stray from its parents' mean — asked of the
/// file that decides it** (S5b-4).
///
/// The eighth check measures a child against `mix` and `mutate`, and `mutate`'s rate is
/// `beetle.rb`'s: the line is `my_genome.mix(mate).mutate(mutation_rate)`. Until S5b-4 the check
/// kept `const RATE = 0.1` — a copy, and one that said so in its own comment — so **a player who
/// opened the editor and raised the rate turned the check into a FAIL**, on a file this game
/// exists to invite people to edit (`docs/numbers.md` §7-3).
///
/// It is the road [`listens_for`] takes, and the same three tools: the task's `@being` is the
/// creature object, its class is the anonymous subclass `creature "Beetle" do … end` made, and
/// `Creature.mutation_rate` has put the number on that class since the file was loaded.
///
/// `None` where there is nothing to ask — no `@being` yet, or a species file that never names a
/// rate, which is every file written before this stage (they pass the number to `mutate`
/// directly, and it is not readable from out here). The caller then judges on [`MUTATION_RATE`],
/// which is what it always judged on.
fn mutation_rate_of(vm: &Vm, task: ObjId) -> Option<f32> {
    let being = vm.ivar_get(task, BEING_IVAR);
    being.obj()?;
    let class = vm.real_class_of(being);
    match vm.ivar_get(class, MUTATION_RATE_IVAR) {
        Value::Float(n) => Some(n as f32),
        Value::Int(n) => Some(n as f32),
        _ => None,
    }
}

/// What the eighth check judges a child by when the file that made it does not say — every
/// creature file written before S5b-4, including one a player has in their browser.
///
/// It is `ruby/creatures/beetle.rb`'s `mutation_rate` as it ships, and it is a **stand-in** in
/// exactly the sense [`TOUCH_REACH`] is: the number belongs to the file, the check reads the
/// file's, and this is what is left for a file that cannot be asked. **Source**: the line in
/// `beetle.rb`, which has always been the only source there was.
const MUTATION_RATE: f32 = 0.1;

/// The instance variable a creature's own file writes when the sun goes down: `on(:night)` sets
/// it, `on(:day)` clears it, and every other handler in both creature files starts by reading it.
const ASLEEP_IVAR: &str = "@asleep";

/// **Was the creature that was told asleep in that moment?** (2026-09-18)
///
/// `beetle.rb`'s `on(:mate)` is `next if @asleep` before it is anything else, so a pairing the
/// rules make in the night is one no child was ever going to come out of — the file doing exactly
/// what it says, not the road from `on(:mate)` to `garden.spawn` being broken. It belongs out of
/// the eighth check's denominator for the same reason a pair of rabbits does.
///
/// It is **asked of the VM**, by the road [`listens_for`] takes and one ivar shallower, rather
/// than worked out here from the `Sky`. Night and asleep are not the same thing, and this source
/// is not allowed to guess which: whether to sleep at all is the creature file's own decision, a
/// creature born after dark has never seen the `on(:night)` that would have set the flag, and a
/// species whose file has no `on(:night)` walks about all night. A copy of the rule in Rust would
/// be a second opinion about somebody else's script, and wrong in all three cases.
///
/// `None` where there is nothing to ask — no `@being` yet, the task not having reached
/// `run_creature` — and the caller counts the pairing, exactly as it does for [`listens_for`].
fn asleep_now(vm: &Vm, task: ObjId) -> Option<bool> {
    let being = vm.ivar_get(task, BEING_IVAR).obj()?;
    // nil until the night handler has run for the first time, which reads as awake
    Some(vm.ivar_get(being, ASLEEP_IVAR).truthy())
}

/// **The pairings the eighth check is still waiting on, closed one way or the other**
/// (2026-09-18).
///
/// A pairing counted in `SelfTest::courtings` is a question put to a creature's `on(:mate)`: will
/// the road from the rules' message to `garden.spawn` be walked? It is answered when that creature
/// asks the game for a child — `asked`, which is every `garden.spawn` of this frame — and the
/// entry simply leaves, having been measured.
///
/// It is **not** answered when one of the two is no longer there. The beetle's handler reads its
/// partner's genome out of the partner's own `Creature` component, and a partner that starved in
/// the frame between the rule speaking and the handler waking gives it nil; the handler stops
/// there, and no spawn is ever asked for. That is not the road being broken, it is the road never
/// having been walked — so the pairing comes back out of the denominator and is said out loud,
/// because a pairing that quietly disappeared is what the check used to do with it.
fn close_courtings(world: &mut bevy::ecs::world::World, asked: &[Entity]) {
    if world.get_resource::<SelfTest>().is_none() {
        return;
    }
    let open = std::mem::take(&mut world.resource_mut::<SelfTest>().courtings_open);
    if open.is_empty() {
        return;
    }
    let mut still: Vec<(Entity, Entity, f32)> = Vec::with_capacity(open.len());
    let mut lost: Vec<(f32, &'static str)> = Vec::new();
    for (who, partner, at) in open {
        if asked.contains(&who) {
            continue;
        }
        let there = |e: Entity| world.get::<Creature>(e).is_some();
        match (there(who), there(partner)) {
            (true, true) => still.push((who, partner, at)),
            (true, false) => lost.push((at, "the partner was gone before the handler could ask for a child")),
            _ => lost.push((at, "the creature that was told was gone before it could ask for a child")),
        }
    }
    let mut test = world.resource_mut::<SelfTest>();
    test.courtings_open = still;
    test.courtings -= lost.len() as u32;
    test.courtings_lost += lost.len() as u32;
    for (at, why) in lost {
        info!("selftest: --   the pairing at {at:.2} s measured nothing: {why}");
    }
}

/// One creature's `@memory`, read out of the VM.
///
/// Two `ivar_get`s and a conversion — the task's `@being`, that object's `@memory`, and
/// `from_value` into the `serde_json::Value` the save file wants. A script that has not reached
/// `run_creature` yet, or one that has never touched `@memory`, gives `null`, which reads back as
/// an empty memory.
fn read_memory(vm: &mut Vm, task: ObjId) -> serde_json::Value {
    let Some(being) = vm.ivar_get(task, BEING_IVAR).obj() else { return serde_json::Value::Null };
    let memory = vm.ivar_get(being, "@memory");
    match sabiruby_serde::from_value::<serde_json::Value>(vm, memory) {
        Ok(value) => value,
        Err(e) => {
            // a Hash with something in it that JSON has no name for: a Proxy, an entity, a Task.
            // The creature keeps it; the file does not get it.
            warn!("a memory would not convert: {}", vm.describe_error(&e));
            serde_json::Value::Null
        }
    }
}

/// F5, and the end of a `--save` run: the whole world into one file.
///
/// It is an ordinary system with ordinary queries — the only thing in it that is not Bevy is the
/// `ScriptWorld`, and that is only there for the memories. Sorting by position is what makes two
/// saves of one world the same text: Bevy's iteration order is by archetype, and a creature whose
/// script has ended is in a different archetype from one whose script is running, so the order a
/// query walks in is not something a file should inherit.
fn save_world(
    time: Res<Time>,
    sky: Res<Sky>,
    file: Res<SaveFile>,
    mut now: ResMut<SaveNow>,
    mut note: ResMut<SaveNote>,
    mut scripts: ResMut<ScriptWorld>,
    creatures: Query<(&Creature, &Transform, &Hunger, Option<&ScriptTask>)>,
    plants: Query<(&Plant, &Transform)>,
    trees: Query<&Transform, With<Tree>>,
    rocks: Query<&Transform, With<Rock>>,
) {
    if !now.0 {
        return;
    }
    now.0 = false;

    let ground = |at: &Transform| [at.translation.x, at.translation.z];
    let mut save = GardenSave {
        version: SAVE_VERSION,
        tick: world_now(&time, &sky),
        day_phase: sky.phase,
        night: sky.night,
        plants: plants.iter().map(|(plant, at)| PlantSave { at: ground(at), size: plant.size }).collect(),
        trees: trees.iter().map(ground).collect(),
        rocks: rocks.iter().map(ground).collect(),
        creatures: creatures
            .iter()
            .map(|(creature, at, hunger, task)| CreatureSave {
                species: creature.species,
                at: ground(at),
                hunger: hunger.0,
                age: creature.age,
                genome: creature.genome,
                memory: match task {
                    Some(task) => read_memory(&mut scripts.vm, task.task()),
                    None => serde_json::Value::Null,
                },
            })
            .collect(),
    };
    let by_place = |a: &[f32; 2], b: &[f32; 2]| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal);
    save.plants.sort_by(|a, b| by_place(&a.at, &b.at));
    save.trees.sort_by(|a, b| by_place(a, b));
    save.rocks.sort_by(|a, b| by_place(a, b));
    save.creatures.sort_by(|a, b| by_place(&a.at, &b.at));

    let text = match serde_json::to_string_pretty(&save) {
        Ok(text) => text,
        Err(e) => {
            error!("the garden would not turn into JSON: {e}");
            note.say(save.tick, true, format!("the garden would not turn into JSON: {e}"));
            return;
        }
    };
    let bytes = text.len();
    match platform::write(Path::new(&file.path), &text) {
        Ok(()) => {
            info!(
                "saved {} creatures, {} plants at {:.1} s to {} ({bytes} bytes)",
                save.creatures.len(),
                save.plants.len(),
                save.tick,
                file.path
            );
            note.say(
                save.tick,
                false,
                format!("saved {} creatures, {} plants ({bytes} bytes)", save.creatures.len(), save.plants.len()),
            );
        }
        Err(e) => {
            error!("{}: {e}", file.path);
            note.say(save.tick, true, format!("{}: {e}", file.path));
        }
    }
}

/// Reads the file (or the browser's local storage) and hands the result to `load_world`.
///
/// The version is read first and on its own (G5). A whole `GardenSave` cannot do the job: a save
/// from another build fails on whichever field differs, and the message is serde's
/// (``missing field `sight` at line 214``), which says nothing about what actually happened. So
/// a struct with one field goes through the same text first — serde_json ignores what it has no
/// field for, so this parses any JSON object at all — and only a file that says `1` is read the
/// rest of the way. Nothing is loaded when this answers `Err`: the garden the player is in keeps
/// going (F9), or a new one is built (`--load`, the browser's first frame).
fn read_save(path: &str) -> Result<GardenSave, String> {
    let text = platform::read(Path::new(path))?;

    #[derive(Deserialize)]
    struct Stamp {
        version: Option<u32>,
    }
    let stamp: Stamp = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    match stamp.version {
        Some(SAVE_VERSION) => {}
        // the two ways a file can be from somewhere else: a number that is not ours, and no
        // number at all — which is every garden saved before G5
        Some(other) => {
            return Err(format!("{path}: saved with version {other}, this garden reads {SAVE_VERSION}"))
        }
        None => {
            return Err(format!("{path}: saved with no version, this garden reads {SAVE_VERSION}"))
        }
    }
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// The tenth check's file (G5): a save that is this build's format in every respect except the
/// one that matters.
///
/// It is `GardenSave::default()` — an empty world, which is a perfectly loadable one — with the
/// version changed. That is deliberate: every other field is exactly what this build writes, so
/// the only thing that can refuse it is the number, and if `read_save` ever stopped looking at the
/// number the file would load and the check would fail rather than pass by accident. It goes in
/// the temporary directory because the thing under test is `--load`, not where a file lives.
fn write_a_save_from_another_version() -> String {
    let path = platform::another_version_file();
    let save = GardenSave { version: 99, ..Default::default() };
    let text = serde_json::to_string_pretty(&save).expect("an empty garden is JSON");
    if let Err(e) = platform::write(Path::new(&path), &text) {
        error!("the selftest could not write {path}: {e}");
    }
    path
}

/// F9, and `--load PATH` before the first frame: the garden in the file, built.
///
/// Everything that was living is despawned first — which takes each creature's `ScriptTask` with
/// it, and rubevy's `on_remove` hook terminates the task and closes the queues it was waiting on,
/// so a loaded garden has no minds left over from the one before it. Then the same three spawn
/// functions the world was built with the first time, and `give_mind` again: a creature read out
/// of a file is not a special kind of creature.
///
/// **And it is told what the sky is doing** (2026-09-18). A creature made here is exactly as deaf
/// as a creature made by `children_arrive`: its script has not subscribed to anything yet, and
/// `"night"` and `"day"` are said once each, at the turn. A garden saved in the dark and opened
/// again therefore used to come back with every creature walking about until morning — the same
/// hole the newborns had, found from the other end (`docs/worklog/2026-09-18-selftest-fixes.md`,
/// §5). So every creature the file makes goes on [`Newborns`] and hears the sky two frames later,
/// by the same road and with the same wait.
///
/// It is told **whatever the sky is**, not only when it is night. "Only if `sky.night`" would be
/// a second copy of `day_night`'s list of what the sky can say, kept in step by hand, and it
/// would buy one publish per creature on a daytime load — a `"day"` to a creature that is already
/// awake, which is what `on(:day)` does with it anyway. The duplicate is cheaper than the flag,
/// which is the same trade `tell_newborns_the_sky` already makes for a birth on the turning
/// frame.
fn load_world(
    mut commands: Commands,
    time: Res<Time>,
    loading: Res<Loading>,
    mut sky: ResMut<Sky>,
    mut note: ResMut<SaveNote>,
    mut newborns: ResMut<Newborns>,
    look: Option<Res<Look>>,
    ruby: Res<RubyDir>,
    mut brains: ResMut<Brains>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut dice: ResMut<Dice>,
    // S5b-3: a rock's squash and whether a blade is a bush are not in the save (`GardenSave`
    // says why), so they are rolled again here — out of the same two settings `spawn_world`
    // rolls them from, or a garden read back would wear numbers the store no longer holds
    built: Res<Furniture>,
    bodies: Res<Bodies>,
    old: Query<Entity, Or<(With<Plant>, With<Tree>, With<Rock>, With<Creature>)>>,
) {
    let save = loading.save.clone();
    let look = look.as_deref();
    let mut gone = 0;
    for entity in &old {
        commands.entity(entity).despawn();
        gone += 1;
    }
    // whatever was still waiting for its word belonged to the garden that has just been thrown
    // away (F9 over a running world): those entities are gone, and `tell_newborns_the_sky` would
    // drop them anyway for having no `Mind`
    newborns.waiting.clear();

    for plant in &save.plants {
        // whether a plant is a bush or a tuft is the model's business, so it is rolled again
        // rather than written down
        spawn_plant(&mut commands, look, Vec2::from(plant.at), plant.size, dice.roll() < built.round_chance);
    }
    for at in &save.trees {
        spawn_tree(&mut commands, look, &bodies, Vec2::from(*at));
    }
    for at in &save.rocks {
        spawn_rock(&mut commands, look, &bodies, Vec2::from(*at), dice.between(built.rock_squash[0], built.rock_squash[1]));
    }
    let mut pending = Vec::new();
    for creature in &save.creatures {
        let entity = spawn_creature(
            &mut commands,
            look,
            &bodies,
            creature.species,
            Vec2::from(creature.at),
            creature.hunger,
            creature.genome,
            // nothing read back from a file is a newborn: `parent` is what the rules look at to
            // find one, and a creature that was saved has been in the world already
            None,
        );
        // the age it had, not a newborn's: `spawn_creature` makes an ordinary creature and this is
        // the one field of it that a file can be older than
        commands.entity(entity).insert(Creature {
            species: creature.species,
            age: creature.age,
            genome: creature.genome,
            parent: None,
        });
        give_mind(&mut commands, &ruby.0, &mut brains, &mut mrb, entity, creature.species);
        // it has a body and no ears yet, which is the whole of what a newborn is here
        newborns.waiting.push((entity, 0));
        if !creature.memory.is_null() {
            pending.push((entity, creature.memory.clone()));
        }
    }

    // the day goes on from where it stopped
    sky.shift = save.tick - time.elapsed_secs();
    sky.phase = save.day_phase;
    sky.night = save.night;
    info!(
        "loaded {} creatures, {} plants, {} trees, {} rocks at {:.1} s ({gone} entities made way)",
        save.creatures.len(),
        save.plants.len(),
        save.trees.len(),
        save.rocks.len(),
        save.tick
    );
    note.say(
        save.tick,
        false,
        format!("loaded {} creatures, {} plants at {:.1} s", save.creatures.len(), save.plants.len(), save.tick),
    );
    commands.remove_resource::<Loading>();
    commands.insert_resource(Restoring {
        pending,
        tick: save.tick,
        since: time.elapsed_secs(),
        done: false,
    });
}

/// Puts each creature's `@memory` back, as soon as there is something to put it on.
///
/// It runs after the scripts have had their frame, because what it is waiting for is the object
/// `run_creature` makes — the task exists a frame before that object does. Until every memory is
/// in place the world is still (`is_still` below is the run condition of every rule) and its clock
/// is held here, so the creature that got its memory first has not walked a step further than the
/// one that got it last.
fn restore_memory(
    time: Res<Time>,
    mut restoring: ResMut<Restoring>,
    mut sky: ResMut<Sky>,
    mut scripts: ResMut<ScriptWorld>,
    mut reload: Option<ResMut<ReloadAt>>,
    budgets: Res<Budgets>,
    tasks: Query<&ScriptTask>,
) {
    // the world's clock stands still while this lasts
    sky.shift = restoring.tick - time.elapsed_secs();
    let vm = &mut scripts.vm;
    restoring.pending.retain(|(entity, memory)| {
        let Ok(task) = tasks.get(*entity) else { return true };
        // `@being` appears when the script reaches `run_creature`, which is its first frame
        let Some(being) = vm.ivar_get(task.task(), BEING_IVAR).obj() else { return true };
        match sabiruby_serde::to_value(vm, memory) {
            Ok(value) => vm.ivar_set(being, "@memory", value),
            Err(e) => error!("a memory would not go back: {}", vm.describe_error(&e)),
        }
        false
    });
    let waited = time.elapsed_secs() - restoring.since;
    if restoring.pending.is_empty() {
        restoring.done = true;
    } else if waited > budgets.restore_patience {
        warn!("{} creatures never started; the garden is running anyway", restoring.pending.len());
        restoring.done = true;
    }
    // `stop_when_over` runs between this and `save_world`, so a run told to reload at its last
    // second ends on this frame and writes the world it just read
    if restoring.done && let Some(reload) = reload.as_mut() {
        reload.restored = true;
    }
}

/// The last thing in a frame that finished restoring: the world may move again.
///
/// It is a system of its own, and after the save, so that a run which loads and saves in one go
/// (the round-trip check) writes the world it read rather than that world plus one frame.
fn finish_restore(mut commands: Commands, restoring: Res<Restoring>) {
    if restoring.done {
        commands.remove_resource::<Restoring>();
    }
}

/// **The run condition of every rule.** A garden that is still being restored does not move —
/// and, from G9, neither does one the player has stopped with `P`.
///
/// The two reasons are the same shape and they are the same condition on purpose: whatever the
/// world is waiting for, the whole of it waits together. Everything the garden *is* hangs off
/// this — `day_night`, `move_creatures`, `separate`, `grow_plants`, `sprout_plants`,
/// `get_hungry`, `eat`, `startle`, `court`, `starve` and `hatch` — while everything that only
/// *looks* at the garden (the camera, the models, the horizon, the HUD, the editor, the VM
/// panel, saving and loading) runs on.
///
/// `Paused` is an `Option` because the headless build has no keyboard to press `P` with and
/// never inserts the resource; `None` is a world nobody can pause.
fn is_still(restoring: Option<Res<Restoring>>, paused: Option<Res<window::Paused>>) -> bool {
    restoring.is_none() && !paused.is_some_and(|p| p.on())
}

/// The other half of `P` (G9): while the world is stopped, so is its clock.
fn is_paused(paused: Option<Res<window::Paused>>) -> bool {
    paused.is_some_and(|p| p.on())
}

/// **The garden's clock, held still while `P` is on.**
///
/// The world's hour is `world_now` — the process's clock plus `Sky::shift` — so holding it is
/// walking `shift` back by exactly the frame that has just passed. Nothing else is needed and
/// nothing else would do: `day_night` is not running, so `sky.phase` stands where it stood, and
/// when the budget comes back the sun is where it was rather than where the wall clock says. It
/// is `restore_memory`'s trick (`sky.shift = tick - elapsed`) written as a difference, because
/// here there is no hour to go back to — only one to stay at.
///
/// It is why the pause loses no event. `"night"` and `"day"` are published by `day_night` at the
/// moment the phase crosses, and a clock that does not move cannot cross anything: there is no
/// message to be dropped while the scripts cannot read their queues.
fn hold_the_clock(time: Res<Time>, mut sky: ResMut<Sky>) {
    sky.shift -= time.delta_secs();
}

/// `GARDEN_RELOAD_AT` (checks only): F9, pressed by the clock instead of by a finger.
///
/// Nothing here is a second way to load: it puts a `Loading` in exactly as `save_load_keys` does
/// for the key, and `load_world` is the one that does the work.
fn reload_while_running(
    time: Res<Time>,
    mut reload: ResMut<ReloadAt>,
    mut note: ResMut<SaveNote>,
    sky: Res<Sky>,
    mut commands: Commands,
) {
    if reload.asked || time.elapsed_secs() < reload.at {
        return;
    }
    reload.asked = true;
    match read_save(&reload.path) {
        Ok(save) => {
            info!("GARDEN_RELOAD_AT: loading {} into the running garden", reload.path);
            commands.insert_resource(Loading { save });
        }
        Err(e) => {
            error!("{e}");
            note.say(world_now(&time, &sky), true, e);
        }
    }
}

/// F5 and F9, and the HUD's two buttons, in the window. The headless build has no `ButtonInput`
/// at all (`MinimalPlugins` brings no input), which is why this system is only added there and the
/// command line is what asks for a save without one.
///
/// The keys are the same two in both builds (G5). A browser would take F5 for itself — it is
/// Reload, and a reload is the one thing a player who meant to save must not get — so the page
/// takes the key back before the game sees it (the game's `KEYS` in `web/games.sh`, a `keydown`
/// in the capture phase), which is what SabiRuby Battle already does for its own F5. F9 no
/// browser wants. The buttons are
/// there because a key that only works because a page remembered to intercept it is a thin thing
/// to hang a garden on, and because nobody opening a link knows that F5 is Save.
fn save_load_keys(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    sky: Res<Sky>,
    file: Res<SaveFile>,
    mut asked: ResMut<Asked>,
    mut now: ResMut<SaveNow>,
    mut note: ResMut<SaveNote>,
    mut commands: Commands,
) {
    // These two are **not** behind `EguiWantsInput::wants_keyboard_input()`, and `inspect_keys`'
    // two are — the difference is what the key is. `P` is a letter, and a letter pressed while the
    // caret is in the editor belongs to the editor. F5 and F9 are keys egui never wants, and
    // guarding them was measured to be worse than useless: once the editor's text box has taken
    // keyboard focus it does not give it up when the pointer clicks the garden, so a guarded F5
    // is a save key that stops working for the rest of the session after the first edit (seen in
    // a browser: after the window checks had typed into the editor, neither F5 nor P did anything
    // again). The buttons are the other half of the same answer.
    let save = asked.save || keys.just_pressed(KeyCode::F5);
    let load = asked.load || keys.just_pressed(KeyCode::F9);
    asked.save = false;
    asked.load = false;
    if save {
        now.0 = true;
    }
    if load {
        match read_save(&file.path) {
            Ok(save) => commands.insert_resource(Loading { save }),
            Err(e) => {
                error!("{e}");
                note.say(world_now(&time, &sky), true, e);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The models move (G0a). Ruby knows nothing about any of this.
// ---------------------------------------------------------------------------------------------

/// A scene brings its own `AnimationPlayer` when it is spawned, on whichever of its nodes carries
/// the animation. This finds the creature it belongs to by walking up the hierarchy, gives the
/// player the right graph, and remembers where it is.
fn dress_animations(
    mut commands: Commands,
    look: Res<Look>,
    added: Query<Entity, Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    creatures: Query<&Creature>,
    mut dressed: Local<bevy::platform::collections::HashMap<&'static str, bool>>,
) {
    for player in &added {
        let mut at = player;
        let mut owner = None;
        loop {
            if creatures.get(at).is_ok() {
                owner = Some(at);
                break;
            }
            match parents.get(at) {
                Ok(parent) => at = parent.parent(),
                Err(_) => break,
            }
        }
        let Some(owner) = owner else { continue };
        let Ok(creature) = creatures.get(owner) else { continue };
        let gaits = match creature.species {
            Species::Beetle => look.beetle_gaits.clone(),
            Species::Rabbit => look.rabbit_gaits.clone(),
        };
        commands
            .entity(player)
            .insert((AnimationGraphHandle(gaits.graph.clone()), AnimationTransitions::new()));
        commands.entity(owner).insert(Animated { player, playing: Gait::Idle });
        // once per species, so that a run says out loud whether the clips were found: without
        // the `gltf_animation` feature the graph is three handles to assets that do not exist,
        // the scene brings no `AnimationPlayer`, and this line never appears
        if !*dressed.entry(creature.species.name()).or_insert(false) {
            dressed.insert(creature.species.name(), true);
            info!("{} walks, idles and eats from its model", creature.species.name());
        }
    }
}

/// Walk when it is moving, eat when it is eating, idle otherwise — decided from `Velocity` and
/// the `Eating` mark, which are the game's own state. A creature's script never knows that any
/// of this happened, and could not reach it if it wanted to: `Animated`, `Eating` and `Gait`
/// derive no `Reflect` and are registered nowhere.
fn animate_creatures(
    time: Res<Time>,
    look: Res<Look>,
    mut creatures: Query<(&Creature, &Velocity, Option<&Eating>, &mut Animated)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let now = time.elapsed_secs();
    for (creature, velocity, eating, mut animated) in &mut creatures {
        let want = if eating.is_some_and(|e| e.until > now) {
            Gait::Eat
        } else if velocity.0.length() > look.walking_at {
            Gait::Walk
        } else {
            Gait::Idle
        };
        let Ok((mut player, mut transitions)) = players.get_mut(animated.player) else { continue };
        if animated.playing == want && player.playing_animations().count() > 0 {
            continue;
        }
        let gaits = match creature.species {
            Species::Beetle => &look.beetle_gaits,
            Species::Rabbit => &look.rabbit_gaits,
        };
        transitions
            .play(&mut player, gaits.node(want), std::time::Duration::from_millis(look.gait_blend_ms))
            .repeat();
        animated.playing = want;
    }
}

// ---------------------------------------------------------------------------------------------
// The three checks G1 adds (`GARDEN_SELFTEST=1`)
// ---------------------------------------------------------------------------------------------

/// **A hungry creature with a plant in sight reaches it.** The probe beetle was put down with
/// one plant five units away and nothing else within seven, and forty points of hunger — under
/// its own script's threshold of fifty-five. Nothing in Rust moves it; if it gets there, a Ruby
/// task read `me[:Hunger]`, asked `garden.nearest(:Plant)`, read the answer's
/// `[:Transform][:translation]` and wrote `me[:Velocity]`.
///
/// **A run where something walked into the probe on the way has not measured this** (2026-09-18).
/// The corner is only kept clear at spawn time (`clear_of_fixtures`), and nothing stops a rabbit
/// walking into it an hour later. When one does, the game publishes `"touched"` and the beetle's
/// own `on(:touched)` turns it away from the plant at `DASH` and holds the wheel for half a
/// second — and if the rabbit stays, it is touched again every half second and never walks
/// anywhere of its own again. The distance of that frame (1.8 to 2.3 over the three runs that
/// were taken apart) is then what `probe_closest` keeps, and the check prints it as a failure
/// that has nothing to do with whether a script can read a component, ask a question and steer
/// (`docs/worklog/2026-09-17-selftest-flakes.md` §3).
///
/// So a touch before the plant is reached makes the run say *it could not be measured* — neither
/// ok nor FAIL. It is the shape the sixth check already has for a touch it could not read
/// (`looked == 0`, `watch_turning`), and it reads the same `test.last_touch` that `startle`
/// keeps, so nothing new is watched. The difference from the sixth is that there are twenty-odd
/// touches in a run and only ever one probe: a disturbed run loses this check entirely rather
/// than one sample of it. No threshold moved — `REACH + 0.5` is where it was.
fn watch_probe(
    time: Res<Time>,
    mut test: ResMut<SelfTest>,
    probes: Query<(Entity, &Transform, &Probe)>,
    plants: Query<&Transform, With<Plant>>,
) {
    let now = time.elapsed_secs();
    for (beetle, at, probe) in &probes {
        // a rabbit that came before it got there: the run is out, and the moment is kept for the
        // line the check prints. `startle` writes this for every `"touched"` it publishes,
        // before any of the sixth check's own guards
        let touched = test.last_touch.iter().find(|(e, _)| *e == beetle).map(|(_, at)| *at);
        if test.probe_reached.is_none() && test.probe_disturbed.is_none() {
            test.probe_disturbed = touched;
        }
        let here = Vec2::new(at.translation.x, at.translation.z);
        let Ok(dinner) = plants.get(probe.dinner) else {
            // the plant is gone, which happens when it has been eaten: it was reached
            if test.probe_reached.is_none() {
                test.probe_reached = Some(now);
                test.probe_closest = 0.0;
            }
            continue;
        };
        let span = here.distance(Vec2::new(dinner.translation.x, dinner.translation.z));
        if test.probe_from == 0.0 {
            test.probe_from = span;
        }
        test.probe_closest = test.probe_closest.min(span);
        if span < REACH + 0.5 && test.probe_reached.is_none() {
            test.probe_reached = Some(now);
            info!("selftest: the hungry beetle reached its plant at {now:.2} s");
        }
    }
}

/// **A beetle touched by a rabbit changes heading within 0.5 s.** `startle` notes the beetle and
/// the way it was going at the moment the game published `"touched"`; this watches the half
/// second that follows. Only a beetle that was actually walking is counted, because "it turned"
/// means nothing about one that was standing still or asleep.
///
/// **It looks at the whole window, not at the far end of it** (2026-09-17). It used to take one
/// sample, at `at + 0.5`, and ask whether the heading there was a quarter turn off the one at
/// `at`. That is a different question from the one the check's own sentence asks — *changed
/// heading within 0.5 s* — and the difference is not academic: a turn that happened and was over
/// before the sample read as a turn that never happened. Two ways of it being over:
///
/// * **a second `"touched"` inside the window.** The first handler turned the beetle 99°, then a
///   second message arrived and its handler chose its swerve from the *new* heading, landing 38°
///   from the one this check remembers. Both handlers did their job; the sample saw the sum.
///   `TOUCH_SETTLE` keeps a touch from being counted when a message came in the 1.5 s *before*
///   it, and nothing ever excluded one arriving after.
/// * **the wheel coming back.** A handler holds it for `sleep 0.5` and this looked at 0.502 s —
///   the same number on both sides — so the behaviour's next `act` could land inside the sample.
///   Making the reads synchronous moved the reflex a frame earlier and that was enough to put it
///   there (`docs/worklog/2026-09-17-sync-reads.md`, sections 8 and 9).
///
/// Measured over ten ninety-second runs, the old way missed 7 of 248 counted touches and failed
/// the check in 4 runs; in 6 of those 7 the beetle was at `DASH` speed at the sample — *still
/// fleeing*, from a heading newer than `was`.
///
/// **No threshold moved.** The window is still 0.5 s and a turn is still `dot < 0.7`; what
/// changed is that the window is read every frame in it rather than once at its end. Nor did the
/// rule for what counts as turned: `b == Vec2::ZERO || a.dot(b) < 0.7`, the same expression,
/// evaluated more often.
///
/// What it costs: a frame of the window that turns for a reason of its own now counts. The
/// behaviour's `wander` rerolls its course with probability 0.25 a pass and a pass is `sleep 0.2`,
/// so a beetle whose handler never fired has roughly a 38% chance of drifting past a quarter turn
/// on its own inside half a second. That is the check's sensitivity per touch, not its verdict:
/// the verdict is over every counted touch in a run (25 or so), and a handler that never fired
/// would miss most of them. A handler that fires takes the wheel for the whole window, so nothing
/// else can be what turned it.
fn watch_turning(
    time: Res<Time>,
    mut test: ResMut<SelfTest>,
    field: Res<Place>,
    creatures: Query<(&Velocity, &Transform)>,
) {
    let now = time.elapsed_secs();
    let mut still_waiting: Vec<Touch> = Vec::new();
    let mut checked = 0u32;
    let mut turned = 0u32;
    for mut touch in std::mem::take(&mut test.touched) {
        // starved meanwhile: there is nothing to ask about it
        let Ok((going, place)) = creatures.get(touch.beetle) else { continue };
        // and not against a wall: `move_creatures` zeroes the component of `Velocity` that would
        // take a creature through one, so a beetle in the corner reads as going due west both
        // before the handler and after it however it turned. The handler is not what failed
        // there, and a check that says it did would be a check about the walls. A frame like that
        // is skipped rather than judged; a touch whose whole window was like that is not counted.
        if !by_a_wall(&field, place) {
            // the angle between the two headings: a flee is roughly a reversal, and anything past
            // a quarter turn is a different course than the one it was on. A beetle that stopped
            // has changed what it is doing as surely as one that turned, so it counts too — which
            // is what the `Vec2::ZERO` arm of the old one-shot test said, in the same words.
            let a = touch.was.normalize_or_zero();
            let b = going.0.normalize_or_zero();
            let dot = if b == Vec2::ZERO { -1.0 } else { a.dot(b) };
            touch.looked += 1;
            touch.last = going.0;
            if dot < touch.closest {
                touch.closest = dot;
                touch.closest_at = now;
            }
            if dot < 0.7 {
                touch.turned = true;
            }
        }
        if now - touch.at < 0.5 {
            still_waiting.push(touch);
            continue;
        }
        // the window has closed. A touch nobody could read for the whole of it proves nothing
        if touch.looked == 0 {
            continue;
        }
        checked += 1;
        if touch.turned {
            turned += 1;
        } else {
            // **A miss says which one, and what it was doing.** A line reading `FAIL … (24/25)`
            // names no beetle, and this check has been flaky twice now for reasons that were only
            // findable from the headings: G4's was a `@course` written by an `act` that never went
            // out, and 2026-09-17's were the two in this function's doc comment. The length of
            // `is` is half the evidence — `DASH` is a handler, `CRUISE` is the behaviour having
            // taken the wheel back — and `closest` says whether it nearly turned or never moved.
            info!(
                "selftest: miss  {} touched at {:.2}, watched to {:.2} over {} frames: closest dot {:.3} at {:.2}, was {:?} ({:.1}), is {:?} ({:.1})",
                touch.beetle,
                touch.at,
                now,
                touch.looked,
                touch.closest,
                touch.closest_at,
                touch.was,
                touch.was.length(),
                touch.last,
                touch.last.length()
            );
        }
    }
    test.touched = still_waiting;
    test.turn_checked += checked;
    test.turned += turned;
}

/// Within a unit of the edge of the field, where `move_creatures` clips `Velocity` and a heading
/// stops being readable from it.
fn by_a_wall(field: &Place, at: &Transform) -> bool {
    at.translation.x.abs() > field.half_w() - 1.5 || at.translation.z.abs() > field.half_d() - 1.5
}

/// **Creatures sleep at night.** One second after the game published `"night"`, nothing with a
/// script should still be moving: both species have `on(:night) { @asleep = true; stop }`,
/// and their run loops keep it that way. The fasting beetle has no script and is standing still
/// anyway, so it proves nothing and is not counted — `Mind` is the mark of a creature that has
/// a brain to fall asleep with.
///
/// **A creature younger than [`NEWBORN_GRACE`] is not counted** (2026-09-18), which is the same
/// exclusion `startle` makes for the sixth check and for the same reason: a creature spends its
/// first frames with no handlers subscribed, and "did the handler put it to sleep?" cannot be
/// asked of a creature that had no handlers when the word went out. The word does reach it now —
/// `tell_newborns_the_sky` says the sky again to each newborn — and that is what closes the hole;
/// this is only the check saying what it is able to see. Two seconds is [`NEWBORN_GRACE`]'s own
/// number, already in this source, and it is a good deal more than the two frames a newborn is
/// actually deaf for.
fn watch_sleep(
    time: Res<Time>,
    mut test: ResMut<SelfTest>,
    creatures: Query<(&Creature, &Velocity), With<Mind>>,
) {
    let Some(night) = test.night_at else { return };
    if test.asleep_at.is_some() {
        return;
    }
    let now = time.elapsed_secs();
    if now - night < 1.0 {
        return;
    }
    test.asleep_at = Some(now);
    for (creature, velocity) in &creatures {
        if creature.age <= NEWBORN_GRACE {
            continue;
        }
        test.awake_speed = test.awake_speed.max(velocity.0.length());
        test.asleep_counted += 1;
    }
}

// ---------------------------------------------------------------------------------------------
// Running without a window
// ---------------------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn stop_when_over(
    time: Res<Time>,
    headless: Res<Headless>,
    sky: Res<Sky>,
    test: Option<Res<SelfTest>>,
    reload: Option<Res<ReloadAt>>,
    restoring: Option<Res<Restoring>>,
    file: Res<SaveFile>,
    vms: VmReport,
    mut save: ResMut<SaveNow>,
    creatures: Query<(&Creature, &Hunger, &Velocity, &Transform, Option<&Mind>)>,
    panelled: Query<(Entity, &Creature, &Hunger, &Mind)>,
    tasks: Query<(&Mind, &ScriptTask)>,
    world_task: Query<&ScriptTask<World>, With<WorldScript>>,
    plants: Query<&Plant>,
    everything: Query<Entity>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if now < headless.until {
        return;
    }
    // a world that is still being read back is not a world to report on, and `--headless 0
    // --load X --save Y` is exactly that run: it ends on the frame the last memory goes home
    if restoring.is_some_and(|r| !r.done) {
        return;
    }
    // and a run told to reload at `N` with `--headless N` ends on the frame *that* load is whole,
    // not on the frame the clock struck
    if reload.is_some_and(|r| !r.restored) {
        return;
    }
    for (creature, hunger, velocity, at, mind) in &creatures {
        // the last two columns are the script's: what it spent on the frame that has just gone,
        // and the line of its own file it is standing on — which is the line it is *waiting* on,
        // since a creature spends most of its life parked on a `sleep` or on an answer
        info!(
            "{:<14} hunger {:>5.1}  age {:>5.1}  at ({:>6.1}, {:>6.1})  v ({:>5.1}, {:>5.1})  {:>6} insn/frame  {}",
            mind.map(|m| m.name.clone()).unwrap_or_else(|| creature.species.name().into()),
            hunger.0,
            creature.age,
            at.translation.x,
            at.translation.z,
            velocity.0.x,
            velocity.0.y,
            mind.map(|m| m.last_instructions / m.frames.max(1)).unwrap_or(0),
            mind.map(|m| m.at.as_str()).unwrap_or("(no behaviour)")
        );
    }
    info!(
        "{} creatures, {} plants, {} at phase {:.2}",
        creatures.iter().count(),
        plants.iter().count(),
        if sky.night { "night" } else { "day" },
        sky.phase
    );
    // G4: the window's two panels, in words, for a run that has no window — the same fields the
    // HUD draws and the same `VmInspector` the VM panel draws, filled here and printed.
    info!(
        "hud: {} creatures · {} plants · {} {:.2} · VM {:.2} / {:.1} ms this frame ({:.2} ms smoothed)",
        panelled.iter().count(),
        plants.iter().count(),
        if sky.night { "night" } else { "day" },
        sky.phase,
        vms.clock.spent_ms,
        vms.clock.budget_ms,
        vms.clock.mean_ms,
    );
    for row in window::hud_rows(&panelled) {
        info!(
            "hud:   {:<14}{} hunger {:>5.1}  {:>6} insn/frame  {} insn/decision  {}",
            row.name,
            if row.in_memory { "*" } else { " " },
            row.hunger,
            row.insn_per_frame,
            match row.per_decision {
                Some(n) => format!("{n:>6.0}"),
                None => "     –".into(),
            },
            row.at,
        );
    }
    {
        // what a pass of a behaviour's loop cost over the whole run — the number the HUD's
        // `insn/decision` column is the per-creature form of — and, beside it, the one kind of
        // question that still costs a frame. The component reads that used to stand here are
        // gone from the line because they are gone from the frame: rubevy answers them inside
        // the tick that asked them, so there is nothing left to count in frames.
        let (mut ask_trips, mut ask_frames) = (0u32, 0u32);
        let (mut decisions, mut decision_instructions) = (0u32, 0u64);
        for (_, _, _, mind) in &panelled {
            ask_trips += mind.ask_trips;
            ask_frames += mind.ask_frames;
            decisions += mind.decisions;
            decision_instructions += mind.decision_instructions;
        }
        let mean = |f: u32, n: u32| if n == 0 { f32::NAN } else { f as f32 / n as f32 };
        info!(
            "hud: insn/decision — {decisions} passes of a behaviour's loop, {:.1} instructions each; and {ask_trips} questions the game answered, {:.3} frames each (a component read costs no frame at all)",
            if decisions == 0 { f32::NAN } else { decision_instructions as f32 / decisions as f32 },
            mean(ask_frames, ask_trips),
        );
        // W1: and the other VM's, which is one task running one pass a frame — so the
        // instructions it ran between two frames *are* a pass, with no gap to interpret. The two
        // budgets are not capped together (rubevy `docs/host-api.md`, "Two VMs in one app"), so
        // what a frame can cost is `VM` above plus `world` here, and these are the numbers the
        // world's own budget was chosen from.
        info!(
            "hud: the world's rules — {} frames they ran in, {} instructions at the median and {} at the most (of {}); the world's tick {:.2} ms at the median, {:.2} at the 99th frame in a hundred, {:.2} at the most / {:.1} ms{}",
            vms.meter.ran_in.len(),
            match vms.meter.median() {
                Some(n) => n.to_string(),
                None => "–".into(),
            },
            vms.meter.most,
            vms.world.budget,
            vms.meter.milliseconds().0,
            vms.meter.milliseconds().1,
            vms.meter.most_ms,
            vms.world.frame_time.map(|d| d.as_secs_f32() * 1000.0).unwrap_or(0.0),
            match &vms.trouble.0 {
                Some(why) => format!(" — and there are no rules: {why}"),
                None => String::new(),
            },
        );
        let mut panel = VmInspector::default();
        for (mind, script) in &tasks {
            // the same two figures the window's panel is handed (`window::show_vm`)
            panel.spent = mind.spent;
            panel.per_frame = Some(mind.last_instructions / mind.frames.max(1));
            panel.fill(&vms.scripts, script.task(), mind.name.clone(), mind.prelude_lines);
            for line in panel.log_lines() {
                info!("{line}");
            }
        }
        // and the rules, which are the other VM's one task (2026-09-18). `F2` shows this in a
        // window when `F3` has the editor on `world.rb` (`window::show_vm`); here it is the same
        // panel filled from the same VM, printed, so a run with no screen says what the rules are
        // waiting for as well as what a beetle is.
        if let Ok(task) = world_task.single() {
            panel.spent = vms.meter.last_pass;
            panel.per_frame = vms.meter.median();
            panel.prelude_file = Some(WORLD_PRELUDE_FILE.into());
            panel.fill(&vms.world, task.task(), format!("the rules  {WORLD_FILE}"), vms.prelude.0);
            for line in panel.log_lines() {
                info!("{line}");
            }
        }
    }
    // what the population is made of (G2). The world starts with each species' own numbers
    // jittered by a sixth either way; anything the run has moved is breeding and starving —
    // the average of the survivors, not of the born.
    for species in [Species::Beetle, Species::Rabbit] {
        let mine: Vec<Genome> =
            creatures.iter().filter(|(c, ..)| c.species == species).map(|(c, ..)| c.genome).collect();
        if mine.is_empty() {
            continue;
        }
        let n = mine.len() as f32;
        let mean = Genome {
            speed: mine.iter().map(|g| g.speed).sum::<f32>() / n,
            sight: mine.iter().map(|g| g.sight).sum::<f32>() / n,
            appetite: mine.iter().map(|g| g.appetite).sum::<f32>() / n,
        };
        info!("{} × {}: mean genome {} (its species' own is {})", mine.len(), species.name(), mean.describe(), Genome::of(species).describe());
    }

    if let Some(test) = test {
        let ok = |cond: bool, what: String| info!("selftest: {} {what}", if cond { "ok  " } else { "FAIL" });
        // and the third verdict: a check the run put itself in no position to answer. It is not
        // a pass — nothing was proved — and it is not a failure either, and a line that said
        // either of those would be a lie about what the run saw (2026-09-18).
        //
        // **It is written `--`, as Battle writes it and as the pairings below write it** (S5b-5).
        // It used to be `n/a` here and `--` four lines away, two spellings of one verdict in one
        // game, which is one more thing for a reader of a run to work out and one more pattern
        // for `tools/fixedlines.sh` to carry.
        let unmeasured = |what: String| info!("selftest: --   {what}");
        ok(
            test.ate_at.is_some_and(|t| t <= 10.0),
            match test.ate_at {
                Some(t) => format!("somebody ate within 10 s (first at {t:.2} s)"),
                None => "somebody ate within 10 s (nobody ate at all)".into(),
            },
        );
        // **How long a day is, asked of the run rather than written down again** (S5b-5). The
        // threshold is one turn of the sun, and one turn of the sun is `world.rb`'s
        // `day_length` — which reaches the game as `Sky::day_length` (`garden.rules`). It was
        // written here as `60.0`, a copy of the value the file happens to ship with, so a
        // `world.rb` edited to a longer day would have failed this check for being obeyed. It is
        // the shape S5b-4 took out of the mutation rate and S5b-2 out of Battle's turn rate.
        let a_day = sky.day_length;
        ok(
            test.night_at.is_some_and(|t| t <= a_day),
            match test.night_at {
                Some(t) => format!("night arrived within a day of {a_day:.0} s (at {t:.2} s)"),
                None => format!("night arrived within a day of {a_day:.0} s (it never did)"),
            },
        );
        match test.starved {
            Some((entity, at)) => ok(
                !everything.contains(entity),
                format!("the starved creature's entity is gone ({entity} starved at {at:.2} s)"),
            ),
            None => ok(false, "a creature starved and its entity is gone (nobody starved)".into()),
        }
        ok(
            test.overlaps == 0 && test.frames > 0,
            format!(
                "nothing walked through anything over {} frames (closest pair {:.3} of the radii, {} frames under 0.9)",
                test.frames, test.closest, test.overlaps
            ),
        );
        // --- G1: the minds -------------------------------------------------
        match (test.probe_reached, test.probe_disturbed) {
            (Some(at), _) => ok(
                true,
                format!(
                    "a hungry creature with a plant in sight reached it (from {:.1} away, at {at:.2} s)",
                    test.probe_from
                ),
            ),
            // a rabbit walked into the probe before it got there and took the wheel off it: what
            // the beetle would have done on its own is not in this run (`watch_probe`)
            (None, Some(touched)) => unmeasured(format!(
                "a hungry creature with a plant in sight reached it (not measured: a rabbit walked into the probe at {touched:.2} s; it started {:.1} away and got no closer than {:.1})",
                test.probe_from, test.probe_closest
            )),
            (None, None) => ok(
                false,
                format!(
                    "a hungry creature with a plant in sight reached it (it started {:.1} away and got no closer than {:.1})",
                    test.probe_from, test.probe_closest
                ),
            ),
        }
        ok(
            test.turn_checked > 0 && test.turned == test.turn_checked,
            format!(
                "a beetle touched by a rabbit changed heading within 0.5 s ({}/{})",
                test.turned, test.turn_checked
            ),
        );
        match test.asleep_at {
            Some(at) => ok(
                test.asleep_counted > 0 && test.awake_speed < 0.05,
                format!(
                    "the creatures were asleep a second after night fell ({} of them, newborns aside, fastest {:.3} at {at:.2} s)",
                    test.asleep_counted, test.awake_speed
                ),
            ),
            None => ok(false, "the creatures were asleep a second after night fell (night never came)".into()),
        }
        // --- G2: the genome ------------------------------------------------
        //
        // **A run in which nobody was paired who could have answered has measured nothing here**
        // (2026-09-18). `Genome#mix` is called in a creature's `on(:mate)` and nowhere else, so a
        // run with no such pairing in it never asked the thing this check is about a question —
        // the same shape the fifth check took for a probe a rabbit walked into, and the same
        // reason.
        //
        // What makes it happen at all is the meadow corner ([`plant_the_meadow`]): two hungry
        // beetles, each straight behind a blade of its own, placed out of the rules' own `reach`
        // and `mate_reach` so that where they stop eating is inside where the rules pair. That
        // made `courtings == 0` stop happening (64 twenty-second runs, 0 of them) and left the
        // denominator being the thing that was wrong with this check.
        //
        // **`courtings` is the pairings that could have produced a child**, which is not every
        // pairing the rules made. `world.rb` pairs two of a sort, whatever sort it is, and leaves
        // what to do about it to the creature's own file — so a pair of rabbits is a pairing with
        // nobody at the other end of it, a pair told in the night is a handler that reads
        // `@asleep` and stops on its first line, and a pair one of whom starved in the frame
        // between the rule speaking and the handler waking is a handler that read nil and
        // stopped. None of the three is the road from `on(:mate)` to `garden.spawn` being broken,
        // and all three used to be counted: one run in 64 failed this check on two rabbit
        // pairings and no children at all
        // (`docs/worklog/2026-09-18-corner-and-selftest.md` §1.6). They are left out now —
        // [`listens_for`], [`asleep_now`] and [`close_courtings`] — and what is left in the
        // denominator is pairings where somebody was listening, awake, and both of them lived to
        // see it.
        match test.born_at {
            Some(at) => ok(
                test.born_ok,
                format!(
                    "a child was born whose genome is its parents' mixed and mutated (at {at:.2} s: {}) [{} pairings, {} children]",
                    test.born_says, test.courtings, test.births
                ),
            ),
            None if test.courtings == 0 => unmeasured(format!(
                "a child was born whose genome is its parents' mixed and mutated
         (not measured: the rules paired nobody in the whole run who could have answered{}, so nothing asked `Genome#mix` anything)",
                match test.courtings_lost {
                    0 => String::new(),
                    n => format!(
                        " — {n} pairing(s) measured nothing (asleep when they were told, or one of \
                         the two gone before the handler could ask)"
                    ),
                }
            )),
            // a pairing was made and no child came of it: that is the road from `on(:mate)` to
            // `garden.spawn`, and it is this check's to report
            None => ok(
                false,
                format!(
                    "a child was born whose genome is its parents' mixed and mutated (none was, from {} pairings)",
                    test.courtings
                ),
            ),
        }
        // --- G3: the save file ---------------------------------------------
        match &test.bad_spawn {
            Some(why) => ok(
                why.contains("sight"),
                format!("a spawn Hash with a gene missing names the gene ({why})"),
            ),
            None => ok(false, "a spawn Hash with a gene missing names the gene (nothing was refused)".into()),
        }
        // --- W1: the rules are Ruby's ---------------------------------------
        //
        // Two checks, and between them they say the thing the stage is about: that the rules in
        // `ruby/world.rb` really are what makes the garden move, and that they are still a script
        // — a file that can be taken away and given back while the world runs, which no `const`
        // in this source ever was.
        ok(
            test.grew_at.is_some_and(|t| t <= 2.0) && !vms.meter.ran_in.is_empty(),
            match test.grew_at {
                Some(t) => format!(
                    "the rules in world.rb are running the world (the grass grew at {t:.2} s, over {} frames they ran in)",
                    vms.meter.ran_in.len()
                ),
                None => format!(
                    "the rules in world.rb are running the world (nothing ever grew, over {} frames they ran in)",
                    vms.meter.ran_in.len()
                ),
            },
        );
        match test.frozen_at {
            Some(at) => ok(
                test.fell_while_frozen == 0 && test.fell_after_thaw,
                format!(
                    "the rules can be taken away and given back while the world runs (from {at:.2} s no meter fell for {FREEZE_WINDOW:.1} s; {} did, and afterwards hunger {})",
                    test.fell_while_frozen,
                    if test.fell_after_thaw { "came back" } else { "never came back" }
                ),
            ),
            None => ok(
                false,
                "the rules can be taken away and given back while the world runs (the world's script never restarted on them)".into(),
            ),
        }
        // --- W2: the world says something and the creatures hear it ----------
        match &test.season_heard {
            Some((who, season, at)) => ok(
                true,
                format!(
                    "what the world declares reaches a creature's memory ({who} had \"{season}\" in its @memory at {at:.2} s)"
                ),
            ),
            None => ok(
                false,
                format!(
                    "what the world declares reaches a creature's memory (the rules {} and nobody's @memory had a season in it)",
                    if test.season_told { "said it" } else { "never said it" }
                ),
            ),
        }
        // --- G5: the save's version -----------------------------------------
        match &test.version_refused {
            Some(why) => ok(
                why.contains("version 99") && why.ends_with(&format!("reads {SAVE_VERSION}")),
                format!("a save with the wrong version is refused ({why})"),
            ),
            None => ok(
                false,
                "a save with the wrong version is refused (it was loaded, or this run was given a --load of its own)"
                    .into(),
            ),
        }
    }
    // `--save PATH`: the last thing the run does, in `save_world`, which is ordered after this
    if file.on_exit {
        save.0 = true;
    }
    exit.write(AppExit::Success);
}

fn take_shot(mut commands: Commands, time: Res<Time>, mut shot: ResMut<Shot>, mut exit: MessageWriter<AppExit>) {
    if shot.taken {
        if time.elapsed_secs() > shot.after + 2.0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    if time.elapsed_secs() < shot.after {
        return;
    }
    shot.taken = true;
    let path = shot.path.clone();
    info!("screenshot -> {path}");
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
}

// ---------------------------------------------------------------------------------------------
// The camera's arithmetic (G6)
// ---------------------------------------------------------------------------------------------

/// The wheel is the one thing in the game whose bug was invisible on the machine it was written
/// on: a PC sends lines and a browser sends pixels, and the old code read the number without
/// asking which. There is no window in a test and no browser either, so what is checked here is
/// the two functions the messages are put through — that the same turn of the same finger is the
/// same number of notches in both units, and that a notch is a ratio.
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::mouse::MouseScrollUnit;

    // **The compiler's line numbers, put back into the author's file** used to be checked here,
    // with `beetle.rb`'s real message from 2026-09-18 (line 118 reported as 600). The function
    // moved to rubevy in S2 and so did the seven cases, with the creatures' names taken out:
    // rubevy `tests/source.rs`, which also compiles a real program with a real prelude and reads
    // the real compiler's message rather than a remembered one.

    #[test]
    fn a_notch_is_a_notch_in_either_unit() {
        // a PC mouse through winit: one notch, one line
        assert_eq!(notches_of(MouseScrollUnit::Line, 1.0), 1.0);
        // Chromium: one notch, a hundred pixels of would-be scrolling
        assert_eq!(notches_of(MouseScrollUnit::Pixel, 100.0), 1.0);
        assert_eq!(notches_of(MouseScrollUnit::Pixel, -300.0), -3.0);
    }

    #[test]
    fn ten_notches_are_ten_steps_and_not_the_end_of_the_range() {
        // what the author's browser did: ten notches from the default, in pixels, one at a time
        let eye = Eye::default();
        let mut d = eye.distance;
        let mut seen = vec![d];
        for _ in 0..10 {
            d = zoom_by(&eye, d, notches_of(MouseScrollUnit::Pixel, 100.0));
            seen.push(d);
        }
        // ten distinct distances, each a tenth nearer than the last, and nowhere near the stop
        for pair in seen.windows(2) {
            assert!((pair[0] / pair[1] - ZOOM_PER_NOTCH).abs() < 1e-4, "{pair:?}");
        }
        assert!(seen.last().unwrap() > &eye.zoom_min, "{seen:?}");
        // and the same ten in lines land in the same place: this is the whole of the browser fix
        let mut line = eye.distance;
        for _ in 0..10 {
            line = zoom_by(&eye, line, notches_of(MouseScrollUnit::Line, 1.0));
        }
        assert!((line - seen[10]).abs() < 1e-3, "{line} vs {}", seen[10]);
    }

    #[test]
    fn the_range_is_reached_but_not_passed() {
        let eye = Eye::default();
        assert_eq!(zoom_by(&eye, ZOOM_MIN, 5.0), ZOOM_MIN);
        assert_eq!(zoom_by(&eye, ZOOM_MAX, -5.0), ZOOM_MAX);
        // from one end to the other is about thirty notches, not one
        let notches = (ZOOM_MAX / ZOOM_MIN).ln() / ZOOM_PER_NOTCH.ln();
        assert!((29.0..32.0).contains(&notches), "{notches}");
    }

    #[test]
    fn panning_follows_the_screen_and_stops_at_the_wall() {
        // looking down the -Z axis: the screen's right is +X, away is -Z
        let (right, away) = ground_axes(0.0);
        assert!((right - Vec2::new(1.0, 0.0)).length() < 1e-5, "{right}");
        assert!((away - Vec2::new(0.0, -1.0)).length() < 1e-5, "{away}");
        // turned a quarter round, the screen's right is -Z
        let (right, _) = ground_axes(std::f32::consts::FRAC_PI_2);
        assert!((right - Vec2::new(0.0, -1.0)).length() < 1e-5, "{right}");
        // and the eye cannot leave the garden behind
        let (eye, place) = (Eye::default(), Place::default());
        let far = clamp_focus(&eye, &place, Vec2::new(1000.0, -1000.0));
        assert_eq!(far, Vec2::new(place.half_w() + PAN_LIMIT, -place.half_d() - PAN_LIMIT));
        // and a wider garden moves the stop with it, which is what `field_width` is for
        let wide = Place { width: 100.0, ..Place::default() };
        let far = clamp_focus(&eye, &wide, Vec2::new(1000.0, -1000.0));
        assert_eq!(far.x, 50.0 + PAN_LIMIT);
    }

    /// **What a line in `garden.settings.txt` reaches** (S5b-3). One key per resource, because
    /// what is being checked is the wiring — that a key of that name is read at all and lands in
    /// that field — rather than each of the seventy. The reading itself is one `take` for all of
    /// them.
    ///
    /// It is the shape S5b-1 gave the panels' settings and S5b-2 gave Battle's `Look`, with one
    /// addition: **a key nobody wrote leaves the default alone**, which is what makes a store
    /// written by an older build safe to read and is the whole of the browser compatibility
    /// question.
    #[test]
    fn a_line_in_the_store_reaches_the_garden() {
        fn read(path: &Path) -> Result<String, String> {
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
        fn write(path: &Path, text: &str) -> Result<(), String> {
            std::fs::write(path, text).map_err(|e| e.to_string())
        }
        let path = std::env::temp_dir().join("garden-settings-test.txt");
        let _ = std::fs::remove_file(&path);
        let mut settings = games_shell::Settings::load(&path, "a test", read, write);
        settings.set("field_width", "60");
        settings.set("start_plants", "9");
        settings.set("light_moon_lux", "400");
        settings.set("fog_depth", "120");
        settings.set("look_hunger_warn", "70");
        settings.set("eye_distance", "20");
        settings.set("script_budget", "500000");
        settings.set("world_script_frame_time_ms", "0");
        settings.set("start_beetle_speed", "5.5");
        settings.set("start_genome_spread", "0.0");

        let mut place = Place::default();
        place.read_from(&settings);
        assert_eq!(place.width, 60.0);
        assert_eq!(place.half_w(), 30.0, "half the field moves with the field");
        assert_eq!(place.depth, FIELD_D, "and a key nobody wrote is the default");

        let mut built = Furniture::default();
        built.read_from(&settings);
        assert_eq!(built.plants, 9);
        assert_eq!(built.trees, TREES);
        // **the species' own three numbers are the garden's furniture** (S5b-5): what is written
        // moves, what is not is the species' own, and a spread of nothing rolls the base itself
        assert_eq!(built.genome(Species::Beetle).speed, 5.5);
        assert_eq!(built.genome(Species::Beetle).sight, genome::BEETLE.sight);
        assert_eq!(built.genome(Species::Rabbit), genome::RABBIT);
        assert_eq!(built.genome_spread, 0.0);
        assert_eq!(
            Genome::roll(built.genome(Species::Beetle), built.genome_spread, |lo, hi| {
                assert_eq!((lo, hi), (1.0, 1.0));
                1.0
            }),
            built.genome(Species::Beetle)
        );

        let mut light = Light::default();
        light.read_from(&settings);
        assert_eq!(light.moon_lux, 400.0);
        assert_eq!(light.night_sky, NIGHT_SKY);
        // and the hour `--at midnight` means moves with the offset rather than with a constant
        assert!((midnight(&light) - MIDNIGHT_BY_DEFAULT).abs() < 1e-4);
        let dawnless = Light { dawn_offset: 0.0, ..Light::default() };
        assert!((midnight(&dawnless) - 0.75 * DAY_LENGTH).abs() < 1e-4);

        let mut scenery = Scenery::default();
        scenery.read_from(&settings);
        assert_eq!(scenery.fog_depth, 120.0);
        assert_eq!(scenery.sky_sides, SKY_SIDES);

        let mut picture = Picture::default();
        picture.read_from(&settings);
        assert_eq!(picture.hunger_warn, 70.0);
        assert_eq!(picture.window, WINDOW);
        assert_eq!(picture.beetle_model, BEETLE_MODEL, "a name, not a number, and still a default");

        let mut eye = Eye::default();
        eye.read_from(&settings);
        assert_eq!(eye.distance, 20.0);
        assert_eq!(eye.click_reach, window::CLICK_REACH);

        let mut budgets = Budgets::default();
        budgets.read_from(&settings);
        assert_eq!(budgets.creature, 500_000);
        assert_eq!(budgets.world, WORLD_BUDGET, "the world's is its own key");
        // 0 ms is rubevy's `None`: no wall-clock guard, the instruction count alone
        assert_eq!(budgets.world_frame_time_ms, 0.0);
        assert_eq!(Budgets::frame_time(budgets.world_frame_time_ms), None);
        assert_eq!(
            Budgets::frame_time(budgets.creature_frame_time_ms),
            Some(std::time::Duration::from_millis(8))
        );

        let _ = std::fs::remove_file(&path);
    }

    /// **Where a save goes** (S5b-3). The store moves it; the flag still beats the store.
    #[test]
    fn the_store_names_the_save_and_the_flag_beats_it() {
        fn read(path: &Path) -> Result<String, String> {
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
        fn write(path: &Path, text: &str) -> Result<(), String> {
            std::fs::write(path, text).map_err(|e| e.to_string())
        }
        let path = std::env::temp_dir().join("garden-save-name-test.txt");
        let _ = std::fs::remove_file(&path);
        let mut settings = games_shell::Settings::load(&path, "a test", read, write);
        assert_eq!(save_path(&settings, None), platform::SAVE_FILE);
        settings.set("save_file", "mine.json");
        assert_eq!(save_path(&settings, None), "mine.json");
        assert_eq!(save_path(&settings, Some("flag.json")), "flag.json");
        let _ = std::fs::remove_file(&path);
    }

    /// Midnight worked out from the defaults, for the test above to compare against — the number
    /// the `const MIDNIGHT` used to be.
    const MIDNIGHT_BY_DEFAULT: f32 = (0.75 - DAWN_OFFSET) * DAY_LENGTH;

    /// **The checks' patience follows the budget** (S5b-3). S7 derived it as
    /// `2 + ceil(budget / 45,600)` and wrote 200,000 into the sum as a number; now that the
    /// budget is `script_budget` in the store, the sum has to be done against what the run was
    /// actually given or the check judges a slow VM as a broken one.
    ///
    /// S5b-5 is where that stopped being hypothetical: [`CREATURE_BUDGET`] went from 200,000 to
    /// 41,000 and this went from 7 to 3 with no edit of its own.
    #[test]
    fn a_bigger_budget_buys_the_checks_more_frames() {
        let budgets = Budgets::default();
        // `2 + ceil(41,000 / 45,600)`, which is what the default budget now buys
        assert_eq!(window::scheduler_frames(&budgets), 3);
        // five times the budget is five times the frames a restart may take to come round
        let rich = Budgets { creature: 1_000_000, ..Budgets::default() };
        assert_eq!(window::scheduler_frames(&rich), 2 + 22);
        // and a VM given almost nothing still gets the two frames that are structural
        let poor = Budgets { creature: 1, ..Budgets::default() };
        assert_eq!(window::scheduler_frames(&poor), 3);
    }

    #[test]
    fn midnight_is_the_darkest_moment() {
        let light = Light::default();
        let phase = (midnight(&light) / DAY_LENGTH + light.dawn_offset).fract();
        assert!((phase - 0.75).abs() < 1e-5, "{phase}");
        let up = (phase * std::f32::consts::TAU).sin();
        assert!(up < -0.999, "{up}");
    }

    /// `ruby/world.rb` as it ships, for the test below. It is read at compile time rather than
    /// off the disk so that the test says the same thing wherever it is run from — and in a
    /// browser build there is no disk to read it off at all.
    const WORLD_RB: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/ruby/world.rb"));

    /// `def <name> = <number>` out of `world.rb`, which is the shape every number in that file
    /// is written in.
    fn what_world_rb_says(name: &str) -> f32 {
        for line in WORLD_RB.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("def ") else { continue };
            let Some((said, value)) = rest.split_once('=') else { continue };
            if said.trim() != name {
                continue;
            }
            let value = value.trim().split_whitespace().next().unwrap_or_default();
            return value.parse().unwrap_or_else(|_| panic!("{name}: {value:?} is not a number"));
        }
        panic!("ruby/world.rb has no `def {name} = …`");
    }

    /// **The stand-ins are not a second opinion about the rules** (S5b-4).
    ///
    /// Eight numbers of the garden's play live in `ruby/world.rb` and are handed over once by
    /// `garden.rules`; this source keeps one of each as the value a garden runs on *before* the
    /// rules have spoken, and for ever in a garden whose `world.rb` will not compile — which is
    /// a garden this game deliberately goes on running ([`WorldTrouble`]).
    ///
    /// Two numbers for one fact is what this stage exists to remove, and the reason this pair
    /// stays is written above each constant. What can be had instead of removing it is this:
    /// the two are pinned together, so that a `world.rb` edited in the repository and a stand-in
    /// left behind is a failing test rather than a garden that plays differently for its first
    /// frame than for its second. (It says nothing about a `world.rb` a *player* has edited —
    /// that one is supposed to differ, and the whole point of the handover is that it may.)
    #[test]
    fn the_stand_ins_are_what_world_rb_says() {
        let reaches = Reaches::default();
        assert_eq!(reaches.eat, what_world_rb_says("reach"));
        assert_eq!(reaches.touch, what_world_rb_says("touch_reach"));
        assert_eq!(reaches.mate, what_world_rb_says("mate_reach"));
        assert_eq!(Sprouts::default().gap, what_world_rb_says("sprout_gap"));
        let bodies = Bodies::default();
        assert_eq!(bodies.beetle, what_world_rb_says("beetle_radius"));
        assert_eq!(bodies.rabbit, what_world_rb_says("rabbit_radius"));
        assert_eq!(bodies.tree, what_world_rb_says("tree_radius"));
        assert_eq!(bodies.rock, what_world_rb_says("rock_radius"));
        let births = Births::default();
        assert_eq!(births.hunger, what_world_rb_says("child_hunger"));
        assert_eq!(births.cap, what_world_rb_says("pop_max") as usize);
    }

    /// **The neighbour grid is never narrower than a body** (S5b-4). With the rules as they ship
    /// it is [`CELL`] exactly, which is what every measurement of `separate` was taken on; a
    /// `world.rb` that makes a tree three units wide gets a grid that can still find it.
    #[test]
    fn the_grid_is_wide_enough_for_the_widest_body() {
        assert_eq!(Bodies::default().cell(), CELL);
        let wide = Bodies { tree: 3.0, ..Bodies::default() };
        assert_eq!(wide.cell(), 6.0);
        // and a garden of very small creatures does not get a grid so fine that it costs more
        // cells than it saves comparisons
        let small = Bodies { beetle: 0.05, rabbit: 0.05, tree: 0.05, rock: 0.05 };
        assert_eq!(small.cell(), CELL);
    }
}

