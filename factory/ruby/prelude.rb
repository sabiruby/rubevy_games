# The prelude every script in this game is compiled in front of.
#
# **Empty on purpose at F0.** The crate's `build.rs` walks this directory and puts what it finds
# into the browser's binary, and a directory with no `.rb` in it would make that table empty and
# the browser build quietly different from the PC one — so the file the later stages fill exists
# from the first stage.
#
# What goes in here, by `docs/plans/factory-plan.md`:
#
#   F2  the data stage's `item` / `recipe` / `machine`, and reading the tables back
#   F3  the inserter's words — `behind`, `front`, `move`, `idle`, `wants?` — and `idle`'s
#       spreading, so that a thousand inserters do not all wake on the same frame
#   F4  the control stage's `on(:built)` / `on(:crafted)` / `on(:delivered)` and `goal`
#
# Nothing is written here before the stage that needs it: a word in a prelude is an API, and one
# invented a stage early is one nothing has had to use.
