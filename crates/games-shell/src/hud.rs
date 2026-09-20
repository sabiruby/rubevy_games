//! The text over the arena: one status line, and a panel per script.
//!
//! What a panel shows is what makes a VM written for this worth using — where the script is
//! right now (file and line), what it has spent of its instruction budget, and the exception it
//! died of, if it did.

use bevy::prelude::*;

/// The one status line at the top left. Write to it with `Hud::set`.
#[derive(Resource, Default)]
pub struct Hud {
    pub line: String,
}

/// One script's panel: name, state, where it is in its own source, and what it spends.
#[derive(Component, Debug, Clone, Default)]
pub struct ScriptPanel {
    pub name: String,
    pub state: String,
    /// `robots/scout.rb:42`, where the debug info says so.
    pub at: String,
    /// Instructions this script spent on the last frame, and what it may spend.
    pub spent: u64,
    pub budget: u64,
}

pub struct HudPlugin;

#[derive(Component)]
struct HudLine;

#[derive(Component)]
struct PanelText(Entity);

/// The box the lines sit in, so they are readable over whatever the floor is.
#[derive(Resource)]
struct HudRoot(Entity);

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hud>()
            .add_systems(Startup, spawn_hud)
            .add_systems(PostUpdate, (update_line, update_panels));
    }
}

fn spawn_hud(mut commands: Commands) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(8.0),
                left: px(8.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(8.0)),
                row_gap: px(3.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.82)),
        ))
        .id();
    commands.spawn((
        HudLine,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(15.0), ..default() },
        TextColor(Color::srgb(0.95, 0.95, 1.0)),
        TextLayout::no_wrap(),
        ChildOf(root),
    ));
    commands.insert_resource(HudRoot(root));
}

fn px(v: f32) -> Val {
    Val::Px(v)
}

fn update_line(hud: Res<Hud>, mut q: Query<&mut Text, With<HudLine>>) {
    if !hud.is_changed() {
        return;
    }
    for mut text in &mut q {
        **text = hud.line.clone();
    }
}

/// Each panel gets a text node the first time it is seen, and follows it after that.
fn update_panels(
    mut commands: Commands,
    root: Res<HudRoot>,
    panels: Query<(Entity, &ScriptPanel)>,
    mut texts: Query<(&PanelText, &mut Text)>,
) {
    let mut seen: Vec<Entity> = Vec::new();
    for (owner, mut text) in texts.iter_mut().map(|(p, t)| (p.0, t)) {
        if let Ok((_, panel)) = panels.get(owner) {
            **text = render(panel);
            seen.push(owner);
        } else {
            **text = String::new();
        }
    }
    for (entity, panel) in &panels {
        if seen.contains(&entity) {
            continue;
        }
        commands.spawn((
            PanelText(entity),
            Text::new(render(panel)),
            TextFont { font_size: bevy::text::FontSize::Px(13.0), ..default() },
            TextColor(Color::srgb(0.82, 0.86, 0.92)),
            TextLayout::no_wrap(),
            ChildOf(root.0),
        ));
        seen.push(entity);
    }
}

fn render(p: &ScriptPanel) -> String {
    let bar = {
        let filled = if p.budget == 0 { 0 } else { (p.spent * 16 / p.budget).min(16) as usize };
        format!("[{}{}]", "#".repeat(filled), "-".repeat(16 - filled))
    };
    format!("{:<14} {:<9} {bar} {:>7} insn  {}", p.name, p.state, p.spent, p.at)
}
