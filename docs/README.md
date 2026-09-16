# rubevy_games documents

Laid out as every repository of the organization is
([`.github/CONTRIBUTING.md`](https://github.com/sabiruby/.github/blob/main/CONTRIBUTING.md));
small enough to stay flat; plans have their own directory.

| file | what it is |
|---|---|
| [sabiruby-battle.md](sabiruby-battle.md) | SabiRuby Battle: the two kinds of script, the boundary (`Rubevy.ask`), the tank model and the randomness, the DSL, reflexes, the editor, the VM panel, restarting, what is not done yet |
| [garden.md](garden.md) | Garden: the second sample — a 3D world whose creatures read and write their own ECS components from Ruby by name. What exists after G0, G0a and G1: the component table, the rules, the CC0 models and their animations, the DSL the two creatures are written in, the wheel a reflex takes, the two questions, and the numbers beside Battle's |
| [web.md](web.md) | the browser build: what differs from the PC build and where, the two wasm modules, keys, size, how it is checked |
| [wsl-gpu.md](wsl-gpu.md) | running the games on WSL2 without a GPU driver (Docker, WSLg, lavapipe) |
| [plans/showpieces-plan.md](plans/showpieces-plan.md) | (Japanese) to do: `reflex` on a hit, the VM inspector in the game window, the DSL over `Entity#[]` and `Proxy` |
| [plans/garden-plan.md](plans/garden-plan.md) | (Japanese) the second sample, Garden: creatures whose minds read and write ECS components by name (`Entity#[]`), a `Genome` class made from a Rust struct by the macros, save/load through serde and `JSON`, events and a `Proxy` for the two rule questions |
| [worklog/2026-09-17-battle-followups.md](worklog/2026-09-17-battle-followups.md) | (Japanese) taking rubevy `fa37eaa` and sabiruby 0.5.0 into the battle: the reflex priority turned the other way round (and the workaround it made unnecessary), `answer_requests` in `RubevySet::Answer` and the round trip measured again, the paused clock, and the VM panel's `ctx` read from the VM rather than out of a string |
| [worklog/2026-09-17-garden-G0a-G1.md](worklog/2026-09-17-garden-G0a-G1.md) | (Japanese) the Garden's models and its minds: a `bevy_gltf` that cargo swore did not exist, what `Scene` is called in bevy 0.19, why the animal pack had no rabbit, and the four things that had to be fixed before "a startled beetle turns" was true — ending in a wheel that one task holds |
| [worklog/2026-09-17-garden-G0.md](worklog/2026-09-17-garden-G0.md) | (Japanese) building the Garden's world: what `bevy_pbr` dragged in behind it, how a 3D scene runs headless with no renderer, the placeholder brain G1 deletes, and how the starvation check was made to happen on purpose rather than by luck |
| [worklog/2026-09-16-showpieces-d2-d3.md](worklog/2026-09-16-showpieces-d2-d3.md) | (Japanese) what rubevy took off the game's hands (the entity a child task carries, the end of a subscription), building the VM panel — the one entry point the VM has not got and how the `ctx` was got out of it anyway — and the study of writing the robots over `Entity#[]` and `Proxy` |
| [worklog/2026-09-16-reflex.md](worklog/2026-09-16-reflex.md) | (Japanese) building `reflex`: what broke when one robot used two tasks — the entity a `Task.new` task has not got, the nested run loop a task cannot park across, and why last-writer-wins makes the quicker task lose |
