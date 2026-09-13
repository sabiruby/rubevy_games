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
use rubevy_arena::{ArenaPlugin, ArenaSize, Hud, HudPlugin, ScriptPanel, Watch};

const ROBOT_RADIUS: f32 = 1.6;
const MAX_SPEED: f32 = 14.0;
const BULLET_SPEED: f32 = 40.0;
const COOLDOWN: f32 = 0.35;
const BULLET_DAMAGE: f32 = 7.0;

/// A robot in the arena. The Ruby side never sees this; it asks for what it needs.
#[derive(Component, Debug)]
struct Robot {
    name: String,
    file: PathBuf,
    hp: f32,
    cooldown: f32,
    velocity: Vec2,
    /// what its brain had spent as of the last frame, so the HUD can show this frame's share
    last_instructions: u64,
    /// lines the prelude adds in front of the robot's own file
    prelude_lines: u32,
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
                bevy::log::LogPlugin::default(),
                bevy::asset::AssetPlugin::default(),
                RubevyPlugin::default(),
            ))
            .insert_resource(ArenaSize::default())
            .insert_resource(Headless { until: seconds })
            .init_resource::<Hud>()
            .add_systems(Update, stop_when_over);
        }
        None => {
            app.add_plugins((
                DefaultPlugins.set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "SabiRuby Battle".into(),
                        resolution: (900u32, 900u32).into(),
                        ..default()
                    }),
                    ..default()
                }),
                ArenaPlugin::default(),
                HudPlugin,
                RubevyPlugin::default(),
            ));
        }
    }
    app.insert_resource(RubyDir(ruby.clone()))
        .add_systems(Startup, (spawn_robots, spawn_arena))
        .add_systems(
            Update,
            (answer_requests, move_robots, move_bullets, reload_changed, report_ended, update_hud).chain(),
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

fn ruby_dir() -> PathBuf {
    // run from the workspace root or from the crate; both find the Ruby
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("ruby");
    if here.is_dir() { here } else { PathBuf::from("sabibots/ruby") }
}

/// Four thin bars: the walls the robots are clamped to.
fn spawn_arena(mut commands: Commands, arena: Res<ArenaSize>) {
    let (half, thick) = (arena.0, 0.6);
    let wall = Color::srgb(0.25, 0.26, 0.32);
    for (x, y, w, h) in [
        (0.0, half, half * 2.0 + thick, thick),
        (0.0, -half, half * 2.0 + thick, thick),
        (-half, 0.0, thick, half * 2.0 + thick),
        (half, 0.0, thick, half * 2.0 + thick),
    ] {
        commands.spawn((
            Sprite { color: wall, custom_size: Some(Vec2::new(w, h)), ..default() },
            Transform::from_xyz(x, y, 0.0),
        ));
    }
}

fn spawn_robots(mut commands: Commands, ruby: Res<RubyDir>, mut assets: ResMut<Assets<MrbAsset>>) {
    let starts = [("scout", Vec2::new(-18.0, -10.0), Color::srgb(0.35, 0.75, 1.0)),
                  ("hunter", Vec2::new(18.0, 12.0), Color::srgb(1.0, 0.45, 0.35))];
    for (i, (file, at, color)) in starts.into_iter().enumerate() {
        let path = ruby.0.join("robots").join(format!("{file}.rb"));
        let Some((handle, prelude_lines)) = compile(&ruby.0, &path, &mut assets) else { continue };
        commands.spawn((
            Robot { name: file.to_string(), file: path, hp: 100.0, cooldown: 0.0, velocity: Vec2::ZERO, last_instructions: 0, prelude_lines },
            Script::new(handle).with_name(file).with_priority(100 + i as u8),
            ScriptPanel { name: file.to_string(), ..default() },
            Sprite { color, custom_size: Some(Vec2::splat(ROBOT_RADIUS * 2.0)), ..default() },
            Transform::from_xyz(at.x, at.y, 1.0),
        ));
    }
}

/// The prelude and one robot's file, compiled to bytecode in process (the reference compiler),
/// so the game reads `.rb` and nothing has to be built ahead of time.
fn compile(ruby: &Path, robot: &Path, assets: &mut Assets<MrbAsset>) -> Option<(Handle<MrbAsset>, u32)> {
    let prelude = std::fs::read_to_string(ruby.join("prelude.rb")).ok()?;
    let body = match std::fs::read_to_string(robot) {
        Ok(b) => b,
        Err(e) => {
            error!("{robot:?}: {e}");
            return None;
        }
    };
    let name = robot.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let src = format!("{prelude}\n# ---- {name} ----\n{body}\nrun_robot\n");
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

/// The game's side of `Rubevy.ask`: everything a robot can see or do.
fn answer_requests(
    mut world: ResMut<ScriptWorld>,
    mut robots: Query<(Entity, &mut Robot, &Transform)>,
    mut commands: Commands,
    arena: Res<ArenaSize>,
) {
    let positions: Vec<(Entity, Vec2, f32)> = robots
        .iter()
        .map(|(e, r, t)| (e, t.translation.truncate(), r.hp))
        .collect();
    for request in world.take_requests() {
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
            "me" => Answer::List(vec![at.x as f64, at.y as f64, robot.hp as f64, 0.0]),
            "scan" => {
                let range = request.args.first().copied().unwrap_or(40.0);
                match nearest(me, at, &positions, range) {
                    Some((d, dist)) => Answer::List(vec![d.x as f64, d.y as f64, dist as f64]),
                    None => Answer::Nil,
                }
            }
            "thrust" => {
                let (dx, dy) = (request.args.first().copied().unwrap_or(0.0),
                                request.args.get(1).copied().unwrap_or(0.0));
                let v = Vec2::new(dx, dy).clamp_length_max(1.0) * MAX_SPEED;
                robot.velocity = v;
                Answer::Bool(true)
            }
            "fire" => {
                let dir = Vec2::new(request.args.first().copied().unwrap_or(0.0),
                                    request.args.get(1).copied().unwrap_or(0.0));
                if robot.cooldown > 0.0 || dir.length_squared() < 1e-6 || robot.hp <= 0.0 {
                    Answer::Bool(false)
                } else {
                    robot.cooldown = COOLDOWN;
                    let dir = dir.normalize();
                    commands.spawn((
                        Bullet { velocity: dir * BULLET_SPEED, owner: me, life: 2.0 },
                        Sprite { color: Color::srgb(1.0, 0.9, 0.5), custom_size: Some(Vec2::splat(0.5)), ..default() },
                        Transform::from_xyz(at.x + dir.x * ROBOT_RADIUS * 1.2, at.y + dir.y * ROBOT_RADIUS * 1.2, 2.0),
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

/// The nearest living robot other than `me`, as an offset and a distance.
fn nearest(me: Entity, at: Vec2, all: &[(Entity, Vec2, f32)], range: f32) -> Option<(Vec2, f32)> {
    all.iter()
        .filter(|(e, _, hp)| *e != me && *hp > 0.0)
        .map(|(_, p, _)| (*p - at, (*p - at).length()))
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
    }
}

fn move_bullets(
    time: Res<Time>,
    mut commands: Commands,
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
                robot.hp -= BULLET_DAMAGE;
                commands.entity(entity).despawn();
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
    }
    if alive <= 1 && !hud.line.starts_with("winner") {
        if let Some((robot, _, _)) = robots.iter().find(|(r, _, _)| r.hp > 0.0) {
            hud.line = format!("winner: {}", robot.name);
        }
    }
}
