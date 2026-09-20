# SabiRuby Battle の数を、利用者が変えられる場所へ（S5b-2）

計画書 `docs/plans/shared-crate-plan.md` の段階 **S5b-2**。一覧は `docs/numbers.md` の §4
（Battle の Rust）と §5（Battle の Ruby）、それと S5b-1 が「一覧に無い」と報告した共有 crate の
2 件。手本は `docs/worklog/2026-09-20-numbers-shared-crates.md`（S5b-1）。
着手時は main と同じ `b4083c1`、作業は worktree の `shared-crate`。

**著者が決めていたこと**（2026-09-20）: 分類案のとおり——**遊びの数は Ruby 側、動かす側の数は
`Settings` と起動の引数、判定の閾値は検査の側**。出どころ不明の数は「不明」と書いたまま移す。
**既定値は 1 つも動かさない。**

---

## 0. 着手前に取った基準

`git stash` を使わない規則があるので、触る前に測った。走らせた版のバイナリは
スクラッチパッドに `md5sum` つきで取ってある（同じ target に建て直すと消えるため）。

機械は空いていなかった——`vmstat 1 3` の idle は **0%**（96% user）で、隣の worktree
（`rubevy_games-wt-numbers`）で S8 の担当が docker のビルドを回している。判定の揺れは負荷に
依ることが S6 で分かっているので、FAIL が出たら静かになるまで待って取り直す方針で進めた。

### Battle のヘッドレス、3 走行

`SABIBOTS_SELFTEST=1 target/release/sabibots --headless 25` を 3 回。3 走行とも
`tools/fixedlines.sh` の出力は **4 行・FAIL 0** で、`docs/verification/selftest-lines.md` の
一覧と一致した（`the handler tasks of every robot that went down ended` は 3 走行とも `ok`）。

### 試合の決着は、種を固定しても再現しない

計画の確認項目に「乱数の種を固定できるなら固定して前後で同じ決着になることを」とあるので、
**先に固定できるかどうかを試した**。`matches/training.rb` に `seed: 7` を足して 25 秒の
ヘッドレスを 3 走行:

| 走行 | 1 red/scout | 2 red/hunter | 3 blue/scout | 4 blue/scout |
|---|---|---|---|---|
| 1 | hp 69 | hp 8 | **down** | hp 10 |
| 2 | hp 46 | **down** | **down** | hp 16 |
| 3 | hp 77 | hp 1 | **down** | hp 3 |

**同じ種でも決着は毎回違う。** 理由はコードが既に書いている（`Rules` の rustdoc:
"the frame timing still varies, so a replay is alike rather than identical"）——種が決めるのは
雑音と弾道の散りだけで、どのタスクがどのフレームで順番をもらうかは決めない。`seed` を戻し、
**前後の比較は「何走行か回して分布が明らかに変わっていないこと」で行う**ことにした
（§5 に前後 8 走行ずつの表）。
