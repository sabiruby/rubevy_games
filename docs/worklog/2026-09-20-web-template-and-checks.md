# S4: ページを 1 枚にし、確認の基準を数から行の一覧にする

計画書 `docs/plans/shared-crate-plan.md` の段階 **S4**（3.4 節、4 章の S4 の行、7 章で「S4」と
書かれた 5 件）。ブランチ `shared-crate`、worktree は `rubevy_games-wt-shared`。
`[patch]` はもう要らない（rubevy は push 済み、S4a まで main に入っている）。

やったことは 4 つ ——
**(1)** 2 枚の HTML を 1 枚のテンプレートにする、
**(2)** `web/build.sh` が `CARGO_TARGET_DIR` を見る・`docker/run.sh` がゲーム名から環境変数を決め
example も回せる、
**(3)** Battle の「走行次第で入れ替わる行」を n/a の文面 1 つにまとめ、確認の基準を
`docs/verification/selftest-lines.md` に**行の一覧**として置く、
**(4)** `F5` が押されることを確かめる段を足す。
最後に docs（`web.md`・`README.md` の目次・`README.md` の Layout）。

---

## 1. HTML を 1 枚に

### 何が同じで何が違ったか

`web/garden.html` と `web/sabibots.html` は 71 行と 69 行で、`diff` は 15 行。違っていたのは
7 種類だった:

| 違い | garden | sabibots |
|---|---|---|
| タイトル（2 か所: `<title>` と読み込み画面） | `Garden` | `SabiRuby Battle` |
| 背景・文字・薄い文字の 3 色 | `#12150f` / `#d8dcd2` / `#8d9385` | `#15161a` / `#d8dae0` / `#8a8f99` |
| canvas の id | `garden` | `sabibots` |
| `window.<game>Compile` / `Highlight` の名前 | `gardenCompile` ほか | `sabibotsCompile` ほか |
| `preventDefault` するキーの配列 | `F1 F2 F5 F9 Tab` | `F1 F2 F5 Tab` |
| コンパイラの橋の説明文（2 行 / 1 行） | 「creature の behaviour」+ 名前がゲームのものである理由 | 「robot の behaviour」 |
| キーの説明文（7 行 / 6 行） | F5 は**セーブ**、F9、HUD のボタン | F5 は **Apply**、F1 はガイド |

canvas の id と 2 つの関数名は**ゲームの語そのもの**なので、値を 1 つ（`GAMES_ALL` の 1 語）
にすれば 3 つとも決まる。`localStorage` の接頭辞も同じ語なので、ここを別の値にすると
「語を変えたら公開版のセーブが消える」という罠を 1 つ増やすことになる。だから値にしない。

### 説明文をどうするか — 迷った 1 点

到達点は「できあがる `dist/<game>/index.html` が前と同じ（空白の差を除いて diff 0）」。
ところが違いの下 2 つは**散文**で、1 枚にまとめると必ずどちらかの文面が消える。
3 つ案があった:

1. 中立の文面を 1 つ書いてテンプレートに置く → 出力が変わる（到達点に反する）。
2. 散文もゲームごとの値にする → 出力は保たれるが、3 本目は散文を 2 つ書くことになる。
3. 説明文を消す → `view-source` を開いた人に何も言わなくなる。

**2 を採った。** この 2 つの散文は「このゲームがどのキーをブラウザから取り返すか」と
「このゲームは何をコンパイルするか」で、**中身がゲーム固有**（garden の F5 はセーブ、
Battle の F5 は Apply。正反対）。共通の文にすると嘘になるか、何も言わない文になる。
3 本目が自分のキーについて 6 行書くのは、コピーではなく仕事。

結果の値は 7 つ（`TITLE` / `BG` / `FG` / `DIM` / `KEYS` / `COMPILE_NOTE` / `KEYS_NOTE`）+ 語 1 つ。
テンプレートの `{{…}}` は 8 種類。

### 差し込み方 — sed を捨てた理由

最初は `sed -e "s/{{TITLE}}/$title/"` を書きかけて捨てた。値には `/`・`&`・`'`・`` ` ``・改行が
全部入っている（garden の説明文には「`— the name is the game's`」のアポストロフィ、sabibots の
説明文には「`` `web/garden.html` ``」のバッククォート）。sed に渡すならエスケープ層が要り、
そのエスケープ自体が 2 本目のバグの住処になる。

bash のパラメータ展開 `${page//'{{TITLE}}'/$value}` なら**置換側は一切解釈されない**ので、
何も escape しなくていい。`{` `}` はパターン側でもメタ文字ではない。道具も増えない。
値の側に `{{…}}` が無いことだけ守れば置換の順序も自由で、最後に
`case "$page" in *'{{'*)` で「値の無いプレースホルダが残っていないか」を見て落とす。

値の置き場所は `web/games.sh`（`web/build.sh` が `.` で読む）。散文は
`VALUE[game]=$(cat <<'NOTE' … NOTE)` の形にした。クォート付きヒアドキュメントなので中身は
完全に字義どおりで、`$( )` が末尾の改行を落としてくれるのでテンプレートの行数が合う。
連想配列に `'…'` で多行を書く案は、散文のアポストロフィのたびに `'\''` を書くことになるので捨てた。

### 前後の実測

`web/build.sh` の `write_page` を awk で抜き出して `web/games.sh` と一緒に source し、
旧 2 ファイルと `diff`:

* `garden.html`: **完全一致（0 行）**
* `sabibots.html`: **1 行だけ差**
  ```
  -// belongs to the address bar. The garden's page keeps the same list (`web/garden.html`).
  +// belongs to the address bar. The garden's page keeps the same list (the block below).
  ```

この 1 行は**消えるファイルを指している参照**で、直さなければ自分の変更が作った壊れた参照に
なる。直した。到達点の「diff 0」に対する唯一の逸脱として報告する。

その後の `web/build.sh all` 実走行でも、`web/dist` の 59 ファイルの一覧が前と完全一致、
`dist/garden/index.html` と `dist/index.html` はバイト一致、`dist/sabibots/index.html` は上の
1 行だけ差。

### 3 本目を足してみる

架空の `foundry` を `GAMES_ALL` に 1 語 + 値 1 組（8 行）だけ足して測った。**触った場所**:

| | |
|---|---|
| `web/games.sh` | `GAMES_ALL` に 1 語、値のブロック 1 つ |
| `web/index.html` | 入口ページの `<li>` 1 つ（名前と 1 文の紹介。**散文であって値ではない**） |

それ以外は 0。`web/build.sh` の usage も `GAMES_ALL` から出るので変わらない。`.github/workflows/pages.yml`
は `web/build.sh all` を呼ぶだけなので**変更不要**、`docker/run.sh` も下記のとおりゲーム名から
環境変数を決めるので**変更不要**。できあがった `foundry` のページを読んで、canvas の id・
`window.foundryCompile` / `foundryHighlight`・色・キー配列が全部入れ替わっていることを確認した。
足したものはコミットしていない（`git checkout -- web/games.sh` で戻した）。

入口ページも同じ仕掛けにすれば「1 語と値 1 組」ちょうどになるが、計画書が言うのは
「**テンプレート 1 枚**」なので 2 枚目を作るのはやめた。入口の `<li>` はそのゲームが何なのかを
1 文で書くところで、値というより本文。ここは著者判断に残す（報告の「気づいた点」に書いた）。

この試しで 1 つ見つかった: usage が `[sabibots garden|all]` と空白区切りで出て、
「sabibots garden」という 1 つの選択肢に読める。`IFS='|'` で繋ぐようにした。

---

## 2. 道具 2 つ

### `web/build.sh` と `CARGO_TARGET_DIR`

`wasm-bindgen` に渡すパスが `target/wasm32-unknown-unknown/web/$game.wasm` と相対だった
（S2 が気づいた点に挙げていた）。`CARGO_TARGET_DIR` を立てて呼ぶと、cargo は立てた先に建て、
`wasm-bindgen` は `target/` の**古い成果物**を読む（無ければ落ちる）。
`${CARGO_TARGET_DIR:-$ROOT/target}` にした。

両方で実走行して確かめた。`CARGO_TARGET_DIR=/…/rubevy_games/target` を立てた場合と立てない場合
（worktree の `target/`）で、`web/build.sh all` の出す wasm の大きさが 1 バイトも違わない
（sabibots 35,111,618 / garden 35,828,300）。立てない場合は前からの道なので、これで
「直して壊していない」も言えている。

### `docker/run.sh`

2 つ直した。

**(a) selftest の環境変数**。`-e SABIBOTS_SELFTEST` と書いてあったので、箱庭の窓のチェックは
このスクリプトからは**呼べなかった** —— S1 も S2 も S4a も、`docker run` を手で書き直して
回していた（S1 の worklog に「実際に回せず手で書いた」とある）。ゲーム名を大文字にして
`_SELFTEST` を付ける形にしたので、3 本目は何も足さなくていい。`-e NAME`（`=` 無し）は
呼び出し元の環境から渡す形なので、使い方は `GARDEN_SELFTEST=1 docker/run.sh garden release` のまま。

**(b) example**。`--example <name>` で `/target/<mode>/examples/<name>` を回す。
S3 が `crates/games-shell/examples/camera` を作ったときも、同じ理由で手書きの `docker run`
（scratchpad の `drun-example.sh`）が要っていた。回すには建てる口も要るので
`docker/build.sh --example <name>` も足した（`cargo build --release --example camera`）。

引数の形は壊していない。stub の `docker` を PATH に置いて 7 通り試し、最後に実行される
バイナリと引数を読んだ:

```
docker/run.sh                          → /target/release/sabibots
docker/run.sh sabibots release --headless 15 → /target/release/sabibots --headless 15
docker/run.sh garden release           → /target/release/garden        （-e GARDEN_SELFTEST）
docker/run.sh --example camera         → /target/release/examples/camera
docker/run.sh --example camera debug --foo → /target/debug/examples/camera --foo
docker/run.sh --example camera --shot 3    → /target/release/examples/camera --shot 3
docker/run.sh sabibots --headless 15   → /target/release/sabibots --headless 15
```

最後の 1 行は**前は壊れていた**（`MODE="${1:-release}"` が `--headless` を掴んで
`/target/--headless/sabibots` を探しに行く）。`-` で始まる語は mode ではないという 1 行を足した。

Docker は動いていたので実走行でも確かめた。`docker/build.sh sabibots release` /
`… garden release` で建て、`GARDEN_SELFTEST=1 docker/run.sh garden release` が
**箱庭の窓のチェックを 3 走行とも回した**（43 行・FAIL 0）。これはこのスクリプトでは初めて。

---

## 3. 確認の基準を「数」から「行の一覧」に

### なぜ数が基準にならないか

これまでの基準は「箱庭 ok 43」「Battle 固定 31 行 + 当たり 2 行」。数は**どの行かを言わない**ので、
黙って消えた判定と黙って増えた判定が打ち消し合う。実際、段階ごとに使い捨ての grep を書き直して
比べていて（S2 の `fixedlines.sh`、S3・S4a がコピーしたもの）、その grep が何を捨てているかは
scratchpad にしか書いてなかった。

### 走行次第で入れ替わる 2 組

Battle には「測れた走行と測れなかった走行で**別の文**を出す」判定が 2 組あった。

**(a) 当たりごとの 2 行**（`sabibots/src/main.rs` の `handler_selftest`）。当たりが数えられた
ときは 2 行（`… ran a handler within 0.3 s of the hit at 4.16 s` と `… turned …`）、
数えられなかったときは**別の 1 行**（3 通りの文面: 撃破された／brain が差し替わった／
編集チェックが brain を配っていた）。つまり同じ当たりが 1 行にも 2 行にもなり、文面も 4 通り。

**(b) 撃破の有無**。誰も落ちなかった走行は
`no robot with a handler was down long enough to check its tasks`、落ちた走行は
`the handler tasks of every robot that went down ended (2/2)`。**別の文**。

どちらも n/a の文面を「測れたときと同じ文 + 先頭の判定だけ `--`」に直した。
判定の中身（何を除外するか、閾値、ok/FAIL の条件）は 1 つも動かしていない。

* (a) → 除外された当たりも `ran a handler` と `turned` の 2 行を出し、末尾に理由を付ける。
  どの走行でも「当たり 1 発 = 2 行」。文面は 1 通り。
* (b) → 1 文 3 判定。`--headless 25` で `ok   … ended (1/1)`、`--headless 3` で
  `--   … ended (none was down long enough to check)`。両方実走行で出した。

(b) は**判定が走行によって動く唯一の行**なので、一覧にそう書いた。ここを「除外して比べない」
にしていたのが今までで、それは永久に比べないということ。

### `tools/fixedlines.sh` と `docs/verification/selftest-lines.md`

使い捨ての grep を `tools/` に常設し、**定義**を与えた:

* 残すのは**判定を持つ行**（`ok` / `FAIL` / `--` / `n/a` / `done`）。ト書き
  （`selftest: first meal at 0.55 s` のような、判定ではなく見たこと）は落とす。
* そのうち「走行が持つ数だけ出る行」を名指しで落とす —— Battle の当たりごとの行、
  箱庭のつがいごとの行（`the rules paired two of a species …`、`… measured nothing`）。
  どちらも最後に総数を言う判定が別にあり、それは残る。
* 数を伏せて `sort`。Playwright のログは Chromium の `%c` のスタイル引数を先に落とす
  （PC のログとブラウザのログで同じスクリプトが使える）。

落とす理由はスクリプトの頭に 1 件ずつ書いた。これまでの使い捨て版は「なぜ落とすか」が
worklog にしかなかった。

`docs/verification/selftest-lines.md` には 6 通りの走行（2 ゲーム × ヘッドレス・窓・ブラウザ）の
**行の一覧を全部**書き出し、走行の起こし方、4 つの判定の意味、判定が動く 1 行、
そして箱庭の**揺れる 3 件**（S6 の調査対象）を書いた。揺れる 3 件を書いたのは、
「この 3 つの FAIL は証拠にならない／それ以外の FAIL は証拠になる」を読む人が判断できるようにするため。

`docs/README.md` は、目次に 1 行足したうえで、先頭に「基準は数ではなくこの一覧」と書いた。
worklog の行に出てくる「ok 43」「固定 31 行」はその日の走行の記録なので**書き換えていない**
（過去の記録を後から書き換えると記録でなくなる）。

### 前後（`tools/fixedlines.sh` を通した比較）

S4a の生ログを同じスクリプトに通したものを「前」とした:

| 走行 | 前 | 後 | 差 |
|---|---|---|---|
| Battle ヘッドレス | 3 | 4 | +1（今まで落としていた `handler tasks … ended`） |
| Battle 窓 | 31 | 32 | +1（新しい `F5 is Apply`） |
| Battle ブラウザ | 32 | 33 | +1（同上） |
| 箱庭 ヘッドレス | 13 | 13 | **完全一致** |
| 箱庭 窓（3 走行） | 43 | 43 | **3 走行とも完全一致** |
| 箱庭 ブラウザ | 44 | 44 | 2 走行目が完全一致（1 走行目は下記） |

FAIL は Battle が全走行 0。箱庭のブラウザ 1 走行目だけ
`Revert puts every beetle back on the file` が FAIL —— 計画書 7 章が挙げている**無改変でも
揺れる 3 件のうちの 1 件**で、箱庭のコードは 1 行も触っていない（`web/dist/garden` の wasm は
35,828,300 バイトで S4a と同じ大きさ）。2 走行目は一覧と完全一致で FAIL 0。

---

## 4. `F5` が押されることを確かめる

S4a が見つけた穴: Battle の窓のチェックは `editor.action = Some(EditorAction::Apply)` と
**自分で書いていた**ので、`draw_editor` のキーボードを一度も通っていない。
`Editor::apply_key` が別のキーでも `None` でも 31 件全部通る。S4a で crate の既定が `None` に
なり、`F5` がゲーム側の 1 行（`sabibots/src/main.rs` の `setup_editor`）に移ったぶん、この穴が効く。

窓のチェックの step 2（`sleep 0.05` を `sleep 0.5` に書き換えて Apply するところ）を
`keys.release(KeyCode::F5); keys.press(KeyCode::F5);` に替え、答えを読む step 3 で release して
1 行足した: `F5 is Apply: the key alone applied the text`（条件は `r3.brain.is_some()`。
直前の step で構文エラーが撥ねられて `brain` は `None` なので、brain があることが
「F5 が届いて Apply になった」そのもの）。偽造は箱庭の `F2` / `F3` と同じ手。
`just_pressed` は `PreUpdate` で消えるが `draw_editor` は `EguiPrimaryContextPass`（`Update` の後）
なので、`Update` で押せば同じフレームで見える。

**箱庭に同じ穴はあるか。** 無い。理由を 2 つ確かめた: `garden/src/window.rs:264` で
`editor.apply_key = None` と**わざと**置いている（箱庭の F5 は庭のセーブで、Apply は
Ctrl+Enter）。そして `draw_editor` のキーの道（`editor.rs:314` の `apply_key || (ctrl && Enter)`）は、
箱庭自身のチェックが step 11 → 12 で**本物の Ctrl+Enter を偽造して**通していて
（`window.rs:1439`）、その結果を `Ctrl+Enter: the garden is running the edited rules, without stopping`
が見ている。だから箱庭は触っていない。

実測: 窓（コンテナ、WSLg + lavapipe）で新しい行が ok、残り 31 行は S4a の一覧と完全一致、FAIL 0。
ブラウザ（`sabibots/?selftest`）でも ok —— ページが `F5` を `preventDefault` で取り返しているので
リロードにならず、ゲームに届いていることも同時に見えている。

---

## 5. Playwright のスクリプトを常設するか

計画書 3.4 節は「スクリプトは scratchpad に置く（リポジトリに入れない慣行）」と書き、
`docs/web.md` は「playwright-core, a throwaway script」と呼ぶ。**読んでみると、`web.md` は
理由を書いていない** —— 「使い捨て」と言っているだけ。理由はスクリプトの中にあった:

* `import { chromium } from '/home/kishima/book/kishima/sabiruby-playground/node_modules/playwright-core/index.mjs'`
  —— **隣の repo の `node_modules`**。この repo には `package.json` も `node_modules` も無く、
  ブラウザ確認 1 つのために JS のツールチェインを持つ理由も無い。
* `/home/kishima/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome`
  —— **ビルド番号がディレクトリ名に入った Chromium**。Playwright が更新されれば黙って古くなる。

どちらも 1 台の機械の絶対パス。常設すると「playground を先に用意した人しか動かせず、
黙って腐るファイル」になる。**常設しない**ことにした。

代わりに、**スクリプトが間違えてはいけないこと**を `docs/verification/selftest-lines.md` に
文章で残した: `--use-angle=swiftshader --enable-unsafe-swiftshader`、
`page.goto(url, { waitUntil: 'commit' })`（モジュールスクリプトが `await game.default()` で
返らないので `'load'` は正常なページでタイムアウトする）、`pageerror` と `requestfailed` を数えて
どちらも 0、そして console のログを `tools/fixedlines.sh` に通す。

常設する価値があったのは**比べ方のほう**（`tools/fixedlines.sh`）で、そちらは依存が無く、
一覧はそれが無いと意味を持たない。

---

## 6. docs

* `docs/web.md`: ページの作り方を節にした（テンプレート、値の表、語を値にしない理由、
  sed を使わない理由、3 本目の費用）。Battle の `?selftest` の行に書いてあった「31 行 + 当たり 2 行」は
  一覧への参照に。**wasm の大きさの表を取り直した** —— 前の表は sabiruby 0.5.1 のころ、
  G6 の日本語フォント（+90 KB）より前のもので、4 列が同じ日の数ではなかった。
  今回は 2 ゲーム × 4 列を同じ数分の間に測った:

  | | bytes | gzip -9 | `wasm-opt -Os` | その gzip |
  |---|---|---|---|---|
  | `sabibots/pkg/game_bg.wasm` | 39,100,735 | 10,026,050 | 35,111,618 | 10,736,516 |
  | `garden/pkg/game_bg.wasm` | 39,873,551 | 10,243,952 | 35,828,300 | 10,947,996 |

  生の列は `wasm-bindgen` の出力を別ディレクトリに出して測った（`web/build.sh` は
  `wasm-opt` を同じファイルに上書きするので、走行を 2 回に分けずに両方取れる）。
  表から導いていた 2 つの数も引き直した: `wasm-opt` は生を 10.2% / 10.1% 減らして gzip を
  7.1% / 6.9% 増やす（結論は変わらない）、箱庭は Battle より **2.0%** 大きい（1.2% ではなく）。
  S4a の +6,952 / +4,551 バイトと S4 の +92 バイトは、それぞれ自分の版に対する差として別に書いた。
* `docs/README.md`: 目次に `verification/selftest-lines.md` を足し、冒頭に基準の置き場所を書いた。
* `README.md`: Layout の `web/` の行をテンプレートと値に、`tools/` を一覧に足した
  （フォント以外のものが入ったので）。コマンド例は crate が 2 つになった後も全部そのままで正しい
  （`cargo run -p sabibots` ほか 7 行、`web/build.sh` の 2 行）。
* 消えた `web/garden.html` / `web/sabibots.html` を指していた**doc コメント 5 か所**
  （2 本の `platform.rs` の橋の定数、箱庭の `reload_asked_at` とセーブのキーの注、
  `games_shell::checks::asked_number`）を `web/games.sh` とテンプレートに向け直した。

---

## 確認したこと（まとめ）

* `cargo build --workspace` 警告 0、`cargo test --workspace` **34 passed**（着手前と同じ）。
* Battle ヘッドレス（`--headless 25`、`--headless 3`）: 新旧両方の n/a の枝を実際に出した。FAIL 0。
* Battle 窓（コンテナ）: 32 行、S4a の 31 行 + `F5 is Apply`。FAIL 0。
* 箱庭 窓（コンテナ、3 走行）: 43 行が 3 走行とも S4a と完全一致。FAIL 0。
  **`docker/run.sh garden` で回せたのは今回が初めて**。
* 箱庭 ヘッドレス（`--headless 90`）: 13 行が完全一致。FAIL 0。
* `web/build.sh all`: `CARGO_TARGET_DIR` を立てた場合と立てない場合の両方で成功し、
  wasm の大きさが一致。`web/dist` の 59 ファイルの構成が前と同一。
* ブラウザ（headless Chromium + SwiftShader、`waitUntil:'commit'`）:
  `sabibots/?selftest` は pageerror 0 / requestfailed 0 / 33 行（S4a + `F5 is Apply`）、
  `garden/?selftest` は 2 走行とも pageerror 0 / requestfailed 0 で、2 走行目が 44 行完全一致、
  1 走行目だけ既知の揺れ 3 件のうち 1 件が FAIL。

ログとスクリプトは scratchpad の `s4/`。

---

## 気づいた点

1. **入口ページ `web/index.html` だけ手書きのまま**。3 本目を足すとき、`web/games.sh` の
   1 語 + 値 1 組のほかに、ここに `<li>`（ゲーム名 + 1 文）を手で足すことになる。
   計画書が「テンプレート 1 枚」と言っているので 2 枚目は作らなかった。
   紹介文は値というより本文なので、このままでよい気もする。 —— `web/index.html:32-39` ／
   道具（3 本目の費用）／著者判断。
2. **`docker/run.sh` は `<GAME>_SELFTEST` しか渡さない**。箱庭には `GARDEN_RELOAD_AT` という
   2 つ目の環境変数があり（`checks::asked_number`、チェックが F9 を押す唯一の道）、
   コンテナからは渡せない。今の窓のチェック 43 件はこれを使わないので困っていないが、
   使う判定が増えたら同じ手当てが要る。 —— `docker/run.sh` ／ 道具。
3. **example を建てるのは `cargo test --workspace` のほうで、`cargo build --workspace` は建てない**。
   仮説として「example はどちらでも建たない」と書きかけたので測った: `examples/camera.rs` に
   型の合わない 1 行を足すと `cargo test --workspace` は
   `error: could not compile games-shell (example "camera")` で落ち、`cargo build --workspace` は
   0.16 秒で `Finished` と言う。つまり各段階の合否に使っている
   「`cargo build --workspace` 警告 0」は example を見ていない。見ているのは `cargo test` のほう
   なので穴は無いが、`build` だけで済ませた段階があれば見落としている。 ——
   `crates/games-shell/examples/camera.rs` ／ 共有 crate・確認の作法。
4. **`--` と `n/a` が混ざっている**。Battle は「測れなかった」を `--` と書き、箱庭は
   `--`（つがい）と `n/a`（probe）の両方を使う。意味は同じ。今回は文面だけ揃えて記号は
   触らなかった（動きを変えない範囲に留めるため）が、1 つにすれば
   `tools/fixedlines.sh` の判定の正規表現も一覧も 1 語短くなる。 —— `sabibots/src/main.rs`、
   `garden/src/main.rs:4564,4990,5898`、`garden/src/window.rs:1275` ／ 確認の作法。
5. **箱庭のブラウザ走行でも揺れが出た**。`Revert puts every beetle back on the file` が
   2 走行中 1 走行で FAIL。計画書 7 章の揺れ 3 件は PC の窓で記録されたものだが、
   同じ判定がページでも落ちる。S6 の調査は PC だけで完結しないかもしれない。
   再現: `web/build.sh all` → `web/serve.sh` → `garden/?selftest` を 160 秒。 —— ゲーム固有
   （箱庭の判定）／ S6 への材料。
6. **`web/build.sh` の usage が `GAMES_ALL` をそのまま展開していた**（`[sabibots garden|all]`）。
   3 本目を足してみて気づいたので直した（`IFS='|'`）。書いておく。 —— `web/build.sh` ／ 道具。
