# rubevy_games documents

Laid out as every repository of the organization is
([`.github/CONTRIBUTING.md`](https://github.com/sabiruby/.github/blob/main/CONTRIBUTING.md));
small enough to stay flat; plans have their own directory.

| file | what it is |
|---|---|
| [sabiruby-battle.md](sabiruby-battle.md) | SabiRuby Battle: the two kinds of script, the boundary (`Rubevy.ask`), the tank model and the randomness, the DSL, reflexes, the editor, the VM panel, restarting, what is not done yet |
| [web.md](web.md) | the browser build: what differs from the PC build and where, the two wasm modules, keys, size, how it is checked |
| [wsl-gpu.md](wsl-gpu.md) | running the games on WSL2 without a GPU driver (Docker, WSLg, lavapipe) |
| [plans/showpieces-plan.md](plans/showpieces-plan.md) | (Japanese) to do: `reflex` on a hit, the VM inspector in the game window, the DSL over `Entity#[]` and `Proxy` |
| [worklog/2026-09-16-showpieces-d2-d3.md](worklog/2026-09-16-showpieces-d2-d3.md) | (Japanese) what rubevy took off the game's hands (the entity a child task carries, the end of a subscription), building the VM panel — the one entry point the VM has not got and how the `ctx` was got out of it anyway — and the study of writing the robots over `Entity#[]` and `Proxy` |
| [worklog/2026-09-16-reflex.md](worklog/2026-09-16-reflex.md) | (Japanese) building `reflex`: what broke when one robot used two tasks — the entity a `Task.new` task has not got, the nested run loop a task cannot park across, and why last-writer-wins makes the quicker task lose |
