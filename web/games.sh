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
# **Factory joined it at F6** (2026-09-22). From F0 until then its values were below but the word
# was deliberately not in this array, which is what kept a third, unfinished game off the
# published site while `web/build.sh factory` still built its page for a browser check. Adding
# the word and uncommenting the `<li>` in `web/index.html` are one change: the array decides what
# is built, the `<li>` decides what is linked, and a site with one without the other is either a
# dead link or a page nobody can reach. Nothing in `.github/workflows/pages.yml` moved — it names
# no games at all, it runs `web/build.sh all`.
GAMES_ALL=(sabibots garden factory)

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
TITLE[factory]='Factory'
BG[factory]='#1a1512'
FG[factory]='#e6d8c8'
DIM[factory]='#9a8b7a'
KEYS[factory]='"F1", "F2", "F5", "F9", "Tab"'
COMPILE_NOTE[factory]=$(cat <<'NOTE'
// calls window.factoryCompile(source) whenever it needs Ruby as bytecode. This game asks for it
// more often than the other two: the data stage (data.rb, before the first frame), the control
// stage (control.rb) and every inserter's own script, and again for each of them every time the
// editor's Apply is pressed — a data.rb applied in the page can lay the whole map out anew.
NOTE
)
KEYS_NOTE[factory]=$(cat <<'NOTE'
// Keys the game uses and the browser would take. F5 is the one that matters, and this game means
// by it what the garden does: **write the factory down** — the grid, the belts, the ore and what
// every script remembers — where a browser means Reload, the exact opposite of what the player
// pressed it for. F9 reads it back and no browser wants it; F1 is the editor (the three Ruby
// files), F2 the VM panel, and Tab moves the focus off the canvas. The capture phase runs before
// anything else can see the event, so preventDefault here stops the browser and the game still
// gets the key. The guide is H, which no browser claims, and Ctrl+S — Save page to a browser —
// writes the file that is open in the editor, which in a page is a localStorage key of its own.
NOTE
)
