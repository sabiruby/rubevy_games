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
use rubevy_arena::{ArenaPlugin, ArenaSize, CodePanel, CodePanelPlugin, Hud, HudPlugin, ScriptPanel, Watch};

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
                        resolution: (900u32, 900u32).into(),
                        ..default()
                    }),
                    ..default()
                }),
                ArenaPlugin::default(),
                HudPlugin,
                CodePanelPlugin,
                RubevyPlugin::default(),
            ))
            .init_resource::<Watched>()
            .add_systems(Update, (choose_watched, show_code).chain());
        }
    }
    app.insert_resource(RubyDir(ruby.clone()))
        .init_resource::<Shots>()
        .init_resource::<Events>()
        .add_systems(Startup, (spawn_match, spawn_arena))
        .add_systems(
            Update,
            (answer_requests, move_robots, move_bullets, rebuild_walls, reload_changed, report_ended, update_hud)
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
    source: String,
    /// the file the source came from, so it is read again only when it changes
    from: Option<PathBuf>,
}

/// `1`, `2`, … pick a robot; `Tab` moves on; `C` hides the panel.
fn choose_watched(
    keys: Res<ButtonInput<KeyCode>>,
    robots: Query<Entity, With<Robot>>,
    mut watched: ResMut<Watched>,
    mut panel: ResMut<CodePanel>,
) {
    let all: Vec<Entity> = robots.iter().collect();
    if all.is_empty() {
        return;
    }
    if watched.entity.is_none() {
        watched.entity = Some(all[0]);
        panel.visible = true;
    }
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].into_iter().enumerate() {
        if keys.just_pressed(key) {
            if let Some(e) = all.get(i) {
                watched.entity = Some(*e);
                panel.visible = true;
            }
        }
    }
    if keys.just_pressed(KeyCode::Tab) {
        let at = all.iter().position(|e| Some(*e) == watched.entity).unwrap_or(0);
        watched.entity = Some(all[(at + 1) % all.len()]);
        panel.visible = true;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        panel.visible = !panel.visible;
    }
}

/// The panel follows the watched robot: its file, and the line its brain stands on.
fn show_code(
    mut watched: ResMut<Watched>,
    mut panel: ResMut<CodePanel>,
    robots: Query<(&Robot, &ScriptPanel)>,
) {
    let Some(entity) = watched.entity else { return };
    let Ok((robot, script)) = robots.get(entity) else { return };
    if watched.from.as_deref() != Some(robot.file.as_path()) {
        match std::fs::read_to_string(&robot.file) {
            Ok(text) => {
                watched.source = text;
                watched.from = Some(robot.file.clone());
            }
            Err(e) => {
                error!("{:?}: {e}", robot.file);
                return;
            }
        }
    }
    let name = robot.file.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let source = watched.source.clone();
    panel.show(name, &source);
    // `ScriptPanel::at` is `file:line` in the robot's own numbering, or in the prelude's
    panel.current = robot.own_line;
    // where it stands when that is not this file: inside the DSL, waiting for an answer
    panel.elsewhere = script.at.starts_with("prelude").then(|| script.at.clone());
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
    let n = (half * 2.0 / tile).ceil() as i32;
    for ix in 0..n {
        for iy in 0..n {
            let x = -half + tile * (ix as f32 + 0.5);
            let y = -half + tile * (iy as f32 + 0.5);
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
    let name = format!("{team_name}/{file}");
    let robot = commands
        .spawn((
            Robot {
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
                if was_alive && robot.hp <= 0.0 {
                    // kind 0: a robot is down. The match reads these and decides what they mean
                    events.0.push([0.0, target.to_bits() as f64, bullet.owner.to_bits() as f64]);
                }
                break;
            }
        }
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
    mut hud: ResMut<Hud>,
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
