#!/usr/bin/env bash
# **What one game's page differs by, and nothing else.**
#
# `web/build.sh` sources this and fills `web/page.html.in` with it, once per game, into
# `web/dist/<game>/index.html`. Until 2026-09-20 there were two hand-written pages,
# `web/garden.html` and `web/sabibots.html`, which differed in fifteen lines and were otherwise
# the same file twice; a third game would have been a third copy.
#
# **A third game is one word in `GAMES_ALL` below and one block of values here.** (Plus its own
# `<li>` in `web/index.html`, the entry page, which is prose about the game and not a value.)
#
# The word is the crate's name, and it is also the canvas id and the prefix of the two functions
# the page hangs on `window` — `<game>Compile` and `<game>Highlight` — because the game's
# `WindowPlugin` and its `src/platform.rs` already spell it that way. Changing it would orphan
# every script a player has saved under the `localStorage` prefix of the same name.
#
# The two notes are prose because they are about that game: which keys it takes back from the
# browser, and what it compiles. They are what the page says to whoever opens `view-source`.

# The order is the order `web/build.sh all` builds in — **and it is also the list the published
# site has**, because `all` is what CI runs and `web/index.html` is the entry page beside it.
#
# **Factory is not in it yet** (F0, 2026-09-21). Its values are below, so `web/build.sh factory`
# builds its page and a browser check can be run against it here; leaving the word out of this
# array is what keeps a third, unfinished game off the published site. F6 adds it, and adds the
# `<li>` that is written out and commented in `web/index.html` beside the other two. The two
# things move together, which is why the reason is written here rather than in both places.
GAMES_ALL=(sabibots garden)

declare -A TITLE BG FG DIM KEYS COMPILE_NOTE KEYS_NOTE

# ---- SabiRuby Battle ----------------------------------------------------------------------
TITLE[sabibots]='SabiRuby Battle'
BG[sabibots]='#15161a'
FG[sabibots]='#d8dae0'
DIM[sabibots]='#8a8f99'
KEYS[sabibots]='"F1", "F2", "F5", "Tab"'
COMPILE_NOTE[sabibots]=$(cat <<'NOTE'
// calls window.sabibotsCompile(source) whenever it needs a robot's behaviour as bytecode.
NOTE
)
KEYS_NOTE[sabibots]=$(cat <<'NOTE'
// Keys the game uses and the browser would take. F5 is the one that matters: in the arena it is
// Apply, and in a browser it reloads the page. The capture phase runs before anything else can
// see the event, so preventDefault here stops the browser and the game still gets the key.
// F1 is the guide and F2 the VM panel — both of which `?selftest` presses — and a browser has its
// own idea about F1; Tab moves the focus off the canvas; Ctrl+S is Save page and Ctrl+Enter
// belongs to the address bar. The garden's page keeps the same list (the block below).
NOTE
)

# ---- Garden -------------------------------------------------------------------------------
TITLE[garden]='Garden'
BG[garden]='#12150f'
FG[garden]='#d8dcd2'
DIM[garden]='#8d9385'
KEYS[garden]='"F1", "F2", "F5", "F9", "Tab"'
COMPILE_NOTE[garden]=$(cat <<'NOTE'
// calls window.gardenCompile(source) whenever it needs a creature's behaviour as bytecode — the name
// is the game's, because the two games are two pages and each defines its own (platform.rs).
NOTE
)
KEYS_NOTE[garden]=$(cat <<'NOTE'
// Keys the game uses and the browser would take. F5 is the one that matters: in the garden it
// saves the world, and in a browser it reloads the page — the exact opposite of what the player
// meant. The capture phase runs before anything else can see the event, so preventDefault here
// stops the browser and the game still gets the key. F9 no browser wants; F1 is Help; Tab moves
// the focus off the canvas; Ctrl+S is Save page and Ctrl+Enter belongs to the address bar.
// The HUD's Save and Load buttons do the same two things with no key at all, which is what a
// player who has not read this page will find.
NOTE
)

# ---- Factory ------------------------------------------------------------------------------
# F0: the page builds and can be opened with `web/build.sh factory`, and the word is deliberately
# not in `GAMES_ALL` above until F6 (see the note there).
TITLE[factory]='Factory'
BG[factory]='#1a1512'
FG[factory]='#e6d8c8'
DIM[factory]='#9a8b7a'
KEYS[factory]='"F1", "F2", "F5", "Tab"'
COMPILE_NOTE[factory]=$(cat <<'NOTE'
// calls window.factoryCompile(source) whenever it needs Ruby as bytecode — the data stage, the
// control stage and each inserter's own script. Nothing calls it yet: F0 is the floor and the
// camera, and the first script arrives with the data stage (F2).
NOTE
)
KEYS_NOTE[factory]=$(cat <<'NOTE'
// Keys the game uses and the browser would take, kept the same as the other two games' so that a
// player moving between them is not surprised. F5 is Apply in the editor and Reload in a browser,
// which is the one that matters; F1 is the guide, F2 the VM panel, and Tab moves the focus off
// the canvas. The capture phase runs before anything else can see the event, so preventDefault
// here stops the browser and the game still gets the key. None of those panels is in this game
// yet (F5 brings them); the list is here because the page is written once.
NOTE
)
