# 著者が後で確認する判断の一覧

作成 2026-09-22。著者「判断が必要だった部分は記録してあとで確認できるようにして。公開後に確認します」「実装にちょっと時間が掛かりすぎなので」。
**実装は止めずに、本体が原則（`/home/kishima/book/CLAUDE.md`）に照らして決めて進め、決めたことをここに 1 行ずつ足す。** 著者は Factory の公開（F6）の後にここを上から見る。
覆したいものは、その行の「変えるなら」を見れば場所が分かる。理由の全文は各段階の worklog と、計画書 7 章（`factory-plan.md`・`shared-crate-plan.md`）にある。
sabiruby・rubevy・playground の件もここに集める（3 つの repo に散らさない）。

**2026-09-22 の著者の追加の決定**: 「公開とバージョンアップもしてよい。VM 本体の unsafe は禁止。公開中のページの内容は今は開発中なので変えてもよい」。
だから **crates.io への公開と版上げも本体がやり、やったことを下の C に記録する**。**VM 本体（sabiruby）の unsafe は聞くものではなく禁止**（選択肢としても進めない）。
公開中のページ（箱庭・Battle・Playground）は開発中なので内容が変わってよい。取り返しのつかない操作（`cargo publish` は yank しかできない）は、先例の手順書どおりに dry-run を通してから打つ。

## A. 本体が決めて進めたもの（覆せる）

| 日付・段階 | 決めたこと | 理由（1 行） | 変えるなら |
|---|---|---|---|
| 09-21 F1 | 詰まったタイルの個数が f32 の丸めで変わる件に、許容誤差を足さなかった | 出どころの無い数になる。→ 著者が F2a で整数の刻みを選び、問題ごと消えた | — |
| 09-21 F1 | ズームを整数倍の画素に丸める仕組みは factory の中に置いたまま | 利用者が 1 本のうちは共有 crate に上げない | `factory/src/draw.rs` の `snap_zoom` → `CameraControls` のつまみに |
| 09-21 F2 | `data.rb` の語を 6 語に分けた（`machine` に任意フィールドを足さない） | 語を分ければ serde の `deny_unknown_fields` が行つきで断れる | `factory/src/data.rs` の各 `*Decl` |
| 09-21 F2 | レシピの時間: かまど 2.0 秒、組立機 1.0 秒（板 2 → 歯車 1）、組立機は 2×2 | ベルトに対する割合で、鎖が整数（採掘機 2 → かまど 4 → 組立機 1 → 毎秒 1 個）になる値 | `factory/ruby/data.rb` |
| 09-21 F3 | インサータの `move` は「request を持ち続け、腕が振り終わったフレームで答える」 | `sleep` 案は腕の長さが Rust と Ruby の 2 か所に、イベント案は台数ぶんの購読が要る | `factory/src/inserters.rs`。rubevy の `Held` が入ったら乗り換える |
| 09-21 F3 | 機械への出入りはインサータだけ。ベルト → 箱、採掘機 → ベルト・箱は直結のまま | 最初に建てる線（採掘機・ベルト・箱）を Ruby 無しで建てられるように | `factory/src/machines.rs` の `Offer` |
| 09-21 F3 | `inserter :arm, seconds_per_item: 1.0`。取れる範囲・`idle` の長さ・ずらしに新しい数を作らなかった | 毎秒 1 個 = 採掘機 1 台ぶん = ベルトの 1/4。`idle` は 1 swing、ずらしは 1 周期ぶんの乱数 | `factory/ruby/data.rb`、`prelude.rb` の `stagger` |
| 09-21 F3 | factory の VM の `script_budget` の既定は 39,000（`frame_time` は rubevy の 8 ms のまま） | 3,000 台の実測 毎 ms 9,800 命令 × 1 フレームの 1/4（4 ms） | `factory.settings.txt` の `script_budget` |
| 09-21 F3 | エディタの Save は F5 まで繋がない | 何をどこへ保存するかがセーブの設計と一体 | F5 |
| 09-21 F3 | 機械を持っているとき `R` は何もしない（F4 の最初に入れる） | 何も変えない操作を受け付けない。機械の向きはもう何も言わない | `factory/src/build.rs` |
| 09-21 F3a | 地図の語は `map :world, size: [w, h]`（計画の例の `tiles:` ではなく `size:`） | `machine … size: [2, 2]` と同じ意味なので同じ綴り | `factory/ruby/data.rb`、`data.rs` の `MapDecl` |
| 09-21 F3a | 畑の置き方は格子 `patches: [2, 2]`（数 + 乱数の種にしなかった） | 種は出どころの無い数で、2 回の走行が同じ所に鉱石を見つける前提が壊れる。既定は今までの 4 隅と同じ | `data.rb` の `ore` の行、`grid.rs` の `Ore::patch_middle` |
| 09-21 F3a | 地図の上限は 2048（一辺）。chunk を分けて外すことはしない | `TilemapChunk` は 1 タイル 1 テクセルの 1 枚のテクスチャで、2048 はどの WebGL2 でも保証される大きさ。メモリは上限にせず log に MB を出す | `factory/src/draw.rs` の `MOST_TILES_ACROSS` |
| 09-21 F3a | 既定の見え方は地図全体にしない（ズームで全体まで引ける） | 既定の 32×32 の絵を変えない。工場ゲームは寄った所から始まる | `factory/src/main.rs` の `point_the_camera_at_the_map` |
| 09-21 S9 | 箱庭のゲーム側の `"frame"` の答えを消した（rubevy の `each_frame` だけにした） | 同じ約束の綴りを 2 つ残さない。利用者の `world.rb` が `Rubevy.ask("frame")` を書いていたら静かに止まる（同梱のものは書いていない） | `garden/src/main.rs` の `answer_world` |
| 09-21 S10 | 待ちの式の「構造の 2 フレーム」は箱庭と Battle で別の引数にした | 別の理由の 2（再起動／publish が届くまで）。1 つの数で賄わない | `games_shell::checks::CheckPace::frames_to_wait` |
| 09-21 sabiruby | `symbol_keys` の意味は変えず `symbol_map_keys` と `Options::symbols()` を足した。`Options` に `#[non_exhaustive]` | 既存の利用者の map が黙って Symbol にならないように。Symbol は GC されない（実測）ので map のキーは利用者が選ぶ | sabiruby `serde/src/lib.rs` |
| 09-21 sabiruby | VM に `Vm::backtrace_line()` を足した（safe、追加のみ） | `next_line`（旧 `current_line`）は次の命令の行で、native の中からは 1 行ずれる | sabiruby `src/vm.rs` |
| 09-22 | rubevy の待ちを F4 の前提にせず、F4 を先に始めた | 著者「時間が掛かりすぎ」。control stage のイベントは publish / subscribe で、`Held` が無くても書ける | factory の `Arms::waiting` の乗り換えは `Held` が main に入った後の小さい段 |

## B. 著者にしか決められないもの（公開後に）

| 件 | 材料 | 場所 |
|---|---|---|
| **factory の地図の既定の大きさ**（今 32×32 = F0 の仮の値） | スクリプトの上限は縛らない（3,000 台で tick p95 2.2 ms）。残る不明は描画で、この機械はソフトウェア描画しか無い。著者の実機のブラウザで `?stress` を 1 度 | `docs/numbers.md` §9.6 |
| `implementer.md` に足す確認の作法（条件で待つのはフレームでも同じ／交互だけでなく向きも入れ替える／版が名乗る行を見る／`main()` の `warn!` は出ない） | S11 の担当が文面の案を出す | `docs/worklog/2026-09-22-s11.md` |
| Battle に機体数の上限がどこにも無い（予算 114,000 は担当が輪の幾何から導いた 36 台で測った） | 上限をゲームが言うか、言わないままにするか | `docs/worklog/2026-09-21-s10.md` §4 |
| 本（book）の findings に写す素材: f32 の詰まりの個数、飽和演算の値段、別々の理由の待ちを 1 つの数で賄う、版を名乗る行、`#[expect(deprecated)]` | — | 各 worklog の「気づいた点」 |

## C. 公開と版上げの記録（本体がやったもの）

| 日付 | 何を | 版 | 記録 |
|---|---|---|---|
| 09-22 | sabiruby 0.6.0 の準備を開始（worktree `sabiruby-wt-release-0.6`。`Vm::current_line` を消した = 破壊的、`sabiruby-serde` は `#[non_exhaustive]` で 0.2.0）。publish とタグは dry-run が通ってから本体が打つ | 準備中 | sabiruby `docs/worklog/2026-09-22-release-0.6.md` |
