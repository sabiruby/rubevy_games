# 2026-09-17 箱庭の世界を Ruby にする — W1（世界 VM と、草・腹・食事・餓死の引っ越し）

`docs/plans/garden-world-plan.md`（第 2 版）の **W1**。worktree
`/home/kishima/book/kishima/rubevy_games-wt-world`、ブランチ `garden-world`、main `aadc7bf` から。
rubevy は main `3124dfa`（S4 = `answer_in_tick` が入った版）。

前提の調査は `docs/worklog/2026-09-17-garden-world-survey.md`（ただし §7 の「読み 1 回 1 フレーム」は
S1 で変わった）と `docs/worklog/2026-09-17-sync-reads.md`（順序・計測・6 番の経緯）。

書きながら進める記録。捨てた案と、なぜ捨てたかも残す。

---

## 0. 着手前 — 何が動いていて、何が揺れているか

最初にやったのは `cargo update -p rubevy` と、**その状態で 10 判定を 3 回測ること**。
W1 は規則の半分を別の言語に移す変更なので、「移す前の揺れ」を知らずに始めると、
移したあとの FAIL がどちらのせいか言えなくなる。

```
Updating rubevy v0.0.1 (https://github.com/sabiruby/rubevy#4f77881b) -> #3124dfaf
```

`cargo update -p rubevy` は sabiruby も `bd6829b3` → `8faf26f2` に上げようとした
（S3 のときと同じ挙動、`docs/worklog/2026-09-17-sync-reads.md` §0）。指示どおり
`cargo update -p sabiruby --precise bd6829b3…` で戻した。`Cargo.lock` の sabiruby / -macros /
-compiler / -serde は 4 か所とも `bd6829b3` の 1 つ、rubevy だけが `3124dfaf`。

`cargo build --release -p garden -p sabibots` は 2 分 50 秒で通る。

### 0.1 着手前の 10 判定（`GARDEN_SELFTEST=1 ./target/release/garden --headless 90` ×3）

| 判定 | 1 回目 | 2 回目 | 3 回目 |
|---|---|---|---|
| 1 somebody ate within 10 s | ok (0.75 s) | ok (0.56 s) | ok (0.75 s) |
| 2 night arrived by 60 s | ok (25.21 s) | ok (25.21 s) | ok (25.20 s) |
| 3 the starved creature's entity is gone | ok (1.89 s) | ok (1.89 s) | ok (1.89 s) |
| 4 nothing walked through anything | ok (0.980) | **FAIL** (0.860、1 フレーム) | ok (0.972) |
| 5 a hungry creature … reached it | ok (1.79 s) | ok (1.79 s) | ok (1.79 s) |
| 6 a beetle touched by a rabbit turned | ok (36/36) | ok (28/28) | ok (21/21) |
| 7 asleep a second after night | ok | ok | ok |
| 8 child's genome | ok | ok | ok |
| 9 spawn Hash names the gene | ok | ok | ok |
| 10 a save with the wrong version | ok | ok | ok |

**4 番は 3 回中 1 回落ちた。** S3 の記録（§9.5）が「順序なし 1/24、順序あり 1/26」と書いた
すり抜けそのもので、`closest pair 0.860 of the radii, 1 frames under 0.9` と、落ち方も同じ。
**着手前から落ちる判定**だということを、この 3 回で押さえておく。

総計行（世界 VM が入る前の、生き物 VM だけの数）:

```
hud: insn/decision — 5479 passes of a behaviour's loop, 196.3 instructions each; and 1003 questions the game answered, 1.000 frames each
hud: insn/decision — 4772 passes of a behaviour's loop, 203.5 instructions each; and 1111 questions the game answered, 1.000 frames each
hud: insn/decision — 5407 passes of a behaviour's loop, 198.7 instructions each; and 1048 questions the game answered, 1.000 frames each
hud: … VM 0.75 / 8.0 ms this frame (0.93 ms smoothed) / 0.70 (1.03) / 1.26 (1.22)
```

sabibots（`SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 25` ×1）は**全部 ok**。
