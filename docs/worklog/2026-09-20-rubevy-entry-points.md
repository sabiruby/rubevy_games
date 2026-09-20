# 2026-09-20 S2: 2 本のゲームが rubevy の口（R6）に載り替える

計画書 `docs/plans/shared-crate-plan.md` の段階 **S2** のみ。ブランチ `shared-crate`（worktree
`rubevy_games-wt-shared`、S1 の `8f836d4` の上。main には触っていない）。
S1 が「2 本が同じ形で書いていた `platform.rs`」を共有 crate へ上げたのに続いて、ここで消すのは
**rubevy に上がったもの** —— `build.rs`、prelude つきプログラムの組み立て、コンパイラのエラーの行番号の補正、
スクリプトの差し替え —— の 4 つ。rubevy 側の記録は rubevy の `docs/worklog/2026-09-20-shared-entry-points.md`
（コミット `cb8bf00`）。

**R6 は両端しか確かめていない。** rubevy の worklog が自分でそう書いている:
「build.rs → `include!` → `EmbeddedHost` の通しは、この repo では通していない。build script は自分の crate の
ビルド中にしか動かないので」。その通しの**最初の客がこの段階**で、下の §2 がその確認にあたる。

## 0. 着手前の基準（2026-09-20、i7-13700。全部この段階で取り直したもの）

| 何を | どう回したか | 結果 |
|---|---|---|
| `cargo test --workspace` | そのまま | **22 通過 / 0 失敗**（garden 6、rubevy-arena 16）。S1 の報告と同じ |
| 箱庭ヘッドレス | `GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90` | S1 の後の走行ログ（`ok 13` / `FAIL 0`）を基準に使った |
| 箱庭の窓 | docker（下記） | S1 の 3 走行目 `ok 43` / `FAIL 0` を基準に使った |
| Battle ヘッドレス / 窓 / ブラウザ | 同上 | S1 の走行ログ。当たりに依らない行の集合で比べる |

窓が要るものは、この機械に GPU ドライバが無いので S1 と同じくコンテナ（lavapipe）で回した。`docker info` は
通ったので**起動はしていない**。`docker/run.sh:23` が `-e SABIBOTS_SELFTEST` 固定なのは S1 の報告のとおりで、
今回も箱庭は自前の `docker run` を書いた（スクラッチパッドの `drun.sh`。ゲーム名から環境変数の名前を作る 1 行）。

## 1. rubevy の取り込み方 —— 一時の `[patch]` と `Cargo.lock`

R6（`cb8bf00`）はまだ push されていないので、GitHub の rubevy には `Program` も `replace_script` も
`rubevy-build` も無い。worktree の根に**コミットしない** `.cargo/config.toml` を置いて、切り離した checkout
`rubevy-wt-r6` を指した:

```toml
[patch."https://github.com/sabiruby/rubevy"]
rubevy = { path = "/home/kishima/book/kishima/rubevy-wt-r6" }
rubevy-build = { path = "/home/kishima/book/kishima/rubevy-wt-r6/rubevy-build" }
```

`.gitignore` に `.cargo/` は無いので、コミットは `git add` でパスを数えて作り、`git status` で
`.cargo/` が `??` のままであることを確かめた。**この段階のコミットは「rubevy が push されれば通るが、
今の GitHub の rubevy では通らない」状態**で、それは承知のうえ（main に入れるのは rubevy の push の後）。

`rubevy-build` は新しい package なので `[workspace.dependencies]` に
`rubevy-build = { git = "https://github.com/sabiruby/rubevy" }` を足し、2 本の `[build-dependencies]` は
`rubevy-build.workspace = true` の 1 行にした。**指示は「2 本の `Cargo.toml` に git の行を書く」**だったが、
根の `Cargo.toml` が「One Bevy for the whole repository … keeping the version in one place is what makes
bumping it a single edit」と自分で書いている流儀に合わせた。git の URL が 1 か所で済み、rubevy の行のすぐ下に
並ぶ。patch の表は `rubevy-build` という名前を見るので、どちらの書き方でも効く。

**`Cargo.lock` はコミットに入れていない。** patch が効いた lock は rubevy の `source` 行が消える
（`docs/worklog/2026-09-18-rubevy-published.md` に前例）。実際の差分は

- `rubevy` の `source = "git+…#2d9eb92…"` が 1 行消える、
- `[[package]] rubevy-build` が 4 行増える、
- 2 本のゲームの依存に `rubevy-build` が 1 行ずつ増える、
- （patch とは無関係に）`windows-sys 0.60.2` を引いていた 4 つが `0.61.2` に寄る。

最後のは再解決のついでに起きたもので、この repo のビルドには関係しない（WSL / ブラウザのどちらでもない）。
**どれも rubevy が push された後に本体が取り直すもの**なので、commit の直前に `git restore Cargo.lock` して
パス指定で add した。lock を触らないとビルドが再現しない事情は無かった（patch は `.cargo/config.toml` 側にある）。

## 2. 2 本の `build.rs` —— 35 行が 2 行に、通しが動いた

garden と sabibots の `build.rs` は 35 行で `diff` が 1 行も出ない同じファイルだった。中身は
`rubevy_build::Embed::new("ruby").write()` の 1 行になり、残りは冒頭の説明（両方 13 行）。

**既定値がそのまま使えた**のが大きい。`Embed` の既定は `.rb` / `pub static RUBY_FILES` / `ruby_files.rs` で、
rubevy の rustdoc が「出どころは 2 本のゲームの build.rs がそう書いていたこと」と明記している。おかげで
`src/platform.rs` の

```rust
include!(concat!(env!("OUT_DIR"), "/ruby_files.rs"));
```

は 1 文字も変えていない。**build.rs → `include!` → ゲームが表を引く、の通しが動く**ことは、ブラウザの走行が
証拠になる: `garden/?selftest` と `sabibots/?selftest` はどちらも `platform::read` が埋め込みの表から
`prelude.rb` や `world.rb` や `scout.rb` を引けないと 1 行も進まないので、ok 43 / 32 行が出ている時点で
表は書かれ、`include!` され、引けている（§6）。

ついでに S1 の報告 2 番「`build.rs` の冒頭のコメントが実物と違う」（「The PC build … gets the table too,
but does not use it」）もここで消えた。今の 2 本は「PC は表の代わりに空の表を回している」と書いている。

`Embed::write` は `cargo:rerun-if-changed` をディレクトリと**ファイル 1 つずつ**の両方に出す。前の 35 行は
ディレクトリだけだった（`println!("cargo:rerun-if-changed=ruby")` と、再帰の途中のサブディレクトリ）。
つまり再ビルドの条件は**厳しくなった**方向で、緩んでいない。

## 3. `Program` と `in_the_authors_lines`

garden の `compile_source` / `compile_world_source`、sabibots の `compile_text` の 3 か所が

```rust
let src = format!("{prelude}\n# ---- {name} ----\n{body}\n{tail}\n");
let prelude_lines = prelude.lines().count() as u32 + 2;
```

と書いていたのを `Program::new(&prelude, name, body, tail)` にした。garden の `in_the_authors_lines` と
`one_diagnostic`（合わせて 44 行）と、そのテスト（1 本、7 つの assert、39 行）は消して rubevy のものを呼ぶ。
rubevy の `tests/source.rs` には同じ 7 ケースが creature の名前を抜いた形で入っていて、そのうえ
**本物のコンパイラを走らせて本物のメッセージを動かすテストが 2 本**増えている（ブラウザの
`playground.rb` を模したものを含む）ので、ここに残す価値のあるものは無かった。

### `prelude_lines` が同じ数であること

rubevy は数え方を変えている（`prelude.lines().count() + 2` → 前置きの文字列そのものを数える）。
改行で終わる prelude なら同じ数、終わらない prelude なら 1 少ない、というのが rubevy の worklog の説明。
**3 本の prelude は全部改行で終わっている**（`tail -c 1` が `0a`）ので同じ数になるはずで、それを 2 通りで確かめた:

1. 2 つの式を同じ実物のファイルに当てる小さな Rust（スクラッチパッドの `preludelines.rs`）:

```
garden/ruby/prelude.rb             old  482  new  482  same true
garden/ruby/world_prelude.rb       old  376  new  376  same true
sabibots/ruby/prelude.rb           old  297  new  297  same true
```

2. 実際に走らせて印字（作業中だけ入れた `info!("S2TMP …")`、コミット前に外した）:

```
S2TMP world prelude_lines=376
S2TMP creature prelude_lines=482 (beetle.rb)
S2TMP creature prelude_lines=482 (rabbit.rb)
S2TMP robot prelude_lines=297 (scout.rb)
S2TMP robot prelude_lines=297 (hunter.rb)
S2TMP robot prelude_lines=134 (training.rb)
```

これは `VmInspector::fill` に渡る数そのもの（`Mind::prelude_lines` / `Robot::prelude_lines`）なので、
VM パネルの行番号も前と同じ。`training.rb` の 134 は試合の prelude（`match_prelude.rb`）で、
`compile_with` が prelude のファイル名を引数に取る作りがそのまま生きている。

### 箱庭の行番号が今までどおりであること

`beetle.rb`（135 行）と `world.rb`（314 行）の末尾にそれぞれ `1 < < 2` を足して 3 秒だけ走らせ、
すぐ `git checkout` で戻した:

```
beetle.rb: beetle.rb:136:5: syntax error, unexpected '<'; expected an expression after the operator
the world has no rules: world.rb: world.rb:315:5: syntax error, unexpected '<'; expected an expression after the operator
```

136 行目・315 行目は、足した行そのもの。種のファイルも `world.rb` も、著者の行のままで出ている。

## 4. `replace_script`

`sabibots::restart`（3 行）と `garden::window::restart_species` の中の同じ 3 行を
`rubevy::replace_script` に替えた。`ScriptDone` を外すのを忘れると「一度終わったスクリプトには
二度と別のを載せられない」という見えないバグになる、という rubevy の rustdoc の話をコメントに 1 行ずつ残した。

**計画書が名指しした 2 か所だけ**にした。同じ 3 行はほかに 2 か所ある（`garden::wear_the_rules` の
`Script::<World>::for_vm` 版と、`sabibots` のファイル監視からの再読み込み `main.rs:2304-2308`）。
`replace_script` は `M` で総称化されているのでどちらも通るが、範囲外なので触らず「気づいた点」に挙げる。

## 5. sabibots のエディタに著者の行番号が出る（この段階で直るただ 1 つの振る舞い）

`compile_text` はもともと `prelude_lines` を返していたのに、エラーの文字列はコンパイラのものを素通しで、
`do_editor_actions` の `editor.message = format!("not applied: {e}")` に届く番号が prelude ぶん
（**297 行**。計画書は 295 と書いている。`wc -l` が 295 で、区切りの `# ---- name ----` と空行を足して 297）
ずれていた。

補正は**呼び出し側 4 か所ではなく `compile_text` の中**に置いた。`compile_text` の答えは
「ログに出るか、エディタに出るか、両方か」で 4 通りに分かれていて、そのどれもが著者の行を見たいから。
garden が `compile_source` の中でそうしているのと同じ形でもある。

### selftest に 1 行足した

既存の編集チェック（`selftest`、窓のときだけ走る）の**先頭**に段を 1 つ増やした:

1. 段 0 で robot 3 を選んでファイルを読む（既存）。
2. **新しい段 1**: そのファイルの末尾に `1 < < 2` を足したものをエディタに打ち込み、Apply を押す。
3. **段 2**（元の段 1）の先頭で `editor.message` を見る。

「どの行になるはずか」は書き下さず、打ち込んだテキストから数える
（`a_text_that_will_not_compile`。ファイルが伸びても正しいまま）。判定は
「メッセージに `:<その行>:` が入っている」と「robot 3 の brain が入れ替わっていない」の両方。
段の間隔 0.5 秒は、この列のほかの全部の段と同じ値を使った（`do_editor_actions` は次のフレームで走る）。

実際の出力（窓、コンテナ）:

```
selftest: ok   a syntax error is refused on the author's own line 68:
  not applied: scout.rb: scout.rb:68:5: syntax error, unexpected '<'; expected an expression after the operator
```

`scout.rb` は 67 行なので、足した行は 68 行目。**直す前なら 68 + 297 = 365 と出ていた**数字である。
ブラウザでも同じ段が走り、ファイル名だけが変わる:

```
selftest: ok   a syntax error is refused on the author's own line 68:
  not applied: scout.rb: playground.rb:68:5: syntax error, unexpected '<'; expected an expression after the operator
```

ページの橋はソースしか受け取らないので、どんなプログラムも `playground.rb` と名乗る。
R6 の `one_diagnostic` はファイル名を見ずに「行の中で最初に出る `:数字:数字:`」を探すので、これも素通りする
（rubevy の `the_browser_bridges_message_moves_too` がテストで押さえているとおり）。

段を 1 つ増やしたぶん、Battle の編集チェックの列は 0.5 秒ほど後ろにずれる。**ヘッドレスには影響が無い**
（編集チェックは窓のときだけ登録される。`main.rs:428` の `checks_asked && headless.is_none()`）。

## 6. 確認

| 何を | 前（S1 の後） | 後 |
|---|---|---|
| `cargo test --workspace` | 22 通過 / 0 失敗 | **21 通過 / 0 失敗**（garden 5、rubevy-arena 16）。減った 1 本は rubevy に移した行番号のテスト |
| 箱庭ヘッドレス 90 s | `ok 13` / `FAIL 0` / `-- 6` | `ok 13` / `FAIL 0` / `-- 6`。**13 行の本文は数字を伏せて `diff` して完全一致** |
| 箱庭の窓（docker） | `ok 43` / `FAIL 0` | `ok 43` / `FAIL 0`。**43 行が数字を伏せて完全一致**。揺れは 1 走行では出なかった |
| Battle ヘッドレス 25 s | 当たりに依らない行 3 行 | 同じ 3 行（`diff` 一致） |
| Battle の窓（docker） | 当たりに依らない行 **30** | **31**。増えた 1 行は §5 の新しい判定だけ（`diff` で確認） |
| ブラウザ `garden/?selftest` 160 s | pageerror 0 / requestfailed 0 / `ok 43` | **pageerror 0 / requestfailed 0 / `ok 43`**。43 行が `diff` 一致 |
| ブラウザ `sabibots/?selftest` 70 s | pageerror 0 / requestfailed 0 / 当たりに依らない **31** 行 | **pageerror 0 / requestfailed 0 / 32 行**。増えた 1 行は §5 の判定だけ |
| `web/build.sh all` | — | 通る。`wasm-opt`（`~/.local/binaryen-version_132/bin`）が効いて garden 35.8 MB（gzip 10.9）、sabibots 35.1 MB（gzip 10.7） |

### 比べ方について

Battle は**行数で比べられない**。当たりの数が走行ごとに大きく振れるからで（S1 は 25 秒で 20 発と 9 発）、
S1 の担当が決めたとおり「当たりに依らない行の集合」で比べた。スクラッチパッドの `fixedlines.sh` が落とすのは:

- `… of the hit at 3.61 s`（1 当たりにつき 2 行）、
- `… of the hit on 3 blue/scout at 3.10 s`（編集チェックの最中の当たりを除外した記録。**当たりが無ければ出ない**ので
  これも当たり次第。S1 の比べ方に今回足した 1 件）、
- `the handler tasks of every robot that went down ended` と `no robot with a handler was down long enough`
  （誰かが撃破されたかどうかで入れ替わる。前後で入れ替わったのを見て足した）。

残りは数字を全部伏せて `sort` してから `diff`。この 3 種を落とさないと「無改変でも毎回違う」ので、
比較の基準にならない。

箱庭の窓は 1 走行しかしていない（S1 が「無改変でも 3 走行中 2 走行が 1 件ずつ FAIL した」と記録した揺れがある）。
今回の 1 走行は `FAIL 0` だったが、**1 走行では揺れについて何も言えない**。43 行の本文が一致したことだけが言えること。

## 7. やらなかったこと

- `require` への切り替え（`EmbeddedHost`）は計画書 3.2 のとおり**していない**。`EmbeddedHost` は 1 行も呼んでいない。
- `wear_the_rules` と sabibots のファイル監視の同じ 3 行は、計画書が名指ししていないので替えていない（§4）。
- `cargo fmt` は走らせていない（S1 の worklog に従う）。`git stash` も使っていない。
- docs（`docs/web.md`、`README.md` の Layout）は S4 の仕事。ここでは `docs/README.md` の目次にこの記録を足しただけ。

## 気づいた点（S2 の範囲外。直していない）

1. **`replace_script` に替えられる同じ 3 行がまだ 2 か所ある。** 何を: `ScriptTask` / `ScriptDone` を外して
   `Script` を挿す 3 行。どこで: `garden/src/main.rs` の `wear_the_rules`（`Script::<World>::for_vm` の版）と
   `sabibots/src/main.rs:2304-2308`（ファイルが外から変わったときの再読み込み）。なぜ気になるか: S2 の目的は
   この 3 行を 1 か所にすることで、`replace_script` は `M` で総称化されているのでどちらもそのまま通る。
   計画書 3.2 が名指ししたのは `restart` と `restart_species` の 2 つだけなので触らなかった。
   どこに属するか: 計画書（S2 の範囲の書き漏らし）。次の段階かレビューで拾うのがよい。

2. **`Program::new` の `&str` 4 本は、呼んでみても取り違えなかった。** 何を: R6 の報告 2 番が
   「`name` と `body` を逆に渡してもコンパイルが通る」と心配していた点。どこで: `rubevy::Program::new`。
   なぜ気にならなかったか: 3 か所とも `Program::new(&prelude, name, body, "run_creature")` の形で、
   **変数名がそのまま引数の順に並ぶ**（`prelude` / `name` / `body`）。唯一ひっかかったのは 4 本目の `tail` で、
   これだけリテラル（`"run_creature"` / `"run_world"`）なので、順番ではなく「ここに `run_*` が来る」と
   覚えることになる。ビルダを足す必要は感じなかった。どこに属するか: rubevy（API の使い勝手、R6 への返事）。

3. **足りない口が 1 つあった: `Program` には「コンパイラに渡すファイル名」の置き場所が無い。** 何を:
   `Program::new` の `name` は区切りコメント (`# ---- name ----`) にしか使われず、実際にコンパイラへ渡す
   `filename` は呼び出し側が別に持つ（`platform::compile(&program.source, name)`）。どこで:
   `rubevy/src/source.rs:55` の rustdoc がそう明言している。なぜ気になるか: 3 か所とも同じ `name` を
   2 回書くことになり、片方だけ変えると区切りコメントとコンパイラのファイル名が食い違う（エラー文の見た目が変わる）。
   rustdoc は「コンパイラのオプションは呼び出し側のもの、ブラウザでは選べない」と理由を書いていて筋は通っているので、
   直すなら `Program` に `name` を残して読めるようにする程度。どこに属するか: rubevy（API）。

4. **`docker/run.sh:23` の `-e SABIBOTS_SELFTEST` は今回も踏んだ。** S1 の報告 1 番と同じで、箱庭の窓の
   チェックはこのスクリプトでは回せない。今回もスクラッチパッドに自前の `drun.sh` を書いた（ゲーム名から
   `GARDEN_SELFTEST` / `SABIBOTS_SELFTEST` を作るだけ）。計画書は S4 に置いている。
   どこに属するか: 道具（`docker/`）。

5. **コンテナのビルドは `[patch]` のパスをコンテナの中に持っていない。** 何を: `docker/build.sh` と
   `docker/run.sh` は repo を `/app` に mount するだけなので、`.cargo/config.toml` が指す
   `/home/kishima/book/kishima/rubevy-wt-r6` はコンテナの中に無い。どこで: `docker/build.sh:14-18`。
   なぜ気になるか: 今回は同じ絶対パスに読み取り専用で mount して逃げたが、これは S2 の一時 patch のためだけの
   事情なので、rubevy が push されれば消える。記録として残す。どこに属するか: この段階の事情（S2 限り）。

6. **Battle の selftest の「当たりに依らない行」は、S1 が数えた 30 行より狭い。** 何を: 上の §6 に書いた 2 種
   （編集チェック中の当たりの除外行、撃破された robot のハンドラの行）も走行ごとに出たり出なかったりする。
   どこで: `sabibots/src/main.rs:1308`、`:1429`。なぜ気になるか: 「31 行」「30 行」という基準が
   `docs/README.md` と計画書に書かれているが、その中に走行次第で入れ替わる行が混じっている。
   前後比較のたびに気づき直すより、判定の側で n/a の行を 1 つの決まった文面にするか、基準の書き方を
   「この N 行」と列挙する方が確実だと思う。どこに属するか: 確認の作法／本（自動テストの比べ方）の素材。

7. **`web/build.sh` は `CARGO_TARGET_DIR` を見ない。** 何を: `target/wasm32-unknown-unknown/web/<game>.wasm`
   を相対パスで読むので、`CARGO_TARGET_DIR` を立てて呼ぶと `failed reading 'target/…'` で落ちる
   （最初にそれで 1 回落とした）。どこで: `web/build.sh:48`。なぜ気になるか: この repo の作法として
   `CARGO_TARGET_DIR` を使う指示が担当に出ることがある。`${CARGO_TARGET_DIR:-target}` の 1 語で済む。
   どこに属するか: 道具（`web/`）。
