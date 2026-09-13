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
use rubevy_arena::{ArenaPlugin, ArenaSize, Editor, EditorPlugin, Hud, HudPlugin, ScriptPanel, Watch};

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
                HudPlugin,
                EditorPlugin,
                RubevyPlugin::default(),
            ))
            .init_resource::<Watched>()
            .add_systems(Update, (choose_watched, show_code, spawn_nameplates, follow_nameplates).chain());
        }
    }
    app.insert_resource(RubyDir(ruby.clone()))
        .init_resource::<Shots>()
        .init_resource::<Events>()
        .add_systems(Startup, (spawn_match, spawn_arena))
        .add_systems(
            Update,
            (answer_requests, move_robots, move_bullets, fade_blasts, rebuild_walls, reload_changed, report_ended, update_hud)
                .chain(),
        );
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
        let brain = robot.file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let selected = watched.entity == Some(owner.robot);
        let down = robot.hp <= 0.0;
        **text = match (selected, down) {
            (_, true) => format!("{} {brain}  down", robot.number),
            (true, false) => format!("> {} {brain}  hp {} <", robot.number, robot.hp as i32),
            (false, false) => format!("{} {brain}  hp {}", robot.number, robot.hp as i32),
        };
        if !owner.shadow {
            let (r, g, b) = TEAM_COLORS[robot.team.min(TEAM_COLORS.len() - 1)];
            *color = TextColor(if selected {
                Color::srgb(1.0, 1.0, 0.55)
            } else if down {
                Color::srgba(r, g, b, 0.5)
            } else {
                Color::srgb(r, g, b)
            });
        }
        // the shadow sits a little down and right of the text it darkens
        let nudge = if owner.shadow { 0.12 } else { 0.0 };
        transform.translation.x = at.translation.x + nudge;
        transform.translation.y = at.translation.y + ROBOT_RADIUS * 2.0 - nudge;
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
                    label: format!("{} {brain}", r.number),
                    color: ((cr * 255.0) as u8, (cg * 255.0) as u8, (cb * 255.0) as u8),
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
    editor.show(&robot.file);
    editor.label = robot.name.clone();
    editor.current = robot.own_line;
    // where it stands when that is not this file: inside the DSL, waiting for an answer
    editor.elsewhere = script.at.starts_with("prelude").then(|| script.at.clone());
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
    let prelude = std::fs::read_to_string(ruby.join(prelude_file)).ok()?;
    let body = match std::fs::read_to_string(robot) {
        Ok(b) => b,
        Err(e) => {
            error!("{robot:?}: {e}");
            return None;
        }
    };
    let name = robot.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let src = format!("{prelude}\n# ---- {name} ----\n{body}\n{start}\n");
    let opts = sabiruby_compiler::Options { filename: name.clone(), debug_info: true, ..Default::default() };
    // the prelude sits in front, so a line in the compiled program is `prelude_lines` further
    // down than the same line of the robot's own file
    let prelude_lines = prelude.lines().count() as u32 + 2;
    match sabiruby_compiler::compile(src.as_bytes(), &opts) {
        Ok(bytes) => Some((assets.add(MrbAsset { bytes }), prelude_lines)),
        Err(e) => {
            error!("{name}: {e}");
            None
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
    robots: Query<(Entity, &Robot)>,
    mut hud: ResMut<Hud>,
) {
    let Some(watch) = watch else { return };
    for path in watch.changed() {
        for (entity, robot) in &robots {
            let reload = path == robot.file || path.ends_with("prelude.rb");
            if !reload {
                continue;
            }
            let Some((handle, _)) = compile(&ruby.0, &robot.file, &mut assets) else {
                hud.line = format!("{}: compile error (see the log)", robot.name);
                continue;
            };
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
    mut robots: Query<(&mut Robot, Option<&ScriptTask>, &mut ScriptPanel)>,
) {
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
    }
    let _ = alive;
}
