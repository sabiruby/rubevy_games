# 2026-09-20 S1: `platform` と `checks` と引数解析を共有 crate へ

計画書 `docs/plans/shared-crate-plan.md` の段階 **S1** のみ。ブランチ `shared-crate`（worktree
`rubevy_games-wt-shared`、main には触っていない）。**動きは変えないこと**が到達点の半分なので、着手前に基準を取り、
同じものを取り直して突き合わせた。

## 0. 着手前の基準（2026-09-20、i7-13700、24 スレッド）

| 何を | どう回したか | 結果 |
|---|---|---|
| `cargo test --workspace` | そのまま | 18 通過（garden 6、rubevy-arena 12）、0 失敗 |
| 箱庭ヘッドレス | `GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90` | `ok 13` / `FAIL 0` / `-- 5` |
| 箱庭の窓 | 下記の docker（WSL に GPU ドライバが無い） | `ok 43` / `FAIL 0`（3 走行のうち 1 走行。残り 2 走行は後述） |
| Battle ヘッドレス | `SABIBOTS_SELFTEST=1 cargo run -p sabibots -- --headless 25` | `ok 44` / `FAIL 0`（固定 4 行 + 当たり 20 発 × 2） |
| Battle の窓 | `SABIBOTS_SELFTEST=1 docker/run.sh` | `ok 36` / `FAIL 0` / `-- 2`。当たりに依らない行は **30** |
| wasm | `cargo build -p garden -p sabibots --target wasm32-unknown-unknown --profile web` | 通る |

窓が要る 2 つは、この機械では素の `cargo run` が
`Unable to find a GPU!`（`bevy_render-0.19.1/src/renderer/mod.rs:286`）で落ちる。`docs/wsl-gpu.md` の言うとおり
コンテナ（lavapipe）経由にした。Battle は `docker/run.sh` がそのまま使える。箱庭は `docker/run.sh:23` が
`-e SABIBOTS_SELFTEST` しか渡さないので、`docs/garden.md:45` が言うとおり `docker run` を手で書き、
`-e GARDEN_SELFTEST=1` と箱庭用のボリューム（`rubevy-games-target-garden1`）を指定した。Docker は既に動いていたので
起動はしていない（`docker info` が通ることだけ確かめた）。

**箱庭の窓のチェックは揺れた。** 3 走行のうち 2 走行で 1 件ずつ FAIL が出た —— 1 回目は
`FAIL Revert puts every beetle back on the file`、2 回目は `FAIL every restarted beetle's new task has run`。
どちらもコンテナのビルド直後・走行直後で機械が温まっている時間帯で、間を置いた 3 回目は 43 件すべて ok。
計画書 5 章の「混んでいると揺れる」に当たる。**これは着手前の、コードを 1 行も変えていない状態の話**なので、
以後の比較は「43 件の ok の集合が同じか」で行い、FAIL は出た走行を明記することにした。

## 1. 何をどこへ置いたか

2 本の `platform.rs` は、コメントと crate 名を除けば差が 4 つ（箱庭の `SAVE_FILE`・`SAVE_WHERE`・`reload_asked_at`・
`another_version_file`）しかない。箱庭側の冒頭が「共有するには 4 つを渡すことになり、2 本のうちは割に合わない。
3 本目が来たら価値がある」と予告していたので、その 4 つを引数にした:

- **`localStorage` の接頭辞**（`garden:` / `sabibots:`）
- **ブラウザの binary に埋め込む `ruby/` の表**（`build.rs` が `OUT_DIR` に書くもの）
- **ページ側の橋の名前**（`window.gardenCompile` / `gardenHighlight` ほか）
- **その crate 自身のディレクトリ**（PC の `ruby/` と `assets/`）

置き場所は `crates/rubevy-arena/src/` の新しい 3 モジュール。

| モジュール | 中身 |
|---|---|
| `platform` | `read` / `write`、`compile` / `highlight`、`clock_seed`、`SAVE_LABEL`、`pick_dir`、マクロ `crate_dir!` |
| `checks` | `selftest_asked(env_name)`、`asked_number(env_name)`、`CHECKS_EXIT_WHEN_DONE` |
| `args` | `Args`（`has` / `value` / `number` / `headless` / `shot`） |

起動の 10 行は `settings::remembered(path, header, read, write, lang_asked)`（`Settings::load` → `GuideLang::pick` を
1 つにしたもの）と `Guide::opening(lang, open)` に分けた。`Guide` を作るのはゲームの `guide_text::guide()` のままで、
`opening` は `lang` と `open` を差し替えるだけ。egui の窓の id（`guide.rs:276` の `"rubevy-arena-guide"`）も、
`Settings` の `"lang"` というキーも触っていない。

結果、2 本の `platform.rs` は「ゲーム固有の定数と包み」だけになった。

| ファイル | 前（全行 / コメントと空行を除く） | 後 |
|---|---|---|
| `garden/src/platform.rs` | 261 / 121 | 141 / **51** |
| `sabibots/src/platform.rs` | 201 / 103 | 98 / **33** |

共有側は `platform.rs` 115 行・`checks.rs` 22 行・`args.rs` 79 行（うちテスト 45 行）で、いずれも
「コメントと空行を除く」数。2 本から消えた 140 行が 1 か所になり、そこに PC とブラウザの両方の実装と
新しい単体テスト 4 本が乗っている。

## 2. 決めきれない点として計画書が挙げていた 2 つ

### `Settings` の `fn` ポインタの受け口（計画書 3.1）

`Settings::load` は素の `fn` ポインタで `read` / `write` を受ける（`settings.rs:34-37`、H2 の worklog によれば
「arena に関数を渡す唯一の先例」として選ばれた形）。接頭辞を値で持つと素の `fn` にならない。
**受け口は変えず、ゲーム側に包みを残す**方を採った。理由は小ささで測れる: 包みは 1 ゲームにつき

```rust
pub fn read(path: &Path) -> Result<String, String> { platform::read(STORE, RUBY_FILES, path) }
```

の 3 行 × 2 本 = 6 行で済む。受け口を変える側は、`Settings` の 2 つの型別名と構造体のフィールドと `load` の
引数に加えて、`Settings` を `Resource` として持ち回るために `Box<dyn Fn>` か `&'static` のクロージャが要り、
`EditorPlugin::with_highlighter(platform::highlight)` のように**他にも関数ポインタを渡している口**（`highlight` は
`Highlighter = fn(&str) -> Vec<u8>`）と形が食い違う。包みは `platform::read` という呼び名を 40 箇所以上の
呼び出し側でそのまま保つという副産物もある（`garden/src/window.rs` と `sabibots/src/main.rs` の全ての
`platform::read` / `platform::write` は 1 文字も変えていない）。

### `ruby_dir` をマクロにするか（計画書 3.1）

`env!("CARGO_MANIFEST_DIR")` は展開された場所の crate を答えるので関数では共有できない。
**マクロにした**。ただしマクロの中身は 1 行で、実体は関数:

```rust
macro_rules! crate_dir {
    ($sub:literal, $from_root:literal) => {
        $crate::platform::pick_dir(env!("CARGO_MANIFEST_DIR"), $sub, $from_root)
    };
}
```

`#[cfg]` を式の位置に置くマクロ（`#[cfg] { … }` を 2 つ並べる形）は最初に書いて捨てた: ブロック式に属性を付けると
先頭のブロックが文になって型が合わない。「`cfg` は関数の側、マクロは `env!` を運ぶだけ」にすると、ゲーム側は

```rust
pub fn ruby_dir() -> PathBuf { rubevy_arena::crate_dir!("ruby", "garden/ruby") }
```

の 1 行になり、`platform.rs` から `#[cfg]` の入れ子（`mod imp` を 2 つ）が消える。ゲームに残す案（計画書が並べた
もう一方）は 4 行 × 2 関数 × 2 本 = 16 行の `cfg` 付き本体が残るので、こちらが小さい。

## 3. ブラウザの橋を `js_sys::Reflect` に替えた

`#[wasm_bindgen(js_name = gardenCompile)]` は属性にリテラルを書くので、名前を引数にできない。
`js_sys::Reflect::get(&window, &JsValue::from_str(name))` で `window` の同じ名前の property を引き、
`js_sys::Function` に `dyn_ref` して `call1` する形にした。**`unsafe` は 1 行も要らなかった**（計画書の見立てどおり）。
ページ側の名前（`window.gardenCompile` / `gardenHighlight` / `sabibotsCompile` / `sabibotsHighlight`）は変えていない。

振る舞いの差は 2 点あり、どちらもページが正しければ同じ:

- **関数が無いとき**。前は `catch` 付きの束縛で、呼び出しの時点で `ReferenceError: gardenCompile is not defined` が
  飛び、その文字列がエラーになっていた。今は `Reflect::get` が `undefined` を返し、`dyn_ref` が外れて
  `this page defines no window.gardenCompile` という自前の文になる。`highlight` 側はどちらでも
  `_ => vec![0; src.len()]`（H2 の「色が無い表」）に落ちるので、見えるものは同じ。
- **返り値の検査**。wasm-bindgen が生成する束縛は返り値を無検査で `Uint8Array` として扱うが、`dyn_into` は
  検査する。ページが bytes を返す限り同じで、返さなくなったときに panic ではなくエラーになる。

エラーの文面 `format!("{name}: {message}")`（`name` は Ruby のファイル名）は前のまま。投げられた値の文字列化も
`e.as_string().unwrap_or_else(|| format!("{e:?}"))` のまま移した。

## 4. 数について

計画書と著者の方針（「マジックナンバーは基本的に禁止。ユーザが変えられるようにするべき」）に従い、
**共有 crate 側に数を 1 つも置かなかった**。引数解析の既定値は全部呼び出し側が渡す:

- `--headless` の既定 10.0（2 本とも同じ）、`--shot` の既定 `shot.png` と **箱庭 6.0 / Battle 3.0**（違う）。
  共有側に置けばどちらかに寄せることになり、それは「動きを変えない」に反する。`Args::headless(default)` /
  `Args::shot(default_file, default_seconds)` にして、ゲームの `main.rs` に `HEADLESS_SECONDS` などの名前で
  据えた。コメントには由来として**「S1 以前にここの `main` に直接書かれていた値」**とだけ書いた。
  この 3 つの数の**本当の出どころは記録が無い**ので、作らずに S5a（`docs/numbers.md` の棚卸し）へ送る。
- 既存の数（箱庭の `ZOOM_MIN` / `ZOOM_MAX` / `MIDNIGHT` / `NIGHT_DIAL_*`）は触っていない。

## 5. 終わってから取り直した数

| 何を | 前 | 後 |
|---|---|---|
| `cargo test --workspace` | 18 通過 / 0 失敗 | **22 通過 / 0 失敗**（新規 4 本: `args` 3、`platform` 1） |
| 箱庭ヘッドレス 90 s | `ok 13` / `FAIL 0` / `-- 5` | `ok 13` / `FAIL 0` / `-- 6` |
| 箱庭の窓（docker、3 走行） | `ok 43` / `FAIL 0` が 3 走行中 1、残り 2 走行は `ok 42` + `FAIL 1` | **3 走行とも `ok 43` / `FAIL 0`** |
| Battle ヘッドレス 25 s | `ok 44`（当たり 20 発） | `ok 22`（当たり 9 発）、`FAIL 0` |
| Battle の窓（docker） | `ok 36` / `-- 2`、当たりに依らない行 **30** | `ok 40` / `-- 3`、当たりに依らない行 **30** |
| wasm ビルド | 通る | 通る（警告 0） |
| `Cargo.lock` | — | rubevy-arena の依存が 3 行増えるだけ。版は 1 つも動かない（`wasm-bindgen 0.2.128` のまま） |

数が動いて見えるものは全部「走行ごとに変わるもの」で、判定は動いていない:

- 箱庭ヘッドレスの `--` は「`on(:mate)` を持たない種のつがい」と「夜に眠っていた」で、走行ごとに回数が変わる
  （2026-09-18 の `check-holes.md` が入れた n/a）。**13 件の ok の本文は前後で同じ**。
- Battle は当たり 1 発につき 2 行出る。ヘッドレスは 25 秒で 20 発と 9 発、窓は 3 発と 11 発。
  **当たりに依らない 30 行は前後で同じ集合**（`diff` して違ったのは
  `the next one is due in 39 ticks` → `13 ticks` の 1 語だけ）。
- 箱庭の窓の 43 行も、数字を伏せて `diff` すると**前後で完全に一致**した。

### ブラウザ（S1 では必須ではないが、橋を書き換えたので回した）

`web/build.sh garden` と `web/build.sh sabibots` の後、`web/serve.sh` 相当（`python3 -m http.server 8099`）と
Playwright（隣の `sabiruby-playground/node_modules/playwright-core` 1.63.0、`~/.cache/ms-playwright/chromium-1243`、
`--use-angle=swiftshader --enable-unsafe-swiftshader`、`page.goto` は `waitUntil:'commit'`）。
スクリプトは scratchpad に置き、リポジトリには入れていない。

| ページ | 長さ | pageerror | requestfailed | selftest |
|---|---|---|---|---|
| `garden/?selftest` | 160 s | **0** | **0** | `ok 43` / `FAIL 0` / `-- 2` / `done 1` |
| `sabibots/?selftest` | 70 s | **0** | **0** | `ok 54` / `FAIL 0` / `done 1`、当たりに依らない行 **31** |

箱庭の `ok 43` は `docs/README.md` の基準そのもの。Battle の「固定 31 行 + 当たり 2 行」も基準どおりで、
PC の窓が 30 行なのは `CHECKS_EXIT_WHEN_DONE` が `true` で `done` の 1 行を書かずに終わるから（差はこの 1 行だけ）。

**橋が本当に通っていること**は行の中身で分かる: `` selftest: ok `def` in the listing is painted in the keyword colour ``
は `window.gardenHighlight` を、`selftest: ok Apply restarts every beetle on the edited text` と
`after Apply the text is what the world runs` は `window.gardenCompile` を通らないと出ない。
この機械には `wasm-opt` が無いので module は縮んでおらず（39.8 MB）、`docs/web.md` の寸法とは比べていない。

## 6. やらなかったこと

- 使われていない `HudPlugin` / `CodePanel` / `read_script` は計画書どおり消していない。
- `Editor` の `Default`（`noun: "robot"`）は S1 の範囲外なので触っていない。
- `docker/run.sh` の `SABIBOTS_SELFTEST` のハードコードは S4。手で `docker run` を書いて回避した。
- `build.rs`・行番号補正・スクリプト差し替えは S2（rubevy の R6 待ち）。

## 気づいた点（S1 の範囲外。直していない）

1. **`docker/run.sh:23` が `-e SABIBOTS_SELFTEST` を固定している**（`docker/run.sh`）。ゲーム名を引数に取るのに
   環境変数だけ Battle のものなので、箱庭の窓のチェックはこのスクリプトでは回せない。今回は `docker run` を
   手で書いた。計画書が S4 に挙げているとおりで、実際に回せないことをこの目で見た、という追認。
   → 共有 crate／ゲーム固有の外、`docker/` の道具の話。
2. **`garden/build.rs`・`sabibots/build.rs` の冒頭の説明が実物と違う**（両方の 3〜4 行目、「The PC build … gets the
   table too, but does not use it」）。S1 の前は `include!` がブラウザ側の `mod imp` の中だけにあり、PC の binary は
   表を受け取っていなかった。S1 の後も PC 側は空の表を渡す形にしたので（埋め込みを増やさないため）、
   この 1 文は今も正しくない。S2 で `build.rs` を rubevy のヘルパに移すときに書き直すのが自然。
   → 文書と実物のずれ（コードのコメント）。
3. **箱庭の窓のチェックに揺れがある。** 着手前・無改変の状態で 3 走行中 2 走行が 1 件ずつ FAIL した
   （`Revert puts every beetle back on the file`、`every restarted beetle's new task has run`）。どちらもエディタの
   Apply / Revert / 再起動の周りで、機械が温まっている時間帯に出た。`docs/worklog/2026-09-17-selftest-flakes.md` と
   `2026-09-18-selftest-fixes.md` が扱ったのは**ヘッドレスの**揺れで、窓側のこの 2 件は記録に無い。
   再現手順は「コンテナのビルドの直後に `-e GARDEN_SELFTEST=1` で release の garden を連続で 2 回回す」。
   → ゲーム固有（箱庭の判定）／バグ候補。
4. **`--headless` の既定 10.0 と `--shot` の既定 6.0 / 3.0 / `shot.png` に出どころが無い。**
   `git log -S` が指すのは導入したコミット（箱庭は `2ece3a6` G0、Battle の `--shot` は `579a5b6`、
   Battle の `--headless` は `0f2111f` v0.1）で、どのコミットのメッセージにも `docs/` にも既定値の理由は無い。
   コメントには「S1 以前にここに書かれていた値」とだけ書いた。
   `docs/garden.md` も `docs/sabiruby-battle.md` も常に数を渡すので、既定値が効く走行は文書に 1 つも無い。
   → 出どころの無い埋め込みの数（S5a の一覧に入る）。
5. **Battle のヘッドレスの当たりの数が走行ごとに大きく振れる**（25 秒で 20 発と 9 発）。判定は
   「当たり N 発すべてでハンドラが走ったか」なので健全だが、**行数は基準にならない**。S2 以降で前後を比べるときは
   「当たりに依らない行の集合」で比べる必要がある（今回そうした）。
   → 本の素材（自動テストの比べ方）。
6. **`rubevy-arena` が PC ビルドで `sabiruby-compiler` に依存するようになった。** `platform::compile` /
   `highlight` を共有側に持ってきた結果で、2 本のゲームが既に同じ依存を持っていたので `Cargo.lock` の版は動かず、
   ビルド時間も増えない。ただし「共有 crate は画面・ファイル・ブラウザに依存し、ゲームの語彙は無いもの」という
   計画書 1 章の線引きからすると、Ruby のコンパイラは新顔ではある。`rubevy-arena` の構成見直し（`b1ce042`）の
   ときに一緒に見るべき点として挙げておく。ブラウザ側は依存が増えていない（`js-sys` / `wasm-bindgen` /
   `web-sys` の feature 2 つは、どちらのゲームも既に要求している）。
   → 共有 crate（構成の判断）。
7. **この機械には `wasm-opt` が無い。** `web/build.sh` は警告を出して縮めずに通す（39.8 MB / gzip 10.2 MB）。
   `docs/web.md` の寸法は binaryen のある機械のものなので、寸法を測り直すときは同じ条件で。
   → 環境（確認の条件）。
