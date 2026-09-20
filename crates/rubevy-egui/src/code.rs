//! The code panel: the script a game is showing, with the line it is standing on marked.
//!
//! A game fills [`CodePanel`] — the title, the lines, which line is current, which one raised —
//! and this draws it down the right-hand side. Read-only for now; the plan is to make it
//! editable, which is why the lines are kept as a `Vec<String>` rather than one string.

use bevy::prelude::*;

/// What the panel shows. Set it from a system of the game; the panel follows.
#[derive(Resource, Default)]
pub struct CodePanel {
    /// Shown above the code, e.g. `robots/scout.rb`.
    pub title: String,
    pub lines: Vec<String>,
    /// 1-based, as an editor counts: the line the script is standing on.
    pub current: Option<u32>,
    /// 1-based: the line an exception came from, if one did.
    pub failed: Option<u32>,
    /// Where the script is when that is not in this file (inside the DSL, say), for the title.
    pub elsewhere: Option<String>,
    pub visible: bool,
}

impl CodePanel {
    /// Replaces the source shown. Keeps `current` and `failed`, which a game sets per frame.
    pub fn show(&mut self, title: impl Into<String>, source: &str) {
        let title = title.into();
        if self.title == title && self.lines.len() == source.lines().count() {
            // same file: leave the nodes alone, the text is rebuilt below anyway
        }
        self.title = title;
        self.lines = source.lines().map(|l| l.to_string()).collect();
        self.visible = true;
    }
}

pub struct CodePanelPlugin;

#[derive(Component)]
struct PanelRoot;

#[derive(Component)]
struct PanelTitle;

/// One row of the listing: the line number it shows (1-based).
#[derive(Component)]
struct PanelLine(usize);

/// How many lines fit; the window is scrolled so that the current line is inside it.
const ROWS: usize = 28;

impl Plugin for CodePanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CodePanel>()
            .add_systems(Startup, spawn_panel)
            .add_systems(PostUpdate, draw_panel);
    }
}

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            PanelRoot,
            Node {
                position_type: PositionType::Absolute,
                // below the HUD rows, so the two do not overlap
                top: Val::Px(120.0),
                right: Val::Px(8.0),
                width: Val::Px(430.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.82)),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                PanelTitle,
                Text::new(""),
                TextFont { font_size: bevy::text::FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.95, 0.75, 0.45)),
            ));
            for row in 0..ROWS {
                panel.spawn((
                    PanelLine(row),
                    Text::new(""),
                    TextFont { font_size: bevy::text::FontSize::Px(12.0), ..default() },
                    TextColor(Color::srgb(0.62, 0.66, 0.72)),
                    // a listing does not reflow: a line too long is cut, not wrapped
                    TextLayout::no_wrap(),
                ));
            }
        });
}

fn draw_panel(
    panel: Res<CodePanel>,
    mut root: Query<&mut Visibility, With<PanelRoot>>,
    mut title: Query<&mut Text, (With<PanelTitle>, Without<PanelLine>)>,
    mut rows: Query<(&PanelLine, &mut Text, &mut TextColor)>,
) {
    for mut visibility in &mut root {
        *visibility = if panel.visible { Visibility::Inherited } else { Visibility::Hidden };
    }
    if !panel.visible {
        return;
    }
    // scroll so the current line sits in the middle of the window
    let current = panel.current.unwrap_or(1).max(1) as usize;
    let first = current.saturating_sub(ROWS / 2).max(1);
    let first = first.min(panel.lines.len().saturating_sub(ROWS).max(1));

    for mut text in &mut title {
        **text = match (panel.current, panel.elsewhere.as_deref()) {
            (Some(line), _) => format!("{}:{line}", panel.title),
            (None, Some(at)) => format!("{}   (in {at})", panel.title),
            (None, None) => panel.title.clone(),
        };
    }
    for (row, mut text, mut color) in &mut rows {
        let number = first + row.0;
        match panel.lines.get(number - 1) {
            Some(line) => {
                let mark = if panel.failed == Some(number as u32) {
                    "!"
                } else if panel.current == Some(number as u32) {
                    ">"
                } else {
                    " "
                };
                **text = format!("{mark}{number:>4} {}", clip(line));
                *color = TextColor(if panel.failed == Some(number as u32) {
                    Color::srgb(1.0, 0.45, 0.4)
                } else if panel.current == Some(number as u32) {
                    Color::srgb(1.0, 0.95, 0.6)
                } else {
                    Color::srgb(0.62, 0.66, 0.72)
                });
            }
            None => **text = String::new(),
        }
    }
}

/// The panel is 430px of a monospace 12px font: about 58 characters.
fn clip(line: &str) -> String {
    const WIDTH: usize = 44;
    if line.chars().count() <= WIDTH {
        line.to_string()
    } else {
        let mut s: String = line.chars().take(WIDTH - 1).collect();
        s.push('…');
        s
    }
}
