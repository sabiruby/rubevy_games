# rubevy_games documents

Laid out as every repository of the organization is
([`.github/CONTRIBUTING.md`](https://github.com/sabiruby/.github/blob/main/CONTRIBUTING.md));
small enough to stay flat; plans have their own directory.

| file | what it is |
|---|---|
| [sabiruby-battle.md](sabiruby-battle.md) | SabiRuby Battle: the two kinds of script, the boundary (`Rubevy.ask`), the tank model and the randomness, the DSL, the editor, restarting, what is not done yet |
| [web.md](web.md) | the browser build: what differs from the PC build and where, the two wasm modules, keys, size, how it is checked |
| [wsl-gpu.md](wsl-gpu.md) | running the games on WSL2 without a GPU driver (Docker, WSLg, lavapipe) |
| [plans/showpieces-plan.md](plans/showpieces-plan.md) | (Japanese) to do: `reflex` on a hit, the VM inspector in the game window, the DSL over `Entity#[]` and `Proxy` |
| [worklog/2026-09-16-reflex.md](worklog/2026-09-16-reflex.md) | (Japanese) building `reflex`: what broke when one robot used two tasks — the entity a `Task.new` task has not got, the nested run loop a task cannot park across, and why last-writer-wins makes the quicker task lose |
