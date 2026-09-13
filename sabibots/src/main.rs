//! SabiRuby Battle — the robots' brains are Ruby, running as tasks in one VM.
//!
//! Each robot is an entity with a `Script`: `ruby/prelude.rb` (the DSL) followed by
//! `ruby/robots/<name>.rb` (the robot itself). The script asks the game for what it needs
//! (`me`, `scan`, `thrust`, `fire`) and is parked until the game answers, so a robot that is
//! thinking costs nothing and a robot that never yields is preempted at its timeslice.
//!
//!     cargo run -p sabibots
//!
//! Save a robot file while it runs and that robot starts again with the new brain.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use rubevy::{Answer, MrbAsset, RubevyPlugin, Script, ScriptEnded, ScriptTask, ScriptWorld};
use rubevy_arena::{ArenaPlugin, ArenaSize, Editor, EditorAction, EditorPlugin, Hud, ScriptPanel, Watch};

const ROBOT_RADIUS: f32 = 1.6;
const MAX_SPEED: f32 = 14.0;
const BULLET_SPEED: f32 = 40.0;
const COOLDOWN: f32 = 0.35;
const BULLET_DAMAGE: f32 = 7.0;

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
    /// what its brain had spent as of the last frame, so the HUD can show this frame's share
    last_instructions: u64,
    /// lines the prelude adds in front of the robot's own file
    prelude_lines: u32,
    /// the line of its own file the brain is inside, however deep in the DSL it stands
    own_line: Option<u32>,
    /// where the hull points, kept from the last direction it moved or fired in
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
}

/// A robot that has lost: its hull is swapped for a grey one, once.
#[derive(Component)]
struct Downed;

fn gray_out_downed(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut robots: Query<(Entity, &Robot, &mut Sprite), Without<Downed>>,
) {
    for (entity, robot, mut sprite) in &mut robots {
        if robot.hp > 0.0 {
            continue;
        }
        // Kenney's dark hull, dimmed a little more: grey whatever team it was on
        sprite.image = server.load("sprites/tankBody_dark_outline.png");
        sprite.color = Color::srgb(0.7, 0.7, 0.7);
        commands.entity(entity).insert(Downed);
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
    life: f32,
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

fn main() {
    let ruby = ruby_dir();
    // `--headless N`: no window, N seconds, the match reported on stdout. It is how the game is
    // tested where there is no GPU, and it runs exactly the same systems as the windowed one.
    let args: Vec<String> = std::env::args().collect();
    let headless = args.iter().position(|a| a == "--headless").map(|i| {
        args.get(i + 1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(10.0)
    });
    // `--shot FILE [SECONDS]`: a window, a picture of it, and out. For checking the HUD where
    // the window itself cannot be looked at.
    let shot = args.iter().position(|a| a == "--shot").map(|i| {
        (
            args.get(i + 1).cloned().unwrap_or_else(|| "shot.png".into()),
            args.get(i + 2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(3.0),
        )
    });

    let mut app = App::new();
    match headless {
        Some(seconds) => {
            app.add_plugins((
                MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    std::time::Duration::from_secs_f32(1.0 / 60.0),
                )),
                bevy::log::LogPlugin { filter: "info,bevy_asset=off".into(), ..default() },
                bevy::asset::AssetPlugin {
                    file_path: assets_dir().to_string_lossy().into_owned(),
                    ..default()
                },
                RubevyPlugin::default(),
            ))
            // no renderer here, but the same systems run and they load sprites
            .init_asset::<Image>()
            .insert_resource(ArenaSize::default())
            .insert_resource(Headless { until: seconds })
            .init_resource::<Hud>()
            .add_systems(Update, stop_when_over);
        }
        None => {
            app.add_plugins((
                DefaultPlugins
                    .set(AssetPlugin {
                        file_path: assets_dir().to_string_lossy().into_owned(),
                        ..default()
                    })
                    .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "SabiRuby Battle".into(),
                        resolution: (1600u32, 900u32).into(),
                        ..default()
                    }),
                    ..default()
                }),
                ArenaPlugin::default(),
                EditorPlugin,
                RubevyPlugin::default(),
            ))
            .init_resource::<Watched>()
            .init_resource::<Hud>()
            .add_systems(
                Update,
                (choose_watched, show_code, do_editor_actions, spawn_nameplates, follow_nameplates, spawn_life_bars, follow_life_bars)
                    .chain(),
            )
            .add_systems(bevy_egui::EguiPrimaryContextPass, draw_scoreboard);
        }
    }
    app.insert_resource(RubyDir(ruby.clone()))
        .init_resource::<Shots>()
        .init_resource::<Events>()
        .add_systems(Startup, (spawn_match, spawn_arena))
        .add_systems(
            Update,
            (answer_requests, move_robots, move_bullets, gray_out_downed, fade_blasts, rebuild_walls, reload_changed, report_ended, update_hud)
                .chain(),
        );
    if std::env::var("SABIBOTS_SELFTEST").is_ok() && headless.is_none() {
        app.insert_resource(SelfTest { at: 2.0, ..default() }).add_systems(Update, selftest);
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
) {
    use bevy_egui::egui;
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let mut rows: Vec<(Entity, &Robot)> = robots.iter().collect();
    rows.sort_by_key(|(_, r)| r.number);

    egui::Window::new("scoreboard")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::LEFT_TOP, [8.0, 8.0])
        .show(ctx, |ui| {
            // a scoreboard, not a document: nothing in it is text to select
            ui.style_mut().interaction.selectable_labels = false;
            if !hud.line.is_empty() {
                ui.label(egui::RichText::new(&hud.line).strong().size(16.0));
                ui.separator();
            }
            egui::Grid::new("robots").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label(egui::RichText::new("robot").weak());
                ui.label(egui::RichText::new("health").weak());
                ui.label(egui::RichText::new("thinking").weak())
                    .on_hover_text("instructions its Ruby runs per frame, averaged over about a second");
                ui.label(egui::RichText::new("mostly doing").weak())
                    .on_hover_text("the line of its file it has spent the most time on lately, not counting sleep");
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
                    let name = egui::RichText::new(format!("{marker}{} {brain}{star}", robot.number)).color(team).strong();
                    // the name picks the robot, as its button in the editor does
                    if ui.add(egui::Label::new(name).sense(egui::Sense::click())).clicked() {
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

                    // a timeslice's worth of instructions fills the bar: the point where a brain
                    // starts taking turns away from the others
                    let cpu = (robot.cpu / 3000.0).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(cpu)
                            .fill(egui::Color32::from_rgb(90, 140, 210))
                            .desired_width(80.0)
                            .text(format!("{:.0}", robot.cpu)),
                    );

                    let doing = if down { String::new() } else { mostly_doing(robot) };
                    ui.label(egui::RichText::new(doing).monospace());
                    ui.end_row();
                }
            });
        });
}

/// The source line the brain has spent the most time on lately, trimmed — `strafe(target, 0.9)`,
/// `patrol`. Waiting in `sleep` is left out: a brain spends most of its time there, and "it is
/// sleeping" says nothing about what it is up to.
fn mostly_doing(robot: &Robot) -> String {
    let lines: Vec<&str> = robot.source.lines().collect();
    let Some((i, _)) = robot
        .heat
        .iter()
        .enumerate()
        .filter(|(i, h)| **h > 0.0 && !lines.get(*i).is_some_and(|l| l.trim_start().starts_with("sleep")))
        .max_by(|a, b| a.1.total_cmp(b.1))
    else {
        return String::new();
    };
    let line = robot.source.lines().nth(i).unwrap_or("").trim();
    let mut s: String = line.chars().take(26).collect();
    if line.chars().count() > 26 {
        s.push('…');
    }
    s
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
fn show_code(watched: Res<Watched>, mut editor: ResMut<Editor>, robots: Query<(Entity, &Robot, &ScriptPanel)>) {
    // the buttons along the top of the editor: every robot, by number, in its team's colour
    let mut choices: Vec<(usize, rubevy_arena::editor::EditorChoice)> = robots
        .iter()
        .map(|(e, r, _)| {
            let (cr, cg, cb) = TEAM_COLORS[r.team.min(TEAM_COLORS.len() - 1)];
            let brain = r.file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            (
                r.number,
                rubevy_arena::editor::EditorChoice {
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

    let Some(entity) = watched.entity else { return };
    let Ok((_, robot, script)) = robots.get(entity) else { return };
    // what the robot is running: its applied brain, or its file
    editor.show(entity.to_bits(), || {
        robot.brain.clone().unwrap_or_else(|| std::fs::read_to_string(&robot.file).unwrap_or_default())
    });
    editor.file = brain_name(robot);
    editor.in_memory = robot.brain.is_some();
    editor.label = robot.name.clone();
    editor.current = robot.own_line;
    editor.heat = robot.heat.clone();
    // where it stands when that is not this file: inside the DSL, waiting for an answer
    editor.elsewhere = script.at.starts_with("prelude").then(|| script.at.clone());
}

/// `SABIBOTS_SELFTEST=1`: drives the editor's buttons the way a click would (by setting
/// `Editor::action`) and checks what happened to the robots and to the file — the part of the
/// editor that cannot be clicked where there is no mouse. Logs `selftest:` lines and exits.
#[derive(Resource, Default)]
struct SelfTest {
    step: usize,
    at: f32,
    original: String,
}

fn selftest(
    time: Res<Time>,
    mut test: ResMut<SelfTest>,
    mut editor: ResMut<Editor>,
    mut watched: ResMut<Watched>,
    robots: Query<(Entity, &Robot)>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if now < test.at {
        return;
    }
    let by_number = |n: usize| robots.iter().find(|(_, r)| r.number == n);
    let ok = |cond: bool, what: &str| info!("selftest: {} {what}", if cond { "ok  " } else { "FAIL" });
    match test.step {
        0 => {
            // show robot 3 and type a different brain into the editor
            let Some((e, r)) = by_number(3) else { return };
            test.original = std::fs::read_to_string(&r.file).unwrap_or_default();
            watched.entity = Some(e);
            test.step = 1;
            test.at = now + 0.5;
        }
        1 => {
            editor.text = editor.text.replace("sleep 0.05", "sleep 0.5");
            ok(editor.changed(), "typing marks the text edited");
            editor.action = Some(EditorAction::Apply);
            test.step = 2;
            test.at = now + 0.5;
        }
        2 => {
            let (_, r3) = by_number(3).unwrap();
            let (_, r4) = by_number(4).unwrap();
            let on_disk = std::fs::read_to_string(&r3.file).unwrap_or_default();
            ok(r3.brain.as_deref().is_some_and(|b| b.contains("sleep 0.5")), "Apply gives robot 3 the edited brain");
            ok(r4.brain.is_none(), "Apply leaves robot 4 (same file) alone");
            ok(on_disk == test.original, "Apply does not touch the file");
            ok(!editor.changed(), "after Apply the text is what the robot runs");
            editor.action = Some(EditorAction::ApplyAll);
            test.step = 3;
            test.at = now + 0.5;
        }
        3 => {
            let (_, r4) = by_number(4).unwrap();
            let (_, r2) = by_number(2).unwrap();
            ok(r4.brain.is_some(), "Apply to all reaches robot 4 (same file)");
            ok(r2.brain.is_none(), "Apply to all leaves robot 2 (another file) alone");
            editor.action = Some(EditorAction::Revert);
            test.step = 4;
            test.at = now + 0.5;
        }
        4 => {
            let (_, r3) = by_number(3).unwrap();
            let (_, r4) = by_number(4).unwrap();
            ok(r3.brain.is_none(), "Revert puts robot 3 back on its file");
            ok(r4.brain.is_some(), "Revert is for the shown robot only: robot 4 keeps its brain");
            ok(editor.text == test.original, "Revert shows the file again");
            let on_disk = std::fs::read_to_string(&r3.file).unwrap_or_default();
            ok(on_disk == test.original, "nothing was written");
            exit.write(AppExit::Success);
            test.step = 5;
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
    robots: Query<(&Robot, &ScriptPanel, &Transform)>,
    mut exit: MessageWriter<AppExit>,
) {
    let over = hud.line.starts_with("winner") || time.elapsed_secs() >= headless.until;
    if !over {
        return;
    }
    for (robot, panel, t) in &robots {
        info!(
            "{:<8} hp {:>4}  at ({:>6.1}, {:>6.1})  {:>6} insn/frame  {}",
            robot.name,
            robot.hp.max(0.0) as i32,
            t.translation.x,
            t.translation.y,
            panel.spent,
            panel.at
        );
    }
    info!("{}", if hud.line.is_empty() { "time" } else { hud.line.as_str() });
    exit.write(AppExit::Success);
}

/// Where the sprites live. Bevy looks next to the executable by default, which is not where a
/// workspace puts them.
fn assets_dir() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    if here.is_dir() { here } else { PathBuf::from("sabibots/assets") }
}

fn ruby_dir() -> PathBuf {
    // run from the workspace root or from the crate; both find the Ruby
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("ruby");
    if here.is_dir() { here } else { PathBuf::from("sabibots/ruby") }
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
    let crate_: Handle<Image> = server.load("sprites/crateMetal.png");
    let step = 2.6;
    let count = (half * 2.0 / step).ceil() as i32;
    for i in 0..=count {
        let t = -half + step * i as f32;
        for (x, y) in [(t, half), (t, -half), (-half, t), (half, t)] {
            commands.spawn((
                Wall,
                Sprite { image: crate_.clone(), custom_size: Some(Vec2::splat(step)), ..default() },
                Transform::from_xyz(x, y, 0.0),
            ));
        }
    }
}

fn spawn_match(
    mut commands: Commands,
    ruby: Res<RubyDir>,
    mut assets: ResMut<Assets<MrbAsset>>,
) {
    // the match is a script like a robot is, at a higher priority: it spawns the field and
    // decides when the fight is over, and the game only does what it is told
    let path = ruby.0.join("matches").join("training.rb");
    let Some((handle, _)) = compile_with(&ruby.0, "match_prelude.rb", &path, "run_match", &mut assets) else {
        return;
    };
    commands.spawn((Script::new(handle).with_name("match").with_priority(10),));
}

/// `Rubevy.ask("spawn", file, team, x, y)` from the match: a robot with its own brain.
fn spawn_robot(
    commands: &mut Commands,
    ruby: &Path,
    assets: &mut Assets<MrbAsset>,
    server: &AssetServer,
    shots: &mut Shots,
    file: &str,
    team: usize,
    at: Vec2,
) -> Option<Entity> {
    let path = ruby.join("robots").join(format!("{file}.rb"));
    let (handle, prelude_lines) = compile(ruby, &path, assets)?;
    let (team_name, hull, bullet) = TEAMS[team.min(TEAMS.len() - 1)];
    let number = shots.0.len() + 1; // one entry per robot spawned so far
    let name = format!("{number} {team_name}/{file}");
    let source = std::fs::read_to_string(&path).unwrap_or_default();
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
                last_instructions: 0,
                prelude_lines,
                own_line: None,
                heading: 0.0,
                brain: None,
                source,
                heat: Vec::new(),
                cpu: 0.0,
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
    let body = match std::fs::read_to_string(robot) {
        Ok(b) => b,
        Err(e) => {
            error!("{robot:?}: {e}");
            return None;
        }
    };
    let name = robot.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    compile_text(ruby, prelude_file, &name, &body, start, assets).map_err(|e| error!("{e}")).ok()
}

/// The same, from text rather than a file: what the editor applies. The error is the compiler's
/// message, for the editor to show.
fn compile_text(
    ruby: &Path,
    prelude_file: &str,
    name: &str,
    body: &str,
    start: &str,
    assets: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    let prelude = std::fs::read_to_string(ruby.join(prelude_file)).map_err(|e| format!("{prelude_file}: {e}"))?;
    let src = format!("{prelude}\n# ---- {name} ----\n{body}\n{start}\n");
    let opts = sabiruby_compiler::Options { filename: name.to_string(), debug_info: true, ..Default::default() };
    // the prelude sits in front, so a line in the compiled program is `prelude_lines` further
    // down than the same line of the robot's own file
    let prelude_lines = prelude.lines().count() as u32 + 2;
    sabiruby_compiler::compile(src.as_bytes(), &opts)
        .map(|bytes| (assets.add(MrbAsset { bytes }), prelude_lines))
        .map_err(|e| format!("{name}: {e}"))
}

/// Starts a robot over with another brain: dropping its task and giving it a new `Script`.
fn restart(commands: &mut Commands, entity: Entity, name: &str, handle: Handle<MrbAsset>) {
    commands
        .entity(entity)
        .remove::<rubevy::ScriptTask>()
        .remove::<rubevy::ScriptDone>()
        .insert(Script::new(handle).with_name(name).with_priority(128));
}

fn brain_name(robot: &Robot) -> String {
    robot.file.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// What the editor's buttons asked for. Nothing here writes a file except `Save`: applying runs
/// the text in memory, so trying something in a match does not rewrite the project.
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
            hud.line = format!("new brain: {what}");
        }
        EditorAction::Save => {
            if let Err(e) = std::fs::write(&file, &text) {
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
            let Ok(source) = std::fs::read_to_string(&file) else {
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
    let half = arena.0;
    let crate_: Handle<Image> = server.load("sprites/crateMetal.png");
    let step = 2.6;
    let count = (half * 2.0 / step).ceil() as i32;
    for i in 0..=count {
        let t = -half + step * i as f32;
        for (x, y) in [(t, half), (t, -half), (-half, t), (half, t)] {
            commands.spawn((
                Wall,
                Sprite { image: crate_.clone(), custom_size: Some(Vec2::splat(step)), ..default() },
                Transform::from_xyz(x, y, 0.0),
            ));
        }
    }
}

/// The game's side of `Rubevy.ask`: everything a robot can see or do.
fn answer_requests(
    mut world: ResMut<ScriptWorld>,
    mut robots: Query<(Entity, &mut Robot, &Transform)>,
    mut commands: Commands,
    mut arena: ResMut<ArenaSize>,
    mut shots: ResMut<Shots>,
    mut events: ResMut<Events>,
    mut hud: ResMut<Hud>,
    time: Res<Time>,
    ruby: Res<RubyDir>,
    mut assets: ResMut<Assets<MrbAsset>>,
    server: Res<AssetServer>,
) {
    let positions: Vec<(Entity, usize, Vec2, f32)> = robots
        .iter()
        .map(|(e, r, t)| (e, r.team, t.translation.truncate(), r.hp))
        .collect();
    for request in world.take_requests() {
        // what the match asks for. It has no entity of its own: it is the game, not a thing in it
        match request.kind.as_str() {
            "clock" => {
                world.answer(&request, Answer::Num(time.elapsed_secs() as f64));
                continue;
            }
            "spawn" => {
                let file = request.text(0).unwrap_or("scout").to_string();
                let team_name = request.text(1).unwrap_or("red").to_string();
                let team = TEAMS.iter().position(|(n, _, _)| *n == team_name).unwrap_or(0);
                let at = Vec2::new(request.num_or(2, 0.0) as f32, request.num_or(3, 0.0) as f32);
                let answer =
                    match spawn_robot(&mut commands, &ruby.0, &mut assets, &server, &mut shots, &file, team, at) {
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
            "shrink" => {
                arena.0 = (arena.0 - request.num_or(0, 0.0) as f32).max(9.0);
                hud.line = format!("the walls close in: {:.0}", arena.0);
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
            "me" => Answer::List(vec![at.x as f64, at.y as f64, robot.hp as f64, robot.team as f64]),
            "scan" => {
                let range = request.num_or(0, 40.0) as f32;
                match nearest(me, robot.team, at, &positions, range) {
                    Some((d, dist)) => Answer::List(vec![d.x as f64, d.y as f64, dist as f64]),
                    None => Answer::Nil,
                }
            }
            "thrust" => {
                let (dx, dy) = (request.num_or(0, 0.0) as f32, request.num_or(1, 0.0) as f32);
                let v = Vec2::new(dx, dy).clamp_length_max(1.0) * MAX_SPEED;
                robot.velocity = v;
                Answer::Bool(true)
            }
            "fire" => {
                let dir = Vec2::new(request.num_or(0, 0.0) as f32, request.num_or(1, 0.0) as f32);
                if robot.cooldown > 0.0 || dir.length_squared() < 1e-6 || robot.hp <= 0.0 {
                    Answer::Bool(false)
                } else {
                    robot.cooldown = COOLDOWN;
                    let dir = dir.normalize();
                    robot.heading = dir.y.atan2(dir.x);
                    let image = shots.image_for(me);
                    commands.spawn((
                        Bullet { velocity: dir * BULLET_SPEED, owner: me, life: 2.0 },
                        Sprite { image, custom_size: Some(Vec2::new(0.7, 1.6)), ..default() },
                        Transform::from_xyz(
                            at.x + dir.x * ROBOT_RADIUS * 1.4,
                            at.y + dir.y * ROBOT_RADIUS * 1.4,
                            2.0,
                        )
                        .with_rotation(Quat::from_rotation_z(robot.heading - std::f32::consts::FRAC_PI_2)),
                    ));
                    Answer::Bool(true)
                }
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

/// The nearest living robot of another team, as an offset and a distance.
fn nearest(
    me: Entity,
    team: usize,
    at: Vec2,
    all: &[(Entity, usize, Vec2, f32)],
    range: f32,
) -> Option<(Vec2, f32)> {
    all.iter()
        .filter(|(e, t, _, hp)| *e != me && *t != team && *hp > 0.0)
        .map(|(_, _, p, _)| (*p - at, (*p - at).length()))
        .filter(|(_, d)| *d <= range)
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

fn move_robots(time: Res<Time>, arena: Res<ArenaSize>, mut robots: Query<(&mut Robot, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut robot, mut transform) in &mut robots {
        robot.cooldown = (robot.cooldown - dt).max(0.0);
        if robot.hp <= 0.0 {
            robot.velocity = Vec2::ZERO;
            continue;
        }
        let step = robot.velocity * dt;
        let limit = arena.0 - ROBOT_RADIUS;
        transform.translation.x = (transform.translation.x + step.x).clamp(-limit, limit);
        transform.translation.y = (transform.translation.y + step.y).clamp(-limit, limit);
        if robot.velocity.length_squared() > 0.5 {
            robot.heading = robot.velocity.y.atan2(robot.velocity.x);
        }
        // Kenney's tanks are drawn pointing up, so the sprite is a quarter turn behind the heading
        transform.rotation = Quat::from_rotation_z(robot.heading - std::f32::consts::FRAC_PI_2);
    }
}

fn move_bullets(
    time: Res<Time>,
    mut commands: Commands,
    mut events: ResMut<Events>,
    server: Res<AssetServer>,
    arena: Res<ArenaSize>,
    mut bullets: Query<(Entity, &mut Bullet, &mut Transform)>,
    mut robots: Query<(Entity, &mut Robot, &Transform), Without<Bullet>>,
) {
    let dt = time.delta_secs();
    for (entity, mut bullet, mut transform) in &mut bullets {
        bullet.life -= dt;
        transform.translation += (bullet.velocity * dt).extend(0.0);
        let at = transform.translation.truncate();
        if bullet.life <= 0.0 || at.x.abs() > arena.0 || at.y.abs() > arena.0 {
            commands.entity(entity).despawn();
            continue;
        }
        for (target, mut robot, t) in &mut robots {
            if target == bullet.owner || robot.hp <= 0.0 {
                continue;
            }
            if t.translation.truncate().distance(at) <= ROBOT_RADIUS {
                let was_alive = robot.hp > 0.0;
                robot.hp -= BULLET_DAMAGE;
                commands.entity(entity).despawn();
                let big = was_alive && robot.hp <= 0.0;
                let size = if big { ROBOT_RADIUS * 3.5 } else { ROBOT_RADIUS * 1.2 };
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
                    if let Ok(source) = std::fs::read_to_string(&robot.file) {
                        editor.reset_to(source, "the file changed");
                    }
                }
            }
            let Some((handle, _)) = compile(&ruby.0, &robot.file, &mut assets) else {
                hud.line = format!("{}: compile error (see the log)", robot.name);
                continue;
            };
            fresh.push((entity, std::fs::read_to_string(&robot.file).unwrap_or_default()));
            // dropping ScriptTask and giving the entity a new Script starts it over
            commands
                .entity(entity)
                .remove::<rubevy::ScriptTask>()
                .remove::<rubevy::ScriptDone>()
                .insert(Script::new(handle).with_name(&robot.name).with_priority(128));
            hud.line = format!("{} reloaded", robot.name);
            info!("reloaded {}", robot.name);
        }
    }
    for (entity, source) in fresh {
        if let Ok((_, mut r)) = robots.get_mut(entity) {
            r.source = source;
            r.heat.clear();
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
