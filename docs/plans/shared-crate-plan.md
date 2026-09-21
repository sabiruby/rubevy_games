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
| S1 | **済み**（2026-09-20、`shared-crate` の `8f836d4`）。`rubevy_arena::{platform, checks, args}` と `settings::remembered` / `Guide::opening`。2 本の `platform.rs` は 261 → 141 行、201 → 98 行（コメントと空行を除くと 121 → 51、103 → 33）。ブラウザの橋は `js_sys::Reflect` で unsafe 0。共有 crate に数は 1 つも置いていない（`--shot` / `--headless` の既定はゲームが渡す）。動きは不変: テスト 18 → 22、箱庭ヘッドレスの 13 件は本文まで一致、窓は 3 走行とも ok 43、ブラウザは 2 ゲームとも pageerror 0・requestfailed 0（garden ok 43、Battle は当たりに依らない 31 行）。計画に無かった判断 1 つ: PC の `compile` / `highlight` も共有側へ移したので `rubevy-arena` が PC で `sabiruby-compiler` に依存する。記録は `docs/worklog/2026-09-20-shared-platform.md` |
| S2 | **済み**（2026-09-20、`shared-crate` の `cce11ba`。**rubevy が push されるまで GitHub の rubevy では通らない** — 一時の `[patch]` で `rubevy-wt-r6` を指して作った。`Cargo.lock` はコミットに入れていない。rubevy の push の後に本体が取り直す）。2 本の `build.rs` は 35 行 → `rubevy_build::Embed::new("ruby").write()`、build.rs → `include!` → 表を引く、の通しはブラウザの 2 ゲームが走ったことで確認。`Program::new` に替えて `prelude_lines` は前と同じ数（482 / 376 / 297）。**sabibots のエディタが著者の行番号を言う**: `scout.rb:68:5`（直す前なら 365）、ブラウザは `playground.rb:68:5`。selftest に 1 段足した。garden の `beetle.rb:136` / `world.rb:315` も今までどおり。動きは不変: テスト 22 → 21（減った 1 本は rubevy に移った）、箱庭はヘッドレス 13 行・窓 43 行・ブラウザ 43 行が完全一致、Battle は当たりに依らない行が窓 30 → 31・ブラウザ 31 → 32（増えたのは新しい判定だけ）、pageerror 0・requestfailed 0、`web/build.sh all` + `wasm-opt` 通る。記録は `docs/worklog/2026-09-20-rubevy-entry-points.md` |
| S3 | **済み**（2026-09-20、`shared-crate` の `f39f1e7` と `d00ee4e`、main に取り込み済み）。`ViewInsets { left, right, top, bottom }`（論理 px。書くのは `EditorPlugin` — egui の窓の実物の矩形から。読むのはカメラ）で `ArenaPlugin` は `Editor` を知らなくなった。`CameraPlugin::showing(half_height)`: 右ドラッグ / Shift+ドラッグ / WASD・矢印 / ホイール / `Home`、クリックは `WorldClick` メッセージ、`CameraView` はゲームが書いてよい（Ruby からの `look_at` / `zoom` はここに書くだけ）、`Lens::world_at` / `screen_at`。数は全部 `CameraControls`（Resource、`Settings` の `camera_*` 7 キー、キー割り当ては `CameraKeys`）で、既定値ごとに出どころ 1 行（garden 側が不明のものは不明と書いた。出どころ不明の `PAN_PER_PIXEL` と `PAN_LIMIT` は引き継がず、前者は「カーソルの下の点がカーソルの下に居続ける」から 1.0 を導いた）。新しく決めた数は 0。テスト 21 → 31（main では 32）、Battle の窓 31 行・ブラウザ 32 行が一致、箱庭ヘッドレス 13 行一致。**見た目の変化 1 つ**: sabibots でエディタを開いたときのカメラのずれが 19.911 → 20.533 world unit（画面で **+8 px、窓の 0.5%**）。0.16 は「窓の 32%」の推測で、実物のパネルは 528 px = 33%。本体の判断: 受け入れる（推測の数を実測に替えた結果で、元に戻すには 0.16 を残すしかない）。記録は `docs/worklog/2026-09-20-shared-camera.md` |
| S4a | **済み**（2026-09-20、`shared-crate` の `47b95cb`、main に取り込み済み）。`crates/rubevy-arena` は無くなり、`crates/rubevy-egui`（`Editor`・`VmInspector`・`CodePanel`・`Watch`・`ViewInsets`。依存は bevy・bevy_egui・rubevy・sabiruby・PC で `notify`。殻への参照 0、ゲームの語彙 0）と `crates/games-shell`（`platform`・`checks`・`args`・`guide`・`settings`・`hud`・`ArenaPlugin`・`camera`。`publish = false`）に。依存は一方向（逆向きは doc リンク 1 件だけ出て、書き直した）。`Editor` の既定は中立（`noun: "script"`、`▶ Apply`、`apply_key: None`）で、sabibots に 3 行書いて見た目は同じ（窓の写真で確認）。`read_script` は消した。egui の id・`localStorage` の接頭辞・設定のキー・ページ側の関数名は不変。テスト 32 → 34、箱庭はヘッドレス 13 行・窓 43 行（3 走行とも FAIL 0）・ブラウザ 43 行が一致、Battle は窓 31 行・ブラウザ 32 行が一致、pageerror 0・requestfailed 0、wasm は +0.02%。**計画と違う点**: `follow_arena` の条件に `Changed<Window>` を採らなかった — bevy_winit 0.19.1 はカーソルが動くたび `Window` に書き戻すので常に真になり、ポインタが窓の外にあるリサイズでは真にならない。代わりに読む 3 つ（アリーナの半幅・窓・`ViewInsets`）を憶えて比べる形に（本体のレビューで妥当と判断）。未確認: `F5` を実際に押す確認、窓を手で掴んで大きさを変える走行（単体テスト 2 本で値を固定）。記録は `docs/worklog/2026-09-20-crate-split.md` |
| S4 | **済み**（2026-09-20、`shared-crate` の 8 コミット `f0b5fcf`…`7f5d967`、main に取り込み済み）。`web/page.html.in` + `web/games.sh`（3 本目は 1 語 + 値 1 組 + 入口ページの `<li>`。架空の 3 本目で数えた）、できあがる HTML は garden と入口がバイト一致、sabibots は消えるファイルを指していた 1 行だけ差。`web/build.sh` は `CARGO_TARGET_DIR` を見る。`docker/run.sh` はゲーム名から `<GAME>_SELFTEST` を決め、`--example` を回せる（`docker/run.sh sabibots --headless 15` は前は壊れていた。直した）。**確認の基準は `docs/verification/selftest-lines.md`（6 通りの走行の行の一覧を全文）と `tools/fixedlines.sh`（比べ方の常設）**。入れ替わる 2 組の n/a は文面を 1 つに。Battle の窓のチェックは `F5` を偽造して本当に押す（箱庭に同じ穴は無い、と確かめた）。Playwright のスクリプトは常設しない（隣の repo の `node_modules` とビルド番号入りの Chromium の絶対パスに依る。代わりに「スクリプトが間違えてはいけないこと」を文章で残した）。テスト 34、FAIL 0、pageerror 0・requestfailed 0、wasm は sabibots +92 バイトのみ。記録は `docs/worklog/2026-09-20-web-template-and-checks.md` |
| S6 | **済み**（2026-09-20、`c24db42`、コードは無変更、main に取り込み済み）。PC の窓を 1 本ずつ 56 走行で FAIL 0、同時に 4〜8 本で 136 走行中 FAIL 4、ブラウザ 1 本ずつ 16 走行で FAIL 1 — **揺れは負荷に依る**（0.6 秒の待ちに入るフレームは静かな PC で 5、混んだ PC とブラウザで 3）。原因は 2 つで、どちらも人の手で再現した。**原因 A はゲームのバグ**: Apply / Revert と同じフレームに生まれた個体は、その `Mind` がまだ `Commands` の列の中なので `restart_species` の `Query` に見えず、差し替えから漏れて**古いプログラムを走らせ続ける**（出産を同じフレームに仕込むと 3 走行とも FAIL。ブラウザでは仕込み無しで同じ形が出た）。**原因 B は判定の側**: 壁時計で 0.6 秒待つが、要るのは VM が新しいタスクに順番を回すことで、最低 2 フレーム要る（creature の VM の `frame_time` を 100 µs に絞ると静かな機械でその 1 行だけが落ちる）。計画に無かった 4 件目 `the meters move again` は原因 B の world VM 版。直し方の案と推奨は worklog。記録は `docs/worklog/2026-09-20-window-check-flakes.md` |
| S7 | **済み**（2026-09-20、`shared-crate` の `a693187` と `9806ad4`、main に取り込み済み）。**rubevy を `d347711`（R3〜R10）に**: 上げただけの前後で、ヘッドレス・窓の行の集合は diff 空、ブラウザは pageerror 0・requestfailed 0。R4 は原因 B を悪くも良くもしていない（S6 の 3 つの再現がそのまま出た）。**原因 A**: `Brains` が種の着ているプログラムと世代を持ち、`Mind` が世代を持つ。Apply / Revert / Save / 再読込は世代を進め、毎フレームの `catch_up_minds` が遅れている `Mind` に着せ直す（新しい数 0）。常設の判定 1 行 `a beetle born in the very frame of Apply is restarted too`（Apply のフレームに出産を 1 件積む。窓のチェックのときだけ）— `catch_up_minds` を外すと 2 走行とも FAIL、戻すと ok。**原因 B と 4 件目**: 0.6 秒の壁時計をやめ、条件（再起動された甲虫が走った／メーターが動いた／egui がポインタを取った）で待ち、上限は `SCHEDULER_FRAMES = 7` = 2（`Script` が付くフレーム + タスクが作られて走るフレーム。構造で縮められない）+ ceil(budget 200,000 ÷ 1 フレームが買う 45,600 命令)。45,600 は S6 の実測（5.7 命令/µs × 8,000 µs）。**8 本同時 31 分の交互 A/B**: 狙いの 4 行の FAIL は直す前 4/80 → **直した後 0/88**。ブラウザ 14 走行 FAIL 0（S6 は 16 走行で 1）。creature の VM を 200 µs まで絞っても通る（直す前は 250 µs で落ちた）。100 µs では直した後も落ちる — 本来の 1/80 の時間しか与えない設定で、正しい FAIL。**指示と違う点**: 0.2 秒の待ち 2 か所も条件待ちに変えた — `catch_up_minds` を足したことで Bevy の同期点が動き、触らないと S7 が新しい揺れ（8/16 走行）を持ち込むため（本体のレビューで妥当と判断）。記録は `docs/worklog/2026-09-20-window-check-fixes.md` |
| creature の VM の予算 | **入った（S5b-5、`1ef1fd3`）**。著者判断済み（2026-09-21）: 案 A — `script_budget` の既定を 1.74 × 23,686 ≈ 41,000 に**。S5b-5 で入れる（S5b-4 が前後を測っている最中なので、その途中では動かさない）。出どころは rustdoc と `docs/numbers.md` に式ごと書く（測った日、上限の庭の条件、1.74 は世界の VM の 45,000 ÷ 実測最大 25,837）。`scheduler_frames` は設定から計算するので一緒に動く（既定で 2 + ceil(41,000 ÷ 45,600) = 3）。前後で selftest の行の集合が同じことと、上限の庭で持ち越しが 0 のままであることを確かめる。以下は判断の前の材料: 今は `script_budget` = 200,000（rubevy の既定を引き継いだ。出どころ不明）。測った材料は S5b-3 の行。案 A: 1.74 × 23,686（起動フレーム込みの最悪）≈ **41,000**／案 B: 1.74 × 4,955（定常の最悪）≈ 8,600 — 起動の走り出しが 3 フレームに割れる／案 C: 据え置き。1.74 は世界の VM の 45,000 が実測最大の何倍だったか、という比。本体の推奨は **A**（出どころが書ける数になり、普通の走行では一度も効かず、暴走したスクリプトは今の 1/5 の命令数で止まる。設定なのでプレイヤーが重い頭脳を書いたら `garden.settings.txt` で上げられる）。`frame_time` は据え置き（2 本合わせても 1 フレームの 30%。絞る材料が無い） |
| S9 | 未着手（Factory と並行でよい。急がない）。S5b-5 の残り: (1) Save を一時ディレクトリ／別の `localStorage` 鍵へ向けて押す段（設計は `docs/worklog/2026-09-21-checks-and-leftovers.md` §13。`window_selftest` の引数が 15/16 なので `SystemParam` にまとめる）、(2) egui の待ちに自分の上限を持たせる（下の 7 章の 1 行目）、(3) `docs/numbers.md` §2.14 の抜け 14 行を分類に従って移す（昼の空と光の色、空の勾配、最初の草の下限、Battle の弾・砲塔・名札・灰色・記録板の色。z 座標一式は「重なりの順序が不変量」なので (a)）、(4) 箱庭の `Rubevy.ask("frame")` を rubevy の `next_frame` / `each_frame` に置き換える（games の lock は F0 で rubevy `abfc875` に上がった）、(5) **Battle のハンドラの判定（「当たりから 0.3 秒以内に走った／向きが変わった」）を秒でなく条件で待つ形に** — ブラウザの窓を 1600×900 にすると毎回落ちる（F0 が見つけた。`sabibots/src/main.rs:1908,1912`）、(6) `tools/fixedlines.sh` が CR を落とす、`--shot` と selftest を同時に使える形に |
| S8 | **済み**（2026-09-21、`1c209d6`、コードは無変更、記録を main に取り込み済み）。**rubevy の R4 は無関係で、書きは止まりの中で 1 つも着地していなかった。** 落ちた原因は判定の側: `places() == test.places` は `Vec` の `==` で**並び順まで比べる**が、`Query::iter` は archetype ごとに返すので、止まりの前後に 1 体が別の archetype に移る（rubevy の `start_scripts` が `ScriptTask` を付ける／`dress_animations` が `Animated` を付ける — ブラウザは数フレーム遅い／止める直前に食べた個体の `Eating` が着地する）だけで並びが変わる。ブラウザ 38 走行（S7 の 17 + S8 の 21）で落ちた 1 走行と「止まりの前後に出産があった」1 走行が同一。値を 1 つも変えずに空の marker を付けるだけで 1/1 再現、`P` のフレームに出産を重ねて 2/20、どれも `order-only=true`。R4 の前（`33d851a`）と後（`d347711`）を別の target で交互に 20+20 走行して同じ形が両方に出た。止まりの全フレームで持ち越しは 0。記録は `docs/worklog/2026-09-20-writes-landing-in-a-pause.md` |
| S5a | **済み**（2026-09-20、`a0de41e` を main に取り込み）。`docs/numbers.md`: 数値リテラルを含む 1,246 行から **260 件**。分類案は (a) 不変量 33 / (b) 遊びの数 → Ruby 33 / (c) 動かす側 → Settings・引数 101 / (d) selftest の閾値 28 / (e) 既に変えられる 65。出どころは 測った 14 / 導出 18 / 引用 50 / 理由のみ 53 / **不明 125（48%）**。毎フレーム読まれる数 103 件。コードは無変更。記録は `docs/worklog/2026-09-20-numbers-inventory.md` |
| S5b | **5 つに分けて順に**（同じファイルを広く触るので並行させない。1 つずつレビューして main に入れる）: **S5b-1** 共有 crate 2 つの数（**済み**、2026-09-21、`3161f75`・`0a6b961`・`c2e7424`、main に取り込み済み: `EditorLayout`（Resource）・`EditorColors`・`InspectStyle`・`VmClock::smoothing`・`CodeStyle`・`HudStyle`・`GuideStyle`・`ArenaPlugin::floor_margin`、`Settings` の鍵 27 個を `PanelSettingsPlugin` が読む。`rubevy-egui` は `Settings` を知らず、`read_from(|key| …)` のクロージャで受ける。`ArenaSize` は既定を持たず Battle が `ARENA_HALF_WIDTH = 32.0` を渡す。既定値は 1 つも動かさず、新しく決めた数は 0。一覧は 33 → 42 件（S3 のカメラ 8 件ほかを足した）、全体 269 件・不明 130。6 通りの走行の行の集合は前後で diff 空、`garden.settings.txt` に `editor_width=1400` と `guide_height=100` を置いた窓の走行も 44 行 FAIL 0。テスト 34 → 46、wasm +0.12% / +0.20%）→ **S5b-2** Battle（**済み**、2026-09-21、`0375cd0`・`0918fd9`・`80eb53f` ほか、main に取り込み済み: 遊びの数 32 件は試合の持ち物 `Match::MODEL`（`ruby/match_prelude.rb`。`match "…", numbers: { … }` で試合ごとに上書き）で、`Match#run` の 1 行目が Rust に渡し、robot を出すのはその後 — **受け取るまで試合は始まらず、Rust に模型の既定値は 1 つも無い**（`deny_unknown_fields`、検査は serde の後）。弾の速さの二重は「繋いだ」のではなく**消えた**（robot は起動時に `Rubevy.ask("model")` で 1 回受け取る）。`UNSET = -999.0` は両側から削除（`nil` は `Arg::Value` で届き `Request::num` が `None` を返すので番兵の数が要らない）。体力の満 → `hp_max`、予算バーの満 → `Look::bar_full`。表示 16 件 → `Look`、VM の予算と `frame_time`・`--headless`・`--shot` の既定 → `sabibots.settings.txt`。`selftest` の引数 16 → 10（`MatchUnderTest`）。6 通りの走行は一覧と一致、試合の決着は交互 6 巡で見分けがつかない（当たり 20.7 / 19.5）、前の版の `localStorage` でも起動する。テスト 46 → 53、一覧 272 件・不明 134。**動いた数が 1 つ**: 判定「向きが変わったか」の閾値 0.200 → `turn_rate × 0.3 ÷ 4`（既定で 0.195。下の 7 章））→ **S5b-3** 箱庭の動かす側の数 (c)（**済み**、2026-09-21、`68ba4ab`・`29ceadc`、main に取り込み済み: (c) 64 件 → 7 つの Resource（`Place` 4・`Furniture` 18・`Light` 17・`Scenery` 17・`Picture` 19・`Eye` 13・`Budgets` 7）と旗の既定 5 つ、`garden.settings.txt` の鍵で変えられる。導出できる 3 つ（`HALF_W` / `HALF_D`、`MIDNIGHT`、`MEADOW_AT`）は `const` をやめて関数に。測った数には「変えると何を失うか」を 1 行。`scheduler_frames(&budgets)` は設定から計算（既定で 7 のまま）。既定値は 1 つも動かさず、新しく決めた数 0。6 通りの走行は前後で diff 空・一覧と一致、古い `localStorage` の設定でも同じ 45 行。設定が効くことは種類ごとに本物の走行で（真夜中の地面 43.31/255 は G6b の 44/255 とほぼ同じ = 既定が測ったときのままの証拠）。テスト 53 → 56。**creature の VM を上限の庭で測った**（`pop_max` 24、3 走行、10,749 フレーム）: 定常の命令数は中央値 368・99th 3,460・最大 4,955、全フレームの最大は最初のフレームの 23,686（24 体が一斉に走り出す）。既定の 200,000 には一度も近づかない（最悪で 12%）。tick は最大 1.72 ms、2 本の VM の合計は最大 5.01 ms = 60 Hz の 30%。持ち越しは全フレームで 0。**既定値の案は下の行、著者判断待ち**）→ **S5b-4** 箱庭の遊びの数 (b) を Ruby 側へ（**済み**、2026-09-21、`45b8d7a`・`233fa47`・`bef577e`・`63e5a8f`・`3a89f74`、main に取り込み済み: `touch_reach`・`sprout_gap`・体の半径 4 つを `world.rb` が言い、`garden.rules` の受け口は 5 → 11（数だけ渡し、総当たりは Rust）。**Rust の既定値は消せなかった** — 箱庭は「`world.rb` がコンパイルできなくても走る」のが W1 の設計で、世界を「規則が喋るまで止める」と庭ごと遅れ、「規則が永遠に喋らない庭」の判定も要る。代役として残し、`ruby/world.rb` を読んで 10 個の代役と突き合わせる単体テストを足した。`Furniture::plant_max` → `plant_grown`（`world.rb` の `plant_max` とは繋がない — 同じことを言っていない）。**変異率は `mutation_rate 0.1` という名前のある数になり、8 番目の判定は VM から読む**: `beetle.rb` の率を 0.5 にすると前の版は 2 走行とも FAIL、後の版は ok。種ごとに 1 つの `Handle`（S7 の `Brains::wearing` は「編集されたときだけ」埋まる形で、1 匹ごとの `Assets::add` は塞がっていなかった）。既定値は 1 つも動かさず、新しく決めた数 0。6 通りの走行は一覧と一致、90 秒の交互 8 巡で世界の分布は見分けがつかない（出産 4.0/4.3、餓死 3.6/3.1）、上限の庭の世界 VM は予算の 52% のまま、古い `world.rb` / `beetle.rb` を `localStorage` に入れたブラウザで 45 行 FAIL 0。テスト 56 → 58）→ **S5b-5** 判定の側 (d) と残り（**済み**、2026-09-21、`c17abe1`…`6ececab` の 15 コミット、main に取り込み済み。**予算**: `script_budget` の既定 200,000 → **41,000**（1.74 × 23,686 = 41,214 を 1,000 の位に丸め — 世界の 45,000 が 44,956 を同じように丸めた先例。上限の庭を前後交互 3 巡・21,558 フレーム: 最初のフレームは前後とも 23,912 で割れず、持ち越し 0 のまま、`scheduler_frames` は 7 → 3）。**一時停止の 2 行**は entity を鍵に比べる（S8 の仕込みで前 3/3 FAIL → 後 3/3 ok。world の VM の budget も見る）。残りの 0.6 秒 2 か所は条件待ちに。**ホイールの 1 行**は原因を実物で確認（正しくズームしたのを 0.2 秒後の読みで FAIL と呼んでいた）→ `held` を回した瞬間の値に: 8 本同時 88 走行ずつで 6 → 3、うち「遅れて読んだ」3 → 0。残りの 3 は下の 7 章の 1 行目。**狙う点**を設定の矩形の内側から決めたら、2 つのホイールの判定が「狙う点が説明パネルの外に落ちる」ことに 150 px の余裕で黙って寄りかかっていたと分かり、判定の間は説明パネルを閉じる形に → 8 本同時 48 走行で FAIL 0、待ちが一度も上限に達しない。45,600 と 54,800 は少なく買える方 45,600 を採り、2 つの条件を rustdoc に。`--` に統一。実値の写しの閾値を 2 つ直した。ブラウザのつまみ `?selftest&<name>=<value>`、`docker/run.sh` は `<GAME>_` の環境変数を全部渡し、target の volume は worktree ごと。遺伝子の既定 7 つ → `Furniture`。`WorldMeter` の名前を `ran_in` に、`web.md` の表を取り直し。**最後の網**: 数値リテラルを含む行を全部拾い直し、抜け 14 行を `docs/numbers.md` §2.14 に足した（移していない）。6 通りの走行は一覧と一致（箱庭の窓 45 行・ブラウザ 46 行に）、テスト 58 → 60。**残り**: Save を安全に押す段（設計だけ）と、抜け 14 行を移すこと → S9） |
| 取り込み | **2026-09-20、著者「取り込みも push も今やってよい」**: S1・S2 を main に取り込み（merge `f34caf2`）、`Cargo.lock` を push 済みの rubevy（`33d851a`）に取り直し。push は `web/build.sh all` + Playwright の確認の後（Pages に公開されるため） |

## 7. 気づいた点（段階の報告から本体が集める）

実装担当は、仕事の範囲の外で気づいたことを直さずに報告と worklog の末尾に書く（`implementer.md` の報告の形式 6）。
本体は段階をレビューするたびにここへ写し、行き先を決める。消さずに「状況」を更新する。

| 日付・段階 | 気づいた点 | どこ | 属する先 | 状況（計画に足した／著者判断待ち／見送り・理由） |
|---|---|---|---|---|
| 09-20 R2 | （rubevy の R2 から）箱庭は 1 匹ごとに `Assets::add` している（`give_mind` が spawn と出産のたび）。VM の irep は rubevy の R2 で 1 部になったが、`Assets<MrbAsset>` には同じバイト列が匹数ぶん残る。種ごとに 1 つの `Handle` を持てば消える | `garden/src/main.rs:2660-2685` | ゲーム固有（箱庭） | **本体が原則から決めた（09-20、著者「原則を大切に判断して」）**: **S5b のついでに直す**（種ごとに 1 つの `Handle`）。動きは変わらず、同じバイト列を匹数ぶん持つ理由が無い |
| 09-21 S5b-5 | **egui の待ちが VM の数に相乗りしている**: `Turn::EguiHasThePointer` の上限が `scheduler_frames`（予算から計算）で、予算が 1/5 になった結果 7 → 3 フレームになり、egui の待ちが先に足りなくなった（8 本同時 88 走行中 3 が諦めた）。今は説明パネルを閉じたので当たらないが、`script_budget` を下げればまた当たる。S7 の「3 つの待ちのうち最大の 1 つの数でよい」という決定の帰結 — **別々の理由の待ちを 1 つの数で賄うと、片方の理由が動いたとき、もう片方が黙って壊れる** | `garden/src/window.rs` の `Turn` | 箱庭の判定／本の素材 | **本体が原則から決めた: 待ちごとに自分の上限を持たせる**（egui の上限は egui の理由から出す — 測って）。S9。book の findings に写す |
| 09-21 S5b-5 | **41,000 の出どころ 23,686 は S5b-4 の前の数**。今日の最初のフレームは 23,912（S5b-4 が `beetle.rb` に `mutation_rate` を足したぶん、甲虫 14 匹 × 約 16 命令）。出荷される余裕は 1.71 倍。同じ式を今日の実測に当てると 41,607 → 42,000 | `garden/src/main.rs:2446` | 埋め込みの数 | **著者判断待ち**（小）。本体の推奨は **41,000 のまま**: 出どころは「2026-09-21 の実測に式を当てた数」で、スクリプトが 1 行変わるたびに既定値を引き直すと既定値が落ち着かない。rustdoc には「その後 23,912 になった。余裕は 1.71 倍」と書いてある |
| 09-21 S5b-5 | 判定が「書かれていない配置」（説明パネルが窓の中央にあって、狙う点がその外に落ちる）に寄りかかっていた。同じ形がほかに無いかは見ていない。Battle の `script_budget` 200,000 がこの repo に残る唯一の「引き継いだ予算」。古い docker volume と比較用の worktree `rubevy_games-wt-s5b5-before`（target 1.9 GB）が残っている | `garden/src/window.rs`、`sabibots`、docker | 本の素材／埋め込みの数／片付け | Battle の予算は Factory の後に同じやり方（上限で測る）で。volume と worktree は本体が片付ける |
| 09-21 S5b-4 | **種の遺伝子の既定（`Genome::of` の 2×3 と `SPREAD` 0.18）は `world.rb` へ移すと嘘になる**: 読むのは `spawn_world`（`Startup`）だけで、`garden.rules` が届くのは最初の `Update` — 書けるようにしてもどの走行でも 1 度も使われない（「設定にしたが効かない数」になる）。半径が移せたのは `Collider` に載っていて後から書き直せるから。**「移せるかどうか」は「誰がいつ読むか」で決まる** | `garden/src/genome.rs`、`spawn_world` | 分類／一覧の作り | **本体が原則から決めた: 担当の案 A** — `Furniture` の設定へ（`start_beetle_speed` ほか 7 鍵、分類を (b) → (c) に）。庭を建てるときの数は、株の数や個体の数と同じ「最初の家具」。世界の生成を作り替える案 B は、効かない数を作らないために設計を動かすことになり、釣り合わない。S5b-5 で |
| 09-21 S5b-4 | 体の半径（(b)、`world.rb`）と模型の倍率（(c)、`garden.settings.txt`）が 2 つの場所に分かれた。同じ数ではない（倍率は `.glb` の寸法で割った数）ので繋いでいないが、片方だけ動かすと見た目と当たりがずれる。`plant_max` の鍵を `plant_grown` に変えたので、S5b-3 以降に `plant_max=…` と書いた設定は黙って効かなくなる（鍵は S5b-3 で足したばかり） | `garden/ruby/world.rb`、`Picture` | 箱庭（設計） | 本体の判断: 繋がない（両方のコメントに「片方だけ動かすとずれる」と書いてある）。鍵の改名は認める |
| 09-21 S5b-4 | `touch_reach` を大きくしても「触られた」は増えない（publish は接触が作られた瞬間だけなので、届く距離を伸ばすと接触が切れにくくなる）。最初の 3 巡だけ見ると「移したせいで餓死が増えた」と読める形をしていた — **交互に 2 巡でも足りないことがある** | 箱庭の遊びの設計／計測の作法 | 本の素材 | `docs/garden.md` に 1 行入った。`implementer.md` を「交互に 2 巡**以上**、最初の数巡が偏って見えたら足す」に直した。book の findings に写す |
| 09-21 S5b-3 | **`INSTRUCTIONS_A_FRAME_BUYS` 45,600 は緩い側だった**: S6 は `frame_time` を 300 µs に絞って 5.7 命令/µs を測ったが、上限の庭の普通の走行では 6.85 命令/µs で、8 ms が買うのは 54,800。上限のフレーム数は 7 ではなく 6 になる。担当は直していない（(d) の数） | `garden/src/window.rs` | (d) 判定の閾値 | 計画に足した: S5b-5 で出どころを書き直す（どちらの実測を採るか、2 つの条件の違いを書く） |
| 09-21 S5b-3 | 一覧に無い直書きの数が箱庭にまだある: **昼の色の傾き一式**（夜の 3 つは測ってあって一覧にあるのに、昼の同じ形の数は 1 つも無い — S5a の網が `const` と名前のある数に寄っていた）、空の勾配の押し上げ、最初の草の大きさの下限 0.3。`world.rb` の `plant_max` 1.4 と Rust の `plant_max` は今も 2 つの数（同じことを言っていない: 成長の上限と、建てるときの株の大きさ） | `garden/src/main.rs:3210,4012-4069,4159` | 一覧の抜け／(b) と (c) の境目 | 抜けは S5b-5 の最後の網で。`plant_max` は S5b-4 の材料（名前を分けるか繋ぐか） |
| 09-21 S5b-3 | `save_file` の設定は実走行で確かめられていない（窓のチェックは「リポジトリに書くので Save はわざと入れていない」）。`draw_hud` と `window_selftest` が Bevy の引数の上限 16 に張り付いている | `garden/src/window.rs` | 確認の穴／箱庭の判定の形 | 計画に足した: S5b-5 で Save を一時ディレクトリへ向けて押す段を足す |
| 09-21 S5b-3 | **設定になったことで、測るための「作り替えた build」が要らなくなった**（W3 は上限の庭を測るのに build を作り替えたが、今回は設定に 3 行）。数を利用者が変えられる場所に置くと、測る人にとっても変えられる。箱庭は `Dice` が時計から種を取るので同じバイナリでも 2 回同じ文を出さない | — | 本の素材 | book の findings に写す |
| 09-21 S5b-2 | **判定の閾値が 1 つ動いた: 0.200 → 0.195**（`handler_selftest` の「向きが変わったか」）。横のコメントが「旋回の速さの 4 分の 1 を 0.3 秒ぶん」と言っていて、一覧もそれを `TURN_RATE 2.6 × 0.3 ÷ 4 ≈ 0.195` を丸めた数だと記録していた。`turn_rate` が試合の持ち物になったので、0.2 を置けば試合の数の写しになる（速い機体の試合で緩すぎ、遅い機体で通らない）。コメントが言う式そのものにした。動いた向きは緩い側 | `sabibots/src/main.rs` の `handler_selftest` | (d) 判定の閾値 | **本体の判断: 認める**（利用者の既定値ではなく判定の閾値で、写しをやめて自分のコメントどおりの式にしたもの。6 通りの走行は一覧と一致）。**著者が 0.2 に戻したければ 1 行**。S5b-5 で、ほかにも「実値の写し」になっている閾値が無いかを同じ目で見る |
| 09-21 S5b-2 | 旧い定数 `Robot::UNSET` / `SHOT_FAST` / `SHOT_SLOW` の互換は残さなかった。プレイヤーが自分の robot に直接書いていた場合だけ `NameError` になる（同梱の scout / hunter は書いておらず、ガイドも説明していない。前の版の `localStorage` の `scout.rb` では起動を確かめた） | `sabibots/ruby/prelude.rb` | ゲーム固有（公開版の利用者） | 本体の判断: 残さない（互換の定数は写しを 1 つ残すことそのもの）。`docs/sabiruby-battle.md` に「Where the numbers are」がある |
| 09-21 S5b-2 | **`ScriptWorld::budget` はタスクごとの持ち分を持たない**（`tick_scripts` は `task_run_limits` に残り全部を渡す）。予算バーの満 3000 は `budget` から導けなかった。「1 タスクが 1 フレームに使ってよい量」をホストが言いたければ rubevy に口が要る | rubevy `src/lib.rs` の `tick_scripts` | rubevy（API）／本の素材 | rubevy の計画 7 章に写す（Factory の F3 — インサータ数百台 — で要るかが分かる） |
| 09-21 S5b-2 | Battle にまだ一覧に無い直書きの数がある（弾のスプライトの倍率、砲塔、名札の字の大きさと縮尺、撃破機の灰色、z 座標一式）。`crate_size` は分類案 (c) から (b) に動かした（`min_crates` と組でしか意味を持たない） | `sabibots/src/main.rs` | 一覧の抜け／分類 | 計画に足した: S5b-5 の最後に、3 つの一覧（共有 crate・Battle・箱庭）をもう一度網に掛けて抜けを拾う。`crate_size` は認める |
| 09-21 S5b-2 | **隣の担当の docker ビルドの最中に取った「前」の値は、前の版の値ではない**: 最初の前後 8 走行ずつは「移したせいで当たらなくなった」と読める形（当たり 23.1 → 17.5）をしていたが、別 target で交互 6 巡し直すと差は消えた | 計測の作法 | 本の素材／作法 | `implementer.md` の「交互に 2 巡」がそのまま当たる。book の findings に写す |
| 09-21 S8 | `window_selftest` は `RubevySet::Deliver` とも `dress_animations` とも順序を持たず、判定が「フレームのどの時点の世界」を見るかが executor 次第。ただし順序の辺を足すと Bevy が同期点を挿して別の判定が落ちた前例（S7）がある | `garden/src/main.rs` の system の登録 | 箱庭の判定／本の素材 | 本体の判断: 順序は足さない。判定を「並びや時点に依らない述語」で書く（S5b-5）。book の findings に「いつ測るかが測るものを変える」 |
| 09-21 S8 | **つまみをブラウザから渡せない**（wasm に `std::env` が無い。`?selftest` しか読まない）ので、ブラウザでだけ出る揺れをブラウザで追えない。docker の target volume は worktree ごとに分ける必要がある | `games-shell` の `checks`、`docker/` | 道具 | 計画に足した: S5b-5 で `checks` に問い合わせ文字列のつまみ（`?selftest&<name>=<value>`。PC の `<GAME>_<NAME>` と同じ名前で読める口）を足す。docker の volume は `docker/build.sh` が worktree の名前から決める形に |
| 09-21 S8 | `Paused::on()` と `ScriptWorld::budget == 0` は同じ事実を 2 か所で言っていて、判定は creature の VM の budget しか見ていない | `garden/src/window.rs` | 箱庭（小） | S5b-5 で world の VM も見る |
| 09-21 S5b-1 | **VM パネルの色 9 か所と説明パネルの余白 3 つが一覧に無かった**（エディタの色は一覧にあって移したのに、同じ crate の隣のパネルは S5a の網に掛かっていない） | `crates/rubevy-egui/src/inspect.rs`、`crates/games-shell/src/guide.rs` | 共有 crate／一覧の抜け | 計画に足した: S5b-2 の最初に移す（`InspectStyle` / `GuideStyle` に足すだけ） |
| 09-21 S5b-1 | **「設定にした」と「設定が効いている」は別の主張**: 箱庭の窓のチェックの「狙う点」は右上に固定で、右に張り付いたエディタでは幅の設定が効いていなくても通る（担当は `editor_width=760` では証明にならないと気づき 1400 で確かめた） | `garden/src/window.rs` の `FakePointer` | 箱庭の判定の作り／本の素材 | book の findings に写す。判定は S5b-5 で「狙う点を矩形の中から決める」形に |
| 09-21 S5b-1 | `sabibots` の `selftest` システムは Bevy の引数の上限ちょうど 16 個。そのためエディタの色は Resource にできず `Editor::colors` のフィールドになった。`GuidePlugin` / `HudPlugin` / `CodePanelPlugin` がユニット構造体でなくなり、呼び方が `GuidePlugin::default()` に | `sabibots/src/main.rs`、共有 crate | Battle の判定の形／API の一貫性 | S5b-2 で `selftest` の引数を `SystemParam` にまとめる（箱庭の `FakePointer` と同じ）。プラグインの呼び方は `EditorPlugin::with_highlighter` と揃っているのでこのまま |
| 09-20 S7 | **（バグ候補）ブラウザで、一時停止中なのに生き物が動き・空腹が進んだ**（`2 s paused: …` の 2 行が、lock を上げただけの 3 走行中 1 走行で FAIL。同じ走行で `nothing ran while it was paused` は ok — VM は止まっていたのに component への書きだけが止まりの中で着地している）。rubevy の R4（時間切れのフレームで答え切れなかった問いを次のフレームへ回す）が作る形と読めるが**未確認**。ブラウザは 1 フレーム 250 ms で 8 ms を毎フレーム使い切るので PC より桁違いに起こりやすい。最終版の 14 走行では出ていない | `garden/?selftest`、rubevy の `tick_scripts` と `apply_component_writes`、箱庭の `P` | 箱庭の判定（S8 で確定。rubevy は無関係） | **S8 の結論: 判定が `Query` の並び順まで比べていた**。本体が決めた直し方は S8 の (A)1 + (B)1: **entity を鍵に引き当てて比べ**（「消えた／増えた／値が変わった」を別々に言う。許容は置かない）、「`P` は世界の規則と時計と VM を止めるもので、Bevy の世界を凍らせるものではない」とコメントに書く。一時停止の作りは変えない（archetype の移動は世界の状態ではない）。S5b-5 で |
| 09-20 S7 | **5 つ目の揺れ**: `and with the panel closed the same wheel in the same place zooms` が 8 本同時で 88 走行中 4、**直す前も 80 走行中 4**（S7 の回帰ではない）。egui の「ポインタを持っている」が揺れる。`window.rs` に 0.6 秒の待ちが 2 か所残っている（world.rb の Apply / Revert。S7 が直した 2 つと同じ形で、まだ落ちたことが無いだけ） | `garden/src/window.rs` | 箱庭の判定 | 計画に足した: S5b の (d) で、`held` をホイールを回した瞬間の値に・残りの 0.6 秒を `Turn::DayLength` に |
| 09-20 S7 | `SCHEDULER_FRAMES` の式は `budget` 200,000（rubevy の既定）を**数として**入れている。creature の VM の予算を S5b でゲームの設定にしたら、式は設定から計算する形にしないと食い違う | `garden/src/window.rs:1312` | 箱庭の判定 | 計画に足した: S5b で creature の VM の予算を設定にするとき、一緒に |
| 09-20 S7 | `docker/run.sh` は `<GAME>_SELFTEST` 以外の環境変数を渡さない（担当は効かないことに気づかず 4 走行を無駄にした）。共有した docker の target は worktree を切り替えても作り直されない（どの worktree も `/app` に mount されるので cargo から同じパスに見える。`touch` で直る） | `docker/run.sh`、`docker/build.sh` | 道具／作法 | 計画に足した: S5b のついでに `<GAME>_` で始まる環境変数を全部渡す。docker の target の罠は `implementer.md` に足した |
| 09-20 S7 | `.chain()` の末尾に `Commands` を持つシステムを 1 つ足すと `ApplyDeferred` が増え、順序の辺を持たない 2 つのシステムの相対順が実際に変わる（16 走行中 8 走行対 0）。`WorldMeter::passes` は名前に反して「スクリプトが 1 命令でも走ったフレーム」を数えている（HUD の表示も） | Bevy の ECS／`garden` | 本の素材／箱庭（名前） | book の findings に写す。`passes` の名前は S5b のついでに |
| 09-20 R11 | （rubevy の R11 から）箱庭の `Rubevy.ask("frame")`（ゲームが毎フレーム 1 回だけ答える自作の待ち）は rubevy の `Rubevy.next_frame` / `each_frame` に置き換えられる。ただし箱庭の `each_frame` は being ごとの呼び出しも兼ねていて 1 対 1 ではない | `garden/src/main.rs` の `run_world`、`garden/ruby/world_prelude.rb` | ゲーム固有 | 計画に足す: S5b の後に小さな段階で（規則の動きが変わらないことを selftest と、世界の VM の命令数の前後で確かめる） |
| 09-20 S6 | **（バグ）Apply / Revert と同じフレームに生まれた個体が差し替えから漏れ、古いプログラムを走らせ続ける**。`restart_species` が `Query` を回る形は、同じフレームに `Commands` で作られたものを取りこぼす。sabibots の同じ形も同じ穴を持つはず（走行中に robot が生まれないので当たらないだけ） | `garden/src/window.rs:520`、sabibots の `Apply to all` | ゲームのバグ | **本体が原則から決めた: S6 の案 A1**（種の「世代」を `Brains` と `Mind` に持たせ、世代の古い `Mind` を毎フレーム拾って差し替える。新しい数を 1 つも置かない。判定を n/a にして隠す案は採らない — バグが残る）。S7 |
| 09-20 S6 | **判定の待ちが全部「秒」で書かれている**（0.2 / 0.5 / 0.6 / 2.0 / 9.0）。フレームレートが 4 倍違う 3 つの環境を 1 つの秒で賄っていて、混むと VM が新しいタスクに順番を回す前に判定が来る | `garden/src/window.rs` の `WindowTest`、`:1302` | 判定の作り | **本体が原則から決めた: 条件で待ち、上限はフレームで、その上限は導く**（S6 の B1 + B2。「全員が 1 命令でも走ったら次へ」。待つ上限は実測の最低 2 フレームと、予算 ÷ 再起動 1 本の最初の一走りの命令数から出す。0.6 → 1.2 秒のような出どころの無い延長はしない）。S7。ほかの秒の待ちは S5b の (d) で |
| 09-20 S6 | **creature の VM は budget も `frame_time` も rubevy の既定のままで、ゲームはどこでも言っていない**（world の VM は 45,000 を測って決めてあるのに片側だけ） | `garden/src/main.rs` | 出どころの無い数 | 計画に足した: S5b で、上限の庭に座って測り、ゲームの設定として書く（値を動かすかは著者に聞く） |
| 09-20 S6 | **WSL2 では `uptime` の load average が当てにならない**（`vmstat` が 99% idle のとき 15〜20、走行を止めた後も 40 分以上 100 超）。probe に `.before()` を付けると Bevy が同期点を差し込んで、再現しようとしていた競合が消える | 計測の環境／Bevy の ECS | 本の素材／repo の作法 | **本体が決めた**: 混み具合は `vmstat` の idle で見る。`implementer.md` に足した。book の findings に写す |
| 09-20 S6 | games の `Cargo.lock` は rubevy `33d851a` のままで R3・R4・R6b が入っていない。R4（`frame_time` を上限に）は原因 B の道そのものを触る | `Cargo.lock` | 計画 | 計画に足した: S7 の最初に lock を rubevy の今の main に上げ、その状態で直して測る |
| 09-20 S4 | **箱庭の揺れはブラウザでも出た**: `Revert puts every beetle back on the file` が 2 走行中 1 走行で FAIL（箱庭のコードは 1 行も触っておらず、wasm の大きさも同じ）。計画書の揺れ 3 件は PC の窓の記録だった | `garden/?selftest` | ゲーム固有／S6 の材料 | 計画に足した: S6 はブラウザも対象にする |
| 09-20 S4 | **`cargo build --workspace` は example を建てない**（型の合わない 1 行で `cargo test` は落ち、`cargo build` は通る）。各段階の合否に使ってきた「`cargo build --workspace` 警告 0」は example を見ていなかった | `crates/games-shell/examples/camera.rs` | 確認の作法 | **本体が決めた**: 以後の依頼文は `cargo build --workspace --all-targets` にする |
| 09-20 S4 | `--` と `n/a` が混在している（意味は同じ「測れなかった」。Battle は `--`、箱庭は両方） | `sabibots/src/main.rs`、`garden/src/main.rs:4564,4990,5898`、`garden/src/window.rs:1275` | 確認の作法 | 計画に足した: S5b のついでに 1 つに揃える（`tools/fixedlines.sh` と一覧も直す） |
| 09-20 S4 | 入口ページ `web/index.html` だけ手書きのまま（3 本目は `<li>` を手で足す）。`docker/run.sh` は `<GAME>_SELFTEST` しか渡さず、箱庭の `GARDEN_RELOAD_AT` はコンテナから渡せない | `web/index.html:32-39`、`docker/run.sh` | 道具 | 本体の判断: 入口は手書きのまま（紹介文は値ではなく本文）。`GARDEN_RELOAD_AT` は使う判定が出たとき |
| 09-20 S4a | **`F5` が押されることを確かめるチェックが無い**。sabibots の selftest は `editor.action` を直接書くので、`apply_key` を取り違えても通る。S4a で既定を `None` にしてゲーム側に移したぶん、この穴が効くようになった | `sabibots/src/main.rs:1051,1082,1095` | ゲーム固有（Battle の判定の穴） | 計画に足す: S4 で窓のチェックに 1 段（キーを偽造して押す。箱庭の窓のチェックが同じ手を持っている） |
| 09-20 S4a | `docs/numbers.md` の共有 crate 33 件の `位置` 欄が古い（ファイルが 2 crate に分かれ `lib.rs` が 2 つ）。注意書きは足したが行番号は未取得。`docs/web.md` の wasm の大きさも 7 KB ずれた | `docs/numbers.md` §1、`docs/web.md` | 文書と実物のずれ | 計画に足す: 行番号は S5b の最初に取り直す。`web.md` は S4 |
| 09-20 S4a | `CodePanel` と `Hud` は割った後も 1 度も呼ばれていない。`rubevy-egui` を crates.io に出すなら `CodePanel` は「誰も使っていないが公開する」ものになる | `crates/rubevy-egui/src/code.rs`、`crates/games-shell/src/hud.rs` | 共有 crate（公開の判断） | 公開の話が出たときに著者と。今は動かさない |
| 09-20 S3 | **計画書の前提違い**: 「2 本とも `Editor` の既定 4 つを上書きしている」は誤りで、sabibots は `apply_all_label` しか上書きしていない（`noun` / `apply_label` / `apply_key` は既定の `"robot"` ほかに頼っている）。既定を中立語にすると Battle のホバー文が変わる | `crates/rubevy-arena/src/editor.rs:146-149`、`sabibots/src/main.rs` | 計画書の前提／共有 crate | **本体が原則から決めた**: 既定を中立にし、sabibots に 3 行書いて見た目を完全に保つ（crate にゲームの語彙を置かない、動きは変えない、の両方を満たす道はこれだけ）。S4a でやる |
| 09-20 S3 | `Editor` は開いている間ずっと `is_changed()` が真（`draw_editor` が `ResMut` を deref する）。古い `follow_arena` はそれを条件に毎フレーム再計算していて、**そのおかげで**窓の大きさの変更に追随していた。意図ではなく事故で、本当に要るのは `Changed<Window>` | `crates/rubevy-arena/src/editor.rs`、`lib.rs` | 共有 crate（変更検知） | 計画に足す: S4a のついでに `Changed<Window>` を条件にする。振る舞いが変わらないことを窓の大きさを変える確認で |
| 09-20 S3 | `ArenaPlugin` の `+ 3.0`（壁の外に見せる床）が 2 か所に直書き。HUD もパネルなのに `ViewInsets` を書かない。コンテナから example を回す道が無い（`docker/run.sh` は `/target/<mode>/<game>` 固定） | `lib.rs:112,119`、`hud.rs`、`docker/run.sh` | 共有 crate／道具 | `+ 3.0` は S5b（`docs/numbers.md` の (c) に既にある）。HUD は 3 本目が要るとき。`run.sh` は S4 |
| 09-20 S3 | 箱庭の窓のチェックの揺れが変更後も 3 走行中 1 走行出た（`Apply restarts every beetle on the edited text`。S1 は無改変で 3 走行中 2 走行）。これで揺れる判定は 3 つ、どれも種のファイルの Apply / Revert / 再起動の周り | `garden/src/window.rs` | ゲーム固有／バグ候補 | S6 の調査の材料に足した |
| 09-20 著者 | **「アリーナクレートの名前も検討して」**。`rubevy-arena` の名前は 1 本目（Battle の正方形のアリーナ）から来ていて、今の中身（エディタ、VM パネル、ファイル監視、`platform`、selftest の枠、引数、案内、設定、2 種類のカメラ）を言っていない。3 本目は「アリーナ」を持たない | `crates/rubevy-arena`（名前は repo 内 47 ファイルに出る。crates.io には出していない） | 共有 crate（名前と構成） | **著者判断済み（09-20）: 案 1 — 2 つに割る**（`rubevy-egui` と `games-shell`）。段階 S4a（8 章の末尾） |
| 09-20 S2 | `replace_script` に替えられる同じ 3 行があと 2 か所ある: garden の `wear_the_rules`（`Script::<World>` 版）と sabibots のファイル監視からの再読み込み。計画書 3.2 が 2 つしか名指ししていなかった | `garden/src/main.rs`、`sabibots/src/main.rs:2304-2308` | 計画書（S2 の書き漏らし） | 計画に足す: S3 の最初に替える |
| 09-20 S2 | **`Program` にコンパイラへ渡すファイル名の置き場所が無い**。`name` は区切りコメント専用で、呼び出し側が同じ `name` を 2 回書く（片方だけ変えると食い違う）。`&str` 4 本は取り違えなかった（変数名がそのまま引数の順に並ぶ）。ビルダは要らない | rubevy `src/source.rs` | rubevy（API） | rubevy の計画 7 章に写す。直すなら `Program` が `name` を持って読めるように |
| 09-20 S2 | Battle の「当たりに依らない行」の基準に、走行次第で入れ替わる行が混じっている（編集チェック中の当たりの除外行、撃破の有無で入れ替わる `handler tasks … ended` ⇄ `no robot with a handler was down long enough`） | `sabibots/src/main.rs:1308,1429`、`docs/README.md` の基準 | 確認の作法／本の素材 | **本体が原則から決めた（09-20、著者「原則を大切に判断して」）**: **両方やる**（S4）。走行次第で入れ替わる 2 組は n/a の文面を 1 つにし、基準の行は数ではなく**行の一覧**として `docs/verification/` に置く。「31 行」のような数は、何が 31 行なのかを言わないと確認の基準にならない |
| 09-20 S2 | `web/build.sh` は `CARGO_TARGET_DIR` を見ない（`target/…` を相対で読むので、立てて呼ぶと落ちる）。`${CARGO_TARGET_DIR:-target}` で済む | `web/build.sh:48` | 道具 | 計画に足す: S4 で直す |
| 09-20 S1 | **箱庭の窓のチェックに揺れがある**: 無改変の状態で 3 走行中 2 走行が 1 件ずつ FAIL（`Revert puts every beetle back on the file`、`every restarted beetle's new task has run`。どちらもエディタの Apply / Revert / 再起動の周り）。これまでの揺れの記録はヘッドレスだけで、窓側のこの 2 件は記録が無い。S1 の後の 3 走行は全部 ok 43 だったが、3 走行では何も言えない | `garden/src/window.rs` の窓のチェック。再現: コンテナのビルド直後に `GARDEN_SELFTEST=1` で release の garden を続けて 2 回 | ゲーム固有（箱庭の判定）。バグ候補 | **本体が原則から決めた（09-20、著者「原則を大切に判断して」）**: **調査を立てる**（S6。S4 の後、S5b の前）。S5b も Factory も「selftest が前後で同じ」を合否に使う。揺れる物差しで測った合格は確認にならない。前の揺れ（`2026-09-17-selftest-flakes.md`）と同じ形: 何十回か回して数え、原因を割り、直し方を「何を動かすか」つきで並べて**報告して止まる** |
| 09-20 S1 | Battle のヘッドレスは当たりの数が走行ごとに大きく振れる（25 秒で 20 発と 9 発）。selftest の**行数**は前後比較の基準にならず、「当たりに依らない行の集合」で比べる必要がある | `sabibots` の selftest | 本の素材／確認の作法 | 計画に足した: S2 以降の確認は行の集合で比べる。book の findings に写す |
| 09-20 S1 | `rubevy-arena` が PC ビルドで `sabiruby-compiler` に依存するようになった（S1 の判断）。「共有 crate は画面・ファイル・ブラウザのもの」という線引きからは新顔 | `crates/rubevy-arena/Cargo.toml` | 共有 crate（構成） | **本体が原則から決めた（09-20、著者「原則を大切に判断して」）**: **今は認めて記録する**。`rubevy-arena` の構成の見直しと公開は著者の仕事として保留されている（2026-09-19）ので、ここで先回りして割らない。見直しの材料としてこの行を残す |
| 09-20 S1 | `--headless` の既定 10.0、`--shot` の既定 6.0 / 3.0 / `shot.png` に出どころが無い。文書の走行例は常に数を渡すので、既定値が効く走行は文書に 1 つも無い | `garden/src/main.rs`、`sabibots/src/main.rs` | 出どころの無い数 | `docs/numbers.md` の (c) に既にある。S5b で |
| 09-20 S1 | 2 本の `build.rs` の冒頭のコメント（「The PC build … gets the table too」）が実物と違う。`docker/run.sh:23` は `SABIBOTS_SELFTEST` 固定で箱庭の窓のチェックを回せない（実際に回せず手で `docker run` を書いた）。この機械の PATH に `wasm-opt` が無く `web/build.sh` は縮めずに通る | `*/build.rs`、`docker/run.sh:23`、環境 | 文書と実物のずれ／道具／環境 | `build.rs` は S2 で消える。`run.sh` は S4。`wasm-opt` は `~/.local/binaryen-version_132/bin` を PATH に入れる（担当への依頼文に書く） |
| 09-20 S5a | **Battle の数が Rust と Ruby に別々にある**: `UNSET = -999.0`、弾の速さ 55 / 30。繋がっていないので片方を動かすともう片方が黙って嘘をつく（`lead` が外す） | `sabibots/src/main.rs:42-43,51`、`sabibots/ruby/prelude.rb:59-62` | ゲーム固有（Battle） | S5b の Battle で最初に直す（Rust が Ruby に渡す 1 か所に） |
| 09-20 S5a | **変異率 0.1 が 2 か所で、判定が追随しない**: 本物は `beetle.rb` の `mutate(0.1)`、8 番目の判定は Rust の `const RATE = 0.1`。エディタで `beetle.rb` を書き換えると判定が FAIL になる | `garden/ruby/creatures/beetle.rb:81`、`garden/src/main.rs:3955` | ゲーム固有（箱庭、判定の設計） | S5b: 判定が VM から実際の値を読む形に（`@handlers` / `@asleep` を読んだのと同じ道）。著者判断済み（09-20、推奨どおり） |
| 09-20 S5a | 空腹バーの色の境 55.0 と甲虫の `hungry_below` 55.0 が同じ数で繋がっていない | `garden/src/window.rs:1059`、`beetle.rb:10` | ゲーム固有（箱庭） | S5b で**繋がない**（本体の判断）: 記録に意図が無い以上、同じ値であることは偶然として扱い、`docs/numbers.md` に「`hungry_below` と同じ値だが繋がっていない。出どころ不明」と書く。意図があったなら著者が直す |
| 09-20 S5a | Battle の体力の満 100 と VM 予算バーの満 3000 が名前無しで 4 か所・2 か所に散っている。`panel.budget = 3_000` は毎フレーム代入 | `sabibots/src/main.rs:638,715,1086,1565`、`:659,2308` | ゲーム固有（Battle）／共有 crate の `ScriptPanel` | S5b の Battle の最初の作業 |
| 09-20 S5a | `--shot` の既定秒数が 2 本で違う（箱庭 6.0、Battle 3.0） | `garden/src/main.rs:1474`、`sabibots/src/main.rs:271` | 共有 crate（S1） | S1 の担当に伝えた: 寄せない、既定値はゲームが渡す |
| 09-20 S5a | コメントと数の食い違い 3 つ: `CodePanel` の「58 文字」と `WIDTH = 44`（同じコミット）、`ArenaPlugin` の「3 分の 1」と 0.16（計算は合うが 1 行で繋がらない）、`CLICK_REACH` 1.6 と「体の幅くらい」（体は 0.8〜1.0） | `crates/rubevy-arena/src/code.rs:147-149`、`lib.rs:117-118`、`garden/src/window.rs:82,92-93` | 共有 crate／箱庭 | `ArenaPlugin` は S3 で言い直す。ほかは S5b で出どころを書くときに |
| 09-20 S5a | 新しい芽の最小間隔 1.5 だけが W1 の後も Rust に残っている。`TOUCH_REACH` 1.3 は `garden.rules` の受け口（今 5 つ）を増やすか `startle` ごと移すかで話が変わる。エディタの 9 色は変えられるようにすると測った保証（コントラスト、ΔE）を破れる | `garden/src/main.rs:3526`、`:253`、`:4446`、`editor.rs:454` | 分類に迷った数（`docs/numbers.md` §8 に 10 件） | 著者判断済み（09-20）: 本体の案で進めて結果を報告。原則は「遊びの数は Ruby、走査は Rust」「測った保証のある数は変えられるが、保証が破れることを rustdoc に書く」 |

## 8. 名前の検討（`rubevy-arena`）

著者（2026-09-20）「アリーナクレートの名前も検討して」。

**今の名前が合わない理由**: 「arena」は Battle の闘技場。crate が今持っているのは、その闘技場のカメラ（`ArenaPlugin`）を除けば
「Ruby をゲームの中で書き換えるゲームが、ゲームの外側に要るもの」— エディタ、VM パネル、コード表示、ファイル監視、PC とブラウザの差（保存・コンパイルの橋）、
selftest の枠、引数、案内（英日）、設定、パン・ズームのカメラ。3 本目（工場）にアリーナは無い。

**名前を決める原則**（`rubevy` は汎用、games は利用例。ゲーム固有のものを rubevy の名前で外に出さない）: crate の名前は**外の人が読んで中身が分かる**こと、
`rubevy-` を冠するなら **rubevy の利用者が使えるもの**であること。今の crate は 2 種類が同居している:

| 種類 | 中身 | 外の rubevy 利用者に意味があるか |
|---|---|---|
| A. スクリプトを見せる・書かせる道具 | `Editor`、`VmInspector`（+ `VmClock`）、`CodePanel`、`Watch` | **ある**。rubevy + bevy_egui だけに依存し、ゲームの語彙を持たない。crates.io に出す候補はこれだけ |
| B. このサンプル集の殻 | `platform`（`localStorage`・JS の橋）、`checks`、`args`、`Guide`（日本語フォント同梱）、`Settings`、`Hud`、`ArenaPlugin`、パン・ズームのカメラ | 薄い。このリポジトリのゲームの作り方（1 サイト 3 ゲーム、英日、`?selftest`）に合わせたもの。カメラは rubevy と無関係の Bevy の部品 |

**案**（crates.io の `cargo search` で、どれも未使用であることを確かめた。2026-09-20）:

1. **2 つに割って、それぞれ中身を言う名前にする（本体の推奨）**
   - A → **`rubevy-egui`**: 「rubevy のための egui の部品」。Bevy の周りの慣習（`bevy_egui`、`bevy-inspector-egui`）と同じ言い方で、依存が名前に出る。
     将来 crates.io に出すならこの名前。
   - B → **`games-shell`**（`rubevy-` を冠さない。公開しない）: このサンプル集のゲームが共通に着る殻。
   - 代償: crate が 2 つになり、B が A に依存する所（`Guide` の egui の窓、塞がれ px を書く側）を整理する必要がある。S3 が入れる
     「塞がれ px の Resource」がちょうどその継ぎ目になる。
2. **1 つのまま、名前だけ変える**: `rubevy-workbench`（ゲームの中の作業台 — 書く・見る・試す）。割らないので S4 で機械的に済む。
   代償: A と B の同居は残り、公開の話は先送りのまま。`rubevy-kit` / `rubevy-tools` は中身を何も言わないので勧めない。
3. **変えない**: 名前が中身を言っていない状態が 3 本目にも持ち越される。勧めない。

**本体の推奨は 1**。理由: 著者が 2026-09-19 に保留した「構成を見直してから crates.io に」の見直しの中身がまさにこの 2 種類の同居で、
名前を決めることと割ることは同じ 1 つの判断。3 本目が入る前（S4）にやれば、Factory は最初から正しい名前を `use` できる。
割るのが今は重いなら 2 を採り、割るのは公開のときに、でもよい。

### 決定（著者、2026-09-20）: 案 1 — 段階 S4a

`crates/rubevy-arena` を 2 つに割り、`rubevy-arena` という名前は無くす。

| 新しい crate | 入るもの | 依存 | 公開 |
|---|---|---|---|
| **`crates/rubevy-egui`** | `editor`（`Editor`・`EditorPlugin`・`EditorAction`・`EditorChoice`・`Highlighter`・寸法の設定）、`inspect`（`VmInspector`・`VmInspectorPlugin`・`Waiting`・`VmClock`・`VmClockSet`）、`code`（`CodePanel`）、`Watch`、**塞がれ px の Resource**（エディタが書く側なのでこちら） | bevy、bevy_egui、rubevy、sabiruby（`inspect` が要る）、`notify`（PC の `Watch`） | しない（crates.io に出すかは著者が別に決める。出せる形にはしておく: ゲームの語彙なし、`games-shell` に依存しない） |
| **`crates/games-shell`** | `platform`、`checks`、`args`、`guide`（日本語フォント同梱）、`settings`、`hud`、`ArenaPlugin`（固定の 2D カメラ）、`camera`（パン・ズーム） | bevy、bevy_egui、`rubevy-egui`（塞がれ px を読む）、PC で `sabiruby-compiler`、wasm で `js-sys` / `wasm-bindgen` / web-sys | しない |

- **依存の向きは `games-shell` → `rubevy-egui` の一方向。** 逆向きの参照が 1 つでも残るなら、どちらに置くかを見直す（`rubevy-egui` が殻を知ってはいけない）。
  S3 が実際にどう切ったか（塞がれ px の Resource の置き場所、`Editor` の `Default`）を先に読み、上の表と食い違う所は実物に合わせて表を直し、理由を worklog に。
- `use rubevy_arena::…` は 2 本のゲームと docs に広く出る（repo 内 47 ファイル）。**互換の別名 crate は残さない**（名前が中身を言わない状態を残すことになる）。
  機械的な置換だが、`docs/worklog/` の過去の記録は**書き換えない**（その時点の名前が正しい）。`docs/README.md`・`README.md`・`docs/garden.md`・`docs/sabiruby-battle.md`・`docs/web.md`・
  `docs/numbers.md`・計画書・`tools/subset-font.sh`（フォントと `guide.rs` の場所）・`CREDITS.md` は今の名前に直す。
- フォントの `include_bytes!` の相対パス、`crate_dir!` マクロ（`env!("CARGO_MANIFEST_DIR")` を運ぶ）、egui の窓の id（`"rubevy-arena-guide"` — **変えない**。利用者の egui のメモリに残る名前なので、
  変える理由が無い。rustdoc に「旧名が id に残っている」と 1 行）に注意。
- 動きは変えない。確認は S1〜S3 と同じ: `cargo test --workspace`、2 本の selftest（行の集合で比べる）、wasm ビルド、`web/build.sh all` + Playwright（pageerror 0、requestfailed 0）。
- 死んでいるもの（`HudPlugin`、`CodePanel`、`read_script`）の扱い: `read_script` は未使用で wasm でも動かないので**移さずに消す**（割るときに行き先の無いものを運ばない）。
  `CodePanel` と `Hud` は使い道が文書に書いてある（egui を使わないゲーム向け、Battle が `Hud` を文字列の受け皿に使う）ので移す。

