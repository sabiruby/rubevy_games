# The prelude every script in this game is compiled in front of.
#
# **Empty on purpose at F0.** The crate's `build.rs` walks this directory and puts what it finds
# into the browser's binary, and a directory with no `.rb` in it would make that table empty and
# the browser build quietly different from the PC one — so the file the later stages fill exists
# from the first stage.
#
# What goes in here, by `docs/plans/factory-plan.md`:
#
#   F3  the inserter's words — `behind`, `front`, `move`, `idle`, `wants?` — and `idle`'s
#       spreading, so that a thousand inserters do not all wake on the same frame
#   F4  the control stage's `on(:built)` / `on(:crafted)` / `on(:delivered)` and `goal`
#
# Nothing is written here before the stage that needs it: a word in a prelude is an API, and one
# invented a stage early is one nothing has had to use.
#
# **F2 did not need any of it.** The data stage's `item` / `recipe` / `machine` / `belt` / `miner`
# / `chest` are native methods that `sabiruby_serde::declare` puts on `Object`
# (`factory/src/data.rs`), and the three that read the tables back — `item_of`, `recipe_of`,
# `machine_of` — are `expose`'s. So **`data.rb` is compiled on its own, with nothing in front of
# it**, and that is worth keeping: a compiler's line 4 and a backtrace's line 4 are the author's
# line 4, with no arithmetic in between and nothing to get wrong.
