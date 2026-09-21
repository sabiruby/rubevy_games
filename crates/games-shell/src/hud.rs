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

// ---------------------------------------------------------------------------------------------
// The numbers (S5b-1). The `const`s name the defaults; the settings are `HudStyle`, which a game
// hands `HudPlugin::styled` or a player changes through the `key=value` store.
// ---------------------------------------------------------------------------------------------

/// How many characters wide the budget bar is. **Source unknown**.
pub const BAR_TICKS: u64 = 16;

/// The size of the one status line, and of a script's panel under it. **Source unknown** for
/// both.
pub const LINE_FONT: f32 = 15.0;
pub const PANEL_FONT: f32 = 13.0;

/// How far the box sits from the top left of the window, how much padding it has inside, and the
/// gap between two of its lines. **Source unknown** for all three.
pub const MARGIN: f32 = 8.0;
pub const PADDING: f32 = 8.0;
pub const ROW_GAP: f32 = 3.0;

/// **Every number the HUD has**, in one resource a game can hand [`HudPlugin::styled`] or write
/// into while it runs. The box itself is built at startup, so its margins are read once; the bar
/// is redrawn every frame and follows [`HudStyle::bar_ticks`] straight away.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct HudStyle {
    /// [`BAR_TICKS`]
    pub bar_ticks: u64,
    /// [`LINE_FONT`]
    pub line_font: f32,
    /// [`PANEL_FONT`]
    pub panel_font: f32,
    /// [`MARGIN`]
    pub margin: f32,
    /// [`PADDING`]
    pub padding: f32,
    /// [`ROW_GAP`]
    pub row_gap: f32,
}

impl Default for HudStyle {
    fn default() -> Self {
        HudStyle {
            bar_ticks: BAR_TICKS,
            line_font: LINE_FONT,
            panel_font: PANEL_FONT,
            margin: MARGIN,
            padding: PADDING,
            row_gap: ROW_GAP,
        }
    }
}

impl HudStyle {
    /// **What a player left in the `key=value` store** ([`crate::Settings`]).
    ///
    /// | key | field |
    /// |---|---|
    /// | `hud_line_font` | [`HudStyle::line_font`] |
    /// | `hud_panel_font` | [`HudStyle::panel_font`] |
    /// | `hud_margin` | [`HudStyle::margin`] |
    /// | `hud_bar_ticks` | [`HudStyle::bar_ticks`] |
    ///
    /// A key that is not there leaves the field alone.
    pub fn read_from(&mut self, settings: &crate::Settings) {
        let take = |key: &str, slot: &mut f32| {
            if let Some(value) = settings.number(key) {
                *slot = value;
            }
        };
        take("hud_line_font", &mut self.line_font);
        take("hud_panel_font", &mut self.panel_font);
        take("hud_margin", &mut self.margin);
        // **a bar of no ticks is a bar nobody drew**, which is a thing somebody may want; what
        // is not a thing is half a tick or minus three of them (S11)
        self.bar_ticks = settings.counted("hud_bar_ticks", 0, self.bar_ticks);
    }
}

#[derive(Default)]
pub struct HudPlugin {
    pub style: HudStyle,
}

impl HudPlugin {
    /// The HUD at a size of the game's choosing.
    pub fn styled(style: HudStyle) -> HudPlugin {
        HudPlugin { style }
    }
}

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
            .insert_resource(self.style.clone())
            .add_systems(Startup, spawn_hud)
            .add_systems(PostUpdate, (update_line, update_panels));
    }
}

fn spawn_hud(mut commands: Commands, style: Res<HudStyle>) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(style.margin),
                left: px(style.margin),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(style.padding)),
                row_gap: px(style.row_gap),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.82)),
        ))
        .id();
    commands.spawn((
        HudLine,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(style.line_font), ..default() },
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
    style: Res<HudStyle>,
    panels: Query<(Entity, &ScriptPanel)>,
    mut texts: Query<(&PanelText, &mut Text)>,
) {
    let mut seen: Vec<Entity> = Vec::new();
    for (owner, mut text) in texts.iter_mut().map(|(p, t)| (p.0, t)) {
        if let Ok((_, panel)) = panels.get(owner) {
            **text = render(panel, style.bar_ticks);
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
            Text::new(render(panel, style.bar_ticks)),
            TextFont { font_size: bevy::text::FontSize::Px(style.panel_font), ..default() },
            TextColor(Color::srgb(0.82, 0.86, 0.92)),
            TextLayout::no_wrap(),
            ChildOf(root.0),
        ));
        seen.push(entity);
    }
}

fn render(p: &ScriptPanel, ticks: u64) -> String {
    let bar = {
        let filled = if p.budget == 0 { 0 } else { (p.spent * ticks / p.budget).min(ticks) };
        format!("[{}{}]", "#".repeat(filled as usize), "-".repeat((ticks - filled) as usize))
    };
    format!("{:<14} {:<9} {bar} {:>7} insn  {}", p.name, p.state, p.spent, p.at)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The bar is as many characters as the setting says** (S5b-1), and the default draws
    /// exactly the sixteen it always did.
    #[test]
    fn the_bar_is_as_wide_as_it_was_asked_to_be() {
        let half = ScriptPanel { spent: 500, budget: 1000, ..ScriptPanel::default() };
        let line = render(&half, BAR_TICKS);
        assert!(line.contains(&format!("[{}{}]", "#".repeat(8), "-".repeat(8))), "{line}");

        let wide = render(&half, 32);
        assert!(wide.contains(&format!("[{}{}]", "#".repeat(16), "-".repeat(16))), "{wide}");
        // a script with no budget draws an empty bar rather than dividing by it
        let idle = ScriptPanel { spent: 40, budget: 0, ..ScriptPanel::default() };
        assert!(render(&idle, BAR_TICKS).contains(&"-".repeat(16)));
        // and one over its budget fills the bar and no more
        let over = ScriptPanel { spent: 4000, budget: 1000, ..ScriptPanel::default() };
        assert!(over.spent > over.budget && render(&over, BAR_TICKS).contains(&"#".repeat(16)));
    }
}
