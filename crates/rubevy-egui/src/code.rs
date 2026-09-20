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

#[derive(Default)]
pub struct CodePanelPlugin {
    pub style: CodeStyle,
}

#[derive(Component)]
struct PanelRoot;

#[derive(Component)]
struct PanelTitle;

/// One row of the listing: the line number it shows (1-based).
#[derive(Component)]
struct PanelLine(usize);

// ---------------------------------------------------------------------------------------------
// The numbers (S5b-1). The `const`s are the names of the defaults; the settings are
// `CodeStyle`, which a game hands `CodePanelPlugin::sized` or writes into while it runs.
// ---------------------------------------------------------------------------------------------

/// How many lines fit; the window is scrolled so that the current line is inside it.
/// **Source unknown**.
pub const ROWS: usize = 28;

/// **How many characters of a line are kept.**
///
/// **Source unknown, and the comment beside it has never agreed with it**: `clip`'s own line says
/// "430px of a monospace 12px font: about 58 characters" and the number is 44. Both arrived in
/// one commit (`23c5ad3`), so one of them was wrong from the first day and nothing in the
/// repository says which (`docs/numbers.md` §1.5, §7-1). It is left at 44 because S5b-1 changes
/// no default; a game that wants the 58 its own comment describes can now ask for it.
pub const CHARS: usize = 44;

/// Where the panel sits and how wide it is, in logical pixels. *Reason only* for the top —
/// "below the HUD rows, so the two do not overlap" — and **source unknown** for the other two.
pub const TOP: f32 = 120.0;
pub const RIGHT: f32 = 8.0;
pub const WIDTH: f32 = 430.0;
pub const PADDING: f32 = 8.0;

/// The size of the title and of one line of the listing. **Source unknown** for both; the 12 is
/// the one the disagreeing comment above does its arithmetic with.
pub const TITLE_FONT: f32 = 13.0;
pub const LINE_FONT: f32 = 12.0;

/// **Every number the code panel has**, in one resource a game can hand the plugin or write into
/// while it runs.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct CodeStyle {
    /// [`ROWS`]
    pub rows: usize,
    /// [`CHARS`]
    pub chars: usize,
    /// [`TOP`]
    pub top: f32,
    /// [`RIGHT`]
    pub right: f32,
    /// [`WIDTH`]
    pub width: f32,
    /// [`PADDING`]
    pub padding: f32,
    /// [`TITLE_FONT`]
    pub title_font: f32,
    /// [`LINE_FONT`]
    pub line_font: f32,
}

impl Default for CodeStyle {
    fn default() -> Self {
        CodeStyle {
            rows: ROWS,
            chars: CHARS,
            top: TOP,
            right: RIGHT,
            width: WIDTH,
            padding: PADDING,
            title_font: TITLE_FONT,
            line_font: LINE_FONT,
        }
    }
}

impl CodeStyle {
    /// **What a player left in a `key=value` store.** The store is the game's
    /// (`games_shell::Settings`), which this crate does not depend on, so what comes in is a
    /// function that answers a key.
    ///
    /// | key | field |
    /// |---|---|
    /// | `code_rows` | [`CodeStyle::rows`] |
    /// | `code_chars` | [`CodeStyle::chars`] |
    /// | `code_width` | [`CodeStyle::width`] |
    /// | `code_font` | [`CodeStyle::line_font`] |
    ///
    /// The panel is built once, at startup, so a number changed after that is not seen until the
    /// next run — which is what a store read before the first frame is for.
    pub fn read_from(&mut self, number: impl Fn(&str) -> Option<f32>) {
        if let Some(v) = number("code_rows") {
            self.rows = v.max(0.0) as usize;
        }
        if let Some(v) = number("code_chars") {
            self.chars = v.max(1.0) as usize;
        }
        if let Some(v) = number("code_width") {
            self.width = v;
        }
        if let Some(v) = number("code_font") {
            self.line_font = v;
        }
    }
}

impl CodePanelPlugin {
    /// The panel at a size of the game's choosing.
    pub fn sized(style: CodeStyle) -> CodePanelPlugin {
        CodePanelPlugin { style }
    }
}

impl Plugin for CodePanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CodePanel>()
            .insert_resource(self.style.clone())
            .add_systems(Startup, spawn_panel)
            .add_systems(PostUpdate, draw_panel);
    }
}

fn spawn_panel(mut commands: Commands, style: Res<CodeStyle>) {
    commands
        .spawn((
            PanelRoot,
            Node {
                position_type: PositionType::Absolute,
                // below the HUD rows, so the two do not overlap
                top: Val::Px(style.top),
                right: Val::Px(style.right),
                width: Val::Px(style.width),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(style.padding)),
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
                TextFont { font_size: bevy::text::FontSize::Px(style.title_font), ..default() },
                TextColor(Color::srgb(0.95, 0.75, 0.45)),
            ));
            for row in 0..style.rows {
                panel.spawn((
                    PanelLine(row),
                    Text::new(""),
                    TextFont { font_size: bevy::text::FontSize::Px(style.line_font), ..default() },
                    TextColor(Color::srgb(0.62, 0.66, 0.72)),
                    // a listing does not reflow: a line too long is cut, not wrapped
                    TextLayout::no_wrap(),
                ));
            }
        });
}

fn draw_panel(
    panel: Res<CodePanel>,
    style: Res<CodeStyle>,
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
    let first = current.saturating_sub(style.rows / 2).max(1);
    let first = first.min(panel.lines.len().saturating_sub(style.rows).max(1));

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
                **text = format!("{mark}{number:>4} {}", clip(line, style.chars));
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

/// One line, cut to [`CodeStyle::chars`].
///
/// **The sentence that used to stand here said something else than the number did**: "the panel
/// is 430px of a monospace 12px font: about 58 characters", beside a cut at 44. See [`CHARS`] —
/// neither of the two has been moved towards the other, because nothing says which was meant.
fn clip(line: &str, chars: usize) -> String {
    if line.chars().count() <= chars {
        line.to_string()
    } else {
        let mut s: String = line.chars().take(chars.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A number in the store changes where a line is cut** (S5b-1), and the default cuts it
    /// exactly where it always did — at 44, which is not what `clip`'s old comment said.
    #[test]
    fn the_store_changes_the_cut_and_leaves_the_rest() {
        let line = "a".repeat(80);
        assert_eq!(clip(&line, CHARS).chars().count(), CHARS);
        assert!(clip(&line, CHARS).ends_with('…'));
        assert_eq!(clip("short", CHARS), "short", "what fits is not touched");

        let mut style = CodeStyle::default();
        style.read_from(|key| if key == "code_chars" { Some(58.0) } else { None });
        assert_eq!(style.chars, 58);
        assert_eq!(clip(&line, style.chars).chars().count(), 58);
        assert_eq!(style.rows, ROWS, "a key nobody wrote leaves the default alone");
    }
}
