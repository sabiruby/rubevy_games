# 共有部分を 3 本目の前に上げる — 実装指示書

作成 2026-09-20。著者「先に crate へ上げる」「（カメラは）案 1 で。要は Ruby からカメラとかゲームに必要な IF を使える口があるとうれしい」。
調査は `docs/worklog/2026-09-20-factory-survey.md`（§3・§5 と著者の返事。**着手前に全部読む**）。
rubevy 側の対になる計画は rubevy `docs/plans/generalize-plan.md`（段階 R0〜R10）。3 本目そのものは `factory-plan.md`。
対象: rubevy_games main `b1ce042`、Bevy 0.19.1。行番号は調査時点のもの。着手前に確かめる。

---

## 0. はじめの一歩

```bash
cd /home/kishima/book/kishima
git -C rubevy_games worktree add ../rubevy_games-wt-shared -b shared-crate main
cd rubevy_games-wt-shared
cargo test --workspace
# 基準を控える: 2 本の selftest を今の手順どおりに回し、ok / n/a / FAIL の行数を worklog に書く。
# 回し方（環境変数 GARDEN_SELFTEST / SABIBOTS_SELFTEST、走らせる長さ）は docs/garden.md と docs/sabiruby-battle.md の
# 今の記述に従う。長さをここで新しく決めない。
```

作法は `/home/kishima/book/.claude/agents/implementer.md` と `/home/kishima/book/CLAUDE.md`。段階ごとに 1 コミット、push しない、main に触らない、
過程は `docs/worklog/` に書きながら。**Docker Desktop を起動しない。**
S2 以降は rubevy の `generalize` ブランチ（R6 まで済んだもの）を使う。取り込み方（`[patch]` で隣の worktree を指す／git の rev）は、
コミットに残してよい形かどうかを含めて着手時に本体に確かめる。

---

## 1. 何を作るのか（30 秒版）

garden と sabibots が同じ形で 2 回書いているものを、3 本目が 3 回目を書く前に 1 か所へ上げる。置き場所は 3 つ:

- **rubevy**（スクリプトのつなぎこみそのもの。rubevy 側の計画 R6）: prelude つきプログラムと行番号補正、スクリプトの差し替え、埋め込みの `Host`、`.rb` の埋め込み。
  → ここでは**使う側に回る**（S2）。sabibots の「エラーの行番号が prelude 295 行ぶんずれる」バグ（`sabibots/src/main.rs:1685`）がこれで直る。
- **共有 crate**（`crates/rubevy-arena`。画面・ファイル・ブラウザに依存し、ゲームの語彙は無いもの）: `platform` の共通部分、`?selftest` の枠、引数解析、
  起動の Settings / Guide、**パン・ズームできる 2D カメラ**と Ruby からの問いに答える側。
- **ゲームに残すもの**: エディタの選択肢、何を再起動するか、selftest の中身、語彙。

**動きは変えない。** 2 本の selftest の行数と結果（garden は ok 43、Battle は固定 31 行 + 当たり 2 行。`docs/README.md`）が前後で同じであること。
直るのは sabibots の行番号だけ。

---

## 2. 決まっていること・既定

### 決まっている（著者）

- 人が操作するカメラは共有 crate。Ruby からカメラを動かす口は rubevy（R8〜R9）で、共有 crate は**答える側**を持つ。
- `rubevy-arena` は crates.io に出さない（構成を見直してから。2026-09-19）。**この計画は出す準備ではない。** crate 名も変えない。
- unsafe を増やさない。wasm でも動くこと（`std::time::Instant`・スレッド・`std::fs` を wasm の道に置かない）。

### 既定（違和感があれば止めて報告する）

- 共有 crate の中はモジュールで分ける（`platform`、`checks`、`camera`、既存の `editor` / `inspect` / `guide` / `settings`）。crate を割らない。
- 使われていないもの（`HudPlugin`、`CodePanel`、`read_script`）は**消さない**。報告に挙げるだけ。
- `Editor` の `Default`（`noun: "robot"` ほか、`editor.rs:146-149`）は中立の語に変える。2 本とも 4 つを上書きしているので見た目は変わらないはず。変わるなら止まって報告。
- **数は利用者が変えられる場所に置く**（著者、2026-09-20「マジックナンバーは基本的に禁止。ユーザが変えられるようにするべき」）。
  共有 crate が持つ数（カメラの速さ・ズームの刻みと範囲・`CLICK_SLOP`・エディタの `MARGIN` / `WIDTH` / `HEIGHT`・カメラをずらす 16%）は
  **Resource かプラグインの設定値**にし、app が起動時に変えられ、向くものは `Settings`（`key=value`）からも変えられる形に。
  `const` は既定値の名前としてだけ残し、出どころをコメントに 1 行。移すついでに見つけた埋め込みの数は一覧にして報告する。
- HTML は 1 枚のテンプレートにして `web/build.sh` が名前を差し込む。preventDefault するキーの配列はゲームごとなので、差し込む値にする。

---

## 3. 設計

### 3.1 `platform`（S1）

2 本の `platform.rs`（garden 261 行 / sabibots 201 行）は、コメントと crate 名を除けば、差は garden の追加 4 つ
（`SAVE_FILE`、`SAVE_WHERE`、`reload_asked_at`、`another_version_file`）だけ。`garden/src/platform.rs:5-11` が「3 本目が来たら共有する価値がある」と書いている。

- **`read` / `write`**（PC はファイル、ブラウザは `localStorage` の `"<game>:" + path`）。ゲームごとに違うのは接頭辞 1 つ。
  `Settings::load` は素の `fn` ポインタで `read` / `write` を受ける（`settings.rs:34-37`）。接頭辞を値で持つと素の `fn` にならないので、
  `Settings` の受け口を変えるか、ゲーム側に 2 行の包みを残すかを選ぶ。小さい方を採り、理由を worklog に。
- **ブラウザのコンパイルとハイライトの橋**: `#[wasm_bindgen(js_name = gardenCompile)]` は属性にリテラルを書くので名前を引数にできない。
  `js_sys::Reflect::get(&window, name)` で名前を `&str` で引く形にする（ページ側の名前 `window.<game>Compile` / `<game>Highlight` は変えない）。
  unsafe は要らないはず。要るなら止まって報告。
- **`ruby_dir` / `assets_dir`** は `env!("CARGO_MANIFEST_DIR")` が「展開された crate の場所」になるので関数では共有できない。`macro_rules!` にするか、ゲームに残す。
- **`clock_seed`**（`SystemTime` ⇄ `Date.now()`）は共有 crate へ。
- **`selftest_asked(env_name)` と `CHECKS_EXIT_WHEN_DONE`** は `checks` モジュールへ。ブラウザでは `AppExit` を書かない理由（`docs/web.md:360-377`）を rustdoc に移す。
- 引数解析（`--headless N` / `--shot FILE SEC` / `--vm` / `--lang`）と、起動の 10 行（`Settings::load` → `GuideLang::pick` → `Guide`）を共有 crate へ。
  garden 固有の引数（`--save` / `--load` / `--at` / `--eye`）は残す。

### 3.2 rubevy の口に載せ替える（S2）

- 2 本の `build.rs` → rubevy の build ヘルパ（R6）。
- `compile_source` / `compile_text` と `in_the_authors_lines`（`garden/src/main.rs:2727-2795`、`sabibots/src/main.rs:1630-1645`）→ rubevy の prelude つきプログラムと行番号補正。
  garden のテストは rubevy に移っているはずなので、ここでは呼び替えと、**sabibots のエディタに正しい行番号が出ること**の確認。
- `restart`（`sabibots/src/main.rs:1648-1654`）と garden の `restart_species` の中の同じ 3 行 → rubevy の差し替えの口。
- `require` への切り替え（文字列連結をやめて埋め込みの `Host` にする）は**しない**。行番号の補正が要らなくなる利点はあるが、
  「ネストした run loop の中ではタスクを park できない」（`garden/ruby/world_prelude.rb:191-193`）に当たる。3 本目で要るかを `factory-plan.md` で決める。
- rubevy を取り込んだので **`web/build.sh all` と Playwright**（3.4）をこの段階で必ず回す。

### 3.3 カメラ（S3）

- `ArenaPlugin`（`crates/rubevy-arena/src/lib.rs:79-125`）は「正方形が必ず入る固定の 2D カメラ」で、sabibots が使う。残す。
  ただし `follow_arena` が `Option<Res<Editor>>` を読んでカメラを画面幅の 16% ずらしている（`:103,118`）のを、
  **「画面の右（左・上・下）が何 px 塞がれているか」の数値の Resource** を読む形に替える。書くのはエディタを開く側。sabibots の見た目が変わらないこと。
- 新しく **パン・ズームの 2D カメラ**（`camera` モジュール）:
  - 右ドラッグ / Shift+ドラッグ / WASD・矢印でパン、ホイールでズーム、`Home` で戻す。garden の 3D の実装（`garden/src/main.rs:2916-3040`、`zoom_by` `:1229`、`notches_of` `:6245-6256` と単体テスト）の知見をそのまま持ってくる:
    **`MouseWheel::unit` を読み、差ではなく比でズーム**（ブラウザは 1 ノッチ 100 px、PC は 1 ライン）、
    **egui がポインタを持っていれば触らない**（`wants_pointer_input()` と `is_pointer_over_area()` の両方）、**ドラッグはボタンが下りた瞬間に決める**、
    **押した位置と離した位置でクリックとドラッグを分ける**（`CLICK_SLOP`。`garden/src/window.rs:96-196`）。
  - 速度・ズームの刻み・範囲は Resource の設定値にし、既定は garden の値から持ってくる（出どころをコメントに）。2D に合わない値は測って決める。
  - 塞がれている px の Resource を読み、見えている領域の中心を基準にする。
  - クリックは「どのワールド座標を押したか」のメッセージとして出す。タイルに直すのはゲーム。
- **Ruby からの問いに答える側**: rubevy の任意の Ruby 層（R9）が投げる `camera.world_at` などに答える system（`RubevySet::Answer` か `answer_in_tick`）。
  R9 が済んでいなければ、この項目は `factory-plan.md` の F5 へ送る。
- garden の 3D オービットは**触らない**（共有化は 2 本目の 3D の客が来てから）。

### 3.4 Web と確認（S4）

- `web/garden.html` と `web/sabibots.html`（diff 15 行）を 1 枚のテンプレートに。`web/build.sh` が canvas の id、`window.<game>Compile` / `Highlight` の名前、
  タイトル、背景色、preventDefault するキーを差し込む。3 本目の追加が「`GAMES` に 1 語と値 1 組」になること。
- `docker/run.sh:23` の `-e SABIBOTS_SELFTEST` のハードコードを、ゲーム名から決まる形に（Docker が動いていなければ未確認と報告）。
- **Playwright**: `web/build.sh all` → `web/serve.sh` → 隣の `sabiruby-playground/node_modules/playwright-core` と `~/.cache/ms-playwright` の Chromium
  （`--use-angle=swiftshader --enable-unsafe-swiftshader`、`page.goto` は `waitUntil:'commit'`）。2 ゲームとも pageerror 0、requestfailed 0、
  `?selftest` の行数が基準と同じ。スクリプトは scratchpad に置く（リポジトリに入れない慣行）。

### 3.5 既にある数の棚卸しと設定化（S5）

著者（2026-09-20）「基本は設定可能なように、理由があるなら理由も残して」。共有 crate に移す数（2 章）だけでなく、**既存 2 本と共有 crate に今ある数を全部**同じ形にする。
rubevy 側の同じ作業は rubevy の計画 R10。やり方も同じ:

1. **一覧**（S5a）: `crates/rubevy-arena/src/`、`garden/src/`、`sabibots/src/`、`*/ruby/*.rb` の `const` と数値リテラル。箱庭は規則の数がほぼ全部 Rust の `const`
   （`docs/worklog/2026-09-17-garden-world-survey.md` に一度数えた表がある。`world.rb` へ移った分を引く）。sabibots は機体・弾・レーダー・試合の数。
   数そのものに意味が無いもの（0、1、添字、単位換算）は入れず、基準を書く。
2. **出どころ**: コメント（箱庭の予算 45,000 のように 39 行の根拠が既にあるものもある。`garden/src/main.rs:4285-4323`）、`docs/garden.md`、`docs/sabiruby-battle.md`、
   `docs/worklog/`、`git log -S`。見つからないものは**「出どころ不明」**と書く。理由を後から作らない。
3. **分類と置き場所**: (a) 不変量（セーブの版、素材の寸法、形式）は `const` のまま、壊れる理由を 1 行。
   (b) **遊びの数**（速さ、視界、空腹、寿命、弾の威力、試合の長さ）→ **Ruby 側**（箱庭は `world.rb` / 種の `.rb`、Battle は `matches/*.rb`）。
   エディタで変えられるのが、この 2 本が見せたいことそのもの。Rust は Ruby が渡した値を受ける（`garden.rules` と同じ道）。
   (c) **動かす側の数**（予算、`frame_time`、表示、カメラ、`NEWBORN_GRACE` のような判定の猶予）→ `Settings`（`*.settings.txt`）と起動の引数。
   (d) selftest の閾値は検査の側に残し、出どころを 1 行。
4. **記録**: `docs/numbers.md`（新規。`docs/README.md` の目次に足す）に、ゲームごとの表: 名前、既定値、どこで変えるか、出どころ（測った日と条件／導出／引用／**不明**）、分類。
5. **動きを変えない。** 既定値は今の値のまま。2 本の selftest の行数と結果が前後で同じ。セーブと `*.settings.txt` の互換を保つ。

一覧は大きくなる（箱庭の `main.rs` は 6,292 行）。**S5a で一覧と分類案を報告して止まり、著者が見てから S5b で移す。**
(b) は Ruby と Rust の受け渡しが増えるので、S5b は 1 ゲーム 1 コミット以上に分け、箱庭の規則のように毎フレーム読む数は読む費用（1 回 2.2 µs）を測ってから。

---

## 4. 段階

| 段階 | 到達点 | 確認 |
|---|---|---|
| **S1** | `platform` の共通部分・`checks`・引数解析・起動の 10 行が共有 crate にあり、2 本の `platform.rs` はゲーム固有の定数と包みだけ | `cargo test --workspace`、2 本の selftest（PC）が基準と同じ、wasm ビルドが通る。2 本の `platform.rs` の行数の前後 |
| **S2** | 2 本が rubevy の口（R6）を使う。重複していた `build.rs`・連結・行番号補正・差し替えが消えている | 同上 + **sabibots のエディタでわざと構文エラーを作り、著者の行番号が出る**（selftest に 1 行足すか、手順を worklog に）+ `web/build.sh all` と Playwright で 2 ゲームとも pageerror 0・selftest の行数一致 |
| **S3** | 塞がれ px の Resource、`ArenaPlugin` が `Editor` を知らない、パン・ズームの 2D カメラ（単体テストつき）、小さな example（`crates/rubevy-arena/examples/`。画像なしでタイル状の四角を敷いて動かせる） | sabibots の見た目と selftest が変わらない。example を PC で動かし、`web/` に載せずに wasm ビルドだけ通す。ホイールの `unit` のテストが garden のものと同じ値で通る |
| **S4** | HTML のテンプレート化、`docker/run.sh`、docs（`docs/web.md`、`docs/README.md` の目次、`README.md` の Layout） | Playwright 一式をもう一度。`web/dist` の中身が前後で同じ構成 |
| **S5a** | `docs/numbers.md` の一覧と分類案（共有 crate、garden、sabibots）。コードは変えない | 出どころ不明の数、分類に迷った数を挙げて**報告して止まる** |
| **S5b** | 著者が見た後: 調整値が Ruby か `Settings` / 引数から変えられる。各々に出どころか「不明」 | 既定値のままで 2 本の selftest が前後同じ（PC とブラウザ）。変えると効くことを、移した種類ごとに 1 つ確かめる。Playwright 一式 |

S1 は rubevy を待たずに始められる。S5a も待たずに始められる（読むだけ）。S2 は R6 の後。S3 のうち Ruby に答える側は R9 の後（無ければ F5 へ送る）。

---

## 5. 分かっている罠

- egui は窓のタイトルを id にする（言語を切り替えると窓が別物になる。`guide.rs:276` は id を固定している）。共有 crate に移すとき id を変えない。
- `Guide` は `Settings` の `"lang"` というキーを握っている（`guide.rs:344`）。起動の 10 行を動かすとき、このキーと既存の設定ファイル（`*.settings.txt`）の互換を保つ。
- ブラウザのセーブは `localStorage` に残り続ける。接頭辞（`garden:` / `sabibots:`）を変えると公開版の利用者のセーブとスクリプトが消える。**変えない。**
- `Watch` の対応付けはファイル名の厳密比較（`world.rb` と `world_prelude.rb` を取り違えた修正が `garden/src/window.rs:571-578`）。触らない。
- selftest は機械が混んでいると揺れる項目がある（`docs/worklog/2026-09-18-selftest-fixes.md`、`check-holes.md`）。FAIL が出たら、まず変更と無関係かを静かな機械で確かめる。
- `wasm-bindgen-cli` の版は `Cargo.lock` と一致していること（`web/build.sh:37-39`）。依存を足して lock が動いたら確認する。

## 6. 状況

| 段階 | 状況 |
|---|---|
| S1〜S5b | 未着手（2026-09-20、計画のみ） |

## 7. 気づいた点（段階の報告から本体が集める）

実装担当は、仕事の範囲の外で気づいたことを直さずに報告と worklog の末尾に書く（`implementer.md` の報告の形式 6）。
本体は段階をレビューするたびにここへ写し、行き先を決める。消さずに「状況」を更新する。

| 日付・段階 | 気づいた点 | どこ | 属する先 | 状況（計画に足した／著者判断待ち／見送り・理由） |
|---|---|---|---|---|
| — | まだ無し | | | |
