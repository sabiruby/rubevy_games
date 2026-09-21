//! **The panel an inserter's script is edited in**, and nothing else a window has.
//!
//! This is F3's half of the window and not F5's: there is no HUD here, no VM panel, no guide and
//! no save. What there is, is the thing the game is about — click an inserter and its Ruby is in a
//! panel; type; run it in that one arm or in every arm on the same script; throw the edits away.
//!
//! **Clicking an inserter with an inserter in hand opens it.** There is no mode to switch into and
//! no key to learn: the tool that builds an arm is the tool that opens one, because a click on a
//! tile that already has an inserter cannot have meant "build an inserter" ([`crate::build`]).
//! Building a new one opens it too, which is the order a player does it in — put the arm down,
//! then tell it what to do.
//!
//! **Apply, Apply to all, Revert.** The panel is `rubevy_egui`'s and takes the two labels
//! (`apply_label`, `apply_all_label`) and the noun; what each one means here is:
//!
//! | | what it does |
//! |---|---|
//! | Apply | this one arm runs the text, and no other arm changes |
//! | Apply to every inserter | every arm runs it, including the ones that had a text of their own |
//! | Revert | every arm back to `ruby/inserter.rb` |
//! | Save | **F5's**, and it says so rather than doing half of it |
//!
//! Applying does not despawn anything: the entity keeps its place and is handed a new script
//! (`replace_script`), which is [`crate::inserters::keep_the_crew`]'s job — this file only writes
//! down which text is whose.
//!
//! **Every distinct text is an irep the VM keeps for ever** (SabiRuby has no way to drop one), so
//! [`watch_the_programs`] says how many have been handed over and how many programs the VM is
//! holding, every time either number moves. An editor used for an hour spends that.

use bevy::prelude::*;
use rubevy::ScriptWorld;
use rubevy_egui::{Editor, EditorAction};

use crate::belts::Rules;
use crate::data::Data;
use crate::grid::{Grid, What};
use crate::inserters::{Crew, Prelude, SCRIPT_FILE};
use crate::platform;

/// Which inserter the panel is showing, by its tile.
#[derive(Resource, Debug, Default)]
pub struct Watched(pub Option<usize>);

/// **A click on a tile that already has an inserter opens it**, and so does building a new one.
///
/// It reads the same [`crate::build::Order`] the building reads, and it is ordered **before** the
/// building so that "was there an inserter here already?" is answered about the grid as the
/// player saw it. An order that builds one is an order that opens the one it built; an order that
/// builds anything else, or takes something away, closes nothing — a player laying belts past an
/// open panel is not asking for it to shut.
pub fn follow_the_orders(
    mut orders: MessageReader<crate::build::Order>,
    grid: Res<Grid>,
    mut watched: ResMut<Watched>,
    mut editor: ResMut<Editor>,
) {
    // this one may take the panel as a parameter of its own: nothing else in it does
    for order in orders.read() {
        if order.what != Some(What::Inserter) {
            continue;
        }
        if order.at.x >= grid.tiles || order.at.y >= grid.tiles {
            continue;
        }
        watched.0 = Some(grid.index(order.at));
        editor.open = true;
    }
}

/// The panel follows the inserter being watched: its text, its name, and whether what it is
/// running came out of the file or out of the panel.
pub fn show_code(watched: Res<Watched>, grid: Res<Grid>, mut crew: Crew) {
    // the panel is reached through `Crew` and not as a parameter of its own: two `ResMut` of one
    // resource in one system is a panic, and `Crew` already holds it (`crate::inserters::Crew`)
    let Some(editor) = crew.panel.as_mut() else { return };
    // **No tab bar.** The garden's three buttons are three *files*; here every arm is its own
    // thing and there may be a thousand of them, so what chooses one is the map.
    editor.choices.clear();
    editor.selected = None;
    editor.noun = "inserter".into();
    editor.apply_label = "▶ Apply to this one (Ctrl+Enter)".into();
    editor.apply_all_label = Some("Apply to every inserter".into());
    editor.save_label = Some("Save — not yet (F5)".into());
    let Some(tile) = watched.0 else { return };
    // the arm was taken away while its script was on the screen
    if grid.at(tile).map(|b| b.what) != Some(What::Inserter) {
        return;
    }
    let text = crew.minds.text_for(tile).to_string();
    editor.show(tile as u64, || text);
    let at = grid.tile_of(tile);
    editor.file = SCRIPT_FILE.into();
    editor.label = format!("the inserter at {}, {}", at.x, at.y);
    editor.in_memory = crew.minds.in_memory(tile);
    // **the mark a stopped arm wears, said in words as well.** The picture puts an orange mark
    // over it; the panel says where it stopped, which is the half a player can act on.
    editor.elsewhere = crew.arms.stopped_at(tile).map(|at| format!("stopped at {at}"));
    editor.current = None;
    editor.heat.clear();
}

/// What the buttons asked for. Nothing here writes a file.
#[allow(clippy::too_many_arguments)]
pub fn do_editor_actions(
    watched: Res<Watched>,
    prelude: Res<Prelude>,
    data: Res<Data>,
    rules: Res<Rules>,
    stagger: Res<crate::inserters::Stagger>,
    mut crew: Crew,
    mut mrb: ResMut<Assets<rubevy::MrbAsset>>,
) {
    let Some(action) = crew.panel.as_mut().and_then(|p| p.action.take()) else { return };
    let Some(tile) = watched.0 else { return };
    let text = crew.panel.as_ref().map(|p| p.text.clone()).unwrap_or_default();
    // **A text that will not compile is not applied at all**, and the arms go on running what
    // they were running — the same answer a `data.rb` that will not read gets, and the opposite
    // of what a panic would say about a file a player is invited to edit. It is compiled here,
    // before anything is written down, so that a refusal changes nothing.
    let compiles = |crew: &mut Crew, mrb: &mut Assets<rubevy::MrbAsset>, text: &str| {
        crate::inserters::would_compile(&mut crew.minds, &prelude.0, &data, &rules, *stagger, text, mrb)
    };
    // what each button does, and what the panel is told about it afterwards
    let said: Result<(String, Option<String>), String> = match action {
        EditorAction::Apply => compiles(&mut crew, &mut mrb, &text).map(|()| {
            crew.minds.give_to_one(tile, text.clone());
            ("this inserter is running it — in memory, and no other one has changed".into(), None)
        }),
        EditorAction::ApplyAll => compiles(&mut crew, &mut mrb, &text).map(|()| {
            let n = crew.how_many();
            crew.minds.give_to_all(text.clone());
            (format!("all {n} inserters are running it — in memory, and any that had a script of their own have lost it"), None)
        }),
        EditorAction::Revert => {
            let file = crew.minds.file().to_string();
            crew.minds.back_to_the_file();
            Ok((format!("every inserter back to {SCRIPT_FILE}"), Some(file)))
        }
        // **F5's, and it says so.** Writing the file is the same three lines the garden has, but
        // what a Save means here — the file, the world, or both — is a question F5 answers.
        EditorAction::Save => Err(format!("saving {SCRIPT_FILE} is F5's; Apply runs it now")),
    };
    let Some(panel) = crew.panel.as_mut() else { return };
    match said {
        Ok((message, None)) => panel.applied(message),
        Ok((message, Some(source))) => panel.reset_to(source, message),
        Err(why) => {
            // a text that would not compile leaves every arm running what it was running, and
            // the first line of the complaint — the one with the place in it — is what the panel
            // has room for; the log has the whole thing
            if why.contains(':') {
                error!("{why}");
            }
            panel.message = format!("not applied — {}", why.lines().next().unwrap_or(&why));
        }
    }
}

/// **What the editor is spending**, said whenever either number moves.
///
/// Every distinct text a script is started from is an irep the VM keeps for the life of the
/// process — SabiRuby has no way to drop one — so an editor is a thing that spends memory as it
/// is used. `loaded_programs` is rubevy's count of distinct programs and `swaps` is how many
/// times an arm has been handed one; the two apart say whether a hundred arms cost a hundred
/// programs (they do not: one text is one program, however many arms run it).
pub fn watch_the_programs(
    scripts: Res<ScriptWorld>,
    crew: Crew,
    mut said: Local<(u64, usize)>,
) {
    let now = (crew.minds.swaps, scripts.loaded_programs());
    if *said == now {
        return;
    }
    *said = now;
    info!(
        "scripts: {} handed over, {} programs in the VM, {} that would not load",
        now.0,
        now.1,
        scripts.broken_programs()
    );
}

/// The window's own setup: the panel, at whatever size the store says.
pub fn the_editor(settings: &games_shell::Settings) -> rubevy_egui::EditorPlugin {
    let mut layout = rubevy_egui::EditorLayout::default();
    layout.read_from(|key| settings.number(key));
    rubevy_egui::EditorPlugin::with_highlighter(platform::highlight).sized(layout)
}
