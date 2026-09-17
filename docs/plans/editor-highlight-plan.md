# エディタの構文色付け（Prism の字句解析で）— 実装指示書

作成 2026-09-18。著者「エディタのコードに色付きハイライトがないので読みにくい。family-core のエディタでは対応した実績がある」→ 検討の結果、自前の字句解析（精度 95%、正規表現・ヒアドキュメント・% リテラル・式展開の中身で塗り間違う）ではなく、**family-mruby と同じ Prism の字句解析**で 9 種に分類する案 2 を著者が選んだ（2026-09-18「2 を計画に入れて」）。

**この文書だけで着手できるように書いてある。** 3 リポジトリにまたがる（sabiruby → sabiruby-playground → rubevy_games）。読む順は 1 → 2 → 3 → 4。5 は着手前に必ず目を通す。

---

## 0. はじめの一歩

```bash
# 段階ごとに repo が違う。各 repo で main から枝を切り、push しない（本体がマージして push してから次の段階へ）
git -C /home/kishima/book/kishima/sabiruby worktree add -b highlight ../sabiruby-wt-highlight main
git -C /home/kishima/book/kishima/sabiruby-playground switch -c highlight            # 小さい repo。worktree 不要
git -C /home/kishima/book/kishima/rubevy_games worktree add -b editor-highlight ../rubevy_games-wt-highlight main
```

作法は `/home/kishima/book/CLAUDE.md` と `/home/kishima/book/.claude/agents/implementer.md`。**unsafe を書かない（sabiruby の FFI 層は既存の `unsafe extern "C"` の中に足す。新しい unsafe ブロックを増やさない）。根拠のない数を置かない。**

---

## 1. 何を作るのか（30 秒版）

両ゲームのエディタ（`crates/rubevy-arena/src/editor.rs`）は egui の `TextEdit` に自前の `layouter` を渡していて、`listing()` が行ごとの色と背景（脳の熱の帯）を `LayoutJob` に積んでいる。ここを**トークンごとの前景色**に広げる。分類は family-mruby の `picoruby-syntax-highlight`（`fmruby-core/lib/add/picoruby-syntax-highlight/src/syntax_highlight.c`、325 行、Prism の `pm_lex_callback_t` で 1 トークンずつ分類してバイト表に書く）と同じ 9 種:

| # | 分類 | 例 |
|---|---|---|
| 0 | 既定 | 演算子、括弧、識別子 |
| 1 | キーワード | `def` `do` `end` `if` `class` `loop` |
| 2 | 文字列 | `"..."` `'...'`、式展開の外側 |
| 3 | コメント | `# ...` |
| 4 | 数値 | `0.06` `100` |
| 5 | シンボル | `:Plant` `key:` のラベル |
| 6 | 定数 | `Rubevy` `MyApp` |
| 7 | 変数 | `@memory` `$rubevy` |
| 8 | メソッド名 | `def` の後、`.` の後 |

字句解析は **Prism**（`sabiruby-compiler` に `vendor/prism` が入っている）。ネイティブはそのまま呼び、ブラウザ版はページの `sabi.js`（playground の wasm）に同じ口を足して `window.gardenHighlight(source)` で受ける（`gardenCompile` と同じ形）。

**境界**: 分類は Prism（正確）、色と描画は `rubevy-arena`（egui）。トークナイザを自前で書かない。

---

## 2. 決まっていること・既定

### 決まっている（著者）

1. Prism の字句解析を使う（案 2）。分類 9 種は family-mruby と同じ。
2. 熱の帯（背景）は残す。色は前景。

### 既定（違和感があれば止めて報告）

| # | 項目 | 既定 |
|---|---|---|
| 1 | 出力の形 | **バイトごとの分類表** `Vec<u8>`（長さ = 入力のバイト数、値 0〜8）。family-mruby と同じ。行やトークン境界の表より単純で、`listing()` が UTF-8 の文字境界で区切って塗るだけでよい |
| 2 | 解析に失敗したとき | Prism は構文エラーがあってもトークンを出す（回復する）。**エラーでも分類表を返す**（編集中は常に壊れているため）。表を返せないのは入力が `HIGHLIGHT_MAX_SOURCE_SIZE` 相当を超えたときだけで、その上限は family-mruby の 32 KiB を**そのまま採らず**、`prelude.rb`（21 KB）+ 種のファイルの合計が今いくつかを測って決める（根拠を書く） |
| 3 | ネイティブの口 | `sabiruby-compiler` の `pub fn highlight(src: &[u8]) -> Vec<u8>`。C 側は `csrc/shim.c` に `sabiruby_mrc_highlight(src, len, out)`（family-mruby の `token_type_to_category` と `highlight_callback` を移す）。feature は `host`（既存の `compile` と同じ側） |
| 4 | ブラウザの口 | playground の `wasm/src/lib.rs` に `sabi_highlight(src, len) -> u32` + `sabi_take_highlight`（`sabi_take_binary` と同じ「take」の形）。`web/sabi.js` に `highlight(src)`。`rubevy_games/web/garden.html` と `sabibots.html` に `window.gardenHighlight` / `window.battleHighlight`（`gardenCompile` の隣、同じ try の中）。`garden/src/platform.rs` の wasm 側に `js_highlight`、ネイティブ側は `sabiruby_compiler::highlight` |
| 5 | いつ解析するか | **本文が変わったときだけ**（`Editor.text` のハッシュか長さ+変更フラグで比較）。毎フレームの `layouter` の中では呼ばない（Prism は 20 KB を数百 µs で通すが、60 fps で毎回はもったいない）。結果は `Editor.highlight: Vec<u8>` に持つ |
| 6 | 色 | 9 種の色は暗い配色（今の本文 `210,214,222`、熱い行 `255,236,150`、gutter `120,128,140`）と**同じ系統**で選ぶ。熱の帯（背景 `150,110,20` の α）と重ねて読めること。**既定の文字色（分類 0）は今の `210,214,222` のまま**。コメントは gutter と同じ弱い色。選んだ色の根拠（何と何を区別したいか、帯の上で読めるか）を worklog に |
| 7 | prelude の行 | エディタが映すのは種のファイル・`world.rb` 本文だけ（prelude は見えない）。**解析も本文だけ**に掛ける（prelude を付けて解析すると本文の頭が別の文脈になることは無い。逆に `run_creature` を末尾に付けないので末尾も素直）。行番号のずれの補正（`2026-09-18-garden-leftovers.md` の件）とは無関係 |
| 8 | ON/OFF | 作らない（全ファイルが Ruby） |
| 9 | Battle | `rubevy-arena` を共有しているので同じ変更で色が付く。`sabibots.html` にも橋を足す。Battle のセルフテストが無変更で通ること |
| 10 | pages.yml | `PLAYGROUND_REF` を H1 の playground のコミットに進める（今は `3f47c9d`。`docs/web.md` と `pages.yml` の冒頭のコメントに理由が書いてある） |

---

## 3. 設計

### 3.1 sabiruby-compiler（H0）

* `csrc/shim.c`: family-mruby の `token_type_to_category`（`PM_TOKEN_KEYWORD_*` → 1、`PM_TOKEN_STRING_*` / `PM_TOKEN_STRING_CONTENT` → 2、`PM_TOKEN_COMMENT` / `EMBDOC_*` → 3、`PM_TOKEN_INTEGER` / `FLOAT` / `RATIONAL` / `IMAGINARY` → 4、`PM_TOKEN_SYMBOL_BEGIN` / `LABEL` → 5、`PM_TOKEN_CONSTANT` → 6、`PM_TOKEN_INSTANCE_VARIABLE` / `GLOBAL_VARIABLE` / `CLASS_VARIABLE` → 7、`def` の直後と `.` の直後の `IDENTIFIER` → 8）と `highlight_callback`（`pm_lex_callback_t`、トークンの `start..end` を表に書く）を移す。`pm_parser_init` → `lex_callback` を設定 → `pm_parse` → `pm_node_destroy` → `pm_parser_free`。family-mruby の版と Prism の版の差（`vendor/prism` の版）でトークン名が変わっていれば合わせる。
* `src/ffi.rs`: 既存の `unsafe extern "C"` ブロックに宣言を 1 つ足す。`src/lib.rs`: `pub fn highlight(src: &[u8]) -> Vec<u8>`（`host` feature）。
* テスト（`compiler/tests/highlight.rs`）: 箱庭の `beetle.rb` 相当の小さな Ruby を入れ、キーワード・文字列・コメント・シンボル・定数・ivar・メソッド名が期待の分類になること、構文エラーのある入力でも表が返ること、式展開 `"a #{b} c"` で `b` が 0 で外側が 2 になること、正規表現 `x =~ /re/` の `/re/` が 2 か（Prism が何と言うかを実測して**その値を期待にする**。推測で書かない）。
* `no_std` の VM crate は触らない（compiler は std）。`tools/check_no_std.sh` が通ること。版は `0.5.2`（`Cargo.toml`）。CHANGELOG があれば 1 行。

### 3.2 sabiruby-playground（H1）

* `wasm/src/lib.rs`: `sabi_highlight(src, len) -> u32`（0 = OK）と `sabi_take_highlight`（表を返す）。`sabi_compile` / `sabi_take_binary` と同じ書き方（`#[unsafe(no_mangle)]` は既存の形。新しい unsafe ブロックは増やさない）。
* `web/sabi.js`: `highlight(src)`（`compile` と `binary` の形）。
* `tools/build.sh` で `web/sabiruby.wasm` を組み直し、`test/` にあるテストの形で 1 本（`highlight("def a; end")` の先頭 3 バイトが 1）。サイズの増分を README の表に。
* playground の自分のエディタ（CodeMirror）は**触らない**（CodeMirror は自分の色付けを持つ）。

### 3.3 rubevy_games（H2）

* `garden/src/platform.rs` / `sabibots/src/platform.rs`（相当）: `highlight(src: &str) -> Vec<u8>`。ネイティブ = `sabiruby_compiler::highlight`、wasm = `window.gardenHighlight` / `battleHighlight`（無ければ全部 0 = 色なしで動く。橋が無い古いページでも壊れない）。
* `crates/rubevy-arena/src/editor.rs`: `Editor.highlight: Vec<u8>` と `Editor.highlighted_for: u64`（本文のハッシュ）。`Editor` は `rubevy-arena` にあり `platform` はゲーム側なので、**解析関数はゲームがプラグインに渡す**（`EditorPlugin::with_highlighter(fn(&str) -> Vec<u8>)` か `Resource` の `Highlighter(Box<dyn Fn>)`。既存の `compile` の渡し方に揃える）。`listing()`: 行ごとに、その行のバイト範囲を分類表で区切り、分類が変わるところで `job.append` を分ける。熱の帯は行の `background` のまま。
* 色の表（9 種）は `editor.rs` の定数 1 か所。既定 6 のとおり選び、根拠を worklog に。
* `web/garden.html` / `web/sabibots.html`: `window.gardenHighlight` / `battleHighlight`。`web/build.sh` は playground の `web/sabiruby.wasm` と `sabi.js` を写す既存の道なので変更なし（playground を H1 の版にしてから組む）。
* `pages.yml`: `PLAYGROUND_REF` を H1 のコミットに。
* セルフテスト（窓）: 両ゲームに 1 行「エディタを開いたとき `def` が キーワードの色で描かれている」（`LayoutJob` の `sections` を見るか、`Editor.highlight` の該当バイトが 1 であることを見る。描画の画素は見ない）。
* docs: `docs/garden.md` のエディタの節、`docs/sabiruby-battle.md` の該当、`docs/web.md` の表（playground の wasm のサイズが変わる）、`docs/README.md`。

---

## 4. 段階

| 段階 | repo | 内容 | 確認 |
|---|---|---|---|
| H0 | sabiruby | `highlight()`（3.1） | `cargo test`（compiler のテスト含む）、`tools/check_no_std.sh`、本家テスト基準を下回らない、clippy 増減なし |
| H1 | sabiruby-playground | `sabi_highlight`（3.2）。**H0 が sabiruby main に入ってから**（`wasm/Cargo.toml` は `../../sabiruby` の path 依存） | `tools/build.sh`、`test/` の 1 本、ページで `highlight` が動くこと（Playwright で console から呼ぶ） |
| H2 | rubevy_games | エディタと橋と色（3.3）。**H1 が playground main に入ってから** | 両ゲーム `--headless` selftest、窓のセルフテスト（新 1 行ずつ）、`web/build.sh`（両ゲーム）+ Playwright で pageerror 0 と `?selftest`、`--shot` で色が付いた絵を 1 枚（`docs/` に置き換え） |

各段階は 1 コミット（+docs）。段階の間に本体がマージ・push する。

---

## 5. 分かっている罠

1. **Prism の版**: family-mruby の `syntax_highlight.c` が書かれた Prism と `sabiruby/compiler/vendor/prism` の版が違えばトークン名が変わっている。`vendor/prism/include/prism/ast.h` の `pm_token_type_t` を見て合わせる。
2. **式展開**: Prism は `"a #{b} c"` を `STRING_BEGIN` / `STRING_CONTENT` / `EMBEXPR_BEGIN` / `IDENTIFIER` / `EMBEXPR_END` / `STRING_CONTENT` / `STRING_END` と刻む。中の `b` は 0（既定）で正しい。family-mruby の分類表がどうしていたかを見て揃える。
3. **メソッド名（分類 8）は字句だけでは決まらない**: family-mruby は字句の後に**構文木をもう一度歩く第 2 段**（`pm_visit_node`: `PM_CALL_NODE.message_loc` → 8、`PM_DEF_NODE.name_loc` → 8、`PM_SYMBOL_NODE` 全体 → 5）を持っている。`tell :all` / `sleep 0.5` / `every 60 do` のように `.` も `def` も付かない呼び出しはこれでしか塗れない。同じ形にする（初版の「直前のトークンで判定」は事実誤認。H0 の担当が見つけた）。
4. **UTF-8**: 分類表はバイト単位、egui の `append` は `&str` を取る。日本語のコメント（箱庭の脚本に多い）で**文字の途中で区切らない**こと。行を `char_indices` で歩き、分類が変わるバイト位置が文字境界であることを確かめる（Prism のトークン境界は文字境界なので合うはずだが、テストで日本語コメントを 1 行入れる）。
5. **ブラウザの橋が無いとき**: `window.gardenHighlight` が未定義（古い playground の `sabi.js` や別のページ）でも落ちない（`catch` して全部 0）。黒画面の再発を避ける（`2026-09-18-web-black-screen.md`）。
6. **毎フレーム解析しない**（既定 5）。`layouter` は毎フレーム呼ばれる。
7. **`pages.yml` の `PLAYGROUND_REF`** を進め忘れると、公開版だけ色が付かない（H2 の確認に「公開版と同じ手順で組む」を入れる）。
8. **同じ `editor.rs` を触る作業**（`2026-09-18-garden-leftovers.md` の残件: ホイール、行番号、世界の VM パネル）が先に走っている。**その枝が main に入ってから H2 を切る**。

---

## 6. 状況

| 段階 | 状態 |
|---|---|
| H0 | 未着手 |
| H1 | 未着手 |
| H2 | 未着手（残件の枝 `leftovers` のマージ後） |
