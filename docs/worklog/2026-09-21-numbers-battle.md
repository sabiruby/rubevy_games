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

---

## 1. 一覧の抜けを先に塞いだ（共有 crate、別コミット `0375cd0`）

S5b-1 の報告の 1 件目——「一覧に入っていない埋め込みの数が VM パネルにまだ 9 か所ある。
同じ形で `guide.rs` の余白 3 つも」——から片付けた。S5a の網は `editor.rs` の 9 色を拾って
隣の `inspect.rs` を拾っていない。**表示の色を「1 組 1 行」に畳む規則が `KINDS` にしか
適用されていなかった**のが原因で、畳んだ結果 `inspect.rs` の色は表に 1 行も無く、
次に数える人が同じ見落とし方をする。

9 か所のうち**値が違うのは 7 つ**だった（`(255,236,150)` が「なぜ待っているか」の行と
「レジスタを見せているフレーム」に 2 回、灰 140 が「自分のではないフレーム」と
「値のクラス」に 2 回）。`const` は値ごとに 1 つ、フィールドも値ごとに 1 つにした——
S5b-1 が行番号の桁の色でやったこと（`kinds[3]` を読む）と同じで、**同じ値が 2 か所にある
状態を残さない**。

ただし**エディタの色とは繋がなかった**。`inspect.rs` の `AMBER` (240,190,90) は
`editor.rs` の `AMBER` と、`REG_NAME` (210,214,222) は `KINDS[0]` と、同じ値である。
同じ crate の隣同士なので「同じ色のつもり」に見えるが、**そう書いた記録はどこにも無い**。
繋ぐと、エディタの `* edited` の色を変えた人が VM パネルまで塗り替えることになる——
それは記録の無い意図を後から作ることで、S5a が箱庭の空腹バー 55.0 と `hungry_below` 55.0
について出した結論（偶然として扱い、繋がない）と同じ場面である。単体テストが
「今日は同じ値である」ことだけを書き留めている。

`guide.rs` の余白は 3 つ（`add_space(6.0)` と grid の `[14.0, 3.0]`）。単独の 1 つには
`guide_note_spacing` という鍵を付け、**組の方には付けなかった**——組を文字列から読むには
構文解析が要る、という S5b-1 の線（エディタのボタン余白、カメラの鍵盤）と同じ。

## 2. 判定の引数 16 個を先に 1 つにまとめた（`0918fd9`）

S5b-1 の報告の 5 件目。`sabibots` の `selftest` は Bevy の上限ちょうど 16 個で止まっていて、
そのせいでエディタの 9 色は Resource になれず `Editor::colors` のフィールドになった。
S5b-2 は「試合が渡した模型」を判定に読ませる必要があるので、**先に天井を上げた**。

世界について読む 7 つ（`bodies`・`WorldClock`・タスク・`ScriptWorld`・`VmInspector`・
壁・`ArenaSize`）を `MatchUnderTest` にまとめ、判定が使っていた 2 つのクロージャ
（全部の脳が使った命令数、全部の戦車の位置）をそのメソッドにした。箱庭の `FakePointer` と
同じ形。10 個になった。判定は 1 行も変えていない。

## 3. 数の持ち主を決める — 「受け取るまで試合を始めない」は、実は既にそうなっていた

指示はこう言っている: 受け取る前のフレームのための既定を Rust に置かざるを得ないなら、
その既定は「Ruby の既定を写したもの」ではなく、**受け取るまで試合を始めない**形にできないかを
先に検討する。

検討したら、**Battle は最初からその形をしていた**。`match_prelude.rb` の `Match#run` の
1 行目が `Rubevy.ask("rules", …)` で、ロボットを場に出す `start` はその後にしか来ない。
だから**試合が数を言うまで、場には何も無い**——戦車も、弾も、爆発も。読む側
（`move_robots`、`separate_robots`、`move_bullets`、`answer_requests`）は全部
`let Some(model) = &the_match.0 else { return }` で始まり、その `return` が起きるフレームには
動かすものが 1 つも無い。**Rust 側に模型の既定値は 1 つも要らなかった**。

`MatchModel` の全フィールドを `Option` でも `#[serde(default)]` でもなく**必須**にしたのは
そのためで、箱庭の `RuleBook`（全部 `Option` + `default`）とは逆の判断である。理由も逆で:
箱庭は `world.rb` が黙っていても庭が回らなければいけない（規則がコンパイルできない庭も
走る、というのが W1 の設計）。Battle は試合が黙っていたら**試合が無い**。

### 1 つだけ Rust に残した数 — `NO_MATCH_YET`

例外が 1 つある。カメラだけは `Startup` に枠取りが要る（`ArenaPlugin::showing(half)` は
プラグインを足す時点で数を求める）。ここで「32.0 を渡す」と、それは Ruby の既定の写しになる。

そこで **1.0 を `NO_MATCH_YET` という名前で置き、rustdoc に「これは闘技場ではない」と書いた**。
成り立つ理由は `games-shell` の `NO_WINDOW`（窓の無いフレームの 16:9）と同じで、
**その値で何も描かれない**:

* 床と壁は `build_field` が**試合の言ったフレームに**置く（`Startup` ではなくなった）。
* カメラは同じフレームの `PostUpdate` で `follow_arena` が枠取り直す（`ArenaView::framed` を
  `build_field` が書く）。`PostUpdate` は描画より前なので、1.0 で枠取った絵は 1 枚も出ない。
* ロボットは試合が出すので、そもそも居ない。

一覧では (c) ではなく **(a) 不変量**に置いた（値に意味は無く、正の数であることだけが意味）。
S5b-1 が `NO_WINDOW` を (c) から (a) へ動かしたのと同じ判断である。

### 捨てた案

* **`ArenaPlugin` に「枠取る正方形を持たない」形を足す**（`ArenaSize` を `Option` にし、
  `spawn_camera` と `follow_arena` を `Option<Res<…>>` にする）。数は完全に消えるが、
  S5b-1 が決めたばかりの共有 crate の口を S5b-2 が開け直すことになり、**S5b-2 は共有 crate を
  0 で触る**という段取り（一覧の抜け 2 件を除く）から外れる。1 行の番兵と rustdoc の方が安い。
* **`spawn_arena` を `Startup` に残し、`rebuild_walls` の変更検知に任せる**。試してから捨てた:
  `rebuild_walls` は `if !arena.is_changed() || arena.is_added() { return; }` で、`ArenaSize` が
  追加されたのは `App` の組み立て時、変更されるのは最初の `Update` ——**同じフレームで
  `is_added` がまだ真**なので `return` し、次のフレームには `is_changed` が偽になっていて、
  **壁が一度も建たない**。`spawn_arena` と `rebuild_walls` を 1 つの `build_field` にまとめ、
  「何を敷いたか」を `Local` で憶えて比べる形にした（床は枠取る幅と敷石、壁は今の幅と木箱の
  大きさ）。`Commands` の despawn と spawn は同じ列に積まれるので、二重には建たない。

## 4. `UNSET` は繋ぐのではなく消えた

S5a は §8-6 でこう書いている: `UNSET = -999.0` は「触らない」の合図だが、`aim` は角度なので
**−999 が「ありえない値」である保証は書かれていない**。`Option` にすれば数が要らなくなる。

rubevy の口を読んだら、その道が既に開いていた。`Rubevy.ask` の引数は
`Arg::{Num, Text, Entity, Value}` に分かれ、**数でも文字列でもエンティティでもないもの
（Hash、Array、`nil`）は `Arg::Value`** になる（`rubevy/src/lib.rs:3826-3837`）。
`Request::num(i)` は `Arg::as_num` を通すので、**`nil` の引数には `None` が返る**。

だから `act(throttle: nil, turn: nil, aim: nil, fire: nil)` はそのまま渡してよく、Rust は
`request.num(0)` を見るだけでよい。**両側から番兵の数が消えた。** 費用も無い:
`RootedValue::new` は `Value::Obj` のときだけ GC に登録するので、`nil` は根を作らない
（`rubevy/src/lib.rs:670-679`）。

## 5. 弾の速さは、写しをやめて「1 回だけ聞く」形にした

`lead` は `shot_speed(power)` を呼び、それが `SHOT_FAST` と `SHOT_SLOW` を読んでいた。
robot が毎フレーム聞く形にはできない（`lead` は思考の中で何度も呼ばれる）。
そこで **`run_robot` が起動時に 1 回だけ `Rubevy.ask("model")` で受け取る**——
既に隣で `srand(Rubevy.ask("seed").pop.to_i)` をしているのと同じ場所、同じ費用（1 フレーム）。

受け取るのは 5 つだけ（`shot_fast`、`shot_slow`、`radar_range`、`incoming_range`、`power_min`）で、
`Model` クラスの 1 行で分解する。順序が 2 つのファイルの間の約束の全部なので、両側に書き、
Rust 側に単体テスト（`the_dsl_is_handed_the_five_it_needs_in_order`）を置いた。

`$model` というグローバルに置いたのは、**1 つの試合のロボットは全部同じ試合を戦っている**から。
ハンドラのタスクも同じオブジェクトのメソッドを呼ぶので、そのまま見える。

## 6. どこへ移したか、一覧

| 何 | 何件 | どこへ | 鍵 |
|---|---|---|---|
| 機体・弾・エネルギー・雑音・闘技場（遊びの数） | 32 | `ruby/match_prelude.rb` の `Match::MODEL` | `match "…", numbers: { … }` |
| バー・爆発・砂・窓（表示） | 16 | `Look`（Resource） | `sabibots.settings.txt` の `look_*` と `window_*` |
| VM の予算と `frame_time` | 2 | `ScriptWorld`（起動時に書く） | `script_budget` / `script_frame_time_ms` |
| `--headless` / `--shot` の既定 | 3 | 引数の既定 | `headless_seconds` / `shot_file` / `shot_seconds` |
| **消えた数** | 3 | — | `UNSET` の Rust 側と Ruby 側、`SHOT_FAST` / `SHOT_SLOW` の Ruby の写し |

VM の予算と `frame_time` については**値を選んでいない**。Battle はこの 2 つをどこでも設定して
いなかった——rubevy の既定（200,000 命令と 8 ms）のまま走っていて、**rubevy 自身が
「どこから来たか不明」と rustdoc に書いている**。S5b-2 がしたのは「言えるようにした」だけで、
書かなければ今までどおり rubevy の既定である（ここに写しを置いていない）。

`crate_size` 2.6 だけ、一覧の分類案 (c) に従わず **(b)** にした。`shrink` が壁を動かす量
そのもので、`min_crates`（案は (b)）と組でしか意味を持たないため（`wall_layout` が 2 つを
一緒に使う）。戻すなら `Look` に 1 行移すだけで済む。報告に挙げる。

## 7. 動きを変えていないこと

### 6 通りの走行

| 走行 | 前 | 後 | 判定 |
|---|---|---|---|
| Battle ヘッドレス（`--headless 25`） | 4 行 FAIL 0 | 4 行 FAIL 0 | 交互 6 巡で `diff`。4 巡で**記録済みの「判定が動く 1 行」だけ**が動き（`the handler tasks of every robot that went down ended` が `--` ⇄ `ok`）、2 巡は `diff` 空 |
| Battle 窓（docker/lavapipe） | 32 行 FAIL 0 | 32 行 FAIL 0 | `verification/selftest-lines.md` の一覧と**完全一致** |
| Battle ブラウザ（`sabibots/?selftest`） | 33 行 | 33 行 FAIL 0 | 一覧と完全一致、pageerror 0・requestfailed 0 |
| 箱庭 ヘッドレス（`--headless 90`） | 13 行 FAIL 0 | 13 行 FAIL 0 | `diff` 空 |
| 箱庭 窓（docker/lavapipe） | 44 行 FAIL 0 | 44 行 FAIL 0 | 一覧と完全一致 |
| 箱庭 ブラウザ（`garden/?selftest`） | 45 行 | 45 行 FAIL 0 | 一覧と完全一致、pageerror 0・requestfailed 0 |

ほかに: `cargo build --workspace --all-targets` **警告 0**、`cargo test --workspace`
**46 → 53 通過**（+1 は §1 の色、+6 は Battle の新しいテスト）、`web/build.sh all` の
`wasm-opt` 後で **sabibots 35,152,714 → 35,216,606（+63,892、+0.18%）、
garden 35,900,116 → 35,900,926（+810、+0.002%）**。

### 試合の決着 — 交互に 6 巡

**最初に取った前後の 8 走行ずつは読み違えだった。** 「前」の 8 走行は隣の担当（S8）が
docker のビルドで CPU を使い切っている間に取ったもので（`vmstat` の idle が **0%**）、
「後」は静かになってから取った。当たりの数は 前 23.1 / 後 17.5、合計 hp は 前 67 / 後 120 で、
**「移したせいで当たらなくなった」と読める形**をしていた。

`implementer.md` の規則どおり**別の `CARGO_TARGET_DIR` に前の版を建て**（`md5sum` で別の
バイナリだと確かめた）、**交互に 6 巡**回し直した。機械は `vmstat` の idle **99%**。

| 巡 | 前: 当たり / 撃破 / 合計 hp / 決着 | 後: 当たり / 撃破 / 合計 hp / 決着 |
|---|---|---|
| 1 | 15 / 1 / 148 / 時間切れ | 25 / 1 / 79 / 時間切れ |
| 2 | 17 / 1 / 109 / 時間切れ | 18 / 3 / 46 / **勝敗** |
| 3 | 19 / 1 / 111 / 時間切れ | 20 / 2 / 56 / 時間切れ |
| 4 | 26 / 2 / 40 / 時間切れ | 13 / 1 / 185 / 時間切れ |
| 5 | 26 / 3 / 16 / **勝敗** | 25 / 2 / 71 / **勝敗** |
| 6 | 21 / 1 / 104 / 時間切れ | 16 / 2 / 116 / 時間切れ |
| 平均 | 当たり **20.7**、hp 88、勝敗 1/6 | 当たり **19.5**、hp 92、勝敗 2/6 |

**前後の分布は見分けがつかない。** 1 巡目だけ見れば 15 対 25 で「後の方が当たる」と読めるし、
4 巡目だけ見れば 26 対 13 で逆に読める——S1 が「当たりの数は走行ごとに 3 倍振れる」と
書いたとおりで、**8 走行を並べただけでは何も言えない**。12 走行とも FAIL 0。

これが `implementer.md` の「前後を比べる計測は必ず交互に 2 巡」の実例になった。
この機械には**同じバイナリが倍遅くなる「遅い状態」がある**と書いてあるが、今回は
「遅い状態」ではなく**隣の担当が回していた**だけで、症状は同じである。

なお**種を固定しても決着は再現しない**（§0）ので、「同じ決着になること」では確かめられない。

## 8. 変えると効くこと

### 遊びの数（Ruby）

**闘技場**: `numbers: { arena: 12.0, min_crates: 5.0 }` で 8 秒走らせると、4 機とも
`(2.8, -2.2)` `(-0.4, 6.2)` `(0.9, 3.3)` `(2.9, 6.1)` に居る。試合は半径 20 の円周に
robot を出すので、**12 の壁の外に出された 4 機が全部中に押し込まれている**——
つまり `arena` が壁（`build_field`）だけでなく `move_robots` の `clamp` にも届いている。

**弾の速さ — 二重が構造的に起きなくなった証拠**。これは一時的な印字を 2 か所に入れて測った
（測った後に外した。コミットには入っていない）: Rust の発射側に「実際にこの速さで出した」、
robot の `run` の頭に「`lead` はこの速さだと思っている」。

| 試合の `numbers:` | `lead` が思う速さ（power 0.5） | 実際に出た弾 |
|---|---|---|
| 既定（55 / 30） | **42.5** | power 0.3 で **47.5**、power 0.9 で **32.5** |
| `shot_fast: 150, shot_slow: 90` | **120.0** | power 0.3 で **132**、power 0.9 で **96** |

どちらも `fast + (slow − fast) × power` で、**同じ 1 組の数から出ている**。
前の版ならここで `lead` は 42.5 のままだったはずで（`prelude.rb` の `SHOT_FAST` は
Rust の `BULLET_SPEED_FAST` と繋がっていない）、scout は的の後ろを撃ち続けたことになる。

**綴り間違いは断られる**: `numbers: { max_sped: 20.0 }` で走らせると

```
ERROR sabibots: the match's numbers were refused: unknown field `max_sped`,
  expected one of `robot_radius`, `hp_max`, `max_speed`, … `min_crates` (TypeError)
INFO  sabibots: script failed: RuntimeError
```

と出て、**ロボットは 1 体も出ない**（`Match#run` が `raise` するので `start` に届かない）。
32 個の名前が全部並ぶのは serde の `deny_unknown_fields` が出すメッセージそのもので、
これが「数を Ruby に置くと綴りを間違えたとき黙って既定が使われる」という心配への答えになる。

### 動かす側の数（`sabibots.settings.txt`）

**旗の既定**: `headless_seconds=5` を置くと `--headless`（数なし）が 6.16 秒で終わり
（5 秒 + 立ち上げと終了）、`--headless 9` は 9.04 秒——**旗が店に勝つ**。

**VM の予算**: `script_budget=300` を置くと起動時に
`setting: the scripts get 300 instructions a frame` と言い、12 秒走行の終わりで
4 機中 3 機が `0 insn/frame`（既定では 2〜3 機が 500 台）。書かなければ 1 行も出ず、
rubevy の既定のままである。

**そのほか**は単体テスト 1 本（`the_store_changes_how_the_match_is_drawn`: 鍵が届くこと、
書いていない鍵が既定のままであること、`look_floor_pattern=-2` が 1 に丸められること——
0 なら剰余が 0 除算になる）。

## 9. ブラウザ: 古い `localStorage` が残っている利用者

**`sabibots:` の下に入りうるのは 2 種類だけ**である。エディタが `Save` で書くのは
`robot.file`（`Watched` は robot なので、試合のスクリプトも `prelude.rb` も**エディタから
保存できない**）と、設定の `sabibots.settings.txt`。鍵は `sabibots:` + パスで
（`games-shell/src/platform.rs` の `key`）、ブラウザでは `ruby_dir()` が裸の `"ruby"` なので
`sabibots:ruby/robots/scout.rb` になる。

前の版の `scout.rb` と、`lang=ja` だけの古い設定ファイルを **`addInitScript` で
ページが動き出す前に localStorage に入れて** `?selftest` を回した:

```
localStorage keys: ["sabibots:ruby/robots/scout.rb","sabibots:sabibots.settings.txt"]
pageerror: 0   requestfailed: 0
selftest: 33 行（一覧と完全一致）  FAIL 0
robots online: 13
```

**古い形でも起動する。** 理由は、robot のファイルが使う DSL の口（`radar` / `incoming` /
`act` / `lead` / `near_wall?` / `on(:hit)` …）が 1 つも変わっていないから——
`radar(45)` のような位置引数も、既定が `60.0` から `nil` になっただけで受け方は同じ。
設定ファイルは書いていない鍵が既定になるので、古い 1 行だけの店もそのまま読める。

**残る 1 つの穴**（直していない、報告する）: プレイヤーが自分の robot に
`Robot::UNSET` や `SHOT_FAST` と**直接書いていた**場合は `NameError` になる。
同梱の scout / hunter は 1 度も書いておらず、`docs/sabiruby-battle.md` も
この 3 つを説明していないので、**書いた人が居るとすれば prelude を読んだ人**である。
互換のために定数を残す案は採らなかった——それは「写しを 1 つ残す」ことそのもので、
この段階が消しに来たものだから。

## 10. 気づいた点

1. **Battle にはまだ一覧に無い直書きの数がある。** 弾のスプライト（`main.rs:2629` の
   `0.7 + 0.6 × power`、`× 0.55` / `× 1.3`）、砲塔のスプライト（`:481` の `1.1 × 2.6`、
   `:499` の 1.0）、名札の字の大きさと縮尺と影のずれ（`:870, 873, 916` の 44 px / 0.045 /
   0.12）、撃破した機体の灰色 2 種（`:459, 503`）、z 座標一式。S5a の網に掛かっておらず、
   **範囲を自分で広げないために移していない**（S5b-1 が同じ判断をしたのと同じ理由）。
   一覧には §4.5 の 2 行として挙げた。属する話: 一覧の抜け。
2. **`crate_size` の分類を案（(c)）から (b) に動かした。** `shrink` が壁を動かす量そのもので、
   `min_crates`（案は (b)）と組でしか意味を持たない。戻すなら `Look` に 1 行。
   属する話: 分類の見直し（著者判断）。
3. **判定の閾値が 1 つ動いた（0.200 → 0.195）。** §7 と `docs/numbers.md` §7-12 に書いた。
   S5b-5 が (d) を見直すときに、ほかにも「実際の値の写し」になっている閾値が無いか
   同じ目で見る価値がある。属する話: (d) の設計。
4. **`ScriptWorld::budget` はタスクごとの持ち分を持たない。** 「予算バーの満 3000 を実際の
   budget から導けないか」を調べた答えで、`tick_scripts` は `task_run_limits` に
   **残り全部**を渡す（`rubevy/src/lib.rs:2760-2775`）。VM の中の時分割は sabiruby の
   スケジューラの話で、rubevy からは見えない。「1 タスクが 1 フレームに使ってよい量」を
   ホストが言いたければ rubevy に口が要る。属する話: rubevy（API）／本の素材。
5. **`Arg::Value` が `nil` を運ぶことは、番兵の値を全部消せるということ。** Battle の
   `UNSET` はこれで消えた。箱庭にも同じ形が無いかは見ていない。属する話: rubevy（API の
   使いどころ）／本の素材（「ありえない値」を約束できないときにどうするか）。
6. **`docs/web.md` の wasm の大きさの表が 2 回ぶんずれた。** S5b-1 が +0.12% / +0.20%、
   S5b-2 が +0.18% / +0.002%。S4 が取り直したきりで、段階ごとに直す約束にはなっていない。
   属する話: 文書と実物のずれ。
7. **隣の担当が回している最中に取った「前」の値は、前の版の値ではない。** §7 の 8 走行ずつが
   そうで、`vmstat` の idle が 0% のときに取った側だけが遅く、**移したせいで当たらなく
   なったように見えた**。`implementer.md` は「同じバイナリが倍遅くなる遅い状態」として
   書いているが、実際には**隣の担当の docker ビルド**でも同じ症状が出る。
   属する話: 計測の作法／本の素材。
8. **「設定にした」の証明は、設定を変えた走行でしか立たない**（S5b-1 の気づき 6 の続き）。
   今回それが効いたのは弾の速さで、**印字を 2 か所に入れて同じ 1 組の数から出ていることを
   見る**までは「たぶん繋がっている」以上のことは言えなかった。属する話: 本の素材。
