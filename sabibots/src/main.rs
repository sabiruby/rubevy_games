//! SabiRuby Battle — the robots' brains are Ruby, running as tasks in one VM.
//!
//! Each robot is an entity with a `Script`: `ruby/prelude.rb` (the DSL) followed by
//! `ruby/robots/<name>.rb` (the robot itself). The script asks the game for what it needs
//! (`radar`, `incoming`, `act`, …) and is parked until the game answers, so a robot that is
//! thinking costs nothing and a robot that never yields is preempted at its timeslice.
//!
//! What the game gives a robot is deliberately raw: a tank's controls (throttle, turn, where the
//! turret should point, how hard to fire) and noisy readings of what is around it. Aiming ahead
//! of a moving target, dodging, and saving energy are the robot's own Ruby — the helpers in the
//! prelude are one way to write them, not the only one.
//!
//!     cargo run -p sabibots
//!
//! Save a robot file while it runs and that robot starts again with the new brain.

/// G6: the words of the `H` panel, and the only file to edit to change them.
mod guide_text;
mod platform;

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use rubevy::{
    in_the_authors_lines, replace_script, Answer, MrbAsset, Program, RubevyPlugin, RubevySet,
    Script, ScriptEnded, ScriptTask, ScriptWorld,
};
use games_shell::{ArenaPlugin, ArenaSize, GuidePlugin, Hud, ScriptPanel};
use rubevy_egui::{Editor, EditorAction, EditorPlugin, VmInspector, VmInspectorPlugin, Watch};
use sabiruby::Value;

const ROBOT_RADIUS: f32 = 1.6;
/// A robot is a tank: it moves along its heading, turns at a limited rate, and its turret turns
/// on its own, also at a limited rate.
const MAX_SPEED: f32 = 12.0;
const REVERSE_SPEED: f32 = 7.0;
const TURN_RATE: f32 = 2.6;
const TURRET_RATE: f32 = 4.0;
/// Energy: driving and firing cost it, time gives it back. A robot that fires everything it has
/// cannot also run away.
const ENERGY_MAX: f32 = 100.0;
const ENERGY_REGEN: f32 = 12.0;
const DRIVE_COST: f32 = 9.0;
const FIRE_COST: f32 = 16.0;
/// A shot's power (0.2 to 1): more damage, a slower shot, a longer reload, more energy.
const BULLET_SPEED_FAST: f32 = 55.0;
const BULLET_SPEED_SLOW: f32 = 30.0;
const BULLET_DAMAGE_MIN: f32 = 4.0;
const BULLET_DAMAGE_MAX: f32 = 16.0;
const COOLDOWN_MIN: f32 = 0.3;
const COOLDOWN_MAX: f32 = 0.8;
/// Even with no noise in the match, a gun is not a laser.
const BASE_SPREAD: f32 = 0.02;
/// "leave this as it is", for any part of `act` the brain does not set
const UNSET: f64 = -999.0;

/// **Half the width of the arena**, in world units: the middle to a wall.
///
/// It was `games_shell::ArenaSize`'s default until S5b-1, which is a game's number living in the
/// shared shell — how much world a window holds is a thing only the game knows, and the third
/// game has no arena at all. **Source unknown**: it arrived with the first match and nothing
/// says why 32 (`docs/numbers.md` §1.1). A match shrinks the field from here
/// (`arena.shrink`), and `restart_match` puts it back.
const ARENA_HALF_WIDTH: f32 = 32.0;

/// The teams a match can put on the field, in the order Ruby names them.
const TEAMS: [(&str, &str, &str); 4] = [
    ("red", "sprites/tankBody_red_outline.png", "sprites/bulletRed1_outline.png"),
    ("blue", "sprites/tankBody_blue_outline.png", "sprites/bulletBlue1_outline.png"),
    ("green", "sprites/tankBody_green_outline.png", "sprites/bulletGreen1_outline.png"),
    ("yellow", "sprites/tankBody_sand_outline.png", "sprites/bulletSand1_outline.png"),
];

/// What happened that a match may want to know about. Drained by `Rubevy.ask("events")`.
#[derive(Resource, Default)]
struct Events(Vec<[f64; 3]>);

/// A robot in the arena. The Ruby side never sees this; it asks for what it needs.
#[derive(Component, Debug)]
struct Robot {
    /// 1, 2, 3 … in the order the match put it on the field: the key that picks it, its HUD row,
    /// and the number over its head
    number: usize,
    /// which team it belongs to, as an index into `TEAMS`
    team: usize,
    name: String,
    file: PathBuf,
    hp: f32,
    cooldown: f32,
    velocity: Vec2,
    /// what its brain last set: -1 (full reverse) to 1 (full ahead), and -1 to 1 of TURN_RATE
    throttle: f32,
    turn: f32,
    /// the turret's world angle, and where it has been told to point
    turret: f32,
    turret_target: f32,
    energy: f32,
    /// what its brain had spent as of the last frame, so the HUD can show this frame's share
    last_instructions: u64,
    /// lines the prelude adds in front of the robot's own file
    prelude_lines: u32,
    /// the line of its own file the brain is inside, however deep in the DSL it stands
    own_line: Option<u32>,
    /// where the hull points; it only changes by turning
    heading: f32,
    /// the brain it runs when that is not its file: text applied from the editor and not saved.
    /// A robot with one is left alone when the file changes on disk.
    brain: Option<String>,
    /// the source it is running (the file's text or the applied brain), for showing a line of it
    source: String,
    /// where the brain has been spending its time, per line of its own file, decaying over a
    /// second or so — the readable version of a current line that changes many times a second
    heat: Vec<f32>,
    /// instructions per frame, smoothed the same way
    cpu: f32,
    /// how many handlers the brain registered (`Rubevy.ask("handler", :ready, n)`)
    handlers: u32,
    /// how many handler blocks have run, and how many are inside their block right now — the
    /// mark next to the robot's name on the scoreboard
    handler_runs: u32,
    handler_running: u32,
    /// how many of its handler *tasks* have ended (`Rubevy.ask("handler", :off)`). A handler task
    /// is parked on a subscription for ever unless something ends it; since rubevy `9104f7c`
    /// that something is the queue closing when the robot's `ScriptTask` goes, and this is what
    /// shows it happened rather than the task being left `WAITING`.
    handler_off: u32,
    /// when it went down, so a check can wait a moment for its handler tasks to notice
    downed_at: Option<f32>,
}

/// A robot that has lost: its hull is swapped for a grey one and its brain is stopped, once.
#[derive(Component)]
struct Downed;

fn gray_out_downed(
    mut commands: Commands,
    server: Res<AssetServer>,
    time: Res<Time>,
    mut robots: Query<(Entity, &mut Robot, &mut Sprite), Without<Downed>>,
) {
    for (entity, mut robot, mut sprite) in &mut robots {
        if robot.hp > 0.0 {
            continue;
        }
        // and its brain stops: taking the task away terminates it (Script too, or the plugin
        // would start it again). Its handler tasks are not the plugin's to terminate, but taking
        // the `ScriptTask` away closes the subscriptions they are parked on, and each of them
        // ends itself on `Rubevy::Unsubscribed` (`prelude.rb`, `start_handlers`).
        robot.downed_at = Some(time.elapsed_secs());
        commands.entity(entity).remove::<(ScriptTask, rubevy::ScriptDone, Script)>();
        robot.cpu = 0.0;
        robot.throttle = 0.0;
        robot.turn = 0.0;
        // Kenney's dark hull, dimmed a little more: grey whatever team it was on
        sprite.image = server.load("sprites/tankBody_dark_outline.png");
        sprite.color = Color::srgb(0.7, 0.7, 0.7);
        commands.entity(entity).insert(Downed);
    }
}

/// The barrel on top of a robot: its own entity, because it turns on its own.
#[derive(Component)]
struct Turret {
    robot: Entity,
}

fn spawn_turrets(mut commands: Commands, server: Res<AssetServer>, robots: Query<(Entity, &Robot), Added<Robot>>) {
    for (entity, robot) in &robots {
        // Kenney draws red and blue barrels; the other teams get the blue one, tinted
        let (image, color) = match robot.team {
            0 => ("sprites/tankRed_barrel1_outline.png", Color::WHITE),
            1 => ("sprites/tankBlue_barrel1_outline.png", Color::WHITE),
            2 => ("sprites/tankBlue_barrel1_outline.png", Color::srgb(0.6, 1.0, 0.6)),
            _ => ("sprites/tankBlue_barrel1_outline.png", Color::srgb(1.0, 0.95, 0.6)),
        };
        commands.spawn((
            Turret { robot: entity },
            Sprite { image: server.load(image), color, custom_size: Some(Vec2::new(1.1, 2.6)), ..default() },
            Transform::from_xyz(0.0, 0.0, 1.5),
        ));
    }
}

fn follow_turrets(
    mut commands: Commands,
    robots: Query<(&Robot, &Transform), Without<Turret>>,
    mut turrets: Query<(Entity, &Turret, &mut Sprite, &mut Transform)>,
) {
    for (entity, turret, mut sprite, mut transform) in &mut turrets {
        let Ok((robot, at)) = robots.get(turret.robot) else {
            commands.entity(entity).despawn();
            continue;
        };
        // the barrel's back end sits over the middle of the hull
        let dir = Vec2::from_angle(robot.turret);
        transform.translation.x = at.translation.x + dir.x * 1.0;
        transform.translation.y = at.translation.y + dir.y * 1.0;
        transform.rotation = Quat::from_rotation_z(robot.turret - std::f32::consts::FRAC_PI_2);
        if robot.hp <= 0.0 {
            sprite.color = Color::srgb(0.45, 0.45, 0.45);
        }
    }
}

/// A puff where a shot landed or a robot went down: it grows, fades and goes.
#[derive(Component)]
struct Blast {
    life: f32,
    span: f32,
    size: f32,
}

#[derive(Component)]
struct Bullet {
    velocity: Vec2,
    owner: Entity,
    team: usize,
    damage: f32,
    life: f32,
}

/// The match's randomness: how noisy the sensors and the guns are, and the dice that make it.
/// Seeded, so the same seed rolls the same numbers (the frame timing still varies, so a replay
/// is alike rather than identical).
#[derive(Resource)]
struct Rules {
    noise: f32,
    dice: u64,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { noise: 0.0, dice: 0x9E37_79B9_7F4A_7C15 }
    }
}

impl Rules {
    /// 0..1, SplitMix64
    fn roll(&mut self) -> f32 {
        self.dice = self.dice.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.dice;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }
    /// about -1..1, bunched in the middle
    fn wobble(&mut self) -> f32 {
        (self.roll() + self.roll() + self.roll()) / 1.5 - 1.0
    }
}

/// Where the Ruby lives. `ruby/prelude.rb` is put in front of every robot's file.
#[derive(Resource)]
struct RubyDir(PathBuf);

/// The bullet sprite each robot fires, in the order the robots were spawned.
#[derive(Resource, Default)]
struct Shots(Vec<(Entity, Handle<Image>)>);

impl Shots {
    fn image_for(&self, robot: Entity) -> Handle<Image> {
        self.0
            .iter()
            .find(|(e, _)| *e == robot)
            .map(|(_, h)| h.clone())
            .unwrap_or_default()
    }
}

/// How long `--headless` runs when it is given no number, and what `--shot` writes to and waits
/// for when it is given neither. **They are the values that were written into `main` here before
/// S1**, moved out only because the parsing they were in is `games_shell::Args`' now and the
/// garden's `--shot` waits a different six seconds; no run's behaviour turns on them, since every
/// line in `docs/sabiruby-battle.md` passes its own number.
const HEADLESS_SECONDS: f32 = 10.0;
const SHOT_FILE: &str = "shot.png";
const SHOT_SECONDS: f32 = 3.0;

fn main() {
    let ruby = platform::ruby_dir();
    // The flags both games take, read by `games_shell::Args` since S1.
    let args = games_shell::Args::from_env();
    // `--headless N`: no window, N seconds, the match reported on stdout. It is how the game is
    // tested where there is no GPU, and it runs exactly the same systems as the windowed one.
    let headless = args.headless(HEADLESS_SECONDS);
    // `--shot FILE [SECONDS]`: a window, a picture of it, and out. For checking the HUD where
    // the window itself cannot be looked at.
    let shot = args.shot(SHOT_FILE, SHOT_SECONDS);
    // `--vm` (G9): open the VM panel. It is closed unless somebody asks, and on a command line
    // this is the asking — `--shot docs/vm-inspector.png 14 --vm` is how the picture in
    // `docs/sabiruby-battle.md` is taken. A player asks with `F2`.
    let wants_vm = args.has("--vm");
    // `--lang en|ja` (G6b): which language the guide opens in, for its two pictures. Not
    // remembered; the player's own click is.
    let lang_asked = args.value("--lang");

    let mut app = App::new();
    match headless {
        Some(seconds) => {
            app.add_plugins((
                MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    std::time::Duration::from_secs_f32(1.0 / 60.0),
                )),
                bevy::log::LogPlugin { filter: "info,bevy_asset=off".into(), ..default() },
                bevy::asset::AssetPlugin {
                    file_path: platform::assets_dir(),
                    ..default()
                },
                RubevyPlugin::default(),
            ))
            // no renderer here, but the same systems run and they load sprites
            .init_asset::<Image>()
            .insert_resource(ArenaSize(ARENA_HALF_WIDTH))
            .insert_resource(Headless { until: seconds })
            .init_resource::<Hud>()
            .add_systems(Update, stop_when_over);
        }
        None => {
            // G6b: the one thing this game remembers between runs — which language the guide is
            // read in. Read before the first frame, because the panel opens by itself and must
            // not show one language and then jump to the other. `platform.rs` decides whether
            // that is a file beside the game or a key in the browser's local storage, and the
            // ten lines that read it are `games_shell::remembered`'s since S1.
            let (settings, lang) = games_shell::remembered(
                platform::SETTINGS_FILE,
                "SabiRuby Battle: what the panel remembers. Delete a line for the default.",
                platform::read,
                platform::write,
                lang_asked.as_deref(),
            );
            app.add_plugins((
                DefaultPlugins
                    .set(AssetPlugin {
                        file_path: platform::assets_dir(),
                        // the sprites have no .meta files; in the browser each would be a 404
                        meta_check: bevy::asset::AssetMetaCheck::Never,
                        ..default()
                    })
                    .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "SabiRuby Battle".into(),
                        resolution: (1600u32, 900u32).into(),
                        // in the browser: the page's canvas, as large as its box
                        canvas: Some("#sabibots".into()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                }),
                ArenaPlugin::showing(ARENA_HALF_WIDTH),
                // H2: the editor, with the lexer behind its colours. Which lexer is
                // `platform.rs`'s to know — the compiler linked in on a PC,
                // `window.sabibotsHighlight` in a page.
                EditorPlugin::with_highlighter(platform::highlight),
                VmInspectorPlugin,
                // G6: the `H` panel, and with it the Japanese font (`games_shell::guide`)
                GuidePlugin::default(),
                RubevyPlugin::default(),
                // S5b-1: whatever the player left in `sabibots.settings.txt` about the panels —
                // how big the editor is, how big the VM panel is, the HUD's text. Each panel
                // knows its own keys; this is the wiring.
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
            .insert_resource(settings)
            .init_resource::<Watched>()
            .init_resource::<Hud>()
            .init_resource::<Paused>()
            // **Closed** (G9): `F2` opens it, and a screenshot gets it only where `--vm` asks.
            // The panel is a debugger and a match is not read through one by default.
            .insert_resource(VmInspector::following().opened(wants_vm))
            .add_systems(
                Update,
                (restart_key, choose_watched, show_code, inspect_keys, show_vm, do_editor_actions, spawn_nameplates, follow_nameplates, spawn_life_bars, follow_life_bars)
                    .chain(),
            )
            .add_systems(bevy_egui::EguiPrimaryContextPass, draw_scoreboard);
        }
    }
    app.insert_resource(RubyDir(ruby.clone()))
        .init_resource::<Restart>()
        .init_resource::<KeptBrains>()
        .init_resource::<Shots>()
        .init_resource::<Events>()
        .init_resource::<Rules>()
        .add_systems(Startup, (spawn_match, spawn_arena))
        // The whole chain sits in `RubevySet::Answer`, which rubevy puts after the set that
        // runs the scripts and collects their questions. That is what makes `answer_requests`
        // see a question on the frame it was asked, so the script wakes on the next frame
        // rather than the one after: a round trip costs one frame, not two
        // (docs/worklog/2026-09-17-battle-followups.md). The chain moves as a whole because
        // the order inside it is the game's own — `answer_requests` sets the controls that
        // `move_robots` then applies, in that order, in the same frame.
        .init_resource::<WorldClock>()
        // G9: `P` stops the world, and the world is the six marked below — the clock, the
        // answers the game gives, the tanks, the tanks pushing each other apart, the bullets and
        // the blasts. The order of the whole chain is unchanged: the condition is hung on the
        // systems it is about, one at a time, rather than by moving them into a group of their
        // own, because the order inside this chain is the game's own (`answer_requests` sets the
        // controls that `move_robots` then applies, in that order, in the same frame).
        .add_systems(
            Update,
            (
                restart_match,
                advance_clock.run_if(world_moves),
                answer_requests.run_if(world_moves),
                move_robots.run_if(world_moves),
                separate_robots.run_if(world_moves),
                spawn_turrets,
                follow_turrets,
                move_bullets.run_if(world_moves),
                gray_out_downed,
                fade_blasts.run_if(world_moves),
                rebuild_walls,
                reload_changed,
                report_ended,
                update_hud,
            )
                .chain()
                .in_set(RubevySet::Answer),
        );
    // `SABIBOTS_SELFTEST=1` on a PC, `?selftest` in the page's address (2026-09-18): a browser has
    // no environment, and `platform.rs` is where the difference between the two builds lives
    let checks_asked = platform::selftest_asked();
    if checks_asked {
        // the handler check wants a fight, not a mouse, so it runs in both modes
        // `EditChecks` is read by the handler check and written by the editor's checks below.
        // It is initialised for both modes, because headless has no editor's checks and an empty
        // one excludes nothing.
        app.init_resource::<HandlerTest>()
            .init_resource::<EditChecks>()
            .add_systems(Update, handler_selftest);
    }
    if checks_asked && headless.is_none() {
        // before `inspect_keys`, so a key it presses is still `just_pressed` when that reads it
        app.insert_resource(SelfTest { at: 2.0, ..default() })
            .add_systems(Update, selftest.before(inspect_keys));
    }
    if let Some((path, after)) = shot {
        app.insert_resource(Shot { path, after, taken: false })
            .add_systems(Update, take_shot);
    }
    if let Some(watch) = Watch::new(&ruby) {
        app.insert_resource(watch);
    } else {
        warn!("could not watch {ruby:?}: saving a robot will not reload it");
    }
    app.run();
}

/// Which robot's code the panel shows, and the source it was read from.
#[derive(Resource, Default)]
struct Watched {
    entity: Option<Entity>,
}

/// `1`, `2`, … pick a robot; `Tab` moves on; `F1` hides the editor — unless the editor has the keyboard.
fn choose_watched(
    keys: Res<ButtonInput<KeyCode>>,
    typing: Option<Res<bevy_egui::input::EguiWantsInput>>,
    robots: Query<(Entity, &Robot)>,
    mut watched: ResMut<Watched>,
    mut editor: ResMut<Editor>,
) {
    let mut numbered: Vec<(Entity, usize)> = robots.iter().map(|(e, r)| (e, r.number)).collect();
    numbered.sort_by_key(|(_, n)| *n);
    let all: Vec<Entity> = numbered.iter().map(|(e, _)| *e).collect();
    if all.is_empty() {
        return;
    }
    if watched.entity.is_none() {
        watched.entity = Some(all[0]);
        editor.open = true;
    }
    // a robot button clicked in the editor
    if let Some(bits) = editor.picked.take() {
        if let Some(e) = all.iter().find(|e| e.to_bits() == bits) {
            watched.entity = Some(*e);
        }
    }
    // keys typed into the editor are the editor's: a `2` in the code must not switch robots
    if typing.is_some_and(|t| t.wants_keyboard_input()) {
        return;
    }
    let digits = [
        KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4,
        KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8,
    ];
    for (i, key) in digits.into_iter().enumerate() {
        if keys.just_pressed(key) {
            if let Some((e, _)) = numbered.iter().find(|(_, n)| *n == i + 1) {
                watched.entity = Some(*e);
                editor.open = true;
            }
        }
    }
    if keys.just_pressed(KeyCode::Tab) {
        let at = all.iter().position(|e| Some(*e) == watched.entity).unwrap_or(0);
        watched.entity = Some(all[(at + 1) % all.len()]);
        editor.open = true;
    }
    if keys.just_pressed(KeyCode::F1) {
        editor.open = !editor.open;
    }
}

/// The text over a robot: its number and brain, in its team's colour, marked when the editor
/// is showing it. A dark copy behind it keeps it readable on the sand.
#[derive(Component)]
struct Nameplate {
    robot: Entity,
    shadow: bool,
}

const TEAM_COLORS: [(f32, f32, f32); 4] = [(1.0, 0.42, 0.36), (0.45, 0.72, 1.0), (0.45, 0.9, 0.45), (1.0, 0.85, 0.35)];

fn spawn_nameplates(mut commands: Commands, robots: Query<Entity, Added<Robot>>) {
    for entity in &robots {
        for shadow in [true, false] {
            commands.spawn((
                Nameplate { robot: entity, shadow },
                Text2d::new(""),
                TextFont { font_size: bevy::text::FontSize::Px(44.0), ..default() },
                TextColor(if shadow { Color::srgba(0.0, 0.0, 0.0, 0.75) } else { Color::WHITE }),
                // world units are large next to pixels: a 44px font scaled to about two units tall
                Transform::from_xyz(0.0, 0.0, if shadow { 6.0 } else { 6.1 }).with_scale(Vec3::splat(0.045)),
            ));
        }
    }
}

fn follow_nameplates(
    mut commands: Commands,
    watched: Res<Watched>,
    robots: Query<(&Robot, &Transform), Without<Nameplate>>,
    mut plates: Query<(Entity, &Nameplate, &mut Text2d, &mut TextColor, &mut Transform)>,
) {
    for (plate, owner, mut text, mut color, mut transform) in &mut plates {
        let Ok((robot, at)) = robots.get(owner.robot) else {
            commands.entity(plate).despawn();
            continue;
        };
        let star = if robot.brain.is_some() { "*" } else { "" };
        let brain = format!(
            "{}{star}",
            robot.file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
        );
        let selected = watched.entity == Some(owner.robot);
        let down = robot.hp <= 0.0;
        **text = match (selected, down) {
            (_, true) => format!("{} {brain}  down", robot.number),
            (true, false) => format!("> {} {brain} <", robot.number),
            (false, false) => format!("{} {brain}", robot.number),
        };
        if !owner.shadow {
            let (r, g, b) = TEAM_COLORS[robot.team.min(TEAM_COLORS.len() - 1)];
            *color = TextColor(if selected {
                Color::srgb(1.0, 1.0, 0.55)
            } else if down {
                Color::srgb(0.62, 0.62, 0.62)
            } else {
                Color::srgb(r, g, b)
            });
        }
        // the shadow sits a little down and right of the text it darkens
        let nudge = if owner.shadow { 0.12 } else { 0.0 };
        transform.translation.x = at.translation.x + nudge;
        transform.translation.y = at.translation.y + ROBOT_RADIUS * 2.25 - nudge;
    }
}

/// The panel at the top left: one row per robot, in words and bars rather than file:line.
fn draw_scoreboard(
    mut contexts: bevy_egui::EguiContexts,
    hud: Res<Hud>,
    watched: Res<Watched>,
    robots: Query<(Entity, &Robot)>,
    mut editor: ResMut<Editor>,
    mut restart: ResMut<Restart>,
) {
    use bevy_egui::egui;
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let over = hud.line.starts_with("winner") || hud.line == "a draw";
    let mut rows: Vec<(Entity, &Robot)> = robots.iter().collect();
    rows.sort_by_key(|(_, r)| r.number);

    // a window with a title bar, to be dragged out of the way; it starts at the top left
    egui::Window::new("SabiRuby Battle")
        .collapsible(true)
        .resizable(false)
        .default_pos([8.0, 8.0])
        .show(ctx, |ui| {
            // a scoreboard, not a document: nothing in it is text to select
            ui.style_mut().interaction.selectable_labels = false;
            ui.horizontal(|ui| {
                if !hud.line.is_empty() {
                    ui.label(egui::RichText::new(&hud.line).strong().size(16.0));
                }
                // once it is over the button says so; before that it is there, quieter
                let label = if over { egui::RichText::new("Play again (R)").strong().size(16.0) } else { egui::RichText::new("Restart (R)") };
                if ui.button(label).on_hover_text("start the match over; behaviours applied in the editor are kept").clicked() {
                    restart.0 = true;
                }
            });
            ui.separator();
            egui::Grid::new("robots").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label(egui::RichText::new("robot").weak());
                ui.label(egui::RichText::new("health").weak());
                ui.label(egui::RichText::new("energy").weak())
                    .on_hover_text("driving and firing spend it, time brings it back; a harder shot costs more");
                ui.label(egui::RichText::new("thinking").weak())
                    .on_hover_text("instructions its Ruby runs per frame, averaged over about a second");
                ui.end_row();

                for (entity, robot) in rows {
                    let (r, g, b) = TEAM_COLORS[robot.team.min(TEAM_COLORS.len() - 1)];
                    let down = robot.hp <= 0.0;
                    let team = if down {
                        egui::Color32::from_gray(150)
                    } else {
                        egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
                    };
                    let brain = robot.file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let star = if robot.brain.is_some() { "*" } else { "" };
                    let marker = if watched.entity == Some(entity) { "> " } else { "  " };
                    // a handler running now is `!`, and the count stays afterwards so a
                    // screenshot shows it too (`--shot`)
                    let handler = match (robot.handler_runs, robot.handler_running > 0) {
                        (0, false) => String::new(),
                        (n, true) => format!(" !{n}"),
                        (n, false) => format!(" x{n}"),
                    };
                    let name = egui::RichText::new(format!("{marker}{} {brain}{star}{handler}", robot.number)).color(team).strong();
                    // the name picks the robot, as its button in the editor does
                    let hint = if robot.handlers > 0 {
                        format!("{} handler(s); {} have run, {} running now", robot.handlers, robot.handler_runs, robot.handler_running)
                    } else {
                        "this behaviour has no handler".to_string()
                    };
                    if ui.add(egui::Label::new(name).sense(egui::Sense::click())).on_hover_text(hint).clicked() {
                        editor.picked = Some(entity.to_bits());
                    }

                    let life = (robot.hp / 100.0).clamp(0.0, 1.0);
                    let fill = if life > 0.5 {
                        egui::Color32::from_rgb(90, 190, 90)
                    } else if life > 0.25 {
                        egui::Color32::from_rgb(220, 180, 60)
                    } else {
                        egui::Color32::from_rgb(210, 70, 60)
                    };
                    let text = if down { "down".to_string() } else { format!("{}", robot.hp as i32) };
                    ui.add(egui::ProgressBar::new(life).fill(fill).desired_width(120.0).text(text));

                    let energy = (robot.energy / ENERGY_MAX).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(if down { 0.0 } else { energy })
                            .fill(egui::Color32::from_rgb(200, 170, 60))
                            .desired_width(70.0)
                            .text(if down { String::new() } else { format!("{}", robot.energy as i32) }),
                    );

                    // a timeslice's worth of instructions fills the bar: the point where a brain
                    // starts taking turns away from the others
                    let cpu = (robot.cpu / 3000.0).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(cpu)
                            .fill(egui::Color32::from_rgb(90, 140, 210))
                            .desired_width(80.0)
                            .text(format!("{:.0}", robot.cpu)),
                    );
                    ui.end_row();
                }
            });
            ui.separator();
            // G6: the one line that says the rest is explained inside the game
            ui.label(
                egui::RichText::new(games_shell::Guide::HINT)
                    .color(egui::Color32::from_rgb(255, 226, 150))
                    .strong(),
            );
        });
}

/// A bar over each robot's name: how much life it has left.
#[derive(Component)]
struct LifeBar {
    robot: Entity,
    fill: bool,
}

const BAR_WIDTH: f32 = 3.6;

fn spawn_life_bars(mut commands: Commands, robots: Query<Entity, Added<Robot>>) {
    for entity in &robots {
        for fill in [false, true] {
            commands.spawn((
                LifeBar { robot: entity, fill },
                Sprite {
                    color: if fill { Color::srgb(0.35, 0.8, 0.35) } else { Color::srgba(0.0, 0.0, 0.0, 0.6) },
                    custom_size: Some(Vec2::new(BAR_WIDTH, 0.45)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, if fill { 5.1 } else { 5.0 }),
            ));
        }
    }
}

fn follow_life_bars(
    mut commands: Commands,
    robots: Query<(&Robot, &Transform), Without<LifeBar>>,
    mut bars: Query<(Entity, &LifeBar, &mut Sprite, &mut Transform, &mut Visibility)>,
) {
    for (bar, owner, mut sprite, mut transform, mut visibility) in &mut bars {
        let Ok((robot, at)) = robots.get(owner.robot) else {
            commands.entity(bar).despawn();
            continue;
        };
        *visibility = if robot.hp <= 0.0 { Visibility::Hidden } else { Visibility::Inherited };
        let life = (robot.hp / 100.0).clamp(0.0, 1.0);
        let y = at.translation.y + ROBOT_RADIUS * 1.35;
        if owner.fill {
            let w = BAR_WIDTH * life;
            sprite.custom_size = Some(Vec2::new(w.max(0.001), 0.45));
            sprite.color = if life > 0.5 {
                Color::srgb(0.35, 0.8, 0.35)
            } else if life > 0.25 {
                Color::srgb(0.9, 0.75, 0.25)
            } else {
                Color::srgb(0.85, 0.3, 0.25)
            };
            // anchored at the bar's left end, so it empties towards the left
            transform.translation.x = at.translation.x - BAR_WIDTH / 2.0 + w / 2.0;
        } else {
            transform.translation.x = at.translation.x;
        }
        transform.translation.y = y;
    }
}

/// The editor follows the watched robot: its file, and the line its brain stands on.
fn show_code(watched: Res<Watched>, mut editor: ResMut<Editor>, robots: Query<(Entity, &Robot)>) {
    // the buttons along the top of the editor: every robot, by number, in its team's colour
    let mut choices: Vec<(usize, rubevy_egui::editor::EditorChoice)> = robots
        .iter()
        .map(|(e, r)| {
            let (cr, cg, cb) = TEAM_COLORS[r.team.min(TEAM_COLORS.len() - 1)];
            let brain = r.file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            (
                r.number,
                rubevy_egui::editor::EditorChoice {
                    id: e.to_bits(),
                    label: format!("{} {brain}{}", r.number, if r.brain.is_some() { "*" } else { "" }),
                    color: if r.hp <= 0.0 {
                        (150, 150, 150)
                    } else {
                        ((cr * 255.0) as u8, (cg * 255.0) as u8, (cb * 255.0) as u8)
                    },
                    dim: r.hp <= 0.0,
                },
            )
        })
        .collect();
    choices.sort_by_key(|(n, _)| *n);
    editor.choices = choices.into_iter().map(|(_, c)| c).collect();
    editor.selected = watched.entity.map(|e| e.to_bits());

    // **The word, the label and the key this game wants** (S4a, 2026-09-20). They used to be the
    // editor's own defaults, which is to say that a crate shared by every game here said `robot`
    // and meant this one. The defaults are neutral now (`script`, `▶ Apply`, no second key) and
    // these three lines are what keeps the panel word-for-word and key-for-key what it was.
    // Before the early returns below, as the garden's are, so that a frame with no robot watched
    // does not draw a differently-worded button.
    editor.noun = "robot".into();
    editor.apply_label = "▶ Apply (F5)".into();
    editor.apply_key = Some(KeyCode::F5);

    let Some(entity) = watched.entity else { return };
    let Ok((_, robot)) = robots.get(entity) else { return };
    // what the robot is running: its applied brain, or its file
    editor.show(entity.to_bits(), || {
        robot.brain.clone().unwrap_or_else(|| platform::read(&robot.file).unwrap_or_default())
    });
    editor.file = brain_name(robot);
    // the editor no longer knows what a robot is (`rubevy-egui` had to grow a second game):
    // the buttons' words are the game's
    editor.apply_all_label = Some(format!("Apply to all {}", brain_name(robot)));
    editor.save_label = Some(platform::SAVE_LABEL.into());
    editor.in_memory = robot.brain.is_some();
    editor.label = robot.name.clone();
    editor.current = robot.own_line;
    editor.heat = robot.heat.clone();
    // not where it stands this instant (`prelude.rb:82`, then `:67`, many times a second): the
    // shading says where it keeps coming back to, which is what can be read
    editor.elsewhere = None;
}

/// **`P`: the match is stopped.** The budget the scripts get when they are not paused, kept
/// while they are — and the flag the rules of the game are gated on (`world_moves`).
///
/// Until G9 it stopped the Ruby and nothing else: the tanks rolled on along the controls their
/// last thought had set, the bullets went on flying, and a match left paused for a minute came
/// back with the walls closed in. The garden's author found the same thing in the other game and
/// decided it for both: `P` stops the world.
#[derive(Resource, Default)]
struct Paused {
    was: Option<u64>,
}

impl Paused {
    fn on(&self) -> bool {
        self.was.is_some()
    }
}

/// **The match's own clock (G9).** Seconds the *world* has run, which is the process's clock
/// minus everything spent paused.
///
/// It is a resource rather than `Time::elapsed_secs()` because the match is written in Ruby
/// against this number: `rule(:sudden_death, after: 20, every: 2.0)` compares it with
/// `Rubevy.ask("clock")`, and a clock that ran while the world did not would come back from a
/// pause with every missed repeat of that rule due at once — the walls closing in five crates in
/// as many passes. `status`'s and `radar`'s `time` are the same number for the same reason: what
/// a robot is told the time is should be the time in the match it is fighting.
#[derive(Resource, Default)]
struct WorldClock(f32);

/// Every frame the world moves, and none that it does not.
fn advance_clock(time: Res<Time>, mut clock: ResMut<WorldClock>) {
    clock.0 += time.delta_secs();
}

/// The run condition of everything that *is* the match, as against everything that draws it or
/// edits it (G9): `P` holds it all still — the questions the game answers, the tanks, the
/// bullets, the blasts and the clock above. The scoreboard, the nameplates, the editor, the
/// reload watcher and `R` go on.
///
/// `Option`, because the headless build has no keyboard to press `P` with and inserts no
/// `Paused`: a world nobody can pause always moves.
fn world_moves(paused: Option<Res<Paused>>) -> bool {
    !paused.is_some_and(|p| p.on())
}

/// `F2` shows and hides the VM panel; `P` pauses the scripts.
///
/// Pausing is the plugin's instruction budget set to 0: `task_run_limits` returns before it hands
/// any task the CPU, so nothing in the VM moves and the snapshot the panel reads stands still.
/// The game keeps drawing, and the tanks keep rolling on the controls their brains last set —
/// it is the Ruby that is stopped, not the match. The scheduler's clock stops with it (rubevy
/// `fa37eaa`: a budget of 0 skips `task_advance_ticks`), so a robot that was half way through a
/// `sleep 0.05` is still half way through it when the budget comes back; there is nothing here to
/// call for that, and nothing to undo.
fn inspect_keys(
    keys: Res<ButtonInput<KeyCode>>,
    typing: Option<Res<bevy_egui::input::EguiWantsInput>>,
    mut panel: ResMut<VmInspector>,
    mut world: ResMut<ScriptWorld>,
    mut paused: ResMut<Paused>,
) {
    // a `p` typed into the editor is the editor's
    if typing.is_some_and(|t| t.wants_keyboard_input()) {
        return;
    }
    if keys.just_pressed(KeyCode::F2) {
        panel.open = !panel.open;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        match paused.was.take() {
            Some(budget) => {
                world.budget = budget;
                panel.paused = false;
            }
            None => {
                paused.was = Some(world.budget);
                world.budget = 0;
                panel.paused = true;
                // pausing is for looking at something: show the panel if it is not up
                panel.open = true;
            }
        }
    }
}

/// The VM panel follows the watched robot, as the editor does.
///
/// The snapshot is taken only while the panel is open: it walks every context of the VM and
/// renders the registers of the innermost frames of each, which is not work to do on a frame
/// nobody is looking.
fn show_vm(
    watched: Res<Watched>,
    world: Res<ScriptWorld>,
    mut panel: ResMut<VmInspector>,
    robots: Query<(&Robot, Option<&ScriptTask>, &ScriptPanel)>,
) {
    if !panel.open {
        return;
    }
    let Some(entity) = watched.entity else { return };
    let Ok((robot, script, hud)) = robots.get(entity) else {
        panel.clear("no robot");
        return;
    };
    let Some(script) = script else {
        panel.title = robot.name.clone();
        panel.clear("this robot is down: the game took its task away, and the VM terminated it");
        return;
    };
    panel.spent = hud.spent;
    panel.fill(&world, script.task(), robot.name.clone(), robot.prelude_lines);
}

/// `SABIBOTS_SELFTEST=1`, or `?selftest` in a page's address (2026-09-18): drives the editor's
/// buttons the way a click would (by setting `Editor::action`) and checks what happened to the
/// robots and to the file — the part of the editor that cannot be clicked where there is no
/// mouse. Logs `selftest:` lines and, where there is something to exit to, exits.
#[derive(Resource, Default)]
struct SelfTest {
    step: usize,
    at: f32,
    original: String,
    /// instructions every brain had run together, for the pause check
    insn: u64,
    /// ticks until the earliest sleeping task is due, sampled on the first frame of the pause
    wake: Option<u32>,
    /// where every robot stood when the pause began, and what the match's clock said (G9)
    places: Vec<(Entity, Vec3)>,
    clock: f32,
    /// the robots there were before the restart, so the new ones can be told from them
    before: Vec<Entity>,
    /// how long a step that polls will wait before it gives up and checks anyway
    deadline: f32,
}

/// **What the editor's checks are doing to the arena's behaviours, where the handler check can
/// read it** (2026-09-18).
///
/// [`selftest`] above drives the editor's buttons: Apply, Apply to all, Revert, and then a
/// restart of the whole match. Every one of those takes a running behaviour away and starts
/// another in its place — `do_editor_actions` calls `restart` on each robot it reaches, and
/// `restart_match` builds four new ones — and a behaviour that has just been started has not
/// subscribed to `hit` yet, while the one it replaced had its swerve cut off mid-turn.
///
/// [`handler_selftest`] is measuring exactly that 0.3 s, and it already declined to count a hit
/// on a robot **whose own** task had changed. That is not enough: `Apply to all` reaches every
/// robot on the file and a restart reaches all four, so a hit taken by *another* robot loses its
/// window just as completely, and the one that was hit is often not the one the editor is
/// showing. That is the FAIL a browser run turned up one time in four — `3 blue/scout ran a
/// handler within 0.3 s of the hit at 3.78 s`, where the checks were between Revert (3.5 s) and
/// the restart (4.0 s) — and it is reachable on a PC with a window open too
/// (`docs/worklog/2026-09-18-corner-and-selftest.md` §2.4).
///
/// So the moments are published here, in one place, rather than worked out twice: each step that
/// replaces a behaviour writes down when it asked for it, and the step that sees the restart's
/// four robots standing there closes the span. `handler_selftest` reads it and excludes every
/// robot's hits over that span. **No threshold is moved**: the 0.3 s window is the same 0.3 s,
/// and what changes is which hits are inside a window this check can answer for.
///
/// It is a resource of its own rather than a field of [`SelfTest`] because the two checks do not
/// run together: the handler check runs headless as well, where the editor's checks are not
/// registered at all. There it stays empty, and nothing is excluded.
#[derive(Resource, Default)]
struct EditChecks {
    /// every step that replaces a running behaviour, in the order they happen: when it was asked
    /// for, and what it was
    swaps: Vec<(f32, &'static str)>,
    /// when the last of them was seen to have landed — the restart's four robots standing there.
    /// `None` while the checks are still swapping, which is the honest answer: the game has not
    /// been asked whether a replacement has taken, and until it is seen the span is still open.
    settled: Option<f32>,
}

impl EditChecks {
    fn asked(&mut self, at: f32, what: &'static str) {
        self.swaps.push((at, what));
    }

    fn landed(&mut self, at: f32) {
        self.settled = Some(at);
    }

    /// What the checks were doing to the arena's behaviours at any moment between `from` and
    /// `to`, if they were doing anything. `None` where they had not started, had finished, or
    /// were never registered at all.
    fn over(&self, from: f32, to: f32) -> Option<&'static str> {
        let (first, what) = *self.swaps.first()?;
        if to < first || from > self.settled.unwrap_or(f32::INFINITY) {
            return None;
        }
        // the nearest one at or before the end of the window, for the line it prints
        self.swaps.iter().rev().find(|(at, _)| *at <= to).map(|(_, what)| *what).or(Some(what))
    }
}

/// The text the S2 check types into the editor, and the line its one mistake is on.
///
/// `1 < < 2` is an operator with nothing after it: one diagnostic, and it is on the last line of
/// the text whichever robot's file is being shown. The line is counted rather than written down,
/// so it stays right when a robot's file grows.
fn a_text_that_will_not_compile(file: &str) -> (String, usize) {
    let broken = format!("{}\n1 < < 2\n", file.trim_end());
    let at = broken.lines().count();
    (broken, at)
}

fn selftest(
    time: Res<Time>,
    mut test: ResMut<SelfTest>,
    mut editor: ResMut<Editor>,
    mut watched: ResMut<Watched>,
    robots: Query<(Entity, &Robot)>,
    // G9: where the tanks are, and what the match's own clock says — the two things `P` must
    // hold still besides the VM
    bodies: Query<(Entity, &Transform), With<Robot>>,
    match_clock: Res<WorldClock>,
    tasks: Query<&ScriptTask>,
    world: Res<ScriptWorld>,
    panel: Res<VmInspector>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    walls: Query<&Transform, With<Wall>>,
    arena: Res<ArenaSize>,
    mut restart: ResMut<Restart>,
    mut edits: ResMut<EditChecks>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if now < test.at {
        return;
    }
    let by_number = |n: usize| robots.iter().find(|(_, r)| r.number == n);
    let ok = |cond: bool, what: &str| info!("selftest: {} {what}", if cond { "ok  " } else { "FAIL" });
    // what every brain has run together: the number that must stop moving while the VM is paused
    let spent = || tasks.iter().map(|t| world.vm.task_instructions(t.task())).sum::<u64>();
    // and where the tanks are, which must stop moving with it (G9)
    let places = || -> Vec<(Entity, Vec3)> { bodies.iter().map(|(e, t)| (e, t.translation)).collect() };
    match test.step {
        0 => {
            // show robot 3 and type a different brain into the editor
            let Some((e, r)) = by_number(3) else { return };
            test.original = platform::read(&r.file).unwrap_or_default();
            watched.entity = Some(e);
            test.step = 1;
            test.at = now + 0.5;
        }
        1 => {
            // **S2: a text that will not compile is refused in the author's own line numbers.**
            //
            // Until the prelude and the robot's file were put together by rubevy's `Program`,
            // the number in `not applied:` was the whole compiled program's: `prelude.rb` is
            // 295 lines and the separator and the blank line after it are two more, so the
            // editor said line 302 for a mistake on line 5 and the author was sent somewhere
            // that is not in the file being looked at. The garden was fixed on 2026-09-18 and
            // this one was not.
            let (broken, _) = a_text_that_will_not_compile(&test.original);
            editor.text = broken;
            editor.action = Some(EditorAction::Apply);
            test.step = 2;
            // the gap every step in this sequence uses: `do_editor_actions` runs on a later
            // frame, and the answer is read on the step after this one
            test.at = now + 0.5;
        }
        2 => {
            // the text was refused, the robot kept the brain it had, and the line the editor
            // names is the author's — not that line plus the 297 in front of it
            let (_, r3) = by_number(3).unwrap();
            let said = editor.message.clone();
            let (_, at) = a_text_that_will_not_compile(&test.original);
            ok(
                said.contains(&format!(":{at}:")) && r3.brain.is_none(),
                &format!("a syntax error is refused on the author's own line {at}: {said}"),
            );
            editor.reset_to(test.original.clone(), "back to the file");

            // H2. Both robots' files hold a `def run`, so one `find` reaches a keyword the lexer
            // has to have seen. `drawn_kind` goes the whole way through the panel's own
            // `listing()` and reads the colour back out of the `LayoutJob`: what this says is
            // "it is *painted* in the keyword colour", not "the table says 1". This is the same
            // line the garden's window checks have, and the panel drawing it is the same crate.
            let def = editor.text.find("def ").unwrap_or(usize::MAX);
            let kind = rubevy_egui::editor::drawn_kind(&editor, def);
            ok(
                kind == Some(1),
                &format!("`def` in the listing is painted in the keyword colour (kind {kind:?})"),
            );
            editor.text = editor.text.replace("sleep 0.05", "sleep 0.5");
            ok(editor.changed(), "typing marks the text edited");
            // **Press the key, do not write the action down** (S4). Every step here used to set
            // `editor.action` by hand, which is what a button does — so nothing in these checks
            // ever went through `draw_editor`'s keyboard, and `Editor::apply_key` could have been
            // the wrong key, or `None`, with all of them still passing. S4a made that matter: the
            // crate's default became `None` and the F5 is the game's own line (`setup_editor`).
            // The forgery is the one `keys.press(KeyCode::F2)` below uses — `just_pressed` is
            // cleared in `PreUpdate`, and `draw_editor` runs in `EguiPrimaryContextPass`, after
            // this. Released on the step that reads the answer.
            keys.release(KeyCode::F5);
            keys.press(KeyCode::F5);
            edits.asked(now, "Apply");
            test.step = 3;
            test.at = now + 0.5;
        }
        3 => {
            keys.release(KeyCode::F5);
            let (_, r3) = by_number(3).unwrap();
            let (_, r4) = by_number(4).unwrap();
            let on_disk = platform::read(&r3.file).unwrap_or_default();
            // the step before pressed F5 and wrote no action down, so a robot with a brain at
            // all is `Editor::apply_key` arriving. The next line says what the brain is; this
            // one says the key reached the editor (S4; the garden's page and this one both take
            // F5 back from the browser for it, `web/games.sh`).
            ok(r3.brain.is_some(), "F5 is Apply: the key alone applied the text");
            ok(r3.brain.as_deref().is_some_and(|b| b.contains("sleep 0.5")), "Apply gives robot 3 the edited behaviour");
            ok(r4.brain.is_none(), "Apply leaves robot 4 (same file) alone");
            ok(on_disk == test.original, "Apply does not touch the file");
            ok(!editor.changed(), "after Apply the text is what the robot runs");
            editor.action = Some(EditorAction::ApplyAll);
            edits.asked(now, "Apply to all");
            test.step = 4;
            test.at = now + 0.5;
        }
        4 => {
            let (_, r4) = by_number(4).unwrap();
            let (_, r2) = by_number(2).unwrap();
            ok(r4.brain.is_some(), "Apply to all reaches robot 4 (same file)");
            ok(r2.brain.is_none(), "Apply to all leaves robot 2 (another file) alone");
            editor.action = Some(EditorAction::Revert);
            edits.asked(now, "Revert");
            test.step = 5;
            test.at = now + 0.5;
        }
        5 => {
            let (_, r3) = by_number(3).unwrap();
            let (_, r4) = by_number(4).unwrap();
            ok(r3.brain.is_none(), "Revert puts robot 3 back on its file");
            ok(r4.brain.is_some(), "Revert is for the shown robot only: robot 4 keeps its behaviour");
            ok(editor.text == test.original, "Revert shows the file again");
            let on_disk = platform::read(&r3.file).unwrap_or_default();
            ok(on_disk == test.original, "nothing was written");
            restart.0 = true;
            edits.asked(now, "Restart");
            test.before = robots.iter().map(|(e, _)| e).collect();
            test.step = 6;
            test.at = now;
            test.deadline = now + 2.0;
        }
        6 => {
            // On the frame the four new robots are there, and not two seconds later: since a
            // question costs one frame instead of two the robots decide half again as often, and
            // by two seconds into a fresh match one of them has taken a hit. What is being
            // checked is that a restart hands out new robots at full health, so it is checked
            // the moment the new robots exist.
            let fresh = robots.iter().filter(|(e, _)| !test.before.contains(e)).count();
            if fresh < 4 && now < test.deadline {
                return;
            }
            ok(robots.iter().count() == 4 && fresh == 4, "after a restart there are four robots again, not eight");
            ok(robots.iter().all(|(_, r)| r.hp == 100.0), "every robot starts with full health");
            // the last of the four replacements has landed: from here the arena's behaviours are
            // nobody's but the match's again, and a hit is a hit (`EditChecks`)
            edits.landed(now);
            test.step = 7;
            test.at = now + 1.0;
        }
        7 => {
            let r3 = by_number(3).map(|(_, r)| r);
            let r4 = by_number(4).map(|(_, r)| r);
            ok(r3.is_some_and(|r| r.brain.is_none()), "robot 3 comes back on its file");
            ok(r4.is_some_and(|r| r.brain.is_some()), "robot 4 comes back with its applied behaviour");
            // every side ends on a corner: no crate past the wall's line
            let half = arena.0;
            let edge = walls.iter().map(|t| t.translation.x.abs().max(t.translation.y.abs())).fold(0.0f32, f32::max);
            let corner = walls.iter().any(|t| (t.translation.x - half).abs() < 0.01 && (t.translation.y - half).abs() < 0.01);
            ok((edge - half).abs() < 0.01 && corner, "the wall ends exactly on its corners");
            // the VM inspector: `P` stops the scripts by giving the scheduler a budget of 0 —
            // and, from G9, the match with them. The panel starts closed now, so `F2` opens it
            // first: the checks below are about what it draws.
            test.insn = spent();
            ok(!panel.open, "the VM panel starts closed");
            keys.press(KeyCode::F2);
            test.step = 8;
            test.at = now + 0.2;
        }
        8 => {
            ok(panel.open, "F2 opens it");
            keys.release(KeyCode::F2);
            keys.press(KeyCode::KeyP);
            test.step = 9;
            test.at = now + 0.1;
        }
        9 => {
            // The pause has taken hold. How many ticks the earliest sleeping task still has to
            // wait is the measure of the scheduler's clock: a budget of 0 stops that clock too
            // (rubevy `fa37eaa`), so this number must be the same when the pause ends. The
            // tanks' places and the match's clock are the world's side of the same question.
            test.wake = world.vm.task_next_wakeup_ticks();
            test.places = places();
            test.clock = match_clock.0;
            test.insn = spent();
            test.step = 10;
            // two seconds (G9): a tank crosses several units in that, and the match's clock
            // would have run a tenth of the way to its first rule
            test.at = now + 2.0;
        }
        10 => {
            ok(panel.paused && world.budget == 0, "P pauses: the scripts' budget is 0");
            ok(spent() == test.insn, "nothing ran while it was paused");
            ok(places() == test.places, "2 s paused: every robot is where it was");
            ok(match_clock.0 == test.clock, "2 s paused: the match's clock did not move");
            ok(panel.open && !panel.frames.is_empty(), "the VM panel has the watched robot's frames");
            ok(
                panel.heap.as_ref().is_some_and(|h| h.live > 0),
                "the panel has the heap counters",
            );
            // Two seconds paused is forty times a brain's `sleep 0.05`. Before the clock was
            // stopped as well, every one of those sleeps came due while nothing was running and
            // the lot of them woke on the frame the budget came back. The ticks left before the
            // earliest is due say so exactly: unchanged, and nothing is due yet.
            let what = "nothing that was sleeping woke on the resume frame";
            match test.wake {
                Some(was) if was > 0 => {
                    let left = world.vm.task_next_wakeup_ticks();
                    let same = left == Some(was);
                    info!(
                        "selftest: {} {what}: the next one is due in {was} ticks, as it was two seconds ago{}",
                        if same { "ok  " } else { "FAIL" },
                        if same { String::new() } else { format!(" (now {left:?})") },
                    );
                }
                // Nothing to measure: a task was already due when the pause began (or none was
                // sleeping at all), so the clock standing still cannot be told from it running.
                other => info!("selftest: --   {what}: the next wakeup was {other:?} ticks off when the pause began"),
            }
            test.insn = spent();
            test.places = places();
            test.clock = match_clock.0;
            // `press` on a key already held sets nothing: nothing released it, since nothing
            // here is a real keyboard
            keys.release(KeyCode::KeyP);
            keys.press(KeyCode::KeyP);
            test.step = 11;
            test.at = now + 0.5;
        }
        11 => {
            ok(!panel.paused && world.budget > 0, "P again gives the budget back");
            ok(spent() > test.insn, "the behaviours are running again");
            let moved = places()
                .iter()
                .any(|(e, at)| test.places.iter().any(|(was, place)| was == e && place != at));
            ok(moved, "and the match moves again: somebody has driven");
            ok(match_clock.0 > test.clock, "the match's clock runs again");
            keys.release(KeyCode::KeyP);
            // A PC run was asked for the checks on a command line and should give the prompt
            // back. A page was asked for them in its address, by somebody who is looking at the
            // arena — and `AppExit` there does not end a run, it stops the canvas for good
            // (`platform::CHECKS_EXIT_WHEN_DONE`).
            if platform::CHECKS_EXIT_WHEN_DONE {
                exit.write(AppExit::Success);
            } else {
                info!("selftest: done — the match keeps running (a page has nothing to exit to)");
            }
            test.step = 12;
        }
        _ => {}
    }
}

/// `--shot`: where to put the picture, and when.
#[derive(Resource)]
struct Shot {
    path: String,
    after: f32,
    taken: bool,
}

fn take_shot(mut commands: Commands, time: Res<Time>, mut shot: ResMut<Shot>, mut exit: MessageWriter<AppExit>) {
    if shot.taken {
        if time.elapsed_secs() > shot.after + 1.0 {
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

/// `--headless`: how long the match may last, and the last line printed.
#[derive(Resource)]
struct Headless {
    until: f32,
}

fn stop_when_over(
    time: Res<Time>,
    headless: Res<Headless>,
    hud: Res<Hud>,
    test: Option<Res<HandlerTest>>,
    world: Res<ScriptWorld>,
    robots: Query<(&Robot, &ScriptPanel, &Transform, Option<&ScriptTask>)>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    let over = hud.line.starts_with("winner") || now >= headless.until;
    if !over {
        return;
    }
    for (robot, panel, t, _) in &robots {
        info!(
            "{:<8} hp {:>4}  at ({:>6.1}, {:>6.1})  {:>6} insn/frame  {}  {} handlers  {}/{} handler tasks ended",
            robot.name,
            robot.hp.max(0.0) as i32,
            t.translation.x,
            t.translation.y,
            panel.spent,
            panel.at,
            robot.handler_runs,
            robot.handler_off,
            robot.handlers
        );
    }
    if let Some(test) = test {
        let ok = |cond: bool, what: String| info!("selftest: {} {what}", if cond { "ok  " } else { "FAIL" });
        // a robot that went down took its handler tasks with it. They are `Task.new` tasks, which
        // nothing terminates; what ends them is the subscription closing when the game takes the
        // `ScriptTask` away (rubevy `9104f7c`). Before that they stayed `WAITING` for ever.
        let settled: Vec<&Robot> = robots
            .iter()
            .map(|(r, _, _, _)| r)
            .filter(|r| r.handlers > 0 && r.downed_at.is_some_and(|at| now - at > 0.5))
            .collect();
        let ended = settled.iter().filter(|r| r.handler_off >= r.handlers).count();
        // **One sentence, three verdicts** (S4). A short match where nobody was destroyed does
        // not exercise this, and saying FAIL there would be saying a check failed when it never
        // ran — so it says `--`, which is this game's n/a. What changed in S4 is that the
        // unmeasured run no longer says something *else*: it used to print "no robot with a
        // handler was down long enough to check its tasks", a different line, so the set of
        // lines a run printed depended on whether anybody had died and the two had to be
        // dropped from the comparison altogether. Now the line is always there and it is the
        // verdict in front of it that moves (docs/verification/selftest-lines.md).
        info!(
            "selftest: {} the handler tasks of every robot that went down ended ({})",
            if settled.is_empty() { "--  " } else if ended == settled.len() { "ok  " } else { "FAIL" },
            if settled.is_empty() {
                "none was down long enough to check".to_string()
            } else {
                format!("{ended}/{}", settled.len())
            },
        );
        ok(test.checked > 0, format!("{} hits on a robot with a handler were checked", test.checked));
        ok(
            test.checked > 0 && test.ran == test.checked,
            format!("a handler ran within 0.3 s of the hit ({}/{})", test.ran, test.checked),
        );
        ok(
            test.checked > 0 && test.turned == test.checked,
            format!("the heading changed within 0.3 s of the hit ({}/{})", test.turned, test.checked),
        );
    }
    // the VM inspector's own numbers, where there is no window to draw them in (D2): the frames
    // each brain is standing in with the locals of the innermost of them, and the heap
    let mut panel = VmInspector::default();
    for (robot, _, _, script) in &robots {
        let Some(script) = script else { continue };
        panel.fill(&world, script.task(), robot.name.clone(), robot.prelude_lines);
        for line in panel.log_lines() {
            info!("{line}");
        }
    }
    info!("{}", if hud.line.is_empty() { "time" } else { hud.line.as_str() });
    exit.write(AppExit::Success);
}

/// `SABIBOTS_SELFTEST=1` or `?selftest`: the check the handler is for — a hit is noted with the way the robot was
/// facing, and 0.3 s later it must have run a handler and turned. It runs headless as well as in a
/// window, since a handler needs a fight rather than a mouse:
///
///     SABIBOTS_SELFTEST=1 cargo run -p sabibots -- --headless 25
#[derive(Resource, Default)]
struct HandlerTest {
    /// one per hit taken by a robot with a handler
    watching: Vec<WatchedHit>,
    checked: u32,
    ran: u32,
    turned: u32,
}

/// A hit being watched: the robot, when it was hit, which way it faced then, the number of
/// handlers it had run by then, and the furthest it has turned from that heading since.
///
/// The furthest rather than where it ends up: a robot hit twice swerves one way and then the
/// other, and its heading 0.3 s later can be the one it started with. What the check is about is
/// whether the tank moved at all before the brain's next pass.
#[derive(Clone, Copy)]
struct WatchedHit {
    robot: Entity,
    at: f32,
    heading: f32,
    runs: u32,
    peak: f32,
    /// the task its brain was running then. A different one (or none) when the window is up
    /// means the game took its brain away and started it over inside the window, which is not a
    /// robot that failed to swerve.
    task: Option<sabiruby::value::ObjId>,
}

/// The short way round between two angles, for "how far has it turned".
fn angle_between(a: f32, b: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let d = (b - a).rem_euclid(TAU);
    if d > PI { d - TAU } else { d }
}

fn handler_selftest(
    time: Res<Time>,
    mut test: ResMut<HandlerTest>,
    edits: Res<EditChecks>,
    robots: Query<&Robot>,
    tasks: Query<&ScriptTask>,
) {
    let now = time.elapsed_secs();
    for watch in test.watching.iter_mut() {
        if let Ok(robot) = robots.get(watch.robot) {
            watch.peak = watch.peak.max(angle_between(watch.heading, robot.heading).abs());
        }
    }
    let mut due: Vec<WatchedHit> = Vec::new();
    test.watching.retain(|w| {
        if now - w.at < 0.3 {
            return true;
        }
        due.push(WatchedHit { ..*w });
        false
    });
    for watch in due {
        let Ok(robot) = robots.get(watch.robot) else { continue };
        // **Why this hit could not be measured, if it could not be** — and nothing else changes
        // with it (S4). Each of the three used to print a sentence of its own instead of the two
        // below, so a hit that was excluded left one line where a hit that was counted left two,
        // and the wording moved as well: four different shapes to filter out before two runs
        // could be compared. The same two sentences are printed either way now, with `--` (this
        // game's n/a) in front of them and the reason at the end. What is excluded, and why, is
        // unchanged (docs/verification/selftest-lines.md).
        let unmeasured: Option<String> =
            // A robot destroyed inside the window is not a robot that failed to swerve: the game
            // takes its task away and sets its controls to zero. The hit is not counted either way.
            // (Found here: one run in four ended with `turned (13/14)`, the miss being a scout hit at
            // 19.37 s that went down before the 0.3 s were up — 0.04 rad.)
            if robot.downed_at.is_some_and(|down| down < watch.at + 0.3) {
                Some("it went down inside the 0.3 s".to_string())
            }
            // Nor is a robot whose brain was taken away and started again inside the window (the
            // editor's Apply, or a saved file). The old task is terminated and the new one subscribes
            // afresh, so a `hit` published in between reaches nobody — and a swerve already under way
            // is cut off with it. The task it is running is how that shows from here: a different one
            // is a different brain. It is not rare in the editor's selftest, which applies a brain
            // three times while the match is being fought.
            else if tasks.get(watch.robot).ok().map(|t| t.task()) != watch.task {
                Some("its behaviour was replaced inside the 0.3 s".to_string())
            }
            // **Nor any robot at all, while the editor's checks are handing behaviours out**
            // (2026-09-18). The test above catches the robot the editor was showing; `Apply to all`
            // reaches every robot on the file and a restart reaches all four, and their hits lose
            // the same 0.3 s for the same reason — the task that would have run the handler was
            // terminated and its replacement had not subscribed yet. The moments come from
            // [`EditChecks`], which is where the steps that press the buttons write them down; the
            // window asked about is this check's own, and unchanged.
            else if let Some(what) = edits.over(watch.at, watch.at + 0.3) {
                Some(format!("the editor's checks were handing out behaviours ({what}) inside the 0.3 s"))
            } else {
                None
            };
        if let Some(why) = unmeasured {
            let (at, name) = (watch.at, &robot.name);
            info!("selftest: --   {name} ran a handler within 0.3 s of the hit at {at:.2} s: not counted, {why}");
            info!("selftest: --   {name} turned within 0.3 s of the hit at {at:.2} s: not counted, {why}");
            continue;
        }
        let ran = robot.handler_runs > watch.runs;
        // a quarter of the full turning rate over the 0.3 s: the swerve, not the brain's steering
        let turned = watch.peak > 0.2;
        test.checked += 1;
        test.ran += u32::from(ran);
        test.turned += u32::from(turned);
        let (at, name, peak) = (watch.at, &robot.name, watch.peak);
        info!(
            "selftest: {} {name} ran a handler within 0.3 s of the hit at {at:.2} s",
            if ran { "ok  " } else { "FAIL" }
        );
        info!(
            "selftest: {} {name} turned within 0.3 s of the hit at {at:.2} s ({peak:.2} rad)",
            if turned { "ok  " } else { "FAIL" }
        );
    }
}

/// A crate of the arena wall, so the wall can be rebuilt when the match closes it in.
#[derive(Component)]
struct Wall;

/// The floor, tiled, and a wall of crates around it.
fn spawn_arena(mut commands: Commands, arena: Res<ArenaSize>, server: Res<AssetServer>) {
    let half = arena.0;
    let tile = 8.0;
    let sand: Handle<Image> = server.load("sprites/tileSand1.png");
    let sand2: Handle<Image> = server.load("sprites/tileSand2.png");
    // the window is 16:9 and the arena is square, so the floor reaches past the wall sideways
    let tall = half + tile;
    let wide = tall * 16.0 / 9.0 + tile * 3.0;
    let nx = (wide * 2.0 / tile).ceil() as i32;
    let ny = (tall * 2.0 / tile).ceil() as i32;
    for ix in 0..nx {
        for iy in 0..ny {
            let x = -wide + tile * (ix as f32 + 0.5);
            let y = -tall + tile * (iy as f32 + 0.5);
            let image = if (ix + iy) % 3 == 0 { sand2.clone() } else { sand.clone() };
            commands.spawn((
                Sprite { image, custom_size: Some(Vec2::splat(tile)), ..default() },
                Transform::from_xyz(x, y, -1.0),
            ));
        }
    }
    build_walls(&mut commands, &server, half);
}

/// The entity the match's script runs on.
#[derive(Component)]
struct MatchScript;

fn spawn_match(mut commands: Commands, ruby: Res<RubyDir>, mut assets: ResMut<Assets<MrbAsset>>) {
    start_match(&mut commands, &ruby.0, &mut assets);
}

fn start_match(commands: &mut Commands, ruby: &Path, assets: &mut Assets<MrbAsset>) {
    // the match is a script like a robot is, at a higher priority: it spawns the field and
    // decides when the fight is over, and the game only does what it is told
    let path = ruby.join("matches").join("training.rb");
    let Some((handle, _)) = compile_with(ruby, "match_prelude.rb", &path, "run_match", assets) else {
        return;
    };
    commands.spawn((MatchScript, Script::new(handle).with_name("match").with_priority(10)));
}

/// Set by the scoreboard's button or `R`: start the match over on the next frame.
#[derive(Resource, Default)]
struct Restart(bool);

/// Brains applied from the editor and not saved, by robot number, kept across a restart: the
/// robot that comes back as number 3 gets number 3's brain.
#[derive(Resource, Default)]
struct KeptBrains(std::collections::HashMap<usize, String>);

/// Everything on the field goes — robots (their turrets, names and bars follow them), shots,
/// blasts, the match — the arena goes back to its size, and the match script starts again. Its
/// task and the robots' tasks are terminated as their entities go.
fn restart_match(
    mut restart: ResMut<Restart>,
    mut commands: Commands,
    ruby: Res<RubyDir>,
    mut assets: ResMut<Assets<MrbAsset>>,
    mut arena: ResMut<ArenaSize>,
    mut shots: ResMut<Shots>,
    mut events: ResMut<Events>,
    mut hud: ResMut<Hud>,
    mut kept: ResMut<KeptBrains>,
    robots: Query<(Entity, &Robot)>,
    others: Query<Entity, Or<(With<Bullet>, With<Blast>, With<MatchScript>)>>,
    watched: Option<ResMut<Watched>>,
    editor: Option<ResMut<Editor>>,
) {
    if !std::mem::take(&mut restart.0) {
        return;
    }
    kept.0.clear();
    for (entity, robot) in &robots {
        if let Some(brain) = &robot.brain {
            kept.0.insert(robot.number, brain.clone());
        }
        commands.entity(entity).despawn();
    }
    for entity in &others {
        commands.entity(entity).despawn();
    }
    arena.0 = ARENA_HALF_WIDTH;
    shots.0.clear();
    events.0.clear();
    hud.line = "the match starts again".into();
    if let Some(mut watched) = watched {
        watched.entity = None;
    }
    if let Some(mut editor) = editor {
        editor.clear();
    }
    start_match(&mut commands, &ruby.0, &mut assets);
    info!("restart ({} applied behaviours kept)", kept.0.len());
}

fn restart_key(
    keys: Res<ButtonInput<KeyCode>>,
    typing: Option<Res<bevy_egui::input::EguiWantsInput>>,
    mut restart: ResMut<Restart>,
) {
    if typing.is_some_and(|t| t.wants_keyboard_input()) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        restart.0 = true;
    }
}

/// `Rubevy.ask("spawn", file, team, x, y)` from the match: a robot with its own brain.
fn spawn_robot(
    commands: &mut Commands,
    ruby: &Path,
    assets: &mut Assets<MrbAsset>,
    server: &AssetServer,
    shots: &mut Shots,
    kept: &KeptBrains,
    file: &str,
    team: usize,
    at: Vec2,
) -> Option<Entity> {
    let path = ruby.join("robots").join(format!("{file}.rb"));
    let number = shots.0.len() + 1; // one entry per robot spawned so far
    // a brain applied before a restart comes back with the robot's number, if it still compiles
    let brain = kept.0.get(&number).cloned();
    let applied = brain.as_ref().and_then(|text| {
        let name = format!("{file}.rb");
        compile_text(ruby, "prelude.rb", &name, text, "run_robot", assets).map_err(|e| warn!("{e}")).ok()
    });
    let brain = if applied.is_some() { brain } else { None };
    let (handle, prelude_lines) = match applied {
        Some(c) => c,
        None => compile(ruby, &path, assets)?,
    };
    let (team_name, hull, bullet) = TEAMS[team.min(TEAMS.len() - 1)];
    let name = format!("{number} {team_name}/{file}");
    let source = brain.clone().unwrap_or_else(|| platform::read(&path).unwrap_or_default());
    // it starts facing the middle of the arena
    let facing = (-at.y).atan2(-at.x);
    let robot = commands
        .spawn((
            Robot {
                number,
                team,
                name: name.clone(),
                file: path,
                hp: 100.0,
                cooldown: 0.0,
                velocity: Vec2::ZERO,
                throttle: 0.0,
                turn: 0.0,
                turret: facing,
                turret_target: facing,
                energy: ENERGY_MAX,
                last_instructions: 0,
                prelude_lines,
                own_line: None,
                heading: facing,
                brain,
                source,
                heat: Vec::new(),
                cpu: 0.0,
                handlers: 0,
                handler_runs: 0,
                handler_running: 0,
                handler_off: 0,
                downed_at: None,
            },
            Script::new(handle).with_name(&name).with_priority(100),
            ScriptPanel { name, ..default() },
            Sprite {
                image: server.load(hull),
                custom_size: Some(Vec2::splat(ROBOT_RADIUS * 2.4)),
                ..default()
            },
            Transform::from_xyz(at.x, at.y, 1.0),
        ))
        .id();
    shots.0.push((robot, server.load(bullet)));
    Some(robot)
}

/// The prelude and one robot's file, compiled to bytecode in process (the reference compiler),
/// so the game reads `.rb` and nothing has to be built ahead of time.
fn compile(ruby: &Path, robot: &Path, assets: &mut Assets<MrbAsset>) -> Option<(Handle<MrbAsset>, u32)> {
    compile_with(ruby, "prelude.rb", robot, "run_robot", assets)
}

/// A script: one DSL file in front, the author's file behind it, and the call that starts it.
/// The two are compiled as one program, which is why neither needs a `require`; the answer says
/// how many lines the prelude added, so line numbers can be reported in the author's own terms.
fn compile_with(
    ruby: &Path,
    prelude_file: &str,
    robot: &Path,
    start: &str,
    assets: &mut Assets<MrbAsset>,
) -> Option<(Handle<MrbAsset>, u32)> {
    let body = match platform::read(robot) {
        Ok(b) => b,
        Err(e) => {
            error!("{e}");
            return None;
        }
    };
    let name = robot.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    compile_text(ruby, prelude_file, &name, &body, start, assets).map_err(|e| error!("{e}")).ok()
}

/// The same, from text rather than a file: what the editor applies. The error is the compiler's
/// message, for the editor to show.
///
/// **In the author's own line numbers since S2.** The prelude sits in front, so every line the
/// compiler names is a line of the whole program — 295 lines further down than the same line of
/// the robot's file, which is the number the editor's `not applied:` used to show and nobody
/// could find. rubevy's [`Program`] puts the two halves together and says how far down that
/// pushed the first one, and [`in_the_authors_lines`] takes it off again (R6). The garden has
/// had the second half since 2026-09-18; this is where it was still missing.
///
/// The correction is here rather than at the four call sites so that there is one answer: what
/// this returns is a message in the author's terms whether it is logged, shown in the editor, or
/// both.
fn compile_text(
    ruby: &Path,
    prelude_file: &str,
    name: &str,
    body: &str,
    start: &str,
    assets: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    let prelude = platform::read(&ruby.join(prelude_file))?;
    let program = Program::new(&prelude, name, body, start);
    match platform::compile(&program.source, name) {
        Ok(bytes) => Ok((assets.add(MrbAsset { bytes }), program.prelude_lines)),
        Err(e) => Err(in_the_authors_lines(&e, program.prelude_lines, prelude_file)),
    }
}

/// Starts a robot over with another brain: dropping its task and giving it a new `Script`.
///
/// S2: the three lines this was are rubevy's [`replace_script`] (R6). The `ScriptDone` is the
/// one that does not show: without it a robot whose brain had run to its end could never be
/// given another.
fn restart(commands: &mut Commands, entity: Entity, name: &str, handle: Handle<MrbAsset>) {
    replace_script(commands, entity, Script::new(handle).with_name(name).with_priority(128));
}

fn brain_name(robot: &Robot) -> String {
    robot.file.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// What the editor's buttons asked for. Nothing here writes a file except `Save`: applying runs
/// the text in memory, so trying something in a match does not rewrite the project.
///
/// **Why `Apply to all` may go round a `Query` here when the garden's may not** (S7). The garden
/// grew a generation number because a creature born in this very frame has a `Mind` that is still
/// in the `Commands` queue, so the loop below cannot see it and it went on running the old brain
/// for the rest of the run (`garden/src/window.rs`, `catch_up_minds`). A robot is never made
/// while a match runs: the only thing that makes one is `start_match`, and the only thing that
/// calls it is `restart_match`, which despawns **every** robot in the same frame first and clears
/// the editor with them. So a `Robot` that this loop cannot see is a `Robot` that has no brain to
/// be given yet and is about to be spawned with one from `KeptBrains`; there is no robot left
/// running the wrong text, which is the thing the generation number is for. Giving Battle one
/// would mean inventing the thing it would count — this game has no per-file program resource,
/// because here the brain is a field of the robot — for a hole that cannot open.
fn do_editor_actions(
    mut editor: ResMut<Editor>,
    watched: Res<Watched>,
    ruby: Res<RubyDir>,
    mut commands: Commands,
    mut assets: ResMut<Assets<MrbAsset>>,
    mut robots: Query<(Entity, &mut Robot)>,
    mut hud: ResMut<Hud>,
) {
    let Some(action) = editor.action.take() else { return };
    let Some(shown) = watched.entity else { return };
    let Ok((_, robot)) = robots.get(shown) else { return };
    let file = robot.file.clone();
    let name = brain_name(robot);
    let label = robot.name.clone();
    let text = editor.text.clone();

    match action {
        EditorAction::Apply | EditorAction::ApplyAll => {
            let (handle, _) = match compile_text(&ruby.0, "prelude.rb", &name, &text, "run_robot", &mut assets) {
                Ok(c) => c,
                Err(e) => {
                    // the robot keeps the brain it has
                    editor.message = format!("not applied: {e}");
                    return;
                }
            };
            let mut count = 0;
            for (entity, mut r) in &mut robots {
                let target = if action == EditorAction::Apply { entity == shown } else { r.file == file };
                if !target {
                    continue;
                }
                r.brain = Some(text.clone());
                r.source = text.clone();
                r.heat.clear();
                restart(&mut commands, entity, &r.name, handle.clone());
                count += 1;
            }
            let what = if count == 1 { label.clone() } else { format!("{count} robots with {name}") };
            editor.applied(format!("applied to {what} (in memory: Save to file to keep it)"));
            hud.line = format!("new behaviour: {what}");
        }
        EditorAction::Save => {
            if let Err(e) = platform::write(&file, &text) {
                editor.message = format!("could not save {name}: {e}");
                return;
            }
            // the file is what this text is now: robots running exactly it are file-brained
            // again, and the watcher restarts the ones on the file (the shown one included)
            for (_, mut r) in &mut robots {
                if r.file == file && r.brain.as_deref() == Some(text.as_str()) {
                    r.brain = None;
                }
            }
            if let Ok((_, mut r)) = robots.get_mut(shown) {
                r.brain = None;
            }
            editor.applied(format!("saved to {name}"));
            hud.line = format!("{name} saved");
        }
        EditorAction::Revert => {
            let Ok(source) = platform::read(&file) else {
                editor.message = format!("could not read {name}");
                return;
            };
            match compile_text(&ruby.0, "prelude.rb", &name, &source, "run_robot", &mut assets) {
                Ok((handle, _)) => {
                    if let Ok((entity, mut r)) = robots.get_mut(shown) {
                        r.brain = None;
                        r.source = source.clone();
                        r.heat.clear();
                        restart(&mut commands, entity, &r.name, handle);
                    }
                    editor.reset_to(source, format!("back to {name}"));
                }
                Err(e) => editor.message = format!("the file does not compile: {e}"),
            }
        }
    }
}

/// The wall follows the arena: a match that closes it in changes `ArenaSize`, and the crates
/// move to the new edge.
fn rebuild_walls(
    mut commands: Commands,
    arena: Res<ArenaSize>,
    server: Res<AssetServer>,
    walls: Query<Entity, With<Wall>>,
) {
    if !arena.is_changed() || arena.is_added() {
        return;
    }
    for wall in &walls {
        commands.entity(wall).despawn();
    }
    build_walls(&mut commands, &server, arena.0);
}

/// How many crates make a side of an arena this size, and how far apart they are.
fn wall_layout(half: f32) -> (i32, f32) {
    let count = (half * 2.0 / 2.6).round().max(1.0) as i32;
    (count, half * 2.0 / count as f32)
}

/// The walls stop closing in at this many crates a side: room for a last fight.
const MIN_CRATES: i32 = 7;

/// A crate of about 2.6 units every step along each side, the step stretched a little so that
/// the side is a whole number of crates and every side ends exactly on a corner — at any size the
/// match shrinks the arena to.
fn build_walls(commands: &mut Commands, server: &AssetServer, half: f32) {
    let image: Handle<Image> = server.load("sprites/crateMetal.png");
    let (count, step) = wall_layout(half);
    let mut place = |x: f32, y: f32| {
        commands.spawn((
            Wall,
            Sprite { image: image.clone(), custom_size: Some(Vec2::splat(step)), ..default() },
            Transform::from_xyz(x, y, 0.0),
        ));
    };
    for i in 0..=count {
        let t = -half + step * i as f32;
        place(t, half);
        place(t, -half);
        // the corners are already there
        if i > 0 && i < count {
            place(-half, t);
            place(half, t);
        }
    }
}

/// The game's side of `Rubevy.ask`: everything a robot can see or do.
fn answer_requests(
    mut world: ResMut<ScriptWorld>,
    mut robots: Query<(Entity, &mut Robot, &Transform)>,
    bullets: Query<(&Bullet, &Transform), Without<Robot>>,
    mut commands: Commands,
    mut arena: ResMut<ArenaSize>,
    mut rules: ResMut<Rules>,
    kept: Res<KeptBrains>,
    mut shots: ResMut<Shots>,
    mut events: ResMut<Events>,
    mut hud: ResMut<Hud>,
    // the match's clock, which stops with the match (G9), rather than the process's
    clock: Res<WorldClock>,
    ruby: Res<RubyDir>,
    mut assets: ResMut<Assets<MrbAsset>>,
    server: Res<AssetServer>,
) {
    let positions: Vec<(Entity, usize, Vec2, f32)> = robots
        .iter()
        .map(|(e, r, t)| (e, r.team, t.translation.truncate(), r.hp))
        .collect();
    // what a radar can see of a robot: where it is, where it is going, where it faces
    let seen: Vec<(Entity, usize, f32, Vec2, Vec2, f32, f32)> = robots
        .iter()
        .map(|(e, r, t)| (e, r.team, r.hp, t.translation.truncate(), r.velocity, r.heading, r.turret))
        .collect();
    for request in world.take_requests() {
        // asked just before a restart took its entity away: nobody is waiting for the answer
        if request.entity.is_some_and(|e| commands.get_entity(e).is_err()) {
            world.answer(&request, Answer::Nil);
            continue;
        }
        // what the match asks for. It has no entity of its own: it is the game, not a thing in it
        match request.kind.as_str() {
            // `match "…", noise: 0.3, seed: 7`: how noisy this match is, and its dice
            "rules" => {
                rules.noise = request.num_or(0, 0.0).clamp(0.0, 1.0) as f32;
                let seed = request.num_or(1, -1.0);
                rules.dice = if seed >= 0.0 {
                    seed as u64
                } else {
                    platform::clock_seed()
                };
                world.answer(&request, Answer::Num(rules.noise as f64));
                continue;
            }
            "clock" => {
                world.answer(&request, Answer::Num(clock.0 as f64));
                continue;
            }
            "spawn" => {
                let file = request.text(0).unwrap_or("scout").to_string();
                let team_name = request.text(1).unwrap_or("red").to_string();
                let team = TEAMS.iter().position(|(n, _, _)| *n == team_name).unwrap_or(0);
                let at = Vec2::new(request.num_or(2, 0.0) as f32, request.num_or(3, 0.0) as f32);
                let answer =
                    match spawn_robot(&mut commands, &ruby.0, &mut assets, &server, &mut shots, &kept, &file, team, at) {
                        Some(e) => Answer::Num(e.to_bits() as f64),
                        None => Answer::Nil,
                    };
                world.answer(&request, answer);
                continue;
            }
            "board" => {
                let rows = positions
                    .iter()
                    .map(|(e, team, at, hp)| {
                        vec![e.to_bits() as f64, *team as f64, *hp as f64, at.x as f64, at.y as f64]
                    })
                    .collect();
                world.answer(&request, Answer::Rows(rows));
                continue;
            }
            "events" => {
                let rows = std::mem::take(&mut events.0).into_iter().map(|e| e.to_vec()).collect();
                world.answer(&request, Answer::Rows(rows));
                continue;
            }
            // `shrink(crates)`: every wall moves in by that many crates. The crate size stays the
            // same, so the wall is still a whole number of crates and the corners still meet
            "shrink" => {
                let crates = request.num_or(0, 1.0).max(0.0).round() as i32;
                let (count, step) = wall_layout(arena.0);
                let smaller = (count - 2 * crates).max(MIN_CRATES.min(count));
                let half = step * smaller as f32 / 2.0;
                if half < arena.0 - 0.01 {
                    arena.0 = half;
                    hud.line = format!("the walls close in: {smaller} crates wide");
                }
                world.answer(&request, Answer::Num(arena.0 as f64));
                continue;
            }
            "win" => {
                let team = request.text(0).unwrap_or("none").to_string();
                hud.line =
                    if team == "none" { "a draw".into() } else { format!("winner: {team}") };
                world.answer(&request, Answer::Bool(true));
                continue;
            }
            _ => {}
        }
        let Some(me) = request.entity else {
            world.answer(&request, Answer::Nil);
            continue;
        };
        let Ok((_, mut robot, transform)) = robots.get_mut(me) else {
            world.answer(&request, Answer::Nil);
            continue;
        };
        let at = transform.translation.truncate();
        let answer = match request.kind.as_str() {
            // for `srand`: a robot's own dice, rolled from the match's
            "seed" => Answer::Num((rules.roll() * 1_000_000.0).floor() as f64),
            "status" => Answer::List(status_row(&robot, at, arena.0, clock.0)),
            // the robot itself first, then every other robot still running within range, read
            // through the match's noise: the further away, the less exact
            "radar" => {
                let range = request.num_or(0, 60.0) as f32;
                let mut rows = vec![status_row(&robot, at, arena.0, clock.0)];
                for (other, team, hp, pos, vel, heading, turret) in &seen {
                    let dist = pos.distance(at);
                    if *other == me || *hp <= 0.0 || dist > range {
                        continue;
                    }
                    let blur = rules.noise * dist * 0.05;
                    let pos = *pos + Vec2::new(rules.wobble(), rules.wobble()) * blur;
                    let vel = *vel + Vec2::new(rules.wobble(), rules.wobble()) * rules.noise * 2.0;
                    let off = pos - at;
                    rows.push(vec![
                        other.to_bits() as f64,
                        *team as f64,
                        *hp as f64,
                        pos.x as f64,
                        pos.y as f64,
                        vel.x as f64,
                        vel.y as f64,
                        *heading as f64,
                        *turret as f64,
                        off.length() as f64,
                        off.y.atan2(off.x) as f64,
                    ]);
                }
                Answer::Rows(rows)
            }
            // shots from other teams within range: where they are and where they are going
            "incoming" => {
                let range = request.num_or(0, 25.0) as f32;
                let mut rows: Vec<Vec<f64>> = bullets
                    .iter()
                    .filter(|(b, _)| b.team != robot.team)
                    .map(|(b, t)| (b, t.translation.truncate()))
                    .filter(|(_, p)| p.distance(at) <= range)
                    .map(|(b, p)| {
                        let p = p + Vec2::new(rules.wobble(), rules.wobble()) * rules.noise * 1.5;
                        vec![p.x as f64, p.y as f64, b.velocity.x as f64, b.velocity.y as f64, p.distance(at) as f64]
                    })
                    .collect();
                rows.sort_by(|a, b| a[4].total_cmp(&b[4]));
                Answer::Rows(rows)
            }
            // `act(throttle, turn, aim, power)`: the controls, any of them UNSET to leave alone.
            // Answers [fired (1 or 0), energy, cooldown]
            "act" => {
                let set = |i: usize| request.num(i).filter(|v| *v > UNSET + 1.0);
                if robot.hp <= 0.0 {
                    Answer::List(vec![0.0, 0.0, 0.0])
                } else {
                    if let Some(t) = set(0) {
                        robot.throttle = (t as f32).clamp(-1.0, 1.0);
                    }
                    if let Some(t) = set(1) {
                        robot.turn = (t as f32).clamp(-1.0, 1.0);
                    }
                    if let Some(a) = set(2) {
                        robot.turret_target = a as f32;
                    }
                    let mut fired = false;
                    if let Some(power) = set(3).filter(|p| *p > 0.0) {
                        let power = (power as f32).clamp(0.2, 1.0);
                        let cost = FIRE_COST * (0.25 + power);
                        if robot.cooldown <= 0.0 && robot.energy >= cost {
                            robot.energy -= cost;
                            robot.cooldown = COOLDOWN_MIN + (COOLDOWN_MAX - COOLDOWN_MIN) * power;
                            let spread = (BASE_SPREAD + rules.noise * 0.1) * rules.wobble();
                            let angle = robot.turret + spread;
                            let dir = Vec2::from_angle(angle);
                            let speed = BULLET_SPEED_FAST + (BULLET_SPEED_SLOW - BULLET_SPEED_FAST) * power;
                            let damage = BULLET_DAMAGE_MIN + (BULLET_DAMAGE_MAX - BULLET_DAMAGE_MIN) * power;
                            let image = shots.image_for(me);
                            let size = 0.7 + 0.6 * power;
                            commands.spawn((
                                Bullet { velocity: dir * speed, owner: me, team: robot.team, damage, life: 2.5 },
                                Sprite { image, custom_size: Some(Vec2::new(size * 0.55, size * 1.3)), ..default() },
                                Transform::from_xyz(at.x + dir.x * 2.8, at.y + dir.y * 2.8, 2.0)
                                    .with_rotation(Quat::from_rotation_z(angle - std::f32::consts::FRAC_PI_2)),
                            ));
                            fired = true;
                        }
                    }
                    Answer::List(vec![if fired { 1.0 } else { 0.0 }, robot.energy as f64, robot.cooldown as f64])
                }
            }
            // `on(:hit) { … }` telling the game what its tasks are doing, so the scoreboard
            // can mark it. The prelude asks and does not wait for the answer: a question nobody
            // pops is a command, and a handler that parked for a frame here would not be one
            "handler" => {
                match request.text(0).unwrap_or("") {
                    "ready" => robot.handlers = request.num_or(1, 0.0).max(0.0) as u32,
                    "begin" => {
                        robot.handler_running += 1;
                        robot.handler_runs += 1;
                    }
                    "end" => robot.handler_running = robot.handler_running.saturating_sub(1),
                    // the handler task itself is over: its subscription was closed
                    "off" => robot.handler_off += 1,
                    other => warn!("unknown handler state {other:?}"),
                }
                Answer::Nil
            }
            "arena" => Answer::Num(arena.0 as f64),
            other => {
                warn!("unknown request {other:?}");
                Answer::Nil
            }
        };
        world.answer(&request, answer);
    }
}

/// What a robot knows about itself:
/// [x, y, hp, team, heading, speed, turret, energy, cooldown, arena, time].
fn status_row(robot: &Robot, at: Vec2, arena: f32, now: f32) -> Vec<f64> {
    vec![
        at.x as f64,
        at.y as f64,
        robot.hp as f64,
        robot.team as f64,
        robot.heading as f64,
        robot.velocity.dot(Vec2::from_angle(robot.heading)) as f64,
        robot.turret as f64,
        robot.energy as f64,
        robot.cooldown as f64,
        arena as f64,
        now as f64,
    ]
}

fn move_robots(time: Res<Time>, arena: Res<ArenaSize>, mut robots: Query<(&mut Robot, &mut Transform)>) {
    use std::f32::consts::{PI, TAU};
    let dt = time.delta_secs();
    for (mut robot, mut transform) in &mut robots {
        robot.cooldown = (robot.cooldown - dt).max(0.0);
        if robot.hp <= 0.0 {
            robot.velocity = Vec2::ZERO;
            continue;
        }
        // driving costs energy; an empty tank still crawls, at a third of the speed
        let tired = if robot.energy > 0.0 { 1.0 } else { 0.35 };
        robot.energy = (robot.energy + (ENERGY_REGEN - DRIVE_COST * robot.throttle.abs()) * dt).clamp(0.0, ENERGY_MAX);

        robot.heading = (robot.heading + robot.turn * TURN_RATE * dt).rem_euclid(TAU);
        let top = if robot.throttle >= 0.0 { MAX_SPEED } else { REVERSE_SPEED };
        let wanted = Vec2::from_angle(robot.heading) * robot.throttle * top * tired;
        // a tank takes a moment to get going and to stop: about a fifth of a second
        let grip = 1.0 - (-dt / 0.2).exp();
        robot.velocity = robot.velocity.lerp(wanted, grip);

        // the turret turns towards where it was told, the short way round, at its own rate
        let diff = (robot.turret_target - robot.turret + PI).rem_euclid(TAU) - PI;
        let step = diff.clamp(-TURRET_RATE * dt, TURRET_RATE * dt);
        robot.turret = (robot.turret + step).rem_euclid(TAU);

        let step = robot.velocity * dt;
        let limit = arena.0 - ROBOT_RADIUS;
        let x = transform.translation.x + step.x;
        let y = transform.translation.y + step.y;
        // a wall stops the part of the motion that goes into it
        if x.abs() > limit {
            robot.velocity.x = 0.0;
        }
        if y.abs() > limit {
            robot.velocity.y = 0.0;
        }
        transform.translation.x = x.clamp(-limit, limit);
        transform.translation.y = y.clamp(-limit, limit);
        // Kenney's tanks are drawn pointing up, so the sprite is a quarter turn behind the heading
        transform.rotation = Quat::from_rotation_z(robot.heading - std::f32::consts::FRAC_PI_2);
    }
}

/// Tanks do not drive through each other: two that overlap are pushed apart, half each (a wreck
/// does not move, so the one still running takes all of it), and lose the speed that went into
/// the other.
fn separate_robots(arena: Res<ArenaSize>, mut robots: Query<(&mut Robot, &mut Transform)>) {
    let limit = arena.0 - ROBOT_RADIUS;
    let mut pairs = robots.iter_combinations_mut();
    while let Some([(mut a, mut ta), (mut b, mut tb)]) = pairs.fetch_next() {
        let offset = tb.translation.truncate() - ta.translation.truncate();
        let dist = offset.length();
        let overlap = ROBOT_RADIUS * 2.0 - dist;
        if overlap <= 0.0 {
            continue;
        }
        let normal = if dist > 1e-4 { offset / dist } else { Vec2::X };
        let (share_a, share_b) = match (a.hp > 0.0, b.hp > 0.0) {
            (true, true) => (0.5, 0.5),
            (true, false) => (1.0, 0.0),
            (false, true) => (0.0, 1.0),
            (false, false) => continue,
        };
        ta.translation -= (normal * overlap * share_a).extend(0.0);
        tb.translation += (normal * overlap * share_b).extend(0.0);
        for t in [&mut ta, &mut tb] {
            t.translation.x = t.translation.x.clamp(-limit, limit);
            t.translation.y = t.translation.y.clamp(-limit, limit);
        }
        let into_b = a.velocity.dot(normal).max(0.0);
        a.velocity -= normal * into_b;
        let into_a = b.velocity.dot(-normal).max(0.0);
        b.velocity += normal * into_a;
    }
}

fn move_bullets(
    time: Res<Time>,
    mut commands: Commands,
    mut events: ResMut<Events>,
    mut scripts: ResMut<ScriptWorld>,
    mut test: Option<ResMut<HandlerTest>>,
    server: Res<AssetServer>,
    arena: Res<ArenaSize>,
    mut bullets: Query<(Entity, &mut Bullet, &mut Transform)>,
    mut robots: Query<(Entity, &mut Robot, &Transform), Without<Bullet>>,
    tasks: Query<&ScriptTask>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    // who is whom, read before the loop takes a robot mutably: what a hit tells the brain is the
    // attacker's name (`2 red/hunter`), not its entity — a name is what a script can read, log
    // and compare, and the radar already hands out ids for the rest
    let names: Vec<(Entity, String)> = robots.iter().map(|(e, r, _)| (e, r.name.clone())).collect();
    for (entity, mut bullet, mut transform) in &mut bullets {
        bullet.life -= dt;
        transform.translation += (bullet.velocity * dt).extend(0.0);
        let at = transform.translation.truncate();
        if bullet.life <= 0.0 || at.x.abs() > arena.0 || at.y.abs() > arena.0 {
            commands.entity(entity).despawn();
            continue;
        }
        for (target, mut robot, t) in &mut robots {
            // no friendly fire: a team's shots pass over its own robots
            if target == bullet.owner || robot.team == bullet.team || robot.hp <= 0.0 {
                continue;
            }
            if t.translation.truncate().distance(at) <= ROBOT_RADIUS {
                let was_alive = robot.hp > 0.0;
                robot.hp -= bullet.damage;
                commands.entity(entity).despawn();
                let big = was_alive && robot.hp <= 0.0;
                let size = if big { ROBOT_RADIUS * 3.5 } else { ROBOT_RADIUS * (0.6 + bullet.damage / 16.0) };
                let image = if big { "sprites/explosion3.png" } else { "sprites/explosion1.png" };
                commands.spawn((
                    Blast { life: 0.0, span: if big { 0.7 } else { 0.25 }, size },
                    Sprite { image: server.load(image), custom_size: Some(Vec2::splat(size * 0.4)), ..default() },
                    Transform::from_xyz(at.x, at.y, 3.0),
                ));
                if big {
                    // kind 0: a robot is down. The match reads these and decides what they mean
                    events.0.push([0.0, target.to_bits() as f64, bullet.owner.to_bits() as f64]);
                }
                // and the robot's own brain hears it at once, in whatever task is waiting on
                // `Rubevy.subscribe(:hit)` — the payload is a String and a number, which
                // `Answer::List` (numbers only) cannot carry, so it is built inside the VM
                let by = names
                    .iter()
                    .find(|(e, _)| *e == bullet.owner)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| "?".into());
                let damage = bullet.damage as f64;
                scripts.publish_value(Some(target), "hit", move |vm| {
                    let by = vm.str_new(by.as_bytes());
                    vm.ary_new(vec![by, Value::Float(damage)])
                });
                // The handler check watches the hits taken by a robot that has a handler, is still
                // standing (a wreck cannot turn, so a fatal hit proves nothing) and is not
                // already in the middle of one: the handlers of an event share one task, so a
                // robot hit again mid-swerve gets its second handler when the first has finished.
                if let Some(test) = test.as_mut() {
                    if robot.handlers > 0 && robot.hp > 0.0 && robot.handler_running == 0 {
                        test.watching.push(WatchedHit {
                            robot: target,
                            at: now,
                            heading: robot.heading,
                            runs: robot.handler_runs,
                            peak: 0.0,
                            task: tasks.get(target).ok().map(|t| t.task()),
                        });
                    }
                }
                break;
            }
        }
    }
}

/// A blast grows and fades over its span, and is gone after it.
fn fade_blasts(
    time: Res<Time>,
    mut commands: Commands,
    mut blasts: Query<(Entity, &mut Blast, &mut Sprite, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut blast, mut sprite, mut transform) in &mut blasts {
        blast.life += dt;
        let t = (blast.life / blast.span).clamp(0.0, 1.0);
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        sprite.custom_size = Some(Vec2::splat(blast.size * (0.4 + t)));
        sprite.color = Color::srgba(1.0, 1.0, 1.0, 1.0 - t);
        transform.rotation = Quat::from_rotation_z(t * 1.5);
    }
}

/// Saving a robot's file starts that robot again with the new brain, without restarting the game.
fn reload_changed(
    watch: Option<Res<Watch>>,
    ruby: Res<RubyDir>,
    mut commands: Commands,
    mut assets: ResMut<Assets<MrbAsset>>,
    mut robots: Query<(Entity, &mut Robot)>,
    mut hud: ResMut<Hud>,
    editor: Option<ResMut<Editor>>,
    watched: Option<Res<Watched>>,
) {
    let Some(watch) = watch else { return };
    let mut editor = editor;
    let mut fresh: Vec<(Entity, String)> = Vec::new();
    for path in watch.changed() {
        for (entity, robot) in robots.iter() {
            let reload = path == robot.file || path.ends_with("prelude.rb");
            // a brain applied from the editor is the robot's until it is saved or reverted
            if !reload || robot.brain.is_some() {
                continue;
            }
            // the editor showing this robot, with nothing typed, follows the file
            if let (Some(editor), Some(watched)) = (editor.as_mut(), watched.as_ref()) {
                if watched.entity == Some(entity) && !editor.changed() {
                    if let Ok(source) = platform::read(&robot.file) {
                        editor.reset_to(source, "the file changed");
                    }
                }
            }
            let Some((handle, _)) = compile(&ruby.0, &robot.file, &mut assets) else {
                hud.line = format!("{}: compile error (see the log)", robot.name);
                continue;
            };
            fresh.push((entity, platform::read(&robot.file).unwrap_or_default()));
            // the same three lines the editor's Apply goes through, and for the same reason: this
            // is a robot starting over with another brain (S3, the leftover S2 named)
            restart(&mut commands, entity, &robot.name, handle);
            hud.line = format!("{} reloaded", robot.name);
            info!("reloaded {}", robot.name);
        }
    }
    for (entity, source) in fresh {
        if let Ok((_, mut r)) = robots.get_mut(entity) {
            r.source = source;
            r.heat.clear();
            // the new brain announces its own handlers; the old ones are gone with its task
            r.handlers = 0;
            r.handler_running = 0;
        }
    }
}

fn report_ended(mut ended: MessageReader<ScriptEnded>, mut hud: ResMut<Hud>) {
    for e in ended.read() {
        info!("script on {:?} ended: {:?} {}", e.entity, e.status, e.value);
        if matches!(e.status, rubevy::ScriptStatus::Failed) {
            hud.line = format!("script failed: {}", e.value);
        }
    }
}

fn update_hud(
    world: Res<ScriptWorld>,
    time: Res<Time>,
    mut robots: Query<(&mut Robot, Option<&ScriptTask>, &mut ScriptPanel)>,
) {
    // about a second of memory: a line the brain keeps coming back to stays lit
    let keep = (-time.delta_secs() / 0.8).exp();
    let mut alive = 0;
    for (mut robot, script, mut panel) in &mut robots {
        panel.name = robot.name.clone();
        panel.state = if robot.hp > 0.0 {
            alive += 1;
            format!("hp {:>3}", robot.hp.max(0.0) as i32)
        } else {
            "down".into()
        };
        // what the brain spent on this frame, and the line it is standing on — the VM knows both,
        // for a parked task as well as a running one
        let Some(script) = script else { continue };
        let stats = world.stats(script);
        panel.spent = stats.instructions.saturating_sub(robot.last_instructions);
        // a brain that thinks for a frame spends tens to hundreds; the bar fills as one
        // approaches a timeslice's worth, which is where it starts costing the other robot
        panel.budget = 3_000;
        robot.last_instructions = stats.instructions;
        panel.at = match stats.location {
            Some((file, line)) if line > robot.prelude_lines => {
                format!("{file}:{}", line - robot.prelude_lines)
            }
            Some((_, line)) => format!("prelude.rb:{line}"),
            None => String::new(),
        };
        // the line of the robot's own file that is waiting, even when the brain is standing
        // inside the DSL (which it is whenever it waits for a scan)
        robot.own_line = stats
            .frames
            .iter()
            .find(|(_, line)| *line > robot.prelude_lines)
            .map(|(_, line)| line - robot.prelude_lines);
        let spent = panel.spent as f32;
        robot.cpu = robot.cpu * keep + spent * (1.0 - keep);
        for h in robot.heat.iter_mut() {
            *h *= keep;
        }
        if let Some(line) = robot.own_line {
            let i = line as usize - 1;
            if robot.heat.len() <= i {
                robot.heat.resize(i + 1, 0.0);
            }
            robot.heat[i] += 1.0 - keep;
        }
    }
    let _ = alive;
}
