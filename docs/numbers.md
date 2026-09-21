# 数の一覧 — rubevy_games が今持っている数と、その出どころ

計画書 `docs/plans/shared-crate-plan.md` の段階 **S5a**（一覧）と **S5b**（移す作業）。
対象は `crates/rubevy-egui/src/` と `crates/games-shell/src/`（S5a を取った時点ではどちらも `crates/rubevy-arena/src/` だった。
S4a で 2 つに割れた）、`garden/src/`、`sabibots/src/`、`garden/ruby/**/*.rb`、`sabibots/ruby/**/*.rb`。
拾い方と選び方の過程は `docs/worklog/2026-09-20-numbers-inventory.md`。

**§2〜§5 は S5a のままの提案で、行番号は `numbers` ブランチ（main `b1ce042` から分岐）の時点のもの。**
著者が見てから S5b で 1 節ずつ移す。

**§1（共有 crate）は S5b-1（2026-09-20）で移し終えた。** 行番号は `shared-crate` ブランチの
その時点のもので、`今変えられるか` の欄は移した先を言う。S3 が足したカメラの 8 件（§1.2）を
一覧に足したので、共有 crate の件数は 33 → 42 になった（§6）。**既定値は 1 つも動かしていない。**
過程は `docs/worklog/2026-09-20-numbers-shared-crates.md`。

---

## 0. 読み方

| 欄 | 意味 |
|---|---|
| 位置 | `ファイル:行`。定数なら定義行、無名の数なら書かれている行 |
| 毎F | ● は毎フレーム読まれる数。移すときに読む費用を測る必要があるもの |
| 今変えられるか | 再ビルドせずに動かせるか。`const` = ビルドし直し、`world.rb` = エディタで書き換えられる、`Settings` = `*.settings.txt`、引数 = コマンドライン |
| 分類案 | 下の (a)〜(e) |
| 出どころ | 引用（`ファイル:行` かコミット）。無ければ **不明**。「理由らしき記述はあるが測った記録は無い」は *理由のみ* と書く |

### 分類

* **(a) 不変量** — 変えると壊れる。壊れる理由を 1 行添える。`const` のまま。
* **(b) 遊びの数 → Ruby 側** — 箱庭は `ruby/world.rb` か種の `.rb`、Battle は `ruby/matches/*.rb` か robot の `.rb`。
* **(c) 動かす側の数 → `Settings` と起動の引数** — 予算、`frame_time`、表示、カメラ、判定の猶予。
* **(d) selftest の閾値** — 検査の側に残す。出どころを 1 行。
* **(e) 既に Ruby か設定から変えられる** — もう移す必要が無いもの。

### 入れなかった基準

一覧に**入れなかった**のは、数そのものに意味が無いもの:

1. **0 と 1**（初期化 `= 0`、`+ 1`、`.max(1)`、`unwrap_or(0)`、`% 2` の類）。
2. **添字とスロット番号**（`when 0 then __handler_0`、`args.get(i + 1)`、`sorted[len / 2]`、`TEAMS[team.min(len - 1)]`、`test.step = 7`)。
   スロットの**個数**（`ON_SLOTS`、`EVERY_SLOTS`、`REGS_FRAMES`）は入れた — 上限だから。
3. **単位換算と数学の定数**（`* 255.0`、`/ 1000.0`、`1.0 / 60.0`、`Math::PI / 2`、`TAU`、`(1u64 << 24)`)。
4. **配列の長さとしての自明な数**（`[Species; 2]`、`[(f32, f32, f32); 4]`、`[f64; 3]`、`Vec<[f32; 2]>`）。
   ただし**その長さが上限として効くもの**（`TEAMS` の 4、`Brains` の 3 枠）は入れた。
5. **PRNG の混ぜ定数**（`0x9E37_79B9_7F4A_7C15` ほか。splitmix64 の仕様値で、変えるのは別の乱数にすることであって調整ではない）。
6. **selftest の入力値と台本の待ち時間**（`test.at = now + 0.5` のような段取り、`by_number(3)`、仕込みの座標の `-4.0, -3.0`）。
   selftest が**判定に使う閾値**は (d) として入れた。
7. **浮動小数の比較の ε**（`1e-4`、`1e-6`、`1e-3`、`< 0.01`）。
8. **テストのアサーションに書かれた期待値**（`in_the_authors_lines` の 482、`notches_of` の 100.0 など。本体の数の写し）。
9. **guide の本文に出てくる数**（「2 チーム 8 体」「太陽は 1 分で 1 周」。本文であって設定ではない。ただし**本文と実物がずれる**ので §7 に挙げた）。

迷ったら入れた。§8 に「迷った数と迷った理由」を挙げる。

色は **1 組 1 行**にまとめた（9 色の表を 27 行にしても読めないため）。値は代表だけ書き、残りは位置を見る。

---

## 1. 共有 crate（`crates/rubevy-egui` と `crates/games-shell`）

> **S5b-1（2026-09-20）で、この節の数は全部「利用者が変えられる場所」に移した。** 行番号は
> `shared-crate` ブランチの S5b-1 の時点のもの。`const` は**既定値の名前としてだけ**残っていて、
> どれも rustdoc の 1 行で出どころ（測った／導出／引用／理由のみ／**不明**）を言う。
> **既定値は 1 つも動かしていない**（2 本の selftest の 6 通りの走行が前後で同じ行を出す。
> `docs/verification/selftest-lines.md`）。
>
> `今変えられるか` の欄は移した先を言う。`Settings` の欄に鍵があるものは `*.settings.txt` に
> 1 行書けば次の走行で効く（読むのは `games_shell::PanelSettingsPlugin`、`PreStartup`）。
> 鍵の無いものは app が起動時に渡すか、走行中に Resource へ書く。
> **色と鍵盤は `Settings` に出していない** — 色を文字列から読むには構文解析が要り、
> それは S3 が `CameraKeys` を `Settings` に出さなかったのと同じ理由（`camera.rs`）。
>
> S4a の注意書き（`crates/rubevy-arena` が 2 つに割れてファイルが動いた）はここで解消した。
>
> **S5b-2（2026-09-21）が 2 行足した**（42 → 44 件）。S5b-1 が「一覧に無い数がまだある」と
> 報告した VM パネルの 9 か所の色（→ §1.4 の 1 行、`1 組 1 行` の規則で）と説明パネルの
> 余白 3 つ（→ §1.5 の 1 行）で、**数は 1 つも動かしていない**。行番号も S5b-2 の時点で
> 取り直してある。
>
> **S10（2026-09-21）が 1 行足した**（44 → 45 件）。**新しい数ではない** — 箱庭と Battle が
> 同じ値・同じ出どころで別々に持っていた「1 フレームが買う命令数」45,600 を、両方が届く
> `games_shell::checks` に 1 つにまとめたぶんで、§2.11 から移ってきた（§1.8）。
> 既定値は動いていない。

### 1.1 アリーナのカメラ（`games-shell/src/lib.rs`）

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| アリーナの半幅 | — | — | | **共有 crate から消えた**（S5b-1）。ゲームが渡す: `ArenaPlugin::showing(half)` / `ArenaSize(half)`。Battle の `ARENA_HALF_WIDTH`（`sabibots/src/main.rs:64`、32.0） | (b) ゲームの遊びの数 | **不明**（32.0 の出どころ。`sabibots` の rustdoc に「不明」と書いた） |
| `FLOOR_MARGIN`（壁の外に見せる床） | `lib.rs:67` | 3.0 | | `ArenaPlugin { floor_margin }` / `ArenaPlugin::showing(..).with_floor_margin(..)`。`ArenaView` が運ぶ | (c) | *理由のみ* 「a little floor past the wall, so what stands at the edge is not cut off」。3.0 そのものは**不明** |
| `NO_WINDOW`（窓が無いフレームの代役） | `lib.rs:75` | 16/9 | | `const` のまま | (a) 窓が無いフレームでは何も描かないので値は見えない。0 除算を避けるための比だけが意味 | **引用**（`docs/sabiruby-battle.md`「The window is 16:9 and the arena square」） |
| 床の色 | `lib.rs:102` | `srgb(0.08, 0.08, 0.10)` | | `ArenaPlugin { floor }`（前から） | (e) | **不明** |
| ~~エディタを開いたときずらす幅 0.16~~ | — | — | | **S3 で消えた**（`ViewInsets` が実測の px を書く）。コメントの「3 分の 1」との食い違いも一緒に消えた | — | — |

### 1.2 パン・ズームのカメラ（`games-shell/src/camera.rs`。**S3 が足した。S5a の一覧には入っていない**）

S3 の時点で既に `CameraControls`（Resource、`Settings` の `camera_*` 7 鍵、キー割り当ては
`CameraKeys`）に入っていて、既定値ごとに出どころが rustdoc に 1 行ある。S5b-1 は**この 8 件を
一覧に足しただけで、コードは触っていない**。

| 名前 | 位置 | 既定値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `ZOOM_PER_NOTCH` | `camera.rs:48` | 1.10 | ● | `CameraControls`、`Settings` の `camera_zoom_per_notch` | (e) | **導出**（箱庭で: 全域が約 30 ノッチ＝指 10 回。単体テストが 29〜32 を確かめる） |
| `PIXELS_PER_NOTCH` | `camera.rs:55` | 100.0 | ● | 同上 `camera_pixels_per_notch` | (e) | **引用**: Chromium の `wheel` は 1 ノッチ `deltaY = 100` |
| `ZOOM_IN_LIMIT` | `camera.rs:64` | 6/42 | ● | 同上 `camera_zoom_in_limit` | (e) | **不明**（箱庭の `ZOOM_MIN` ÷ 既定距離。箱庭の 2 つの数に記録が無い。rustdoc に「引き継いだ、正当化していない」と書いてある） |
| `ZOOM_OUT_LIMIT` | `camera.rs:68` | 110/42 | ● | 同上 `camera_zoom_out_limit` | (e) | **不明**（同上） |
| `DRAG_PER_PIXEL` | `camera.rs:80` | 1.0 | ● | 同上 `camera_drag_per_pixel` | (e) | **導出**: 「カーソルの下の点がカーソルの下に居続ける」から 1.0（箱庭の出どころ不明の 0.0016 は引き継がなかった） |
| `KEYS_PER_SECOND` | `camera.rs:88` | 0.9 | ● | 同上 `camera_keys_per_second` | (e) | **不明**（箱庭の `PAN_PER_SECOND` の書き写し。箱庭側に記録が無い） |
| `CLICK_SLOP` | `camera.rs:93` | 6.0 | ● | 同上 `camera_click_slop` | (e) | *理由のみ*「a few pixels」（箱庭 `window.rs` の同名の数と同じ） |
| `CameraControls::bounds` | `camera.rs:153` | `None` | ● | `CameraControls`（`Settings` には出さない） | (e) | **導出**: 世界の端はゲームしか知らないので既定を持たない |

### 1.3 エディタ（`crates/rubevy-egui/src/editor.rs`）

寸法は `EditorLayout`（Resource。`EditorPlugin::sized(..)`、`Settings` の `editor_*` 6 鍵）、
色は `EditorColors`（`Editor::colors` の中。`drawn_kind` が `Editor` だけで答えられるように、
Resource を分けずにここへ置いた — 判定の system が Bevy の引数 16 個の上限に当たっているため）。

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `MARGIN` | `editor.rs:508` | 8.0 | | `EditorLayout::margin`、`Settings` の `editor_margin` | (c) | 名前がある理由は書いてある（窓のチェックが矩形を計算できるように）。値は **不明** |
| `WIDTH` | `editor.rs:509` | 520.0 | | `EditorLayout::width`、`editor_width` | (c) | 同上・**不明** |
| `HEIGHT` | `editor.rs:510` | 640.0 | | `EditorLayout::height`、`editor_height` | (c) | 同上・**不明** |
| `FONT` | `editor.rs:494` | 13.0 | | `EditorLayout::font`、`editor_font` | (c) | **不明** |
| `KINDS`（9 色） | `editor.rs:648` | `(210,214,222)` ほか 8 色 | ● | `EditorColors::kinds`（`Editor::colors`）。`Settings` には出していない | (c) 表示。ただし出どころは強い | **測った**: WCAG コントラスト地 ≥4.96 / 帯 ≥3.09、CIE76 ΔE ≥25.9（`docs/worklog/2026-09-18-editor-highlight.md`）。**変えられるようにしたので、変えるとこの保証は破れる** — rustdoc にそう書いた（著者判断、2026-09-20） |
| 熱の帯の色 `HEAT_BAND` | `editor.rs:605` | `(150,110,20)` | ● | `EditorColors::heat_band` | (c) | **引用**: 合成後の (103,77,17) が 9 色を測るときの前提。ここを変えても上の保証が破れる |
| 熱の帯の最大 α `HEAT_ALPHA` | `editor.rs:606` | 170 | ● | `EditorColors::heat_alpha` | (c) | **不明**（170 そのもの。色の方はこの 170 を前提に測っている） |
| 熱を描き始める下限 `HEAT_FLOOR` | `editor.rs:610` | 0.1 | ● | `EditorColors::heat_floor` | (c) | *理由のみ* 「a line passed through once is not a place」 |
| 琥珀 `AMBER`（`* edited` と「未適用」） | `editor.rs:614` | `(240,190,90)` | ● | `EditorColors::amber` | (c) | *理由のみ* 「暖色は熱のもの」 |
| 行番号の桁の色 | `editor.rs`（`EditorColors::gutter`） | 分類 3 と同じ | ● | 変えられない（**分類 3 の色を引く**ようにした） | (a) コメントと同じ色であることが意味 | **導出**: S5b-1 で数の写しをやめ、`kinds[3]` を読む 1 行にした |
| ボタンの余白と字の大きさ | `editor.rs:515-521` | 10/5, 6.0, 16.0, 8/4 | ● | `EditorLayout::choice_padding` / `choice_spacing` / `choice_font` / `button_padding`（字の大きさと間隔は `Settings` の `editor_choice_font` / `editor_choice_spacing`） | (c) | *理由のみ* 「big enough to hit without aiming」。余白の 2 組は **不明** |

### 1.4 VM パネル（`crates/rubevy-egui/src/inspect.rs`）

`InspectStyle`（`VmInspector::style` の中。`fill` がメソッドで、窓の無いゲームが
`VmInspector` を手で作って呼ぶため Resource を分けられない）と `VmClock::smoothing`。
`Settings` の鍵は `vm_*` 7 つ + `vm_clock_smoothing`。

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `REGS_FRAMES` | `inspect.rs:38` | 6 | ● | `InspectStyle::regs_frames`、`vm_regs_frames` | (c) | *理由のみ* 「A brain waiting for the game stands about three frames deep in the DSL, so six reaches its own code as well」 |
| 命令数/フレームの平滑化 `INSN_SMOOTHING` | `inspect.rs:69` | 0.05（＝ 1 − 0.95） | ● | `InspectStyle::insn_smoothing` | (c) | **不明**（平滑化する理由だけ書いてある） |
| VM の ms の平滑化 `CLOCK_SMOOTHING` | `inspect.rs:715` | 0.2（＝ 1 − 0.8） | ● | `VmClock::smoothing`、`vm_clock_smoothing` | (c) | *理由のみ* 「60 Hz で約 1/6 秒ぶんの記憶」。0.2 そのものは **不明** |
| ログに出すフレーム数 `LOG_FRAMES` | `inspect.rs:64` | 4 | | `InspectStyle::log_frames` | (c) | **不明** |
| 名前を切る長さ `NAME_CHARS` | `inspect.rs:61` | 40（切ると 39 + `…`） | ● | `InspectStyle::name_chars` | (c) | **不明** |
| 値の表示を切る長さ `VALUE_CHARS` | `inspect.rs:57` | 52 | ● | `InspectStyle::value_chars`、`vm_value_chars` | (c) | **不明** |
| `FONT` | `inspect.rs:41` | 12.0 | | `InspectStyle::font`、`vm_font` | (c) | **不明** |
| `PANEL_HEIGHT` / `FRAMES_HEIGHT` / `REGS_HEIGHT` | `inspect.rs:45-47` | 440 / 148 / 150 | | `InspectStyle`、`vm_panel_height` / `vm_frames_height` / `vm_regs_height` | (c) | **不明** |
| パネルの幅 `WIDTH` / `WIDTH_MARGIN` | `inspect.rs:51-52` | 640.0（窓幅 − 16 が上限） | ● | `InspectStyle::width` / `width_margin`、`vm_width` | (c) | **不明** |
| パネルの 7 色（`AMBER` / `PALE` / `LIT` / `DIM` / `REG_NAME` / `REG_TEMP` / `REG_INDEX`） | `inspect.rs:85-109` | `(240,190,90)` ほか 6 色 | ● | `InspectStyle` の 7 フィールド。`Settings` には出していない | (c) 表示 | **不明**（7 色とも）。**S5b-2 で足した**: S5a の網に掛かっておらず、`draw_inspector` の 9 か所に直書きされていた（同じ値が 2 つ、2 回ずつ）。`AMBER` と `REG_NAME` は `editor.rs` の `AMBER` / `KINDS[0]` と**同じ値だが繋いでいない**（意図の記録が無いため。§7-11） |

### 1.5 説明（`games-shell/src/guide.rs`）と HUD（`games-shell/src/hud.rs`）

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| 説明の窓の大きさ `SIZE` / `MAX_HEIGHT` | `guide.rs:202-203` | `[640, 820]`、最大 860 | ● | `GuideStyle`、`Settings` の `guide_width` / `guide_height` / `guide_max_height` | (c) | **引用**: G6 の絵で下が切れていたので広げた（`docs/plans/garden-plan.md`）。測った記録ではない。860 は **不明** |
| キーの色 `KEYCOL` | `guide.rs:341` | `(255,226,150)` | ● | `GuideStyle::key_color`（`Settings` には出さない） | (c) | **不明** |
| 説明の余白 3 つ（`NOTE_SPACING` / `KEY_SPACING`） | `guide.rs:207-211` | 6.0、`[14.0, 3.0]` | ● | `GuideStyle::note_spacing`（`Settings` の `guide_note_spacing`）／`key_spacing`（組なので鍵は無い） | (c) | **不明**。**S5b-2 で足した**（S5a の網に掛かっていなかった。`draw_guide` に直書き） |
| 日本語フォント（部分集合） | `guide.rs:56` | 327 文字・62,780 バイト | | ビルド時（`tools/subset-font.sh`） | (a) 本文と一緒に切らないと字が欠ける | **測った**（`docs/plans/garden-plan.md`。9,589,900 → 62,780 バイト） |
| HUD の予算バーの目盛 `BAR_TICKS` | `hud.rs:33` | 16 | ● | `HudStyle::bar_ticks`、`hud_bar_ticks` | (c) | **不明** |
| HUD の字の大きさ `LINE_FONT` / `PANEL_FONT` | `hud.rs:37-38` | 15.0 / 13.0 | | `HudStyle`、`hud_line_font` / `hud_panel_font` | (c) | **不明** |
| HUD の余白 `MARGIN` / `PADDING` / `ROW_GAP` | `hud.rs:42-44` | 8 / 8 / 3 | | `HudStyle`、`hud_margin`（余白と行間は app が渡す） | (c) | **不明** |

### 1.6 コードパネル（`crates/rubevy-egui/src/code.rs`、**今どのゲームも使っていない**）

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `ROWS` | `code.rs:59` | 28 | ● | `CodeStyle::rows`、`Settings` の `code_rows` | (c) | **不明** |
| `CHARS`（切る文字数） | `code.rs:68` | 44 | ● | `CodeStyle::chars`、`code_chars` | (c) | **不明（コメントと値が最初から食い違う）**: 「430px of a monospace 12px font: about 58 characters」に対して 44。同じコミット `23c5ad3` で両方入っていて、どちらが意図か読んでも分からない。**どちらにも寄せていない**（既定は 44 のまま、食い違いは rustdoc に書いた）。§7-1 |
| パネルの位置と幅 `TOP` / `RIGHT` / `WIDTH` / `PADDING` | `code.rs:72-75` | 上 120 / 右 8 / 幅 430 / 余白 8 | | `CodeStyle`、`code_width` | (c) | *理由のみ* 「below the HUD rows, so the two do not overlap」（上だけ）。ほかは **不明** |
| 字の大きさ `TITLE_FONT` / `LINE_FONT` | `code.rs:79-80` | 13.0 / 12.0 | | `CodeStyle`、`code_font`（本文） | (c) | **不明**。12.0 は上の食い違うコメントが計算に使っている数 |

### 1.7 S5b-1 が足した設定の一覧

| 設定 | どこ | `Settings` の鍵 | 既定値 |
|---|---|---|---|
| `EditorLayout` | `rubevy-egui`（Resource。`EditorPlugin::sized`） | `editor_margin` / `editor_width` / `editor_height` / `editor_font` / `editor_choice_font` / `editor_choice_spacing` | 8 / 520 / 640 / 13 / 16 / 6 |
| `EditorColors` | `rubevy-egui`（`Editor::colors`） | — （色は出さない） | `KINDS` 9 色 / `HEAT_BAND` / `HEAT_ALPHA` 170 / `HEAT_FLOOR` 0.1 / `AMBER` |
| `InspectStyle` | `rubevy-egui`（`VmInspector::style`） | `vm_font` / `vm_panel_height` / `vm_frames_height` / `vm_regs_height` / `vm_width` / `vm_regs_frames` / `vm_value_chars` | 12 / 440 / 148 / 150 / 640 / 6 / 52 |
| `InspectStyle` の 7 色（**S5b-2**） | `rubevy-egui`（同上） | — （色は出さない） | `AMBER` / `PALE` / `LIT` / `DIM` / `REG_NAME` / `REG_TEMP` / `REG_INDEX` |
| `VmClock::smoothing` | `rubevy-egui` | `vm_clock_smoothing` | 0.2 |
| `CodeStyle` | `rubevy-egui`（Resource。`CodePanelPlugin::sized`） | `code_rows` / `code_chars` / `code_width` / `code_font` | 28 / 44 / 430 / 12 |
| `HudStyle` | `games-shell`（Resource。`HudPlugin::styled`） | `hud_line_font` / `hud_panel_font` / `hud_margin` / `hud_bar_ticks` | 15 / 13 / 8 / 16 |
| `GuideStyle` | `games-shell`（Resource。`GuidePlugin::styled`） | `guide_width` / `guide_height` / `guide_max_height` / `guide_note_spacing`（**S5b-2**） | 640 / 820 / 860 / 6（`key_spacing` `[14, 3]` は鍵なし） |
| `ArenaPlugin::floor_margin` | `games-shell`（プラグインの値。`ArenaView` が運ぶ） | — | 3.0 |
| `ArenaPlugin::showing(half)` | `games-shell`（既定を**持たない**。ゲームが渡す） | — | — |
| `PanelSettingsPlugin` | `games-shell`（`PreStartup` に上の鍵を読む） | — | — |

### 1.8 判定の物差し（`games-shell/src/checks.rs`。**S10 が 2 本の写しを 1 つにした**）

| 数 | 位置 | 既定値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `INSTRUCTIONS_A_FRAME_BUYS`（`CheckPace::instructions_a_frame`） | `checks.rs` の `CheckPace` | 45,600 | | **`Settings` の `checks_instructions_a_frame`**（2 本で同じ鍵。読むのは各ゲームの起動時の `CheckPace::of`） | (d) | **測った**（S6、2026-09-20: 箱庭の `frame_time` を 300 µs に絞った走行の最悪フレーム 1,708 命令 = 5.7 命令/µs × 8,000 µs）。**実測は 2 つある**: S5b-3 が上限の庭を本来の 8 ms で測ると 6.85 命令/µs = 54,800。差は機械ではなく条件（300 µs のフレームは tick の出入りの費用を 27 分の 1 の仕事で払う＝「短いフレームの速さ」）。**遅い方を採る**——この数は予算を割って「何フレーム待ってよいか」を出すので、**大きすぎると待ちが短くなり偽の FAIL が出る**（S6・S7 が消しに来たもの）。小さすぎても判定が遅れるだけ |

**なぜ共有 crate にあるのか**: この数は「ゲームの数」ではなく**この機械のこの VM の事実**で、
S9 の時点で `garden/src/window.rs` と `sabibots/src/main.rs` に同じ名前・同じ値・同じ出どころの
段落つきで 2 回書かれていた（2 つのバイナリにまたがるので互いを cite することすらできない）。
**設定にしたのは、測ったのが「この機械」だから** — 別の機械で走らせる人は
`garden.settings.txt` / `sabibots.settings.txt` に 1 行書けば判定の忍耐がその機械のものになる。

**和の形も 1 つ**: `CheckPace::frames_to_wait(structural, budget)` =
`structural + ceil(budget ÷ instructions_a_frame)`。**`structural` は引数で、数ではない** —
箱庭の 2（`window::STRUCTURAL_FRAMES`: `Script` が着くフレーム + タスクが作られて走るフレーム）と
Battle の 2（`HANDLER_STRUCTURAL_FRAMES`: publish は tick の後 → 次のフレームの tick で順番 →
判定は鎖と順序を持たないのでさらに次）は**別の理由から出た別の 2** で、混ぜると
S5b-5 の「別々の理由の待ちを 1 つの数で賄うと、片方が動いたときもう片方が黙って壊れる」を
新しい場所で繰り返すことになる。

---

## 2. Garden（Rust）

> **S5b-3（2026-09-21）で、この節の (c)「動かす側の数」は全部「利用者が変えられる場所」に移した。**
> 行番号は `shared-crate` ブランチのその時点のもので取り直してある（S1〜S7 で大きく動いていた）。
> `const` は**既定値の名前としてだけ**残り、どれも rustdoc の 1 行で出どころ（測った／導出／引用／
> *理由のみ*／**不明**）を言う。**既定値は 1 つも動かしていない**（6 通りの走行の行の集合が前後で
> 同じ。`docs/verification/selftest-lines.md`）。
>
> 置き場所は **7 つの Resource**で、`main` が**最初のフレームの前に** `garden.settings.txt` から
> 読む（窓の大きさもカメラも畑の広さも、App を建てる前に要るため。Battle の `Look` が S5b-2 で
> 同じ場所へ動いたのと同じ理由）。**店は今、ヘッドレスの走行でも読む** — 畑と家具と予算は
> ヘッドレスの世界のものでもあるため。パネルの数（`editor_*` / `vm_*` / `hud_*` / `guide_*`）は
> 今までどおり `games_shell::PanelSettingsPlugin` が `PreStartup` で読む。
>
> | Resource | 何の数か | 鍵の接頭辞 |
> |---|---|---|
> | `Place` | 畑の広さ、壁、押し分けの回数 | `field_*` / `wall_margin` / `separate_passes` |
> | `Furniture` | 新しい庭を建てるときの家具と間隔 | `start_*` / `plant_min` / `plant_grown` |
> | `Light` | 夜と昼の光、影、夜明けの位相 | `light_*` |
> | `Scenery` | 地平線・空のドーム・霧・縁の木 | `horizon_*` / `sky_*` / `fog_*` / `edge_*` |
> | `Picture` | 窓、地面の色、空腹バー、模型の倍率、アニメ | `window_*` / `look_*` |
> | `Eye` | 3D オービットのカメラとクリック | `eye_*` |
> | `Budgets` | 2 本の VM の予算と `frame_time`、待ちと熱 | `script_*` / `world_script_*` ほか |
>
> **(b)（遊びの数 → Ruby 側）と (d)（判定の閾値）の行は触っていない** — S5b-4 と S5b-5 の仕事で、
> 行番号だけ取り直した。
>
> **S5b-4（2026-09-21）が (b) を移した。** 行番号はその時点のもので取り直してある。移った 6 つ
> （`TOUCH_REACH`、新しい芽の最小間隔、体の半径 4 つ）は `ruby/world.rb` に書かれ、
> `garden.rules` で 1 回渡る。`const` は**代役**として残る——`world.rb` がコンパイルできない庭も
> 走る、というのが W1 の設計なので、規則が喋る前の答えが要る（`REACH` / `MATE_REACH` /
> `CHILD_HUNGER` / `POP_MAX` と同じ形）。単体テスト `the_stand_ins_are_what_world_rb_says` が
> `ruby/world.rb` を読んで 10 個の代役と突き合わせるので、**2 つの数が黙って食い違うことはない**。
> **既定値は 1 つも動かしていない**（6 通りの走行が前後で同じ行を出す）。
> 移さなかったのは §2.12 の種の遺伝子で、理由と 3 つの案は
> `docs/worklog/2026-09-21-numbers-garden-play.md` §6。**S5b-5 で決着した**: 著者の判断で案 A
> ——`world.rb` ではなく `Furniture` の設定へ（分類 (b) → (c)、鍵 7 つ、既定値は据え置き）。

### 2.1 場（`Place`）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `FIELD_W` | `main.rs:70` | 40.0 | | `Place::width`、`Settings` の `field_width` | (c) | **引用**: 計画書 `docs/plans/garden-plan.md:51`「40×30 マスの草地」。なぜその 2 つかは**不明** |
| `FIELD_D` | `main.rs:71` | 30.0 | | `Place::depth`、`field_depth` | (c) | **引用**: 同上 |
| ~~`HALF_W` / `HALF_D`~~ | — | — | ● | **`const` が無くなった**（S5b-3）。`Place::half_w()` / `half_d()` が畑から導く | (a)→導出 | **導出**: `width / 2.0` |
| 壁の内側の余白 `WALL_MARGIN` | `main.rs:75` | 0.5 | ● | `Place::wall_margin`、`wall_margin` | (c) | **不明**（`move_creatures` と `inside_the_walls` が同じ 0.5 で clamp する。G0 から） |

### 2.2 昼夜と光（`Light`）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `DAY_LENGTH` | `main.rs:147` | 60.0 秒 | ● | `world.rb:21` の `day_length` が上書きする。この数は「規則が黙っているときの既定」 | (e)＋(c) の既定値 | **引用**: `main.rs:140-146`「It is the default now, not the rule (W1)」 |
| `DAWN_OFFSET` | `main.rs:151` | 0.08 | ● | `Light::dawn_offset`、`light_dawn_offset` | (c) | *理由のみ*「the first thing a run sees is daylight」。0.08 自体は**不明** |
| ~~`MIDNIGHT`~~ | `main.rs:160` | — | | **`const` が無くなった**（S5b-3）。`midnight(&light)` が `dawn_offset` から導く — 夜明けを動かすと `--at midnight` の指す時刻も動く | (a)→導出 | **導出**（式は `main.rs:152-159`） |
| `MOON_LUX` | `main.rs:199` | 950.0 | ● | `Light::moon_lux`、`light_moon_lux`。`NightDial`（`night`）が掛かる | (c) の既定値 | **測った**（2026-09-18、G6b）: 真夜中の地面の平均輝度 44/255（目標 40〜50、G6 は 26、午後は 81）。`--shot --at midnight` の、パネルが覆わない帯で測った。**3 つ一組の測定**で、1 つ動かすと 44 は無くなる — rustdoc にそう書いた |
| `NIGHT_AMBIENT` | `main.rs:200` | 190.0 | ● | `Light::night_ambient`、`light_night_ambient` | (c) の既定値 | **測った**（同じ 1 回の測定。「110 to 190 moved the mean by 1.9」） |
| `NIGHT_SKY` | `main.rs:203` | `[0.14, 0.18, 0.36]` | ● | `Light::night_sky`、`light_night_sky_r` / `_g` / `_b`（色だが 3 つの数なので構文解析が要らない） | (c) の既定値 | **測った**（同じ 1 回の測定） |
| `NIGHT_ZENITH` | `main.rs:372` | 0.55 | ● | `Light::night_zenith`、`light_night_zenith` | (c) | *理由のみ*（縁は測った `NIGHT_SKY`、勾配はその上）。0.55 は**不明** |
| `NightDial` の既定 | `main.rs:363` | 1.0 | ● | `Settings` の `night`（スライダ） | **(e)** | **引用**: 著者に数を尋ねるための仕掛け |
| `NIGHT_DIAL_MIN` / `MAX` | `main.rs:338-339` | 0.5 / 2.0 | | `Light::dial_min` / `dial_max`、`light_dial_min` / `light_dial_max` | (c) | *理由のみ*「Half the night is still a night; twice it is the author saying the screen is darker than ours」 |
| 昼の照度 `DAY_LUX` | `main.rs:209` | `1200 + 9000 × noon` | ● | `Light::day_lux`、`light_day_lux` / `light_day_lux_span` | (c) | *理由のみ*（1,200 は「the day's floor、地平線の太陽」）。9,000 は**不明** |
| 昼の環境光 `DAY_AMBIENT` | `main.rs:210` | `120 + 260 × height` | ● | `Light::day_ambient`、`light_day_ambient` / `light_day_ambient_span` | (c) | **不明** |
| 太陽・月の距離 | `main.rs:4226` | 60.0 | ● | 直書き | (a) 平行光なので距離は影のカスケードに効くだけ | **不明** |
| 太陽の照度 `SUN_LUX` | `main.rs:215` | 8,000.0 | | `Light::sun_lux`、`light_sun_lux`（起動時。毎フレーム `day_night` が上書きする） | (c) | **不明** |
| 影のカスケード `SHADOW_*` | `main.rs:216-218` | 2 / 24.0 / 70.0 | | `Light::shadow_cascades` / `shadow_near` / `shadow_far`、`light_shadow_*` | (c) | **不明** |
| 太陽の高さから昼夜を決める境 | `main.rs:4210` | `height <= 0.0` | ● | 直書き | (a) 「地平線の下＝夜」の定義そのもの | **導出** |
| **昼の空の色** `DAY_SKY_RIM` / `_SPAN` / `DAY_SKY_ZENITH` / `_SPAN` | `main.rs` の `sky_colors` | 地平 `0.35,0.5,0.7` ＋ `0.15,0.22,0.22×noon`、天頂 `0.15,0.33,0.64` ＋ `0.11,0.21,0.24×noon` | ● | `Light`、`light_day_sky_rim_r` / `_g` / `_b`、`light_day_sky_rim_span_*`、`light_day_sky_zenith_*`、`light_day_sky_zenith_span_*` | (c) | **不明**（**S9 で移した**。夜の同じ形の 3 つは G6b が測ってあるのに昼は 1 つも一覧に無かった。地平の noon での値 `0.5+0.22` は G8 以前の `ClearColor` と同じ数で、**G6b の夜はこの空に対して測られている** — `MOON_LUX` の注意書きがここにも掛かる） |
| **昼の太陽の色** `SUN_COLOR` / `_SPAN` | `main.rs` の `day_night` | `1.0, 0.72, 0.45` ＋ `0.0, 0.24, 0.5×noon` | ● | `Light`、`light_sun_color_*` / `light_sun_color_span_*` | (c) | *理由のみ*「low sun is orange, high sun is white」。数は**不明**（**S9 で移した**） |
| **夜の月の色** `MOON_COLOR` | 同上 | `0.62, 0.70, 1.0` | ● | `Light`、`light_moon_color_*` | (c) | *理由のみ*「青くて太陽よりずっと弱い」。数は**不明**（**S9 で移した**） |
| **環境光の色（昼・夜）** `DAY_AMBIENT_COLOR` / `NIGHT_AMBIENT_COLOR` | 同上 | `0.7, 0.8, 1.0` / `0.45, 0.54, 0.85` | ● | `Light`、`light_day_ambient_color_*` / `light_night_ambient_color_*` | (c) | **不明**（**S9 で移した**。夜の側は G6b が測った絵の一部なので `MOON_LUX` の注意書きが掛かる） |

### 2.3 地平線と空（G8。`Scenery`）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `HORIZON_HALF` | `main.rs:355` | 300.0 | | `Scenery::horizon_half`、`horizon_half` | (c) | *理由のみ*（カメラに追従するので「常にこの距離」）。300 自体は**不明** |
| `HORIZON_DROP` | `main.rs:357` | −0.02 | | `const` のまま | (a) 2 枚の平面が画素を取り合わないための隙間。0 にすると z ファイティング | *理由のみ* |
| `SKY_RADIUS` | `main.rs:364` | 500.0 | | `Scenery::sky_radius`、`sky_radius` | (c) | **不明** |
| `SKY_SIDES` / `SKY_RINGS` | `main.rs:365-366` | 12 / 4 | | `Scenery::sky_sides` / `sky_rings`、`sky_sides` / `sky_rings`（最低 3 / 2 に丸める） | (c) | **導出の記述あり**: 84 三角形・49 頂点。12×4 を選んだ理由は**不明** |
| `SKY_FLOOR` | `main.rs:369` | −0.30 | | `const` のまま | (a) 地面が描く地平線より下でなければ空の縁が見える | *理由のみ* |
| `FOG_NEAR` | `main.rs:388` | 14.0 | ● | `Scenery::fog_near`、`fog_near` | (c) | **導出**: 40×30 の遠い隅は中心より約 14 単位遠い |
| `FOG_DEPTH` | `main.rs:389` | 45.0 | ● | `Scenery::fog_depth`、`fog_depth` | (c) | **測った**: 既定のカメラは 42 単位・49° 見下ろしで、画面の最遠は 72 単位、畑の隅は 57 — 奥行きは 15 単位しかない。**`eye_pitch` / `eye_distance` を動かすとこの測定は別の絵についてのものになる** |
| `FOG_NEAR_MAX` | `main.rs:390` | 120.0 | ● | `const` のまま | (a) `horizon_half` の内側に霧の終わりを収めるための上限 | *理由のみ*。120 は**不明** |
| 霧の色と最初の 2 距離 `FOG_COLOR` / `FOG_AT_FIRST` | `main.rs:406-407` | `(0.35,0.5,0.7)`、60→220 | | `Scenery::fog_color` / `fog_at_first`、`fog_color_r` / `_g` / `_b` / `fog_start` / `fog_end`（起動時。毎フレーム `horizon_look` が上書きする） | (c) | **不明** |
| `EDGE_TREES` | `main.rs:396` | 16 | | `Scenery::edge_trees`、`edge_trees` | (c) | *理由のみ*「the edge of the field reads as the edge of the scenery rather than as a fence」。16 は**不明** |
| 縁の木の散らし方 `EDGE_JITTER` / `EDGE_OUT` / `EDGE_SCALE` | `main.rs:399-401` | ±0.4 / 4〜15 / 1.7〜3.1 | | `Scenery`、`edge_jitter` / `edge_out_min` / `_max` / `edge_scale_min` / `_max` | (c) | **不明** |
| 空の勾配の押し上げ `SKY_RIM_GAIN` / `SKY_RIM_HOLD` | `main.rs` の `horizon_look` | `t × 1.25 − 0.1` | ● | `Scenery::rim_gain` / `rim_hold`、`sky_rim_gain` / `sky_rim_hold` | (c) | *理由のみ*「地平の帯は霧と同じ色に見えてほしいので、登り始めを少し上にずらす」。1.25 と 0.1 は**不明**（**S9 で移した**） |

### 2.4 見た目（`Picture`）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `RABBIT_TINT` / `BEETLE_TINT` | `main.rs:498-499` | `(0.93,0.90,0.78)` / `(0.14,0.48,0.56)` | | `pub const` のまま | (a) モデルと HUD が同じ数を使うことが意味（`window::species_color`） | **引用**（G8、著者「ウサギと甲虫が見分けられない」） |
| `WORLD_COLOR` | `window.rs:232` | `(210,214,222)` | ● | `const` のまま | (a) エディタの本文色と同じであることが意味 | **引用** |
| 窓の大きさ `WINDOW` | `main.rs:519` | 1600×900 | | `Picture::window`、`window_width` / `window_height`（**Battle と同じ 2 つの鍵**） | (c) | **引用**（間接）: 「a 1600-wide window」がカメラの既定距離 42 の根拠。900 は**不明** |
| 地面の色 `GROUND_COLOR` | `main.rs:522` | `(0.36,0.46,0.25)` | | `Picture::ground_color`、`look_ground_r` / `_g` / `_b` | (c) | **不明** |
| 空の板の色 `SKY_COLOR` | `main.rs` の `make_look` | `(0.42,0.62,0.86)` | | `Picture::sky_color`、`look_sky_r` / `_g` / `_b` | (c) | **不明**（**S9 で移した**。`horizon_look` が頂点に色を書くまでの 1 フレームだけ見える） |
| 空腹バーの色の境 `HUNGER_LOW` / `HUNGER_WARN` | `main.rs:529-530` | 20.0 / 55.0 | ● | `Picture::hunger_low` / `hunger_warn`、`look_hunger_low` / `look_hunger_warn` | (c)。**55.0 は `beetle.rb:14` の `hungry_below` と同じ値だが繋がっていない**（§7-6。著者の判断で繋がない: 記録に意図が無い以上、同じ値であることは偶然として扱う） | **不明**（両方とも） |
| 空腹バーの大きさ `HUNGER_BAR` | `main.rs:531` | 64×11 | ● | `Picture::hunger_bar`、`look_hunger_bar_width` / `_height` | (c) | **不明** |
| 草のモデルの倍率 `TUFT_SCALE` / `BUSH_SCALE` | `main.rs:536-537` | 2.2 / 2.6 | | `Picture`（`Look` が持ち運ぶ）、`look_tuft_scale` / `look_bush_scale` | (c) | **不明** |
| 木・岩のモデルの倍率 `TREE_SCALE` / `ROCK_SCALE` | `main.rs:538-539` | 2.2 / 3.4×3.0 | | 同上、`look_tree_scale` / `look_rock_scale` / `look_rock_squash` | (c) | **不明** |
| 生き物のモデルの倍率 `BEETLE_SCALE` / `RABBIT_SCALE` | `main.rs:540-541` | 0.55 / 0.75 | | 同上、`look_beetle_scale` / `look_rabbit_scale` | (c) | **不明** |
| `BEETLE_MODEL` | `main.rs:514` | `animal-crab.glb` | | `Picture::beetle_model`、`look_beetle_model`（数ではないが「著者が替えるもの」） | (c) | **引用**: `main.rs:508-513` |
| 歩きに変わる速さ `WALKING_AT` | `main.rs:545` | 0.2 | ● | `Picture::walking_at`（`Look` 経由）、`look_walking_at` | (c) | **不明** |
| アニメの遷移時間 `GAIT_BLEND_MS` | `main.rs:546` | 180 ms | ● | `Picture::gait_blend_ms`、`look_gait_blend_ms` | (c) | **不明** |

### 2.5 世界の家具（生成時に 1 度だけ。`Furniture`）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `PLANTS_AT_START` | `main.rs:649` | 55 | | `Furniture::plants`、`start_plants` | (c) | **不明** |
| `PLANT_MIN` | `main.rs:650` | 0.18 | | `Furniture::plant_min`、`plant_min`（芽の大きさ） | (c) | **不明** |
| `PLANT_GROWN`（元 `PLANT_MAX`） | `main.rs:663` | 1.4 | | `Furniture::plant_grown`、`plant_grown`（**S5b-4 で鍵の名前が `plant_max` から変わった**） | (c) の既定値 | **不明**。`world.rb:31` の `plant_max` と**同じ値だが別の数**——こちらは「ゲームが建てる株の大きさ」、あちらは「成長が止まる上限」。S5b-4 が名前を分けた（§7-15 の決着） |
| `BEETLES` / `RABBITS` | `main.rs:666-667` | 6 / 4 | | `Furniture`、`start_beetles` / `start_rabbits` | (c) | **不明** |
| `TREES` / `ROCKS` | `main.rs:873-874` | 7 / 9 | | `Furniture`、`start_trees` / `start_rocks` | (c) | **引用**（数だけ）: `garden-plan.md:89`「木 7・岩 9」。選んだ理由は**不明** |
| 生成時の空腹 `START_HUNGER` | `main.rs:670` | 45〜90 | | `Furniture::hunger`、`start_hunger_min` / `_max` | (c) | **不明** |
| 配置をやり直す回数 `START_TRIES` | `main.rs:673` | 40 | | `Furniture::tries`、`start_tries`（最低 1 に丸める） | (c) | **不明** |
| 草が丸い株になる確率 `ROUND_CHANCE` | `main.rs:676` | 0.35 | | `Furniture::round_chance`、`start_round_chance`。**3 か所に散っていた同じ数が 1 つになった**（`spawn_world`・`sprout_plants`、`load_world` は自前の roll） | (c) | **不明** |
| 岩の大きさ `ROCK_SQUASH` | `main.rs:677` | 0.8〜1.25 | | `Furniture::rock_squash`、`start_rock_squash_min` / `_max` | (c) | **不明** |
| 初期配置の間隔 5 つ `SOLID_APART` ほか | `main.rs:682-686` | 1.5 / 1.2 / 2.5 / 0.6 / 2.0 | | `Furniture`、`start_solid_apart` / `start_grass_off_solid` / `start_creature_off_grass` / `start_creature_off_solid` / `start_creatures_apart` | (c) | **引用**（1.5 のみ）: `garden-plan.md:93`。残りは**不明** |
| 最初の草の大きさの下限 `PLANT_START_MIN` | `main.rs` の `spawn_world` | 0.3 | | `Furniture::plant_start_min`、`start_plant_min` | (c) | **不明**（**S9 で移した**。`plant_grown` の相方で、芽の大きさ `plant_min` 0.18 とは別の数） |
| 新しい芽の最小間隔 `SPROUT_GAP` | `main.rs:1563` | 1.5 | ● | **`world.rb:103` の `sprout_gap`**（`garden.rules`）。`const` は代役 | **(b) → 移した**（S5b-4） | **不明** |

### 2.6 規則の既定値（`world.rb` が黙っているときに使われる）

**この節は S5b-3 が触っていない**（(b) と (e) で、S5b-4 の仕事）。

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `HUNGER_MAX` | `main.rs:787` | 100.0 | ● | `const`。規則の方は `world.rb:63` | (a) HUD のバーの「満」 | **引用** |
| `REACH` | `main.rs:795` | 1.1 | ● | `world.rb:69` の `reach` が `garden.rules` で上書きする | (e)＋既定値 | **引用** |
| `TOUCH_REACH` | `main.rs:807` | 1.3 | ● | **`world.rb:99` の `touch_reach`**（`garden.rules`）が上書きする。`const` は代役 | **(b) → 移した**（S5b-4）。`startle` の走査は Rust のまま | **不明** |
| `MATE_REACH` | `main.rs:814` | 2.0 | ● | `world.rb:74` が上書き | (e)＋既定値 | **測った** |
| `CHILD_HUNGER` | `main.rs:853` | 50.0 | | `world.rb:84` が上書き | (e)＋既定値 | **引用** |
| `POP_MAX` | `main.rs:859` | 24 | ● | `world.rb:86` が上書き | (e)＋既定値 | **引用**。予算の実測もこの上限に立っている |

### 2.7 当たり

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `BEETLE_RADIUS` | `main.rs:869` | 0.40 | ● | **`world.rb:113` の `beetle_radius`**（`garden.rules`）。`const` は代役 | **(b) → 移した**（S5b-4） | **不明** |
| `RABBIT_RADIUS` | `main.rs:870` | 0.50 | ● | **`world.rb:114`** | **(b) → 移した** | **不明** |
| `TREE_RADIUS` | `main.rs:871` | 0.70 | ● | **`world.rb:115`** | **(b) → 移した** | **不明** |
| `ROCK_RADIUS` | `main.rs:872` | 0.60 | ● | **`world.rb:116`** | **(b) → 移した** | **不明** |
| `CELL` | `main.rs:883` | 1.6 | ● | `const`。ただし**下限**で、実際に使うのは `Bodies::cell()` = `max(1.6, 2 × 最大半径)`（`main.rs:1741`） | (a) 最大半径の 2 倍以上でなければ近傍を取りこぼす。半径が規則のものになったので**導出に変えた**（S5b-4） | **導出**（出荷時は `2 × 0.70 = 1.4 ≤ 1.6` なので 1.6 のまま） |
| `SEPARATE_PASSES` | `main.rs:886` | 4 | ● | `Place::separate_passes`、`separate_passes` | (c) | **測った**: 3 回の 90 秒走行で最接近 0.974 / 0.998 / 1.000、0.9 を下回ったフレーム 0（`garden-plan.md:92`）。**変えるとその測定は無くなる** |
| 押し分けの取り分 | `main.rs:4467` | 0.5 / 0.5 | ● | 直書き | (a) 生き物どうしは半分ずつ、木と岩は不動 — 規則の定義 | **引用** |

### 2.8 カメラ（3D オービット。`Eye`）

`games_shell::CameraControls`（共有 crate の 2D カメラ、`camera_*`）とは**別物**で、箱庭は
`CameraPlugin` を足していない。鍵を `eye_*` にしたのは、`garden.settings.txt` を読む人が
どちらのカメラの話かで迷わないようにするため。

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `PITCH`（`Orbit::default` の pitch） | `main.rs:1945` | 0.85 rad（≈49°） | | `Eye::pitch`、`eye_pitch` | (c) | **引用**（値ではなく効果）: 「looks down at 49°」— **`FOG_DEPTH` の測定はこの角度を前提にしている** |
| `DISTANCE`（同 distance） | `main.rs:1946` | 42.0 | | `Eye::distance`、`eye_distance`。`--eye` が上書きし、`Home` が戻す先でもある | (c) | **引用**: 「At the default 42 a beetle is thirty pixels across in a 1600-wide window」 |
| `ZOOM_PER_NOTCH` | `main.rs:1968` | 1.10 | ● | `Eye::zoom_per_notch`、`eye_zoom_per_notch` | (c) | **測った**（導出も添えてある）: 30 ノッチで全域（単体テストが 29〜32 を確かめる） |
| `PIXELS_PER_NOTCH` | `main.rs:1970` | 100.0 | ● | `const` のまま | (a) ブラウザの 1 ノッチ = 100 px という外の事実。設定にすると「ブラウザと食い違うブラウザ」が作れてしまう | **引用**（Chromium は 100） |
| `ZOOM_MIN` / `ZOOM_MAX` | `main.rs:1973-1974` | 6.0 / 110.0 | ● | `Eye::zoom_min` / `zoom_max`、`eye_zoom_min` / `eye_zoom_max` | (c) | *理由のみ*: 「Six units is a creature filling a third of the window; a hundred and ten has the whole field and its walls in view」 |
| `PAN_PER_PIXEL` | `main.rs:1977` | 0.0016 | ● | `Eye::pan_per_pixel`、`eye_pan_per_pixel` | (c) | *理由のみ*（距離に比例させる理由は書いてある。0.0016 は**不明**） |
| `PAN_PER_SECOND` | `main.rs:1979` | 0.9 | ● | `Eye::pan_per_second`、`eye_pan_per_second` | (c) | **不明** |
| `PAN_LIMIT` | `main.rs:1981` | 8.0 | ● | `Eye::pan_limit`、`eye_pan_limit` | (c) | *理由のみ*「How far past the wall the eye may wander」 |
| オービットの回転感度 `TURN_PER_PIXEL` | `main.rs:1949` | 0.005 | ● | `Eye::turn_per_pixel`、`eye_turn_per_pixel` | (c) | **不明** |
| pitch の範囲 `PITCH_MIN` / `PITCH_MAX` | `main.rs:1950-1951` | 0.12〜1.45 | ● | `Eye::pitch_min` / `pitch_max`、`eye_pitch_min` / `eye_pitch_max` | (c) | **不明** |
| `CLICK_REACH` | `window.rs:87` | 1.6 | ● | `Eye::click_reach`、`eye_click_reach` | (c) | *理由のみ*「within about a body's width」。体の半径は 0.40〜0.50 なので文と数が合わない（§7-5）。**どちらにも寄せていない** |
| `CLICK_SLOP` | `window.rs:90` | 6.0 | ● | `Eye::click_slop`、`eye_click_slop` | (c) | *理由のみ*「a few pixels」。6.0 は**不明** |

### 2.9 実行と予算（`Budgets`）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| 世界 VM の予算 `WORLD_BUDGET` | `main.rs:2349` | 45,000 命令 | ● | `Budgets::world`、`world_script_budget`（`install_world_answers` が書く） | (c) | **測った**（この repo でいちばん出どころのはっきりした数）: 上限（90 株・24 匹）の 392 フレームで最悪 25,837 命令・tick 4.86 ms、45,000 はその 1.74 倍。`docs/worklog/2026-09-17-garden-world.md`。**変えると 1.74 倍という余裕が別の話になる** |
| 世界 VM の `frame_time` `WORLD_FRAME_TIME_MS` | `main.rs:2352` | 8 ms | ● | `Budgets::world_frame_time_ms`、`world_script_frame_time_ms`（0 以下で「壁時計の番人なし」） | (c) | **引用**: 命令数の方が先に効くので rubevy の既定と同じ数を据え置いた。**S5b-3 でゲームが自分の名前で持つようになった**（前は rubevy の既定のまま） |
| 生き物 VM の予算 `CREATURE_BUDGET` | `main.rs:2367` | **41,000 命令**（2026-09-21 に 200,000 から動かした。著者判断・案 A） | ● | `Budgets::creature`、`script_budget`（**Battle と同じ鍵**） | (c) | **測った**: `41,000 = 1.74 × 23,686` を 1,000 の位に丸めた（式どおりなら 41,214）。23,686 は上限の庭（`start_plants=130`・`start_beetles=14`・`start_rabbits=10` = `pop_max` 24）を `--headless 60` で 3 回・10,749 フレーム測った最大で、**最初のフレーム**（24 体が一斉に走り出す。3 走行とも 1 命令まで同じ）。定常の最大は 4,955。1.74 は世界の VM の 45,000 ÷ その実測最大 25,837 という余裕の比の借用。丸め方は世界の 45,000 と同じ（1.74 × 25,837 = 44,956 → 45,000）。丸めた結果、比は 1.73 になる。測定は `docs/worklog/2026-09-21-numbers-garden-settings.md` §4、決着は `2026-09-21-checks-and-leftovers.md` §1。**前の 200,000 は rubevy の既定を引き継いだもので、rubevy 側の出どころは不明** |
| 生き物 VM の `frame_time` `CREATURE_FRAME_TIME_MS` | `main.rs:2368` | 8 ms | ● | `Budgets::creature_frame_time_ms`、`script_frame_time_ms` | (c) | 同上（rubevy の既定を引き継いだ） |
| `RESTORE_PATIENCE` | `main.rs:1905` | 5.0 秒 | ● | `Budgets::restore_patience`、`restore_patience` | (c) | *理由のみ*。5.0 自体は**不明** |
| `SHORTEST_SLEEP` | `main.rs:6005` | 0.05 秒 | ● | `Budgets::shortest_sleep`、`shortest_sleep` | (c) | **引用**: `prelude.rb:438` の `sleep 0.05` と同じ数で、そちらは理由つき |
| 熱の減衰 `HEAT_DECAY` | `main.rs:2372` | 0.985 /フレーム | ● | `Budgets::heat_decay`、`heat_decay` | (c) | **不明** |
| `--headless` の既定 | `main.rs:2313` | 10.0 秒 | | 引数、または `Settings` の `headless_seconds`（旗が勝つ） | **(e)** | **不明**（既定値の理由） |
| `--shot` の既定 | `main.rs:2314-2315` | `shot.png` / 6.0 秒 | | 引数、または `shot_file` / `shot_seconds` | **(e)** | **不明**。Battle は 3.0 で食い違う（§7-7） |
| `--at` の既定 | — | 無し（0 から） | | `at_seconds`（旗が勝つ）。`--at midnight` は `light_dawn_offset` から導く | **(e)** | S5b-3 が足した |
| `--eye` の既定 | — | `Eye::distance` | | `eye_distance`（**写しを置かず、カメラの既定距離そのものを使う**） | **(e)** | S5b-3 |
| 窓のチェックの上限フレーム数 | `window.rs` の `scheduler_frames` | **3** = 2 + ceil(41,000 ÷ 45,600)（S5b-5 で予算が動いたので 7 から動いた。式は無編集） | | **設定から計算する**（S5b-3）。`script_budget` を上げると上限も上がる。割る数も設定になった（S10、§1.8） | (d) | **導出**: `STRUCTURAL_FRAMES` 2（S6 で測った、構造で縮められない）＋ 1 フレームが買う数（§1.8 の `CheckPace`。S6 の実測 45,600）で割った切り上げ。**S10 で割り算そのものは `CheckPace::frames_to_wait` に 1 つ**になり、2 は引数として箱庭が渡す |
| headless のフレーム間隔 | `main.rs:2521` | 1/60 s | ● | 直書き | (a) 60 Hz を模すという定義 | **導出** |
| `Brains` の枠数 | `main.rs:1408, 1437` | 3（種 2 ＋ world） | | `const`（`Species::ALL.len()` から） | (a) 種を増やすと配列を伸ばす必要がある | **導出** |

### 2.10 セーブ

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `SAVE_VERSION` | `main.rs:6132` | 1 | | `const` | **(a)** 形式の版。変えると古いセーブが読めなくなる（10 番目の判定がそれを確かめている） | **引用** |
| `localStorage` の接頭辞 | `platform.rs:36` | `"garden:"` | | `const` | **(a)** 変えると公開版の利用者のセーブが消える | **引用** |
| セーブのファイル名 `SAVE_FILE` | `platform.rs:62` | `garden.save.json` | | `Settings` の `save_file`（`--save PATH` が勝つ）。**ブラウザでは `localStorage` の鍵**なので、書くのは「別のセーブを持つ」ことで、古い鍵は残る | (c) | S5b-3 が足した。名前そのものは **引用**（G3） |

### 2.11 selftest の閾値（(d)）

**S5b-5 が見直した節である。** 変わったのは 3 行: 「夜が来た」の閾値が実値を読むようになり
（`world.rb` の `day_length`）、窓の「規則」の段の 2 か所の `60.0` が**テキストから読む**形に
なり（`day_length_in`）、上限フレーム数が予算に連れて 7 → 3 になった。**写しはこれで無い**
——残りの 19 行は 1 つずつ見て、実値の写しではないことを確かめた。

| 何を見るか | 位置 | 閾値 | 出どころ |
|---|---|---|---|
| 誰かが食べた | `main.rs:7266` | 10 秒以内 | **不明**（実測は 0.55 s） |
| 夜が来た | `main.rs` の `stop_when_over` | **`Sky::day_length` 1 周**（走行が実際に使っている値を読む） | **導出**。**S5b-5 で直した**——前は `60.0` の直書きで、一覧はそれを「`DAY_LENGTH` 1 周の導出」と記録していたが、実際には `world.rb` の `day_length` の**写し**だった（日を伸ばした `world.rb` は規則を守ったまま FAIL になる） |
| すり抜けていない | `main.rs:7289` | 半径の和の 0.9 | **引用**: `garden-plan.md:67` |
| 草に着いた | `main.rs:6926` | `REACH + 0.5` | **不明**（0.5 の余裕の理由） |
| 向きが変わった | `main.rs:7002` | `dot < 0.7`（≈45°） | *理由のみ*「anything past a quarter turn is a different course」 |
| 向きが変わった（窓） | `main.rs:7006` | 0.5 秒 | **引用**: `garden-plan.md:35` |
| 寝ている | `main.rs:7323` | 夜の 1.0 秒後、速さ < 0.05 | **不明** |
| `NEWBORN_GRACE` | `main.rs:823` | 2.0 秒 | **引用** |
| `NEWBORN_DEAF_FRAMES` | `main.rs:836` | 2 フレーム | **測った** |
| `TOUCH_SETTLE` | `main.rs:841` | 1.5 秒 | **導出**: ハンドラが 1 通につき 0.5 秒ハンドルを握る |
| 壁際の除外 `by_a_wall` | `main.rs:7046` | 壁から 1.5 | **不明** |
| 子の遺伝子 | `main.rs:5096` | **VM から読む**（`mutation_rate_of`、`main.rs:6261`）。読めないときだけ代役の `MUTATION_RATE = 0.1`（`main.rs:6279`） | **引用**: `beetle.rb:21` の `mutation_rate 0.1`。**S5b-4 で写しをやめた**（§7-3 の決着）——判定は種のファイルが言った率で測る |
| `FREEZE_AT` | `main.rs:4812` | 20.0 秒 | **導出** |
| `FREEZE_WINDOW` | `main.rs:4808` | 5.0 秒 | **引用**（導出も添えてある） |
| 草が育った（12 番目の判定） | `main.rs:7404` | 2.0 秒以内 | **不明** |
| 仕込みの断食甲虫 | `main.rs:3404` | 空腹 3.0 | **不明** |
| 仕込みのプローブ | `main.rs:3414` | 空腹 40.0、草まで 5 単位 | *理由のみ* |
| 仕込みのつがい | `main.rs:3568` | 空腹 45.0 | **不明**。`START_HUNGER` の下端と同じ数だが、どちらの記録にも繋いだ形跡が無い——**偶然として扱う**（§7-6 の 55.0 と同じ扱い。S5b-5 が見直して確認） |
| `meadow_at`（隅の位置） | `main.rs:3445` | 壁から 6.0 | *理由のみ*（他の 2 つの仕込みから離れた場所）。**S5b-3 で `const` から `fn` になった** — 畑の広さから導くため |
| `MEADOW_CLEAR` | `main.rs:3454` | 8.0 | **引用**: G2 から `clear_of_fixtures` が使っていた数に名前を付けただけ |
| つがいの隅の幾何 | `main.rs:3546-3557` | 規則の `reach` / `mate_reach` / `plant_max` から組む | **導出**: 不等式ごと rustdoc に書いてある |
| 窓のチェックの開始 | `main.rs:2963` | 3.0 秒 | **不明** |
| 窓のチェックの上限フレーム数 | `window.rs:1352` | `scheduler_frames(&budgets)` | **導出**（§2.9 の行） |
| `STRUCTURAL_FRAMES` | `window.rs:1358` | 2 | **測った**（S6 §4.4） |
| 窓の「規則」の段の日の長さ | `window.rs` の step 11・12・13 | **`editor.text` から読む**（`day_length_in`）。編集はその半分 | **導出**。**S5b-5 で直した**——前は `60.0` の直書きと `"day_length 60.0"` の文字列置換で、日を変えた `world.rb` では**置換が何にも当たらず**、その後の判定が「Apply が効かない」と嘘の FAIL を出す形だった |
| ~~`INSTRUCTIONS_A_FRAME_BUYS`~~ | ~~`window.rs:1365`~~ | 45,600 | **S10 で §1.8 へ移った**（`games_shell::checks`。Battle にも同じ値・同じ出どころの写しがあり、2 つのバイナリにまたがって同じ 1 つの事実を 2 回書いていた）。この節の件数からは外した |

### 2.12 遺伝子（`genome.rs`）

**S5b-5 で `Furniture` の設定になった**（著者判断: 担当の案 A。分類は (b) → **(c)**）。
鍵は `start_beetle_speed` / `_sight` / `_appetite`、`start_rabbit_speed` / `_sight` / `_appetite`、
`start_genome_spread` の 7 つ。**既定値は 1 つも動かしていない。**

`world.rb` に移さなかった理由は 1 行で言える: **この 3 行を読むのは `spawn_world`（`Startup`）
だけ**で、`garden.rules` が届くのは最初の `Update` である。`world.rb` に書けるようにしても
**その数はどの走行でも 1 度も使われない**——「エディタで変えられるのに何も起きない数」を 7 つ
作ることになる。体の半径が移せたのは、半径が `Collider` としてエンティティに載っていて、
渡されたあとで書き直せるから（`bodies_wear_the_rules`）。**読み手がいつ読むかが、移せるかどうかを
決める。** 庭を建てるときに 1 度だけ読む数は、株の数や個体の数と同じ「最初の家具」である。

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| 甲虫の種 `genome::BEETLE` | `genome.rs` | speed 2.2 / sight 8.0 / appetite 1.0 | | `Furniture::beetle`、`start_beetle_speed` / `_sight` / `_appetite` | **(c)** | speed・sight は **不明**（G1 が書いた）。appetite 1.0 は **導出**: `world.rb` の `hunger_rate` に掛かるので、1 は「G1 が走らせた世界そのもの」 |
| ウサギの種 `genome::RABBIT` | `genome.rs` | 3.4 / 12.0 / 1.0 | | `Furniture::rabbit`、`start_rabbit_speed` / `_sight` / `_appetite` | **(c)** | 同上（**不明** ／ 導出） |
| `SPREAD` | `genome.rs` | 0.18 | | `Furniture::genome_spread`、`start_genome_spread` | **(c)** | *理由のみ*「甲虫が甲虫でなくなるほどではなく、2 親に平均する差はある」。0.18 自体は **不明** |
| 遺伝子の上下限 | `genome.rs:230-232` | speed 0.2〜8.0、sight 1.0〜24.0、appetite 0.2〜4.0 | | 直書き | (a) `garden.spawn` が受け取る値の検査 | **不明**（範囲の理由） |
| `mutate` の既定 | `genome.rs:179` | 0.5 | | 直書き（定数が引けないときの保険） | (a) | **不明** |

変異率 0.1 の唯一の出どころは `beetle.rb:21` の `mutation_rate 0.1` で、§3.4 に 1 件として数えた。
**S5b-4 で写しが消えた**: 判定は `mutation_rate_of` で VM からその数を読む（§2.11）。
`main.rs:6279` の `MUTATION_RATE` は「率を宣言しないファイル」のための代役で、`beetle.rb` を
書き換えても判定はついてくる（§7-3 の決着）。

### 2.13 S5b-3 が足した設定の一覧

| 設定 | どこ | `Settings` の鍵 | 既定値 |
|---|---|---|---|
| `Place` | `main.rs`（Resource。`main` が `PreStartup` より前に読む） | `field_width` / `field_depth` / `wall_margin` / `separate_passes` | 40 / 30 / 0.5 / 4 |
| `Furniture` | 同上 | `start_plants` / `plant_min` / `plant_grown` / `start_beetles` / `start_rabbits` / `start_trees` / `start_rocks` / `start_hunger_min` / `start_hunger_max` / `start_tries` / `start_round_chance` / `start_rock_squash_min` / `_max` / `start_solid_apart` / `start_grass_off_solid` / `start_creature_off_grass` / `start_creature_off_solid` / `start_creatures_apart` / **`start_beetle_speed` / `_sight` / `_appetite` / `start_rabbit_speed` / `_sight` / `_appetite` / `start_genome_spread`**（S5b-5） | 55 / 0.18 / 1.4 / 6 / 4 / 7 / 9 / 45 / 90 / 40 / 0.35 / 0.8 / 1.25 / 1.5 / 1.2 / 2.5 / 0.6 / 2.0 / **2.2 / 8.0 / 1.0 / 3.4 / 12.0 / 1.0 / 0.18** |
| `Light` | 同上 | `light_dawn_offset` / `light_moon_lux` / `light_night_ambient` / `light_night_sky_r` / `_g` / `_b` / `light_night_zenith` / `light_dial_min` / `light_dial_max` / `light_day_lux` / `light_day_lux_span` / `light_day_ambient` / `light_day_ambient_span` / `light_sun_lux` / `light_shadow_cascades` / `light_shadow_near` / `light_shadow_far` | 0.08 / 950 / 190 / 0.14 / 0.18 / 0.36 / 0.55 / 0.5 / 2.0 / 1200 / 9000 / 120 / 260 / 8000 / 2 / 24 / 70 |
| `Scenery` | 同上 | `horizon_half` / `sky_radius` / `sky_sides` / `sky_rings` / `fog_near` / `fog_depth` / `fog_color_r` / `_g` / `_b` / `fog_start` / `fog_end` / `edge_trees` / `edge_jitter` / `edge_out_min` / `_max` / `edge_scale_min` / `_max` | 300 / 500 / 12 / 4 / 14 / 45 / 0.35 / 0.5 / 0.7 / 60 / 220 / 16 / 0.4 / 4 / 15 / 1.7 / 3.1 |
| `Picture` | 同上（模型の倍率とアニメの 2 つは `Look` が持ち運ぶ） | `window_width` / `window_height` / `look_ground_r` / `_g` / `_b` / `look_hunger_low` / `look_hunger_warn` / `look_hunger_bar_width` / `_height` / `look_tuft_scale` / `look_bush_scale` / `look_tree_scale` / `look_rock_scale` / `look_rock_squash` / `look_beetle_scale` / `look_rabbit_scale` / `look_walking_at` / `look_gait_blend_ms` / `look_beetle_model` | 1600 / 900 / 0.36 / 0.46 / 0.25 / 20 / 55 / 64 / 11 / 2.2 / 2.6 / 2.2 / 3.4 / 3.0 / 0.55 / 0.75 / 0.2 / 180 / `models/animal-crab.glb` |
| `Eye` | 同上 | `eye_pitch` / `eye_distance` / `eye_zoom_per_notch` / `eye_zoom_min` / `eye_zoom_max` / `eye_pan_per_pixel` / `eye_pan_per_second` / `eye_pan_limit` / `eye_turn_per_pixel` / `eye_pitch_min` / `eye_pitch_max` / `eye_click_reach` / `eye_click_slop` | 0.85 / 42 / 1.10 / 6 / 110 / 0.0016 / 0.9 / 8 / 0.005 / 0.12 / 1.45 / 1.6 / 6 |
| `Budgets` | 同上 | `script_budget` / `script_frame_time_ms` / `world_script_budget` / `world_script_frame_time_ms` / `restore_patience` / `shortest_sleep` / `heat_decay` | **41,000**（S5b-5。S5b-3 の時点では 200,000） / 8 / 45,000 / 8 / 5.0 / 0.05 / 0.985 |
| 旗の既定 | `main` が直接読む | `headless_seconds` / `shot_file` / `shot_seconds` / `at_seconds` / `save_file` | 10 / `shot.png` / 6 / （無し） / `garden.save.json` |

### 2.14 S5b-5 の最後の網が拾った数 — **S9 で全部移した**

S5a の網は `const` と名前のある名前に寄っていた。**S5b-5 で数値リテラルを含む行を機械的に
拾い直し**（`const` 宣言と `impl Default` の中を除いた「関数の本体に直接書かれた数」だけを
見る形。箱庭 220 行・Battle 191 行・共有 crate 70 行）、**14 行を分類だけ付けて挙げた**。
移さなかった理由は「この段階（判定の側）の範囲の外で、1 件ずつ『誰がいつ読むか』を見ないと
移し先が決まらない」だった。

**S9 がその 14 行を移した。** 行はそれぞれ属する節へ入り、この節は経緯だけを残す。

| §2.14 の行 | どこへ | どうなったか |
|---|---|---|
| 昼の空の色の傾き一式 | §2.2 | `Light`、`light_day_sky_rim_*` ほか 4 組 12 鍵 |
| 昼の太陽の色 | §2.2 | `Light`、`light_sun_color_*` / `_span_*` |
| 夜の月の色 | §2.2 | `Light`、`light_moon_color_*` |
| 環境光の色（昼・夜） | §2.2 | `Light`、`light_day_ambient_color_*` / `light_night_ambient_color_*` |
| 空の勾配の押し上げ | §2.3 | `Scenery::rim_gain` / `rim_hold`、`sky_rim_gain` / `sky_rim_hold` |
| 最初の草の大きさの下限 | §2.5 | `Furniture::plant_start_min`、`start_plant_min` |
| 空の板の色 | §2.4 | `Picture::sky_color`、`look_sky_*` |
| 弾のスプライトの倍率 | §4.5 | `Look`、`look_bullet_size` / `_span` |
| 砲塔のスプライト（寸法と色 2 つ） | §4.5 | `Look`、`look_turret_width` / `_height` / `look_turret_team3_*` / `_team4_*` |
| 名札の字（44 px・0.045・影 0.75） | §4.5 | `Look`、`look_nameplate_*`（影のずらし 0.12 も同じ行に畳んだ） |
| 撃破機の灰色（2 つ） | §4.5 | `Look`、`look_downed_hull_*` / `look_downed_turret_*` |
| 記録板の色（6 色） | §4.5 | `Look`、名札 2 色と体力バー 4 つ |
| 影のずらし 0.12 | §4.5 | 名札の行に畳んだ |
| **z 座標一式** | §4.5 | **(a) のまま**。`const` に名前が付いただけ（`SAND_Z`…`PLATE_Z`）で、値は動いていない — **重なりの順序が不変量**で、設定にすると砂を機体の上に置けてしまう |
| 爆発の色の褪せ `1.0 − t` | — | (a) 線形の褪せ。**導出**なので移すものが無い |

**数え直し**（§2.14 が「次の段階で 1 つずつ数えて足す」と書いた宿題）: 表の 14 行は
**1 件 1 数ではない**。移した先で数えると **§2 に 7 行・§4 に 8 行**（z は 1 行に畳んだ）で、
合計 **15 行**が §1〜§5 に増えた。§6 の合計はそのぶん動いている。

**既定値は 1 つも動いていない。** 動かしたほうがよいと思った数は S9 の報告に出した。

## 3. Garden（Ruby）

### 3.1 `ruby/world.rb` — 規則。**この節は全部すでに (e)**

エディタ（`F3`）でその場で書き換えられ、`Ctrl+S` でファイルに残る。移す先としての模範。
毎フレーム `each_frame` が読む（`def` なのでメソッド呼び出しが毎フレーム走る）。

**S5b-4 が 6 行足した**（`touch_reach`、`sprout_gap`、体の半径 4 つ）。この 6 つだけは
`each_frame` が読むのではなく、**`garden.rules` で 1 回ゲームに渡る**——どれも「畑の全部の組を
歩く」ループの中で使われる数で、その歩きは Rust に残っているため（`world.rb:8-12` が
`garden.within` について書いているのと同じ話）。Rust 側の `const` は代役で、
単体テストが食い違いを見張る（§2 の前書き）。

| 名前 | 位置 | 値 | 毎F | 出どころ |
|---|---|---|---|---|
| `day_length` | `world.rb:21` | 60.0 | ● | **引用**: 元は `DAY_LENGTH`（`main.rs:74`、§2.2） |
| `growth` | `world.rb:30` | 0.06 /秒 | ● | 元は `PLANT_GROWTH`。**不明** |
| `plant_max` | `world.rb:31` | 1.4 | ● | **引用**: 元は `PLANT_MAX`。Rust 側は S5b-4 で `plant_grown` という別の名前になった（同じ値の別の数。§2.5） |
| `plant_cap` | `world.rb:32` | 90 | ● | 元は `PLANTS_MAX`。**不明**。予算の実測はこの上限に立つ（`main.rs:4292`） |
| `crumb` | `world.rb:33` | 0.02 | ● | **不明** |
| `sprout_rate` | `world.rb:44` | 乾 0.5 / 雨 0.9 | ● | **導出**: 「0.7 を挟んで等距離、1 年ならして元の 0.7」`world.rb:38-43`。元の 0.7 自体は**不明** |
| 季節の長さ | `world.rb:58` | `every 60` | | **導出**: `day_length` と同じ（`world.rb:56`） |
| `hunger_max` | `world.rb:63` | 100.0 | ● | **引用**: 元は `HUNGER_MAX`（§2.6） |
| `hunger_rate` | `world.rb:64` | 1.6 /秒 | ● | **不明** |
| `eat_rate` | `world.rb:67` | 1.0 /秒 | ● | **不明** |
| `food_value` | `world.rb:68` | 60.0 | ● | **不明** |
| `reach` | `world.rb:69` | 1.1 | ● | 元は `REACH`（`main.rs:252`）。1.1 の理由は **不明** |
| `mate_hunger` | `world.rb:73` | 75.0 | ● | **測った**: 85 では 90 秒に 2 回・0 回のこともあった。甲虫の `hungry_below` 55 より十分上（`docs/plans/garden-plan.md:245`） |
| `mate_reach` | `world.rb:74` | 2.0 | ● | **測った**（§2.6 の `MATE_REACH` と同じ） |
| `court_retry` | `world.rb:76` | 2.0 秒 | ● | **不明** |
| `mate_cost` | `world.rb:77` | 30.0 | ● | **不明** |
| `mate_cooldown` | `world.rb:78` | 20.0 秒 | ● | **不明** |
| `child_hunger` | `world.rb:84` | 50.0 | ● | *理由のみ* 「Well under `mate_hunger`, so nothing is born breeding」`world.rb:84-85` |
| `pop_max` | `world.rb:86` | 24 | ● | **引用**: 元は `POP_MAX`（§2.6） |
| 子が親を殺さない下限 | `world.rb:241` | 1.0 | ● | *理由のみ* 「a child does not kill its parent」 |
| つがいを探す前の人数 | `world.rb:299` | 2 匹以上 | ● | **導出** |
| `touch_reach` | `world.rb:99` | 1.3 | ● | **S5b-4 が Rust から移した**（元は `TOUCH_REACH`）。**不明** |
| `sprout_gap` | `world.rb:103` | 1.5 | ● | **S5b-4 が Rust から移した**（元は `sprout_plants` の直書き）。**不明** |
| `beetle_radius` / `rabbit_radius` | `world.rb:113-114` | 0.40 / 0.50 | ● | **S5b-4 が Rust から移した**。**不明**。模型の倍率（`look_beetle_scale` ほか）とは繋がっていない——片方だけ動かすと当たりと見た目がずれる（§8-3 の組） |
| `tree_radius` / `rock_radius` | `world.rb:115-116` | 0.70 / 0.60 | ● | 同上。**不明** |

### 3.2 `ruby/world_prelude.rb`

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `EVERY_SLOTS` | `world_prelude.rb:258` | 4 | | Ruby | (a) `__every_0..3` が書き出されている数と一致していないと壊れる | **引用**: `world_prelude.rb:243-257`（`ON_SLOTS` と同じ理由） |
| 購読キューの長さ | `world_prelude.rb:140` のコメント | 64（rubevy 側） | ● | rubevy | — | **引用**（rubevy の数） |

### 3.3 `ruby/prelude.rb`（生き物の DSL）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `CRUISE` | `prelude.rb:76` | 2.0 | | Ruby | **(e)** | *理由のみ* 「A creature may ask for more speed than its body has」`prelude.rb:73-75`。2.0 は**不明** |
| `DASH` | `prelude.rb:77` | 9.0 | | Ruby | **(e)** | **不明** |
| `ON_SLOTS` | `prelude.rb:357` | 7 | | Ruby | **(a)** `run_handler` に書き出されている名前の数。減らすとハンドラが黙る | **引用**: `prelude.rb:340-356`（「a budget anybody measured ではない」と明記） |
| ハンドラの優先度差 | `prelude.rb:457` | 本体 −20 | | Ruby | **(e)** | *理由のみ* 「a smaller number is a higher priority: a handler is looked at before the brain」`prelude.rb:456` |
| 起動直後の `sleep` | `prelude.rb:438` | 0.05 秒 | | Ruby | **(e)** | **測った**: この行が無いと、セーブから読んだウサギが空の `@memory` を読む（`prelude.rb:430-437`） |
| `wander` の既定の振れ | `prelude.rb:261` | 0.25 | | Ruby | **(e)** | **不明** |
| `turn_away` の試行回数と角度 | `prelude.rb:325-335` | 4 回 / 5.0 / π÷4 / π÷3 | | Ruby | **(e)** | **不明** |

### 3.4 種の `.rb`（**全部 (e)**。エディタで書き換えられるのが見せたいこと）

| 名前 | 位置 | 値 | 出どころ |
|---|---|---|---|
| 甲虫 `hungry_below` | `beetle.rb:14` | 55.0 | *理由のみ*（`beetle.rb:9`）。5 番目の判定がこの数に依る（`docs/worklog/2026-09-17-garden-world-survey.md` §5）。**空腹バーの色の境 `look_hunger_warn` と同じ値だが繋がっていない。出どころ不明**（§7-6。S5b-4 が `beetle.rb` のコメントにもそう書いた） |
| 甲虫 逃げた後の `sleep` | `beetle.rb:44, 54` | 0.5 / 0.25 | **引用**: 0.5 は `TOUCH_SETTLE` の導出の前提（`main.rs:284-286`） |
| 甲虫 変異率 `mutation_rate` | `beetle.rb:21` | 0.1 | **引用**: この行が唯一の出どころ（§2.12）。**S5b-4 で名前が付き、判定が VM からこの数を読むようになった**——書き換えても 8 番目の判定はついてくる |
| 甲虫 子を置く位置 | `beetle.rb:94` | +1.2, +1.2 | **不明** |
| 甲虫 思い出を口にする周期 | `beetle.rb:76` | 5 食に 1 回 | **不明** |
| 甲虫 本体の `sleep` | `beetle.rb:143` | 0.2 秒 | **不明**（1 判断あたりの費用に直結。`docs/worklog/2026-09-17-garden-world-survey.md` §7） |
| ウサギ `hungry_below` | `rabbit.rb:11` | 60.0 | **不明** |
| ウサギ `nosey_range` | `rabbit.rb:14` | 1.2 | **不明** |
| ウサギ 速さの倍率 | `rabbit.rb:78, 86` | `CRUISE × 1.6` / `× 1.4` | **不明** |
| ウサギ 本体の `sleep` | `rabbit.rb:92` | 0.25 秒 | **不明** |
| 両方 起きたときの `sleep` | `beetle.rb:120,126` / `rabbit.rb:66,72` | 0.5 / 0.1 | *理由のみ*（`beetle.rb:107`「One more `act 0, 0` here is what settles it」） |

---

## 4. SabiRuby Battle（Rust）

> **S5b-2（2026-09-21）で、この節の数は全部「利用者が変えられる場所」に移した。** 行番号は
> `shared-crate` ブランチのその時点のもの。**既定値は 1 つも動かしていない**（例外が 1 つだけ
> あり、§4.6 の「向きが変わったか」の閾値 0.200 → 0.195。§7-12）。
>
> 移し先は 2 つに分かれる。**遊びの数 (b) は Ruby 側**——`ruby/match_prelude.rb` の
> `Match::MODEL`（32 件）で、試合ごとに `match "…", numbers: { … }` で上書きできる。Rust は
> `MatchModel` を serde で受け取り（`#[serde(deny_unknown_fields)]`、`Option` も `default` も
> **無い**）、`MatchModel::wrong` が「遊べない数」（速さ 0、機体より小さい闘技場）を断る。
> **Rust 側に既定値は 1 つも残っていない**——試合が数を言うまで、床も壁もロボットも無い。
> **動かす側の数 (c) は `sabibots.settings.txt`**（`Look`、16 件 + VM の 2 件）。
>
> `prelude.rb` の `SHOT_FAST` / `SHOT_SLOW` と `UNSET` の二重（§7-2）は、両方とも**数が
> 1 つになった**: 前者は `Rubevy.ask("model")` で試合から受け取り、後者は `nil` に置き換えて
> **数そのものを無くした**（§5）。
>
> 過程は `docs/worklog/2026-09-21-numbers-battle.md`。

### 4.1 機体・弾・エネルギー — **(b)。全部 `ruby/match_prelude.rb` の `Match::MODEL` へ**

1 つのコミット（`978eb34`「robots become tanks」）で一度に入った。コミットメッセージは
**なぜこの形にしたか**（「thrust/fire in any direction left one obvious brain」）を言うが、
**個々の数を測った記録は無い**。`docs/sabiruby-battle.md:185-206` は同じ数を仕様として
書き直しているだけで、出どころではない。**全部、毎フレーム読まれる**（Rust 側が
`Res<TheMatch>` のフィールドを読むだけで、VM に問うわけではない。Ruby が聞くのは
起動時の 1 回だけ）。

| 名前 | 位置（Ruby） | 値 | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|
| `robot_radius` | `match_prelude.rb:35` | 1.6 | `matches/*.rb` の `numbers:` | (b) | *理由のみ*（`978eb34`） |
| `hp_max` | `match_prelude.rb:36` | 100.0 | 同上 | (b) | **不明**。**S5b-2 が名前を付けた**——生成・バー 2 か所・判定の計 4 か所に無名で散っていた（§7-8） |
| `max_speed` | `match_prelude.rb:37` | 12.0 | 同上 | (b) | *理由のみ*「turns at a limited rate」 |
| `reverse_speed` | `match_prelude.rb:38` | 7.0 | 同上 | (b) | *理由のみ*（同上） |
| `turn_rate` | `match_prelude.rb:39` | 2.6 rad/s | 同上 | (b) | *理由のみ*（同上）。判定もこれを読む（§4.6） |
| `turret_rate` | `match_prelude.rb:40` | 4.0 rad/s | 同上 | (b) | *理由のみ*（同上） |
| `grip` | `match_prelude.rb:41` | 0.2 秒 | 同上 | (b) | **不明**（`main.rs` の `(-dt / 0.2).exp()` に直書きだった） |
| `out_of_energy` | `match_prelude.rb:42` | 0.35 倍 | 同上 | (b) | **不明**（同じく直書きだった） |
| `energy_max` | `match_prelude.rb:45` | 100.0 | 同上 | (b) | *理由のみ*「a robot that fires everything it has cannot also run away」 |
| `energy_regen` | `match_prelude.rb:46` | 12.0 /s | 同上 | (b) | *理由のみ*（同上） |
| `drive_cost` | `match_prelude.rb:47` | 9.0 /s | 同上 | (b) | *理由のみ*（同上） |
| `fire_cost` / `fire_cost_base` | `match_prelude.rb:48-49` | 16.0 × (0.25 + power) | 同上 | (b) | *理由のみ*／0.25 は **不明**（直書きだった） |
| `power_min` | `match_prelude.rb:52` | 0.2 | 同上 | (b) | **不明**（`clamp(0.2, 1.0)` に直書きだった） |
| `shot_fast` / `shot_slow` | `match_prelude.rb:53-54` | 55.0 / 30.0 | 同上 | (b) | *理由のみ*「more damage, a slower shot, a longer reload」。**`prelude.rb` の写しは消えた**——`lead` は `Rubevy.ask("model")` で受け取る（§7-2） |
| `damage_min` / `damage_max` | `match_prelude.rb:55-56` | 4.0 / 16.0 | 同上 | (b) | *理由のみ*（同上） |
| `cooldown_min` / `cooldown_max` | `match_prelude.rb:57-58` | 0.3 / 0.8 秒 | 同上 | (b) | *理由のみ*（同上） |
| `base_spread` | `match_prelude.rb:59` | 0.02 rad | 同上 | (b) | *理由のみ*「a gun is not a laser」 |
| `bullet_life` | `match_prelude.rb:60` | 2.5 秒 | 同上 | (b) | **不明**（直書きだった） |
| `muzzle` | `match_prelude.rb:61` | 2.8 | 同上 | (b) | **不明**（直書きだった） |
| ~~`UNSET`~~ | — | — | **数そのものが無くなった**（S5b-2）。`act` の「触らない」は `nil` | — | — |

### 4.2 雑音とレーダー

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `noise` | `matches/training.rb:4` | 0.3 | ● | **Ruby** | **(e)** | **引用**: `docs/sabiruby-battle.md:202-206` |
| `position_blur` | `match_prelude.rb:66` | 0.05 | ● | `numbers:` | (b) | **引用**（式のみ）: `docs/sabiruby-battle.md:204`「positions by up to noise × distance × 5%」 |
| `velocity_blur` | `match_prelude.rb:67` | 2.0 | ● | `numbers:` | (b) | **不明** |
| `shot_blur` | `match_prelude.rb:68` | 1.5 | ● | `numbers:` | (b) | **不明** |
| `spread_per_noise` | `match_prelude.rb:69` | 0.1 | ● | `numbers:` | (b) | **引用**（式のみ）: `docs/sabiruby-battle.md:205` |
| `radar_range` | `match_prelude.rb:64` | 60.0 | ● | `numbers:` | (b) | **不明**。**Rust と Ruby の二重が消えた**: `prelude.rb` の `radar(range = nil)` はこれを既定にし、Rust の `num_or` もこれを読む |
| `incoming_range` | `match_prelude.rb:65` | 25.0 | ● | `numbers:` | (b) | **不明**（同上） |
| `seed` の桁 | `main.rs:2537` | `× 1,000,000` | | 直書き | (a) Ruby に整数で渡すための桁 | **不明** |

### 4.3 闘技場と壁

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| `arena`（半幅） | `match_prelude.rb:72` | 32.0 | | `numbers:` | (b) | **不明**。S5b-1 が Rust の `ARENA_HALF_WIDTH` で止めてあったもの |
| `crate_size`（木箱の間隔） | `match_prelude.rb:73` | 2.6 | | `numbers:` | (b) | **引用**: `docs/sabiruby-battle.md:646`（「25 crates a side at the start」＝ 64/2.6）。**一覧の分類案は (c) だったが (b) にした**——`shrink` が動かす量そのもので、`min_crates` と組でしか意味がない（§7-13） |
| `min_crates` | `match_prelude.rb:74` | 7 | | `numbers:` | (b) | **引用**: コミット `28f3b53`「down to 7 a side」。7 の根拠は*理由のみ* |
| `NO_MATCH_YET` | `main.rs:43` | 1.0 | | `const` のまま | **(a)** 闘技場ではない。試合が幅を言うまでカメラが枠取る正方形で、**何も描かれない**（床も壁も `build_field` が試合の言ったフレームに置く）。正の数でありさえすればよい | **導出**: `games-shell` の `NO_WINDOW` と同じ形 |
| 床のタイル `FLOOR_TILE` / `FLOOR_PATTERN` | `main.rs:270-271` | 8.0 単位、3 枚に 1 枚別柄 | | `Settings` の `look_floor_tile` / `look_floor_pattern` | (c) | **不明** |
| ロボットの初期配置 | `match_prelude.rb:137-139` | 半径 20、円周に等分 | | **Ruby** | **(e)** | **不明** |
| `TEAMS` の数 | `main.rs:370` | 4 | | `const` | (a) `TEAM_COLORS` と長さが揃っている必要がある | **導出** |

### 4.4 試合の進行（`matches/training.rb`、**すでに (e)**）

`noise` は §4.2 に数えた。

| 名前 | 位置 | 値 | 出どころ |
|---|---|---|---|
| `sudden_death` の開始 | `training.rb:9` | 20 秒 | **引用**: コミット `28f3b53`「shrink one crate every 2 s from 20 s on instead of two jumps」 |
| 縮む周期 | `training.rb:9` | 2.0 秒ごと 1 箱 | **引用**: 同上（`28f3b53`） |
| 試合ループの `sleep` | `match_prelude.rb:196` | 0.2 秒 | **不明** |
| 決着を見る前の `sleep` | `match_prelude.rb:171` | 0.1 秒 | **不明** |

### 4.5 表示と実行 — **(c)。`sabibots.settings.txt` へ**

`Look`（Resource、`main.rs:280`）。鍵は全部 `look_` で始まる（S5b-1 の `editor_` / `vm_` /
`hud_` / `guide_` / `code_` / `camera_` に揃えた）。窓の大きさだけは窓を開ける前に要るので、
`Settings` は 2 つの枝に分かれる前に読む。

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|---|
| 予算バーの満 `BAR_FULL` | `main.rs:266` | 3,000 命令 | ● | `look_bar_full` | (c) | *理由のみ*「the bar fills as one approaches a timeslice's worth」。3,000 は**不明**、そして `ScriptWorld::budget` からは**導けない**（200,000 は VM 全体の 1 フレーム分で、rubevy はタスクごとの持ち分を持たない）。**2 か所の写しは 1 つになり**、`panel.budget` の毎フレーム代入は生成時の 1 回になった（§7-8、§7-9） |
| 熱の減衰 `HEAT_MEMORY` | `main.rs:258` | 0.8 秒の時定数 | ● | `look_heat_memory` | (c) | *理由のみ*「about a second of memory」 |
| 体力バーの色の境 `LIFE_WARN` / `LIFE_LOW` | `main.rs:241-242` | 0.5 / 0.25 | ● | `look_life_warn` / `look_life_low` | (c) | **不明**。2 か所（記録板と機体の上）の写しが 1 つになった |
| 体力バー `LIFE_BAR` | `main.rs:236` | 幅 3.6 / 高さ 0.45 / 浮き 1.35 | ● | `look_life_bar_width` / `_height` / `_lift` | (c) | **不明**（幅 3.6 は `robot_radius × 2.25` と一致するが、そうは書かれていない）。**高さと浮きは一覧に無かった 2 件**——幅と同じバーの寸法なので一緒に足した |
| 機体の見た目の大きさ `HULL_SCALE` | `main.rs:247` | `robot_radius × 2.4` | | `look_hull_scale` | (c) | **不明** |
| 名札の高さ `NAMEPLATE_LIFT` | `main.rs:248` | `robot_radius × 2.25` | ● | `look_nameplate_lift` | (c) | **不明** |
| 爆発 `BLAST_DOWN` / `BLAST_HIT` | `main.rs:253-254` | ×3.5・0.7 秒 ／ ×0.6・0.25 秒 | ● | `look_blast_down` / `_span`、`look_blast_hit` / `_span` | (c) | **不明**。damage の割り算は `damage_max` を読むようになった（`/16.0` の写しが消えた） |
| 窓の大きさ `WINDOW` | `main.rs:274` | 1600×900 | | `window_width` / `window_height` | (c) | **不明** |
| VM の予算 `SCRIPT_BUDGET` | `main.rs` の `SCRIPT_BUDGET` | **114,000 命令**（2026-09-21 に 200,000 から動かした。著者判断・案 A） | ● | `script_budget` | (c) | **測った**（S10、2026-09-21）。測定: 上限の試合 = 4 チーム × 9 台 = **36 台**を 16 走行・**26,398 フレーム**。1 フレームの命令数は中央値 3,872・99 パーセンタイル 45,280・**最悪 67,749**、tick は中央値 0.23 ms・最悪 8.83 ms、**持ち越しは全フレームで 0**、予算に届いたフレームは 0。同梱の試合（4 台）は最悪 2,730 = 新しい既定の **2.4%**。**36 台はゲームが言う上限ではない**——チーム 4 は `Match::TEAMS` が言うが、1 チーム 9 台は半径 20 の輪（周長 125.7）に差し渡し 3.2 の機体が 39 台しか入らないことから**担当が導いた**数で、Battle には機体数の上限がどこにも無い。**114,000 = `1.74 × 65,704` を 1,000 の位に丸めた数**。65,704 は案を書いた時点（10 走行め）の最悪で、その後 67,749 まで伸びたが 3 つ目の数は作らなかった（箱庭が 23,686 → 23,912 でそうしたのと同じ扱い）。**1.74 は箱庭の比の借用で、Battle 自身の理由から出した係数ではない**——世界 VM の 45,000 ÷ 実測最悪 25,837 という、著者が選んだ数から後ろ向きに出た比である。**Battle 自身の理由からは係数を出せなかった**（余裕が守る相手は「まだ書かれていない試合」と「誰も上限を言っていない機体数」。`docs/worklog/2026-09-21-s10.md` §2.6）。**出荷時の余裕は 114,000 ÷ 67,749 = 1.68**。**変えると**: 上げれば 36 台より大きい試合や重い頭脳が 1 フレームで考え切れるようになり、暴走したスクリプト（`sleep` の無いループ）が 1 フレームに使う時間も増える（実測: 200,000 で 1 フレーム約 212,600 命令・tick **7.9 ms**、114,000 で約 122,600 命令・**3.2 ms**。60 Hz の 1 フレーム 16.7 ms の 47% と 19%。§2.9）。下げれば逆。**前の 200,000 は rubevy の既定を引き継いだもので、rubevy 側の出どころは不明** |
| VM の `frame_time` | rubevy の既定 8 ms | — | ● | `script_frame_time_ms` | (c) | **不明**（同上。rubevy の 8 ms も「出どころ不明」と書かれている） |
| `--headless` の既定 | `main.rs:584` | 10.0 秒 | | 引数、または `headless_seconds` | (c)→(e) | **不明** |
| `--shot` の既定 | `main.rs:585-586` | `shot.png` / 3.0 秒 | | 引数、または `shot_file` / `shot_seconds` | (c)→(e) | **不明**（箱庭は 6.0。§7-7） |
| 弾の見た目 | `main.rs:2629` | `0.7 + 0.6 × power` | ● | 直書きのまま | (c) | **不明**。**移していない**——一覧に無く、S5b-1 の作法（範囲を自分で広げない）に従った。§7-14 |
| 砲塔・名札・影の寸法 | `main.rs:481, 499, 870, 873, 916` | 1.1×2.6、1.0、44 px、0.045、0.12 | | 直書きのまま | (c) | **不明**。同上、§7-14 |
| スクリプトの優先度 | `main.rs:2012, 2163, 2241` | 本体 100 / 試合 10 / 差し替え 128 | | 直書き | (a) 順序の約束。`prelude.rb:295` の `+20` と組で効く | **不明**（100/10/128 の理由） |
| `localStorage` の接頭辞 | `platform.rs:37` | `"sabibots:"` | | `const` | **(a)** 変えると利用者のセーブが消える | **引用**: `docs/plans/shared-crate-plan.md:162` |

> **Battle の一覧に無い数**は §2.14 の後半にまとめて挙げた（弾のスプライトの倍率、砲塔、
> 名札、撃破機の灰色、記録板の色、z 座標一式）。S5b-5 の最後の網が拾ったもので、
> **分類だけ付けて移していない。**

### 4.6 selftest の閾値（(d)。**S5b-5 で見直した**）

| 何を見るか | 位置 | 閾値 | 出どころ |
|---|---|---|---|
| ハンドラが走った・向きが変わった | `main.rs` の `HIT_WINDOW` | 当たりから 0.3 秒、**または `handler_frames(budget)` フレーム**（長い方。S9） | **引用**: `docs/sabiruby-battle.md`（scout の `sleep 0.3` と対）。フレームの側は**導出＋実測**（下の行） |
| その窓が持つべきフレーム数 | `main.rs` の `handler_frames` / `HANDLER_STRUCTURAL_FRAMES`（割る数は §1.8 の `CheckPace`） | `2 + ceil(script_budget ÷ 45,600)` = 出荷時 **5**（S10b で予算が 200,000 → 114,000 に動いたので 7 から動いた。式は無編集） | **導出＋実測**（**S9 が足した**）。2 は構造（publish は tick の後 → 次のフレームの tick でハンドラに順番、判定はその鎖と順序を持たないのでさらに次のフレームで確実に見える）で、**静かなヘッドレス 6 走行・145 件の当たりが全部 2**。45,600 は S6 の実測で、**S10 まではこのファイルに書かれた写しだった**（`garden/src/window.rs` に同じ名前・同じ出どころで 1 つ。2 つのバイナリにまたがるので cite できなかった）——今は `games_shell::checks` に 1 つで、鍵は 2 本で同じ `checks_instructions_a_frame`。**実測の最悪は 4**（ブラウザ 1600×900・CPU ×20 の 5 走行、11 件）。**走行のログが上限を名乗る**ようになった（S10 の舞台指示 1 行。verdict が無いので行の一覧は動かない） |
| どれだけ向きが変わったか | `main.rs` の `enough_of_a_swerve` | `turn_rate × (その当たりがもらった窓) ÷ 4` | **導出**: 「a quarter of the full turning rate over the window」。**S5b-2 で `turn_rate` を式にし、S9 で窓も式にした**——窓が広がった走行で「0.3 秒ぶんの 4 分の 1」を訊くと、ただ走っているだけの機体が通る。既定の模型・普通の窓では 0.195（0.200 は同じ導出を丸めた数。§7-12） |
| ハンドラの上限のつまみ | `platform.rs` の `handler_frames_asked` | `SABIBOTS_HANDLER_FRAMES=N` / `?selftest&handler_frames=N` | **(d) の道具**（S9）。この判定の不具合はブラウザでしか出ないので、ページでも回せる形にした（S5b-5 のつまみと同じ道） |
| 落ちたロボットを外す | `main.rs:1728` | 落ちてから 0.5 秒 | **不明** |
| 体力が満で始まる | `main.rs:1570` | `hp_max` | **導出**: S5b-2 で数の写しをやめ、試合が渡した値を読む |
| 壁が隅で終わる | `main.rs:1565-1566` | 誤差 0.01 | **導出**: ε（判定の一部として挙げた） |
| チェックの開始 | `main.rs:770` | 2.0 秒 | **不明** |

---

## 5. SabiRuby Battle（Ruby）

> **S5b-2 の後。** `prelude.rb`（robot の DSL）が持っていた Rust の数の写し 3 つは消えた:
> `SHOT_FAST` / `SHOT_SLOW` は `Rubevy.ask("model")` で試合から受け取り、`UNSET` は `nil` に
> 置き換わって**数そのものが無くなった**。`radar` / `incoming` の既定距離も同じ道で来る。
> 試合の模型 32 件は §4 に数えた（重複して数えない）。

| 名前 | 位置 | 値 | 今変えられるか | 分類 | 出どころ |
|---|---|---|---|---|---|
| ~~`UNSET`~~ | — | — | **無くなった**（S5b-2）。`act(throttle: nil)` の `nil` が `Request::num` に `None` で届く | — | — |
| ~~`SHOT_FAST` / `SHOT_SLOW`~~ | `prelude.rb:68-75`（`Model`） | 試合の `shot_fast` / `shot_slow` | **`matches/*.rb` の `numbers:`** | **(e)** | 写しではなくなった（§7-2 の決着） |
| `ON_SLOTS` | `prelude.rb:144` | 4 | Ruby | **(a)** `run_handler` に書き出されている名前の数 | **引用**: 箱庭の `ON_SLOTS` と同じ理由 |
| ハンドラの優先度差 | `prelude.rb:295-296` | 本体 +20、上限 255 | Ruby | (a) 255 は mruby-task の優先度の上限 | **不明**（+20 の理由） |
| `steer_to` の利得 | `prelude.rb:189` | ×2.0 | Ruby | **(e)** | **不明** |
| `aimed?` の許容 | `prelude.rb:211` | 0.12 rad | Ruby | **(e)** | **不明** |
| `near_wall?` の余白 | `prelude.rb:215` | 6.0 | Ruby | **(e)** | **不明** |
| `on_collision?` の半径 | `prelude.rb:228` | 2.5 | Ruby | **(e)** | **不明**（`robot_radius` 1.6 より大きい。当たり判定ではなく「避けるかどうか」の余裕） |
| `wander_turn` の変わりやすさ | `prelude.rb:239` | 0.04 | Ruby | **(e)** | **不明** |
| `lead` の反復 | `prelude.rb:202` | 2 回 | Ruby | (a) 「one guess … corrected once」 | *理由のみ* |
| `fire` の既定 | `prelude.rb:121` | 0.5 | Ruby | **(e)** | **不明** |
| scout の数 | `scout.rb:18-64` | `sleep 0.3` / `0.05`、`rand < 0.5` / `< 0.01`、`nearest_enemy(45)`、`incoming(18)`、`near_wall?(6)`、距離 14、`lead(_, 0.3)`、`aimed?(_, 0.2)`、エネルギー 25、威力 0.3 | Ruby | **(e)** | `sleep 0.05` と `sleep 0.3` は *理由のみ*。残りは**不明** |
| hunter の数 | `hunter.rb:5-19` | `nearest_enemy(60)`、`near_wall?(5)`、距離 18 / 34、`lead(_, 0.9)`、エネルギー 40、威力 0.9 / 0.8、`sleep 0.08` | Ruby | **(e)** | **不明** |

---

## 6. 集計

数えたのはこの文書の表の行（§1〜§5）で、**300 件**（S9 の後。S5b-5 の時点では 285）。
**S10 は件数を動かしていない** — 1 行が §2.11 から §1.8 へ**引っ越した**だけで（共有 crate 44 → 45、
`garden/src` 126 → 125）、Battle の §4.6 の行はその写しを持つのをやめて §1.8 を指している。
新しく決めた数は 0、既定値も 1 つも動いていない。
`docs/garden.md` の例を引いただけの行と rubevy 側の数を参照しただけの行の 2 つは数えていない。
消えた数（`UNSET` 2 件と `SHOT_FAST` / `SHOT_SLOW` の写し 1 件）は**取り消し線の行として残し、
件数からは外した**——どこへ行ったかを次に数える人が探さずに済むように。

**S9 が 15 件足した**——§2.14 が「分類だけ付けて移していない」と置いた 14 行を、それぞれの節へ
移したぶんである（§2 に 7 行、§4 に 8 行、うち z 座標一式は 1 行に畳んだ）。
**新しく決めた数は 0、既定値も 1 つも動いていない。** 移した先は (c) が 14 行、(a) が 1 行
（z 座標一式）。

**S5a の 260 件からの差は 21 件。** うち 9 件は S5b-1 の分（S3 のパン・ズームのカメラ 8 件、
コードパネルの字の大きさ、アリーナの床の色、S3 で消えた「ずらす幅 0.16」を落とした）、
3 件が S5b-2 の分:

* **+2**: VM パネルの 7 色を 1 行、説明パネルの余白 3 つを 1 行（S5a の網に掛かっていなかった。§1）。
* **+3 / −2**: Battle。無名だった数に名前が付いて 1 行になったもの（体力の満 100 は 4 か所 →
  `hp_max` 1 行、予算バーの満 3000 は 2 か所 → `BAR_FULL` 1 行）と、新しく一覧に入れたもの
  （`NO_MATCH_YET`、VM の予算と `frame_time`、体力バーの高さと浮き、移していない直書き 2 行）と、
  **無くなった 3 件**（`UNSET` の Rust 側と Ruby 側、`SHOT_FAST` / `SHOT_SLOW` の Ruby の写し）。

そして **4 件が S5b-4**（箱庭。281 → 285）。**新しく決めた数は 0、既定値も 1 つも動いていない。**
増えた 4 件は**同じ数が住所を変えたぶんではなく、`world.rb` に新しく現れた行**である——
`TOUCH_REACH`・芽の最小間隔・4 つの半径の 6 つは Rust の行（代役として残る）と `world.rb` の行の
両方に姿があり、§0 の「2 つに跨るものは先に書いた方で 1 件」の規則で Rust 側 1 件、
そのうえで §3.1 に `touch_reach` / `sprout_gap` / 半径 2 行の**4 行**が増えた
（半径は「1 組 1 行」で 2 行に畳んである）。

消えた数は無い。名前が変わったものが 2 つある: `PLANT_MAX` → `PLANT_GROWN`（鍵も
`plant_max` → `plant_grown`。`world.rb` の `plant_max` と**同じことを言っていない**ので
名前を分けた。§7-15 の決着）と、判定の `RATE` → VM から読む + 代役 `MUTATION_RATE`（§7-3 の決着）。

残り **9 件が S5b-3**（箱庭の Rust、117 → 126）。**新しく決めた数は 0 で、既定値も 1 つも
動いていない**——増えたのは、これまで関数の途中に無名で書かれていて一覧に 1 行も無かった数に
名前が付いたぶんである:

* **+8**: 縁の木の散らし方（1 行）、起動時の霧（1 行）、窓の大きさ、地面の色、生成時の空腹、
  配置のやり直し回数、岩の潰し、初期配置の間隔 5 つ（1 行）、オービットの回転感度、pitch の範囲、
  `--at` と `--eye` の既定、セーブのファイル名、`STRUCTURAL_FRAMES` と
  `INSTRUCTIONS_A_FRAME_BUYS`（S7 が `SCHEDULER_FRAMES` の rustdoc に書いていた 2 つの数に
  名前が付いた）。
* **+3**: **一覧の抜けとして挙げただけで移していない** 3 行（昼の色の傾き、空の勾配の押し上げ、
  最初の草の大きさの下限 0.3）。S5b-1 と S5b-2 が同じ判断をしたのと同じ理由——
  一覧に無い数を移すのは段階が自分で範囲を広げることなので、挙げるだけにした。
* **−2**: `HALF_W` / `HALF_D` と `MIDNIGHT` が `const` でなくなった（畑の広さと夜明けの位相から
  導く関数になった）。取り消し線の行として残し、件数からは外した。

### 分類ごと

| 分類 | 件数 | S5b-3 の後から |
|---|---|---|
| (a) 不変量 | **31** | **+1**（S9: Battle の z 座標一式。重なりの順序が不変量なので `const` のまま名前だけ付けた） |
| (b) 遊びの数 → Ruby 側 | **35** | **−3**: 種の遺伝子 3 件が S5b-5 で **(c)** に移った（§2.12。`world.rb` では 1 度も読まれない数になるため、`Furniture` の設定へ）。**6 件は S5b-4 で `world.rb` へ移り終えている**。残る (b) は体の寸法の組（§8-3）|
| (c) 動かす側の数 → `Settings` / 引数 | **127** | **+14**（S9: §2.14 の 14 行。箱庭 7 行 → `Light` / `Scenery` / `Picture` / `Furniture`、Battle 7 行 → `Look`）。その前は **+3**（種の遺伝子。S5b-5 で `Furniture` の 7 鍵になった）|
| (d) selftest の閾値 | 32 | 変わらず（子の遺伝子の 1 行が「写し」から「VM から読む」になっただけ） |
| (e) 既に Ruby か設定から変えられる | 75 | **+4**（`world.rb` に増えた 4 行） |
| **合計** | **300** | **+15**（S9） |

**(c) のうち、箱庭の 64 件は S5b-3 で移し終えた**（§2.13 が移した先の一覧）。
共有 crate は S5b-1、Battle は S5b-2、そして**関数の本体に直書きされていた 14 行は S9** が
済ませている。残る (c) は無い。
この表の (c) は「動かす側の数」という**分類**であって「まだ移していない」という意味ではない。

2 つに跨るもの（`DAY_LENGTH` のように「既に Ruby から変えられるが Rust にも既定値がある」）は、
**先に書いた方**で 1 件として数えた。Battle の `radar` / `incoming` の既定距離はこの形だったが、
S5b-2 で**両側が同じ 1 つの数を読む**ようになったので跨いでいない。

**(b) のうち Battle の 32 件は、もう「移す候補」ではなく移し終えたもの**（`Match::MODEL`）。
箱庭の分は S5b-4 が 6 件を `world.rb` へ移した。**移し終わっていない (b) は 1 件**——
体の寸法と模型の倍率の組（§8-3。半径は `world.rb` へ行き、倍率は `garden.settings.txt` に
残ったので、**2 つで 1 組の数が 2 つの場所に分かれた** — 片方だけ動かすと当たりと見た目がずれる）。
種の遺伝子 3 件は S5b-5 で (c) になった（§2.12）。

**毎フレーム読まれる数は 112 件**（全体の 40%）。Battle の機体模型 20 件はこの数に入ったままだが、
**読んでいるのは Rust で、`Res<TheMatch>` のフィールドを読むだけ**——Ruby に問うのは
robot が起動するときの 1 回（`Rubevy.ask("model")`）と、試合が始まるときの 1 回
（`Rubevy.ask("rules", …)`）だけである。だから「毎フレーム Ruby に聞く」費用は 0 で、
S5a が心配していた測定は要らなかった。

### 出どころごと

| 出どころ | 件数 |
|---|---|
| **測った**（日付・条件つきの実測がある） | 17 |
| **導出**（既にある制約から出ている） | 23 |
| **引用**（文書・計画書・コミットに書いてある） | 52 |
| *理由のみ*（理由らしきことは書いてあるが、測った記録も導出も無い） | 50 |
| **不明** | **143** |
| **合計** | **285** |

**出どころ不明が 143 件、全体の 50%。** S5b-4 が足した 4 行は全部「不明」で、それは
**移す前も不明だったものが住所を変えただけ**である（1.3 も 1.5 も 4 つの半径も、どこにも
記録が無い）。移すことは出どころを作らない。内訳:

| どこ | 件数 | うち不明 |
|---|---|---|
| 共有 crate の `src`（`rubevy-egui` + `games-shell`。**S10 の後**） | 45 | 26 |
| `garden/src`（**S10 の後**） | 125 | 54 |
| `garden/ruby`（S5b-4 の後） | 44 | 24 |
| `sabibots/src` + `sabibots/ruby`（S5b-2 の後） | 71 | 39 |

**「測った」が 14 → 17 に増えたのは、既にあった測定に名前が付いたぶん**である
（`SEPARATE_PASSES`、世界 VM の予算、夜の 3 つを 1 行ずつ数え直した）。
**S5b-3 が新しく取った測定**——上限の庭に座った生き物 VM の 1 フレーム——は、S5b-3 の時点では
「既定値を動かしていないので数の出どころになっていない」ものだった。**S5b-5 で出どころになった**:
著者が案 A を選び、`script_budget` の既定は `1.74 × 23,686` を丸めた 41,000 になり、
出どころの欄は「rubevy の既定を引き継いだ、rubevy 側は不明」から「測った」に変わった。
**箱庭の 2 本の VM はこれで両方とも測って決めた数になった。** Battle の `script_budget` も
**S10 が上限の試合に座って測り、S10b で著者が案 A を選んで 200,000 → 114,000 になった**
（§4.5 の行）——測定と導出と「何を得て何を失うか」は `docs/worklog/2026-09-21-s10.md` §2。
**これで 3 本の VM の予算が全部「測って決めた既定値」になった**（引き継いだまま残っているのは
壁時計の 8 ms 3 本——箱庭の 2 つと Battle の `script_frame_time_ms`——で、どれも
「測って『きつくない』と言えるだけで、もっときつい数がいくつかは測っていない」状態である）。箱庭の測った分布は
`docs/worklog/2026-09-21-numbers-garden-settings.md` §4、決着は
`docs/worklog/2026-09-21-checks-and-leftovers.md` §1。

Battle の 2 つを分けて数えるのをやめたのは、**数の住所が言語で分かれなくなった**ため:
機体の模型 32 件はいまや `sabibots/ruby/match_prelude.rb` にあり、それを読むのは Rust である。

不明がいちばん濃いのは**表示まわり**（字の大きさ、パネルの寸法、色、バーの目盛）と
**Battle の機体模型に付いていた無名の数**（弾の寿命、砲口の位置、グリップ）——後者は
S5b-2 で名前が付いたが、**名前が付いたことと出どころが分かったことは別**で、
`match_prelude.rb` の各行のコメントは「unknown」と書いてある。
いちばん薄いのは**箱庭の夜と霧と予算**で、そこは測った記録がコメントに残っている。

### 機械的に拾った数と、選んだ数

S5a が `grep` で拾った母数は S5a の時点のもの。S5b-1・S5b-2・S5b-3 が既定値に名前を付けたので
リテラルを含む行は増えているが、**利用者から見える数が増えたわけではない**（名前と
`impl Default` と単体テストのぶん）。

| どこ | `const` 宣言（S5a 時点） | 数値リテラルを含む行（S5a 時点） | 選んだ数（今） |
|---|---|---|---|
| 共有 crate の `src` | 20（うち数値 17） | 149 | 44 |
| `garden/src` | 88（うち数値 68） | 538 | 126 |
| `sabibots/src` | 28（うち数値 21） | 322 | — |
| `garden/ruby` + `sabibots/ruby` | 8（`NAME = 数` の行） | 237 | — |
| `sabibots` の `src` + `ruby` | — | — | 71 |
| **合計** | **144（うち数値 114）** | **1,246** | **281** |

`const` 宣言は `grep -nE '^\s*(pub\s+)?const\s'`、リテラルは
`grep -nE '(^|[^A-Za-z0-9_.":])[0-9]+(_[0-9]+)*(\.[0-9]+)?'` で拾い、コメントだけの行を落としてから目で選んだ。
落とした 8 割の内訳は §0 の「入れなかった基準」。
選んだ数がリテラル行より多い節と少ない節があるのは、1 行に数が 2 つある（`spacing([12.0, 6.0])`）のと
同じ数が何行にも出る（`0.35` が 3 か所）のが混ざっているため。

## 7. 気づいた点（一覧の仕事の外。**直していない**）

1. **`code.rs` のコメントと定数が最初から食い違っている。** `crates/rubevy-egui/src/code.rs:147` は
   「430px of a monospace 12px font: about 58 characters」と書いているのに `code.rs:149` は `WIDTH = 44`。
   両方 1 つのコミット（`23c5ad3`）で入っている。どちらが正しいのか読んでも分からない。
   なお `CodePanel` は**どのゲームからも使われていない**（`HudPlugin`、`read_script` も同じ。計画書 §2 が
   「消さない」と決めている）。属する話: 共有 crate。
   **S5b-1 の扱い（2026-09-20）**: どちらにも寄せていない。既定は 44 のまま（`CodeStyle::chars`）、
   食い違いそのものを `CHARS` の rustdoc に書き、58 が欲しい人は `code_chars=58` と書けるようにした。
   「コメントの方が正しい」と決めて 58 にするのは、記録の無い推測で既定値を動かすことになる。
2. **Battle の 3 つの数が Rust と Ruby の両方に別々に書かれている。** `UNSET = -999.0`
   （`sabibots/src/main.rs:51` と `sabibots/ruby/prelude.rb:59`）、`BULLET_SPEED_FAST/SLOW` = 55/30
   （`main.rs:42-43` と `prelude.rb:61-62` の `SHOT_FAST/SHOT_SLOW`）。`prelude.rb:60` は
   「the game's numbers, for leading a target」と**写しであることを自覚している**が、繋がってはいないので
   片方を動かすともう片方が黙って嘘をつく（`lead` が外れる、`act` が「触らない」を取り違える）。
   属する話: S5b で Battle の数を Ruby に移すときの設計そのもの。
   **決着（S5b-2、2026-09-21）**: 3 つとも**数が 1 つになった**。弾の速さは試合の持ち物
   （`match_prelude.rb` の `shot_fast` / `shot_slow`）で、`prelude.rb` は `Rubevy.ask("model")` で
   受け取る——robot が起動するときの 1 回だけで、毎フレームではない。`UNSET` は**消した**:
   `act(throttle: nil)` の `nil` は `Arg::Value` として届き、`Request::num` が `None` を返すので、
   番兵の数そのものが要らない（S5a が §8-6 で「`Option` にすれば数が要らなくなる」と書いた道）。
3. **箱庭の変異率 0.1 が 2 か所にある。** 本物は `garden/ruby/creatures/beetle.rb:81` の `mutate(0.1)`、
   判定側は `garden/src/main.rs:3955` の `const RATE: f32 = 0.1`。`main.rs:3953` は「the rate is the one the
   beetle's script passes to `mutate`, which is 0.1」と写しだと書いているが、**`beetle.rb` を書き換えても
   判定は 0.1 のまま**なので、8 番目の判定は利用者が変異率を上げた瞬間に FAIL になる（エディタで
   その場で書き換えられるのがこのゲームの見せ場なので、踏める）。属する話: (d) の閾値の設計。
   **決着（S5b-4、2026-09-21）**: 写しは消えた。種のファイルが率を**名前のある数**として持ち
   （`beetle.rb:21` の `mutation_rate 0.1`、宣言は `prelude.rb` の `Creature.mutation_rate`）、
   判定は `mutation_rate_of` で VM から読む——`@handlers` / `@asleep` と同じ道で、`ivar_get` 2 回、
   子が要求されたフレームに。確かめ方: `mutation_rate` を 0.5 にしただけのファイルで、前の版は
   2 走行とも FAIL、後の版は ok。率を宣言しないファイル（前の版の `beetle.rb`、つまり公開版の
   ブラウザにあるもの）は代役の `MUTATION_RATE` で判断され、**前と同じ**。
4. **（S3 で解消）`ArenaPlugin` のコメントと数が合っていない。** 2026-09-20 に確かめた:
   `games-shell/src/lib.rs` に `0.16` も「a third of the window」も**もう無い**。S3 が `ViewInsets`
   （パネルが実測の px を書く）に替えたときに、数もコメントも一緒に消えている。残っているのは
   `follow_arena` の rustdoc と単体テストの中で「16% は何だったか」を説明する記述だけで、
   これは歴史の説明であって使われている数ではない。以下は S5a 時点の記述: `crates/games-shell/src/lib.rs:117` は
   「the editor is about a third of the window on the right」と書いて、次の行で `visible_width * 0.16` を
   使っている。1/3 なら中心をずらす量は 1/6 ≈ 0.167 なので**計算としては合っている**が、コメントは
   「エディタの幅」を、数は「ずらす量」を言っていて、読むと食い違って見える。計画 S3 がこの行を
   「塞がれ px の Resource」に替えるので、そのとき 1 行で言い直せる。属する話: 共有 crate。
5. **`CLICK_REACH` のコメントが体の寸法と合わない。** `garden/src/window.rs:92-93` は
   「the creature nearest where it lands — within about a body's width」と言うが、`CLICK_REACH` は 1.6 で、
   生き物の半径は 0.40〜0.50（`main.rs:309-310`）＝体の幅は 0.8〜1.0。1.6 は体 1.6〜2 個分。
   当たりやすくするための余裕だと思われるが、そうは書いていない。属する話: (c) の出どころ。
6. **空腹バーの色の境 55.0 と甲虫の `hungry_below` 55.0 が同じ数で、繋がっていない。**
   `garden/src/window.rs:1059` と `garden/ruby/creatures/beetle.rb:10`。偶然かどうか読んでも分からない。
   もし「甲虫が草を探し始める点で色が変わる」つもりなら、`beetle.rb` を書き換えた瞬間にずれる。
   属する話: (c) と (e) の境目。
   **決着（本体の判断、S5b-4 で記述）**: **繋がない。** 記録に意図が無い以上、同じ値であることは
   偶然として扱う（§1.4 の VM パネルの色と同じ扱い）。`hungry_below` は `world.rb` ではなく
   `beetle.rb` の数で `look_hunger_warn` は `garden.settings.txt` の数、という**住所の違いも
   そのまま**にした。両方の表にそう書き、`beetle.rb:11-13` のコメントにも 3 行で書いてある
   ——読む人が「片方を変えたらもう片方も」と思わないように。出どころは両方とも**不明**。
7. **2 本のゲームで `--shot` の既定秒数が違う。** 箱庭 6.0（`garden/src/main.rs:1474`）、
   Battle 3.0（`sabibots/src/main.rs:271`）。同じ意味の引数で、計画 S1 がこの引数解析を共有 crate に上げる。
   上げるときにどちらに寄せるか（あるいはゲームごとの既定を受け取るか）を決める必要がある。
   `--headless` はどちらも 10.0 で揃っている。属する話: S1。
8. **Battle の「100」と「3000」がそれぞれ 3 か所・2 か所に散っている。** 体力の満は
   `sabibots/src/main.rs:1565`（生成）、`:638` と `:715`（バーの `/ 100.0`）、`:1086`（判定）に
   4 回書かれていて `ENERGY_MAX` のような名前が無い。VM の予算バーの満は `:2308` の `3_000` と
   `:659` の `3000.0`。どれも `const` にすらなっていない。属する話: S5b の最初の作業。
   **決着（S5b-2）**: 100 は試合の `hp_max`（`match_prelude.rb:36`）1 つになり、生成もバー 2 か所も
   判定も同じ 1 つを読む。3000 は `Look::bar_full`（`sabibots.settings.txt` の `look_bar_full`）
   1 つになった。**`ScriptWorld::budget` から導けないか調べたが、導けない**——200,000 は VM 全体の
   1 フレーム分で、rubevy はタスクごとの持ち分を持たない（`task_run_limits` に残り全部を渡す）。
   だから数は残るが、1 つで、変えられる場所にある。
9. **`sabibots/src/main.rs:2308` は毎フレーム同じ定数を代入している。** `panel.budget = 3_000;` が
   ロボット 1 体につき毎フレーム走る。害は無いが、`ScriptPanel` の初期化時に 1 度でよい。
   属する話: 共有 crate の `ScriptPanel`。
   **決着（S5b-2）**: `spawn_robot` の `ScriptPanel { budget: … }` で 1 回だけになった。
10. **`docs/README.md` の目次の Battle の説明（31 行 + 当たり 2 行）と `?selftest` の実際の行数は
    今回確かめていない。** 一覧の仕事はコードを動かさないので、突き合わせていない。属する話: S1 の基準取り。
11. **VM パネルの `AMBER` と `REG_NAME` は、エディタの `AMBER` / `KINDS[0]` と同じ値**
    （`(240,190,90)` と `(210,214,222)`）。同じ crate の隣同士で、片方は「熱の暖色」、片方は
    「止まっている」の印で、**意図の記録はどこにも無い**。S5b-2 は `garden` の空腹バー 55.0 と
    `hungry_below` 55.0（§7-6）と同じ扱いにした——**繋がない**。繋ぐと、エディタの `* edited` の
    色を変えた人が VM パネルまで塗り替えることになり、それは記録の無い意図を後から作ることになる。
    単体テスト（`inspect.rs` の `the_colours_are_settings_and_not_in_the_store`）が「今日は同じ値」
    であることだけを書き留めている。属する話: 共有 crate（色の設計）。
12. **S5b-2 で動いた既定値が 1 つだけある**: 判定「向きが変わったか」の閾値 0.200 → 0.195。
    `handler_selftest` は `watch.peak > 0.2` と書いていて、その横のコメントが
    「a quarter of the full turning rate over the 0.3 s」——つまり `TURN_RATE 2.6 × 0.3 ÷ 4 = 0.195`
    を丸めた数だと、S5a の一覧が既に記録している。`turn_rate` が試合のものになった以上、
    0.2 という数は**試合の数の写し**になる（速い機体の試合では緩すぎ、遅い機体の試合では
    通らない閾値になる）ので、コメントが言っていた式そのものにした
    （`enough_of_a_swerve`）。動く向きは緩い側で、判定が厳しくなることはない。
    属する話: (d) の閾値の設計／著者への報告事項。
13. **`crate_size` 2.6 を (c) ではなく (b) にした。** 一覧の分類案は (c)（動かす側）だったが、
    `shrink` が壁を動かす量そのもので、`min_crates`（分類案 (b)）と組でしか意味を持たない。
    `wall_layout` は 2 つを一緒に使う。`Match::MODEL` に置いた。
    属する話: 分類の見直し（著者判断待ち。戻すなら `Look` へ 1 行移すだけ）。
14. **一覧に無い直書きの数が Battle にまだある。** 弾のスプライトの大きさ
    （`main.rs:2629` の `0.7 + 0.6 × power`、`× 0.55` / `× 1.3`）、砲塔のスプライト
    （`:481` の `1.1 × 2.6`、`:499` の 1.0）、名札の字の大きさと縮尺と影のずれ
    （`:870, 873, 916` の 44 px / 0.045 / 0.12）、灰色 2 種（`:459, 503`）、z 座標一式。
    S5a の網に掛かっておらず、**S5b-2 は範囲を自分で広げないために移していない**
    （S5b-1 が同じ判断をした。§1 の VM パネルの色がその結果 1 段階遅れた）。
    一覧には §4.5 の 2 行として載せた。属する話: 一覧の抜け。
    **決着（S9）**: **全部 `Look` へ移した**（§4.5）。z 座標一式だけは **(a) のまま** — 重なりの
    **順序**が不変量で、設定にすると砂を機体の上に置けてしまう。`const` に名前が付いただけで
    値は動かしていない。**まだ残っているもの**（S9 も範囲を広げなかった）: 弾のスプライトの
    縦横比 `× 0.55` / `× 1.3`、爆発の絵の倍率 `× 0.4`、砲身の付け根のずらし 1.0、床が壁の外へ
    はみ出す幅。§2.14 の 14 行に無かったので手を付けていない。
15. **（S5b-3）箱庭の `world.rb` の `plant_max` 1.4 と Rust の `PLANT_MAX` は、今も 2 つの数である。**
    Rust の方は `Furniture::plant_max`（`garden.settings.txt` の `plant_max`）になり、
    Ruby の方は `world.rb:31` にある。**繋いでいない**: Battle の弾の速さと違って、この 2 つは
    同じことを言っていない——Rust のは「庭を建てるときの株の大きさ」、Ruby のは「成長が止まる
    上限」で、`main.rs:651-653` がその区別を書いている。同じ値であるのは今日のことで、
    片方を動かした人は「育ちきらない草」か「最初から育ちきった草」を見ることになる。
    属する話: (b) と (c) の境目（S5b-4 で `world.rb` 側を見るときに）。
    **決着（S5b-4）**: **名前を分けた**（繋がない）。Rust 側は `PLANT_GROWN` /
    `Furniture::plant_grown`、`garden.settings.txt` の鍵も `plant_grown`。同じ名前が 2 つの
    違う文を言っていたのが 1 日の間の紛らわしさの正体で、値は動かしていない。
    **S5b-3 以降に `plant_max=…` と書いた設定ファイルは、もうその鍵を持たない**
    （知らない鍵は黙って無視されるので、書いた人には効かなくなる）。
16. **（S5b-3）新しい庭の最初の草の大きさの下限 0.3 が一覧に無い。**
    `main.rs:3367` の `dice.between(0.3, plant_grown)` で、`plant_min` 0.18（芽の大きさ）とは
    **別の数**。S5a の網に掛かっておらず、S5b-3 は範囲を自分で広げないために移していない。
    一覧には §2.5 の 1 行として載せた。属する話: 一覧の抜け。
    **決着（S9）**: `Furniture::plant_start_min`（`start_plant_min`）へ移した。名前を
    `PLANT_START_MIN` にして `PLANT_MIN`（芽）と読み分けられるようにした。値は動かしていない。
17. **（S5b-4）`Brains::wearing` は「編集されたときだけ」埋まっていた。** S7 が
    「その種が今着ているプログラム」という名前で入れた `Brains::wearing` は、エディタからの
    引き渡し（`hand_over`）でしか書かれず、**誰も編集していない庭では空**だった。そのため
    `give_mind` は生き物 1 匹ごとに種のファイルをコンパイルし、`Assets<MrbAsset>` に同じバイト列を
    匹数ぶん置いていた（09-20 R2 が報告したもので、S7 で済んでいるはずとされていた）。
    S5b-4 が `Brains::first_program` を足して直した（`garden/src/main.rs`）。
    **名前が約束していることと、実際に入るものが違う**状態が 1 段階ぶん残っていた例。
    属する話: 箱庭（設計）／本の素材。
18. **（S5b-4）`touch_reach` を大きくしても「触られた」は増えない。**
    `Contacts` は接触が**作られた瞬間**だけを publish するので、届く距離を伸ばすと接触が
    切れにくくなり、イベントは減る方向にも動く（6.0 にしても 11〜16 件で、既定 1.3 の 12〜13 件と
    区別がつかなかった）。遊びの数としては直感に反するので `docs/garden.md` に 1 行あってよい。
    属する話: 箱庭（遊びの設計）。
19. **（S5b-3）昼の色の傾きが一覧に無い。** `sky_colors` の昼側（`main.rs:4012-4013` の
    `0.35 + 0.15 × noon` ほか 6 つ）、太陽の色（`:4053`）、月の色（`:4048`）、環境光の 2 色
    （`:4066, 4069`）、空の勾配の押し上げ（`:4159` の `t × 1.25 − 0.1`）。
    **夜の 3 つ（測ってある）は一覧にあって、昼の同じ形の数は 1 つも無い**——S5a の網が
    `const` と名前のある数に寄っていたためで、昼の色は全部関数の途中に書かれている。
    S5b-3 は移していない。一覧には §2.2 と §2.3 の 1 行ずつとして載せた。属する話: 一覧の抜け。
    **決着（S9）**: 全部 `Light` と `Scenery` へ移した（§2.2・§2.3）。昼の空・太陽は
    「夜明けの値 ＋ 太陽の高さが足すぶん」の組にした——`DAY_LUX` と `DAY_AMBIENT` が
    既にその形だったので、同じ形にすれば鍵の名前も同じ規則で付く。値は動かしていない。
    **`MOON_LUX` の注意書きがここにも掛かる**: G6b が測った真夜中の絵は、この空の色と
    この夜の環境光でできている。
20. **（S5b-3）`Picture` の模型の倍率は `Look` が写しを持ち運ぶ。** `make_look` が起動時に
    6 つの倍率とアニメの 2 つを `Look` に複写する。リテラルの重複ではなく起動時の 1 回の代入で、
    `spawn_plant` ほか 4 つのヘルパが今までどおり `Option<&Look>` 1 つだけを取るためにそうした
    （ヘルパが `Look` を持つときだけ倍率を使うので、`Look` は「倍率が要る場所」とちょうど同じ
    集合である）。ただし**走行中に `Picture` を書き換えても `Look` は変わらない**。
    属する話: 箱庭（設計）。

---

## 8. 分類に迷った数と、迷った理由

1. **新しい芽の最小間隔 1.5**（`garden/src/main.rs:3526`）。
   草が生える規則は W1 で全部 `world.rb` に移ったのに、「他の草から 1.5 以内には生やさない」だけが
   Rust の `sprout_plants` に残っている。**(b) 遊びの数**として `world.rb` に足すのが筋に見えるが、
   `world.rb` から見ると「草の位置の総当たり」で、それは Rust に残した理由そのもの（`world.rb:11`）。
   → (b) と書いたが、S5b で「Ruby が数だけ渡し、走査は Rust」という `garden.rules` の道に乗るかを
   著者に確かめたい。
   **決着（著者、2026-09-20 / S5b-4 で実装）**: **数は `world.rb`、走査は Rust。**
   `world.rb:103` の `sprout_gap` が `garden.rules` で 1 回渡り、畑の全部の株を歩くのは
   `sprout_plants` のまま。`garden.sprout` の引数にしなかったのは、それだと**同じ数を毎秒 60 回
   境界の向こうへ運ぶ**ことになるため。確かめ方: 12.0 にすると 1 分後の畑が 41〜48 株から
   12〜16 株になる。

2. **`TOUCH_REACH` 1.3**（`garden/src/main.rs:253`）。
   `startle`（ウサギ→甲虫）は Rust に残った規則で、その距離。**(b)** だが `garden.rules` に
   受け口が無い（今あるのは `day_length` / `child_hunger` / `pop_max` / `reach` / `mate_reach` の 5 つ、
   `main.rs:4446`）。受け口を 1 つ増やすだけなのか、`startle` ごと `world.rb` に移すのかで話が変わる。
   **決着（S5b-4）**: **受け口を 1 つ増やした**（`world.rb:99` の `touch_reach`）。`startle` を
   `world.rb` に移さなかったのは、それが甲虫全部 × ウサギ全部の総当たりだからで、1 と同じ理由。
   確かめ方: 0.5 にすると（体の半径の和 0.90 より短いので）触りが 1 件も起きず、6 番目の判定が
   `(0/0)` で落ちる。

3. **生き物の半径 4 つ**（`main.rs:309-312`）。
   体の寸法は **(a) 不変量**（モデルの大きさと結びついている）か、**(b) 遊びの数**（押し分けの強さ）か。
   `Collider` は Ruby から**読める**（`register_type` 済み、`docs/garden.md:280`）が書けない。
   モデルの倍率（`main.rs:2589-2590` の 0.55 / 0.75）と一緒に動かさないと見た目と当たりがずれるので、
   → (b) と書いたが「2 つで 1 組」であることを S5b で保つ必要がある。
   **決着（S5b-4）**: 半径は **(b)** として `world.rb` へ（4 つとも）。**組は保てていない**——
   模型の倍率は (c) として `garden.settings.txt` に残ったので、**2 つで 1 組の数が 2 つの場所に
   分かれた**。繋がなかったのは、当たりの半径と見た目の倍率が同じ数ではない（倍率は
   `.glb` の寸法で割った数）ため。両方の rustdoc と `world.rb` のコメントに「片方だけ動かすと
   ずれる」と書き、**一度は見る価値のある壊れ方**として残した。著者が組にしたければ、
   `look_*_scale` を `world.rb` へ move するか、倍率を半径から導く関係を書くことになる。

4. **`KINDS` の 9 色**（`crates/rubevy-egui/src/editor.rs:454`）。
   **(c) 表示**に入れたが、これは「測って決めた既定値」の模範例で、利用者が変えられるようにすると
   測った制約（コントラスト 4.9 / ΔE 25）を破れてしまう。**(a) 寄りの (c)**。
   → 変えられるようにするなら「変えると測った保証が消える」と rustdoc に書くべき、と考えるが、
   そこまで踏み込むかは著者判断。
   **決着（著者、2026-09-20 / S5b-1 で実装）**: 変えられるようにする（`EditorColors::kinds`）。
   **測った保証が破れること**を `KINDS` の rustdoc に書いた（コントラスト 4.96 / 帯 3.09 / ΔE 25.9 は
   この 9 色の*集合*についての測定で、1 色差し替えれば 3 つとも成り立たなくなり、それを見張るものは
   コードの中に無い）。`Settings` には出していない — 色を文字列から読むには構文解析が要る。

5. **`REGS_FRAMES` 6**（`inspect.rs:29`）。
   毎フレーム `vm.snapshot(REGS_FRAMES)` を呼ぶので**費用に直結**する。(c) だが、
   「VM の中を何フレーム覗くか」は表示の設定というより**道具の性能の設定**で、
   `Settings` に出すのが適切か、パネルの中のダイヤルにするか、決めていない。

6. **`UNSET = -999.0`**（`sabibots/src/main.rs:51`、`prelude.rb:59`）。
   **(a)** に入れた（「触らない」の合図で、実際に取りうる値と重なってはいけない）が、
   `throttle` は −1〜1、`aim` は角度なので −999 が「ありえない値」である保証は**書かれていない**。
   `Option` にすれば数が要らなくなる — それは S5b ではなく設計変更なので、挙げるだけにした。

7. **`ArenaSize::default()` 32.0**（`lib.rs:43`）。
   共有 crate の既定値で、Battle はそのまま使っている。**(c)** にしたが、闘技場の広さは
   Battle にとっては **(b) 遊びの数**（`shrink` で試合中に変わる）。
   「共有 crate の既定 = ゲームの遊びの数」が重なっている唯一の場所。
   **決着（著者、2026-09-20 / S5b-1 で実装）**: 共有 crate は既定を**持たない**
   （`ArenaPlugin::showing(half)`。`CameraPlugin::showing(half_height)` が既定を持たないのと同じ理由 —
   窓が抱える世界の量はゲームしか知らない）。32.0 は Battle の `ARENA_HALF_WIDTH` に名前つきで置いた。
   これを Ruby 側（`matches/*.rb`）へ移すかどうかは **S5b-2** が決める。

8. **`--headless` / `--shot` の既定秒数**。
   **(e)**（引数で変えられる）に入れたが、既定値そのものは変えられない。
   「既定値は数か」を厳密に取ると (c) だが、そこまで数えると引数のある数は全部二重になるので (e) にした。

9. **`SHORTEST_SLEEP` 0.05**（`main.rs:4747`）と **`prelude.rb:438` の `sleep 0.05`**。
   同じ数だが仕事が違う（片方は判定の下限、片方は Ruby の待ち）。
   (c) と (e) に分けて数えたが、**片方を動かすともう片方の意味が変わる**ので 1 組かもしれない。

10. **`platform.rs` の接頭辞 `"garden:"` / `"sabibots:"`**。
    数ではないが **(a) 不変量**として入れた（変えると利用者のセーブが消える）。
    「数の一覧」に文字列を入れるかは迷った — 計画書 §5 の罠にこの 2 つが名指しで挙がっているので入れた。

11. **（S5b-4）種の遺伝子 `Genome::of` と `SPREAD`**（`genome.rs`）。
    **決着（S5b-5、著者判断: 案 A）**: 分類を **(b) → (c)** にし、`Furniture` の設定にした
    （`start_beetle_speed` / `_sight` / `_appetite`、`start_rabbit_speed` / `_sight` / `_appetite`、
    `start_genome_spread`。既定値は据え置き）。以下は判断の前の材料である。
    理由は分類の迷いではなく、**移し先が無い**ことである: この 3 行を読むのは `spawn_world`
    （`Startup`）だけで、`garden.rules` が届くのは最初の `Update` ——つまり `world.rb` に書けるように
    しても**その数はどの走行でも 1 度も使われない**。体の半径が移せたのは、半径が `Collider` として
    エンティティに載っていて、渡されたあとで書き直せるから（`bodies_wear_the_rules`）。
    **読み手がいつ読むかが、移せるかどうかを決める。**
    3 案（本体の推奨は A。詳しくは `docs/worklog/2026-09-21-numbers-garden-play.md` §6）:
    **A**「新しい庭の stock」として `Furniture`（`start_beetle_speed` ほか 7 鍵。分類は (b) → (c)）、
    **B** 世界の生成を「規則が喋ってから」に作り替えて `world.rb` へ（起動の順序が変わる／
    規則が永遠に喋らない庭の扱いが要る）、**C** 据え置き。

---

## 9. Factory（F0〜F5。2026-09-22）

3 本目の crate `factory/` が持っている数の全部。節の番号が 8 の後ろなのは、
§2〜§5 の番号が他の文書から参照されているため（renumber しない）。
過程は `docs/worklog/2026-09-21-factory-F0.md`・`…-F1.md`・`…-F2.md`・`…-F3.md`・`…-F3a.md`・
`docs/worklog/2026-09-22-factory-F4.md`・`…-F5.md`。

**方針**: 「遊びの数は Ruby、動かす側は `factory.settings.txt`、`const` は不変量だけ」
（計画書 `plans/factory-plan.md` §2）。**F2 で data stage ができて、遊びの数は `ruby/data.rb` へ
移った**。F1 が設定に置いていた 6 件のうち 4 件が F2 で Ruby へ、鉱石の量が F2a で Ruby へ、
**残る 2 件（`map_tiles` と `ore_patch_radius`）も F3a で Ruby へ**。
**世界の組み立ての数は 1 つも設定に残っていない** — `main()` は地図を知らず、
`Startup` の `lay_the_land` が `data.rb` の言うとおりに作る（§9.2 の最後）。

### 9.1 不変量 (a) — `const` のまま

| 位置 | 名前 | 値 | 変えると何が壊れるか | 出どころ |
|---|---|---|---|---|
| `factory/src/draw.rs` | `MOST_TILES_ACROSS` | **2048** | **地図の辺の上限**（F3a）。これを上げると、上げた先の地図が「この機械では描けるが他人のブラウザでは描けない」ものになりうる | **導出 + 実測（2026-09-21）**: `TilemapChunk` はタイル情報を **1 タイル 1 テクセル**の `Rgba16Uint` 2D テクスチャに置く（`bevy_sprite_render-0.19.1/src/tilemap_chunk/tilemap_chunk_material.rs:96`）ので、辺が `max_texture_dimension_2d` を越えると wgpu がテクスチャを作れない。Bevy はアダプタの limits をそのまま要求する（`bevy_render-0.19.1/src/renderer/mod.rs:310`）ので**この値は機械ごとに違う** — 実測は lavapipe 16384、SwiftShader 8192。公開するページなので採ったのは「どの WebGL2 機械でも保証される値」= **2048**（`wgpu-types-29.0.4/src/limits.rs:498` の `downlevel_defaults`、WebGL2 仕様の `MAX_TEXTURE_SIZE` 下限）。2048² は `u32` に収まるので、タイル添字の型が言う上限（辺 65,535）はこれに隠れる。**chunk を分ければ外れる上限だが、分けない**（買えるのは機械のメモリより大きい地図だけ。理由は rustdoc と worklog F3a §5） |
| `factory/src/main.rs` | `TILE_PX` | 16 | 素材の 1 タイルの大きさ。違う数はシートを違う所で切る。**F2a からはベルト 1 タイルの刻みの数でもある**（1 刻み = 素材 1 画素）ので、`items_per_tile` に書ける値（16 の約数）もこの数が決める | 引用: `factory/art/Tilesheet-tiny-factory.txt`「Tile size • 16px × 16px」 |
| `factory/src/main.rs` | `TILESET_LAYERS` | **143**（F2 は 142、F1 は 139） | シートの層の数。**6 の倍数にするとブラウザで何も描かれない**（配列テクスチャがキューブマップ配列として bind される）。判定がこの数を読み返す | 導出: Kenney の 132 + ベルト 4 + 鉱石 2 + 組立機 4 + **インサータの台 1** = 143。143 % 6 = 5 なので `pad_to_a_safe_count` は空白を足さない。根拠は `wgpu-hal-29.0.4/src/gles/mod.rs:458`、実測は 2026-09-21 のブラウザ走行 |
| `factory/src/draw.rs` | `ITEM_PX` | 8 | アイテムの絵の大きさ。シートを切る所 | 導出: 16 px のタイルに 2 個が触れずに載る大きさ。`data.rb` の `items_per_tile` の既定はこの数から出ている（逆ではない）。既定の 2 は 16 の約数でもあるので F2a でも動かない |
| `factory/src/draw.rs` | `ITEM_ICONS` / `HAND` / `STOPPED` / `ICONS` | 3 / 3 / 4 / 5 | `assets/items/items.png` のうち**アイテムの絵は 3 枚**で、`data.rb` の `icon:` はそこへの添字。後ろの 2 枚（F3 の腕の手と、止まった腕の印）は**データファイルから届かない** | 引用: `tools/factory-items.py` の `ITEMS` と `MARKS`。単体テストが `data.rb` の `icon:` が全部 `ITEM_ICONS` の範囲にあることを見る |
| `factory/src/save.rs` | `SAVE_VERSION` | **1** | セーブファイルの**形式のバージョン**（F5）。上げると古いファイルが読まれなくなる — それが目的で、下げると古いファイルが**間違って**読まれる | **導出ではなく約束**: 構造体から導けない（serde が既定値で埋められるフィールドの出し入れは「読めるが意味が違う」ファイルを残す）。動かす規則は箱庭の `SAVE_VERSION` と同じ = 「古いビルドの書いたものが *読めない* ではなく *間違って読める* ならば上げる」。人間にしか決められない数なので `const` にしてある |
| `factory/src/control.rs` | `WRAPPER_LINES` | **1** | ゲームが `control.rb` を包むメソッドの `def` の行数。ずれると例外の行が 1 行ずれる（F4） | **導出**: `in_a_method` が前に足す行は `def the_control_file` の 1 行だけ（後ろの `end` は数えない — ファイルの末尾より後ろの行は報告されない） |
| `factory/src/draw.rs` | `Z_ITEM` / `Z_CARRIED` / `Z_MARK` | 2.0 / 2.1 / 2.2 | スプライトの重なり。手と手の中の物は同じ場所なので片方が上でなければならない。**同じ画像で z が 3 つ = バッチ 3 本**で、それが腕の絵の値段（F2 は 1 本） | 導出: `bevy_sprite_render` のバッチは「同じ z の同じ画像の連続」 |
| `factory/src/draw.rs` | `FLOOR_PLATES` / `GROUND` / `ORE_RICH` / `ORE_POOR` / `BELT_STRAIGHT` / `BELT_CORNER` / `MINER` / `CHEST` | `[0,1,2]` / `3` / `136` / `137` / `[132,133]` / `[134,135]` / `110` / `85` | シートのどのタイルか。違う数は違う絵を描く（設定ではない） | 引用: シートを 2026-09-21 に読んだ（`docs/factory.md` の表、`docs/factory-tiles.png`）。**機械の絵はここに無い** — `data.rb` の `sprite:` が言う |
| `factory/src/draw.rs` | `FACING` / `TURN_LEFT` / `TURN_RIGHT` | `TileOrientation` の 4 + 4 | 1 枚の直線と 1 枚の曲がりから 4 向き + 8 通りの曲がりを作る回し方。違う組み合わせはベルトが逆を向く | 導出: 描いた絵が「左から入って上へ出る」ことと `TileOrientation` の定義から。単体テスト `eight_turns_and_four_directions_out_of_two_pictures` が 8 通りが相異なることを見る |

### 9.2 動かす側 (c) — `factory.settings.txt`

| 鍵 | 既定値 | 出どころ |
|---|---|---|
| `camera_half_height` | 150（= 9.375 タイル、縦 18.75 タイル） | **導出（F1）**: F0 の 128 は既定の 900 px の窓で素材 1 px = 3.515625 画面 px で、**整数倍でないズームがタイルの継ぎ目を滲ませる**（F0 が PC とブラウザで 887 画素の差として測った）。その前後の整数倍のうち**広く見える方** 3 倍 = 900 ÷ (2×3) を採った |
| `camera_snap_zoom` | 1（する） | **導出**: 上と同じ。0 にすると F0 の見え方に戻る |
| `window_width` / `window_height` | 1600 / 900 | **不明**。既存 2 本と同じ値にしただけ（`sabibots/src/main.rs`「Source unknown」） |
| `headless_seconds` | **66**（F3 は 30、F2 は 20、F1 は 10） | **導出（F4 で取り直した）**: 判定自身の上限の和（F1 の線 6 + 詰まり 1 + 機械の線 11 + 壊れた腕 2 + **F4 の 3 つ × 6 = 18** = 44 秒）の 1.5 倍。F4 の 3 つはどれも「採掘機の線が箱にもう 1 個入れるまで」（`mine_seconds + 4 ÷ belt` = 3.0 秒）× `CHECK_SLACK`。何も悪くない走行は **15 秒**（実測 2026-09-22。F3 では 10.5 秒）。F2 の書き方は: : 判定は 2 つの小さな工場を順に建てる。F1 の（採掘機・ベルト 3 枚・箱）は `mine_seconds + 4 ÷ belt_tiles_per_second` = 3.0 秒、F2 の（ベルト→機械→ベルト→箱）は `2 ÷ belt + time ÷ speed + 1 ÷ belt` = かまど 3.5 秒。合わせて 6.5 秒で、20 はその 3 倍強。**上限であって待ち時間ではない** |
| `shot_file` / `shot_seconds` | `shot.png` / **8.0** | **導出（F3 で取り直した。値は動いていない）**: 絵に写したいのは「工場が動いているところ」。機械の線は 4 秒ごろ動き出し、最初の 1 個が箱に入るのが 9.5 秒（実測 2026-09-21）なので、8 はその最中で、腕が振っている途中である |
| `stress_items` | 0（しない） | 計測の仕掛け。`--stress N` / `FACTORY_STRESS` / `?stress=N` でも同じ。**地図の大きさは走行が自分で決める**（√(N ÷ `items_per_tile`)、`data.rb` の地図より小さくはしない、`MOST_TILES_ACROSS` より大きくもしない）。F2 でその計算は `Startup`（data stage の後）へ移り、F3a で**世界が組まれる前**に走るようになった（`Rules::map_tiles` を書き換える） |
| `stress_arms` | 0（しない） | **F3 の計測の仕掛け**。`--arms N` / `FACTORY_ARMS` / `?arms=N`。1 台ぶんは「中身入りの箱・腕・空の箱」の 3 タイルで、**全部の腕に仕事がある**形。地図は √(8 × N) で自分で決める（1 台が 4 タイル × 2 行） |
| `inserter_stagger` | 1.0（1 swing ぶん散らす） | **計測のつまみで、遊びの数ではない**。スクリプトが最初に見るまでの待ちを swing の何割に散らすか。0 にすると全台が同じフレームに起きる（`FACTORY_STAGGER` / `?stagger=0`）。既定の 1.0 は**導出**: 1 周期より広く散らしても意味がない。0 にすると何が起きるかは §9.8 |
| `script_budget` | **39,000** | **測って決めた（2026-09-21）**: §9.8。rubevy の既定 200,000 を継がずにこのゲームが言う |
| `script_frame_time_ms` | 8.0 | **rubevy の既定のまま**、ただし「そのままでよい」は測って言っている（§9.8）。0 以下は「時計の番人なし」という rubevy の意味でそのまま通す |

`camera_*`（ズームの刻み、ドラッグの比、クリックの遊び）は共有 crate の 7 鍵をそのまま使う（§1.2）。
ただし **`camera_half_height` は 3 つとも地図で調整される**（F3a、`main::point_the_camera_at_the_map`）:
端は地図、既定の見え方は「地図より広くは見せない」、ズームの外側は「全体まで引ける所まで上げる（下げはしない）」。
既定の 32×32 では 3 つとも今までと同じ値になる。

**F3a で設定から世界の数が全部消えた。** F2 は 2 件（`map_tiles`・`ore_patch_radius`）を残し、
理由を「どちらも `Ore::laid_out` が 1 回だけ読む世界の組み立ての数だから」と書いた。
F2a がその言い方は片方にしか当たっていないと直し（`ore_per_tile` は `Startup` で読まれるので
`data.rb` へ移した）、残った理由は **`map_tiles` の下限が `main()` の、まだ VM が無い所で要る**、
の 1 つだけになっていた。**F3a はその「要る」のほうを無くした**: 地図そのものが宣言
（`map :world, size: [w, h]`）になり、世界は `Startup` の `lay_the_land` が作るので、
`main()` は地図を一切知らない。**設定に書いてあった 2 鍵は、古いファイルに残っていれば
起動時に log が「`data.rb` へ移った」と 1 度言う**（黙って無視しない。`main::moved_settings`）。

### 9.3 遊びの数 (b) — **`ruby/data.rb`**（F2 で移した）

プレイヤーがファイルを編集して変える数。**Rust の側に既定値は 1 つも無い**（S5b-2 の Battle の
`Match::MODEL` と同じ形: 受け取るまでゲームは始まらない）。0 以下の数は**読み込み時に、その行で**
断る（`data.rb:6: 0 is not more than zero`）。

| 宣言 | 鍵 | 既定値 | 出どころ |
|---|---|---|---|
| `belt :conveyor` | `tiles_per_second` | 2.0 | **遊んで決めた、根拠なし**。これが基準で、下の数はこれに対する比で決まっている |
| | `items_per_tile` | **2**（F2 は `2.0` と綴っていた） | **導出**: アイテムの絵が 8 px、タイルが 16 px なので、触れずに並ぶのは 2 個。**満杯のベルトは毎秒 4 個運ぶ**（2.0 × 2）で、下の全部がこの 4 に対する割合である。**書けるのは 16 の約数だけ**（1・2・4・8・16）で、それ以外は `data.rb:行` 付きで断られる — 隙間は `16 ÷ この数` 刻みで、割り切れない隙間は隙間ではないから（F2a）。`2.0` と書いても同じ意味に読む |
| `miner :drill` | `seconds_per_item` | 1.0 | **導出**: ベルトの **1/4**（毎秒 1 個）。4 台で 1 本が埋まる |

| `chest :crate` | `capacity` | 60 | **導出**: 採掘機 1 台の **1 分ぶん**（60 秒 × 毎秒 1 個） |
| `map :world` | `size` | **[32, 32]** | **F0 の仮の値のまま**（§9.6）。**F3a で `factory.settings.txt` から移した** — 著者「サイズは自由に変えられるようにしたい」。既定値は動いていない。綴りは `machine` の `size:` と同じで、意味が同じだから（タイル何枚ぶん、横と縦）。**正方形でなくてよい**。下限は `ore` の半径と数から導く（§9.4）、上限は描画が言う 2048（§9.1）、どちらも黙って直さず `data.rb:行` で断る |
| `ore :iron_ore` | `patch_radius` | 3.0 | **導出**: 畑は `patches` の格子の各セルの中心に置く。半径 3 なら 15 タイルの地図でも畑が縁（タイル 0 の壁）に掛からない（§9.4 の下限）。**F3a で設定から移した**（綴りから `ore_` が落ちた — 語が言っている） |
| | `patches` | **[2, 2]** | **導出（F3a）**: 「4 隅に 4 つ」を数ではなく**格子**として言い直したもので、`[2, 2]` は今までの置き方とタイル 1 枚も変わらない（`Ore::patch_middle` の `(2i+1)/(2n)` が `tiles/4` と `3×tiles/4` を出す）。数（`patches: 4`）にしなかったのは、散らすには種が要り（出どころの無い数）、2 回の走行が同じ所に鉱石を見つけることが判定の前提でもあるから。格子なら長方形にも意味がある（96×16 には `[6, 2]`）。**地図を大きくしたら増やすもの**で、畑 1 つは半径 3・60/タイルで約 1,920 個 = 鎖 1 本ぶんに足りない |
| `ore :iron_ore` | 名前 | `:iron_ore` | **参照。地面が何でできているかで、それがそのまま掘って出てくる物**（著者、2026-09-21）。F3 までは名前がラベルで `miner` の `digs:` が出てくる物を言っていたが、それだと「何が出るか」が 2 か所にあり、`ore :coal` と `digs: :iron_ore` が黙って共存した。宣言されていないアイテムなら**その行で**断る |
| | `per_tile` | 60 | **導出**: 鉱石 1 タイルで**箱 1 杯**（すぐ上の `capacity` と同じ数で、同じ「1 分」である）。**F2a で `factory.settings.txt` から移した**ので、この導出は 1 つのファイルの中で閉じている。半分を切ると絵が「残り少ない」タイルに変わる |
| `inserter :arm` | `seconds_per_item` | 1.0 | **導出（F3）**: 毎秒 1 個は**満杯のベルトの 1/4** で、**採掘機 1 台の出力そのもの**。だから採掘機 1 台にインサータ 1 台、かまど 2 台にインサータ 1 台（かまどは毎秒 0.5 個食べる）、ベルト 1 本にインサータ 4 台で、鎖が整数のまま。語が `miner` と同じなのは**意味が同じ**だから（この物が 1 個にかける時間）。**`idle` の長さでもある** — 腕がもう振れない速さより頻繁に見ても何も起きないので、別の数ではない。取れる範囲は数にしていない（後ろから取って前に置く、が「向きのある 1 タイル」の意味） |
| `machine :furnace` | `size` / `sprite` / `speed` | `[1,1]` / `[109]` / 1.0 | `sprite` は引用（Kenney の銅のアーチ）。`speed` の 1.0 は**掛け算の単位元** — 「レシピの言う時間で動く」 |
| `machine :assembler` | `size` / `sprite` / `speed` | `[2,2]` / `[138,139,140,141]` / 1.0 | `size` は**導出**: かまど 4 台ぶんを食う機械はかまどより大きく見えるべき。`sprite` は `tools/factory-machine.py` が Kenney のタイル 99 を nine-slice で 2×2 に伸ばした 4 枚 |
| `recipe :iron_plate` | `time` | 2.0 | **導出**: かまど 1 台は毎秒 0.5 枚 = **満杯のベルトの 1/8**。つまり**採掘機 1 台でかまど 2 台**が回り、**かまど 8 台でベルト 1 本**が埋まる。1.0 にしなかったのは、自分の採掘機とちょうど釣り合うかまどは「置いて忘れる」ものになるから |
| `recipe :gear` | `time` | 1.0 | **導出**: 組立機 1 台は毎秒 2 枚食べる = **かまど 4 台ぶん** = 採掘機 2 台ぶん。出るのは毎秒 1 個 = ベルトの 1/4。鎖が全部整数になる: **採掘機 2 → かまど 4 → 組立機 1 → 毎秒 1 個** |
| `control.rb` の `goal` | `deliver: { gear: … }` | **60**（F4） | **導出**: 鎖 1 本（採掘機 2 → かまど 4 → 組立機 1）は**毎秒 1 個**なので、60 は**鎖 1 本を 1 分**回した数。`chest :crate, capacity: 60` と `ore … per_tile: 60` と同じ「1 分」で、**箱 1 杯ぶんの歯車**でもある。**遊び方の数なので `data.rb` ではなく `control.rb`**（目標は世界が何でできているかではない）。数え始めはスクリプトが走り始めた瞬間で、差し替えると数え直す |
| `item :*` | `icon` | 0 / 1 / 2 | 参照。`assets/items/items.png` の何枚目か（`tools/factory-items.py` が一覧を印刷する） |

**「隙間 N 個の列には N+1 個」は、書ける値の全部で厳密に成り立つ**（F2a）。ベルト上の位置が
1 タイル 16 刻みの整数になり、隙間が `16 ÷ items_per_tile` 刻みで割り切れる値しか書けなく
なったので、詰まったタイルの個数も位置も丸めに依らない（`belts.rs` の
`a_jam_holds_one_more_than_the_gaps_that_fit` が 1・2・4・8・16 の全部で個数と位置を
`assert_eq!` で見る）。**F1 の f32 ではそうならなかった** — 1・2・4・5・7・8 は N+1 個、
3 は 3 個で、どちらに転ぶかは「隙間が正確に表せるか」ではなく「引き算の連鎖の丸めがどちらへ
落ちたか」だった。経緯は `worklog/2026-09-21-factory-F1.md` §5.2 と
`…-F2a.md` §6。

**機械の緩衝は数ではない。** 機械は「1 回ぶんの材料が入るまで」受け取り、「作った物を出すまで」
次を始めない。どちらも**最小**（切れ目なく回るための下限）で、選んだ数ではない。F1 の採掘機の
「詰まったら掘り終わったまま待つ」と同じ規則である。

`belt_frame_seconds`（ベルトの 2 コマのアニメーションの周期）は**設定ではなく導出**:
`4 px ÷ (tiles_per_second × 16 px)`。パックは 8 px ごとにシェブロンを描き、2 コマ目はそれを
半周期ずらした絵なので、**4 px 進むごとに 1 回入れ替える**と 1 本の動くベルトに見える。

### 9.4 判定と計測の器の数 (d)

| 位置 | 名前 | 値 | 出どころ |
|---|---|---|---|
| `factory/src/main.rs` | `TILESET_WAIT_FRAMES` | 400 | **測った（2026-09-21）**: タイルセットが `Assets<Image>` に入ったのはコンテナで 3 フレーム目、ブラウザで 4 フレーム目。その 100 倍。**秒ではなくフレーム**なのは、待っているのが「asset server に順番が回ること」で、それが 1 フレームに 1 回だから（S7 の規則） |
| `factory/src/main.rs` | `PANEL_WAIT_FRAMES` | 6 | **導出（F3）**: パネルを動かす判定が待つものは 3 段で、いちばん遠いのは「VM がプログラムを 1 本多く持つこと」＝ボタンから 3 フレーム後（依頼が読まれる → 次のフレームで crew が差し替える → その次の頭で rubevy が読む）。6 はその倍。**秒ではなくフレーム**なのは待っているものが 1 フレームに 1 回起きるから（S7 の規則）。1 フレームで書いていたときは窓で通りブラウザで落ちた |
| `factory/src/main.rs` | `CONTROL_CHECKS` | `31..40` | **段の番号であって閾値ではない**（F4）: 判定は 2 つのシステムに分かれていて（Bevy の system は引数 16 個までで `selftest` がちょうど 16 個だった）、この範囲の step のあいだ `control_checks` が run を持つ |
| `factory/src/main.rs` | `F5_CHECKS` / `WINDOW_CHECKS` | `40..50` / `50..60` | **段の番号であって閾値ではない**（F5、`CONTROL_CHECKS` と同じ理由）: 判定は 4 つのシステムに分かれた。`WINDOW_CHECKS` の側は窓のあるときだけ動く（`Editor`・`Guide`・`Paused`・カメラの 4 つが `Option`） |
| `factory/src/main.rs` | `REBUILD_THE_WORLD_CHECK` | **47** | **順番であって閾値ではない**（F5）: 「`data.rb` を読み直して世界を組み直す」判定は判定の**いちばん最後**でなければならない — 判定自身の工場を道連れにするので。数そのものは `F5_CHECKS` の中の空いている step |
| `factory/src/main.rs` | `CAMERA_WAIT_FRAMES` | 600 | **導出（F5）**: 「勝つとカメラが動く」判定が待つのは、判定自身の線が箱にもう 1 つ入れること（掘り 1 回 + ベルト 4 タイル）で、それを窓の走行のフレームレートでフレーム数にしたもの。**上限であって所要時間ではない** — カメラが動いたフレームで通る |
| `factory/src/main.rs` | `CHECK_SLACK` | 2.0 | **導出**: 判定は「ゲーム自身の数が言う所要時間」を条件の上限に使う。2 倍は、前後 1 フレームの粒度と、1/60 秒でないブラウザのフレームのぶん。実測は 3.0 秒に対して 2.5 秒、3.5 秒に対して 3.5 秒、2.5 秒に対して 2.8 秒 |
| `factory/src/main.rs` | `STRESS_REPORTS` | 3 | **導出**: 最初の 1 回（窓が開き asset が届くフレームが入る）を捨てて、残り 2 回を見比べられる最小 |
| `factory/src/main.rs` | `Stress::every` | 60 | **導出**: 60 fps の 1 秒ぶん。p50 / p95 が 1 フレームのぶれでなくなる長さであり、3 回が数秒で終わる短さ |
| `factory/src/grid.rs` | 地図の辺の下限 = `Ore::smallest_map(patch_radius, patches)` | 15（半径 3・畑 2 のとき） | **導出（F2 で置き換えた。F1 の `8` は「不明」だった。F3a で畑の数を引数にした）**: 畑の中心は辺を `n` 等分したセルの中心、判定は**タイルの中心**との距離。円が横にいちばん広がるのは中心を通る行なので、縁の輪（タイル 0 と `tiles-1`）に掛からない十分条件は `tiles/(2n) − radius − 0.5 > 0`、すなわち **`tiles > 2 × n × (radius + 0.5)`**。その上の最小の整数が下限。**n = 2 を入れると F2 の `4r + 2`** — F2 の式に隠れていた 2 が「畑の数」だったと分かった。**計画書の `4 × (radius + 1)` = 16 ではなく 15**（`radius + 1` は半タイルを切り上げている）。**保証であって毎回ちょうどの最小ではない**（中心を通る行が実在しない半径では 1 タイル早く空く）。**横と縦に別々に当てる**（96×16 に `[6, 2]` なら 43 と 15）。単体テスト `the_smallest_map_is_the_smallest_map_the_patches_clear_the_border_on` が半径 7 × 畑 5 通りを実際に敷いて両方を確かめる |

### 9.5 素材の容量の上限

**64 KB、1 枚のシート + アイテムの絵 + 使ったパックごとのライセンス原文。** 出どころ: シートの実測が
**1 タイルあたり 67 B**（F0 の 145 タイルで 9,662 B、F1 の 139 タイルで 9,369 B、F2 の 142 タイルで
9,612 B、F3 の 143 タイルで 9,777 B = 68.4 B）なので、512 タイルのシートでも約 35 KB。
64 KB はその倍で、既存 2 本の小さい方（sabibots 234,202 B）の 27%。
今の実績は **11,326 B / 4 ファイル**（シート 9,777 + 8 px の絵 5 枚 414 + ライセンス 2 通 1,135）で、
上限の **17%**。詳しくは `docs/factory.md`。

### 9.5a ガイドのフォント（F5）

**81,480 B / 396 グリフ**（`crates/games-shell/assets/fonts/NotoSansJP-Guide.subset.ttf`）。
F5 の前は 66,372 B で、増えたのは Factory のガイドの日本語のぶん。
**上限ではなく実測**: `tools/subset-font.sh` が 4 つのガイドのファイルの非 ASCII 文字を集めて
その字だけを切り出すので、大きさは**文章が決める**。豆腐が無いことは cmap を読み返して確かめた
（4 ファイルの非 ASCII のうちフォントに無い字 0 件）。

### 9.6 決められなかった数 → **利用者が決める**（F3a）

**`map :world, size:`（既定 32×32、F0 の仮の値のまま）。**

**2026-09-21、著者「サイズは自由に変えられるようにしたい」** — 既定値をどうするか聞いたことへの
答えである。だから F3a がやったのは「決める」ことではなく「**利用者が変えられる形にする**」ことで、
それは CLAUDE.md の「根拠があっても、数は利用者が変えられる場所に置く」そのものである。今の状態:

* **どこで変えるか**: `ruby/data.rb` の `map :world, size: [w, h]` の 1 行。正方形でなくてよい。
  ページでも（`factory.settings.txt` はブラウザから書けないので、設定のままでは無理だった）。
* **既定値 32×32 は動かしていない**。出どころは **F0 の仮の値で、今も誰も測っていない**。
  著者の実機の数字（下記）が出たら見直す。**もっともらしい理由を後から作らない。**
* **下限と上限は「壊れる所」が言う**。下限は鉱石（畑が壁に掛かる）、上限は描画（1 タイル 1 テクセルの
  テクスチャ）。どちらも §9.1・§9.4 に出どころつきで書いてあり、黙って直さず `data.rb:行` で断る。
* **メモリは上限にしていない**（機械に依るので）。1 タイル **113 バイト**（PC）/ **69 バイト**（wasm、
  ポインタが 4 バイト）を `lay_the_land` が MB に直して log に 1 行出す。
* **遊べるかの目安**（F3a で測った、`worklog/…-F3a.md` §7）: 空の地図で `step` が
  32×32 で 4 µs、512×512 で 0.6 ms、2048×2048 で 5.9 ms。**タイル数に比例する毎フレームの仕事が
  2 か所ある**（§9 の気づいた点）ので、上限の近くは「起動はするが 1 フレームの 35% を食う」。

以下は F1〜F3 が「何が決まれば既定値が決まるか」を書き足してきた記録で、**そのまま残す**
（既定値を動かすときに読むもの）。計画書 §3.7 は「上限の機械が無理なく置ける広さから」と
言っていて、F1 も F2 もそこに届かなかった。足りないのは 2 つ:

1. **描画が運べる量**。工場の計算のほうは測れる（16,000 個で 0.3 ms、4,000 個で 56 µs）が、
   先に尽きるのは絵のほうで、**この機械にある描画系は 2 つともソフトウェア**である。
   実機の GPU で測るまで、ここから地図の大きさは導けない。
2. **スクリプト付きインサータの上限**（計画書 §3.7 の 2 行目）。F3 の計測。

F2 が 1 つ材料を足した: **鎖 1 本は「採掘機 2・かまど 4・組立機 1」で毎秒 1 個**（§9.3）なので、
満杯のベルト 1 本ぶんの製品は**その 4 倍 = 28 台**である。

**F3 が 2 つめの材料を出し、そのうえで 2 番目の不明を消した。**

* 鎖 1 本にインサータは **10 台**（かまど 4 台 × 出入り 2 + 組立機 1 台 × 2。採掘機はベルトに直接
  出すので要らない）。機械 7 + 腕 10 + ベルトで **1 鎖およそ 30 タイル**なので、既定の 32×32
  = 1,024 タイルは **鎖 34 本＝腕およそ 340 台**まで。
* **スクリプト付きインサータの上限は、もう工場の上限ではない**（§9.8）。散らした状態の 3,000 台で
  VM の 1 フレームは命令 14,040・tick 2.2 ms（p95）で、`frame_time` の 8 ms にも予算にも遠い。
  つまり計画書 §3.7 の 2 行目（「スクリプト付きインサータの上限」）は**地図の大きさを縛っていない**。

**残る不明は 1 つだけ、描画である。** この機械の描画系は 2 つともソフトウェアなので、実機の GPU で
測るまでここから「快適に遊べる地図」は導けない（**描ける上限**のほうは F3a が実物で決めた: §9.1）。
**もっともらしい数を今作らない**（CLAUDE.md）。**既定の 32 は動かしていない** —
著者の機械のブラウザで `?stress` を 1 度回してもらえれば、既定値を動かすかどうかが決まる。

### 9.7 件数

Factory は **58 件**（不変量 (a) 10 行 27 個、動かす側 (c) **11**、**遊びの数 (b) 17**（`data.rb` 16 +
**`control.rb` の目標 1**）、判定と計測の器 (d) **11**、導出で設定でないもの **2**、容量 **2**）。
出どころは **測った 3**（`TILESET_WAIT_FRAMES`・`script_budget`・そこから「そのままでよい」と
言えた `script_frame_time_ms`）+ **測って裏を取った 1**（`MOST_TILES_ACROSS`。導出 + 2 つの描画系の実測）
/ 導出 35 / 引用 5 / **不明 1**（窓の大きさ）/
**根拠なしと正直に書いたもの 1**（`belt_tiles_per_second`）/ **利用者が決めるもの 1**（地図の大きさ。
既定は F0 の仮の値のまま。§9.6）/ 参照 2（`ore` の名前、`icon:`）。

**F5 が足したもの**: 不変量に **1 件**（`SAVE_VERSION`）、判定と計測の器に **4 件**
（`F5_CHECKS`・`WINDOW_CHECKS`・`REBUILD_THE_WORLD_CHECK`・`CAMERA_WAIT_FRAMES`）、容量に 1 件
（ガイドのフォント、§9.5a）。**遊びの数は 1 つも足していない** — 窓は数を持たない
（パネルの大きさは `games_shell::PanelSettingsPlugin` が `factory.settings.txt` から読む既存の鍵で、
3 本に共通の `editor_*` / `vm_*` / `guide_*` / `hud_*`）。**動かす側に足したのは名前だけ**:
`save_file`（既定 `factory.save.json`）は「どこへ書くか」であって調整値ではない。
`Restoring::patience`（1 秒）は**導出**: ロード後にスクリプトが `run` に着くのは数フレーム後で、
待っているのは「着かないスクリプトがある」場合の打ち切り。数フレームより長く、人が気づくより短い。

**F4 が足したもの**: 遊びの数に **1 件**（`control.rb` の `goal deliver:`。**3 本目の Ruby のファイル**で、
`data.rb` と同じ「遊びの数は Ruby」の側にある）、不変量に 1 件（`WRAPPER_LINES`）、判定の器に 1 件
（`CONTROL_CHECKS`）。**F4 が動かした既定値は `headless_seconds` の 1 件だけ**（30 → 66。判定が
3 つ伸びたぶん、上限の和から）。**イベントの `limit:` は足していない** — 計画 §3.7 は「control が
1 フレームに受けうる最大を測って決める。既定の 64 で足りるならそのまま、と書く」と言っていて、
**足りる**（1 フレームに種類ごと 1 件しか流さない形にしたので、天井に当たるには `data.rb` が
64 種類以上のアイテムか機械を宣言するしかない）。腕 3,400 台の走行でも `dropped` は 0。

**F3a が動かしたもの**: 動かす側から **2 件が遊びの数へ**（`map_tiles` → `map :world, size:`、
`ore_patch_radius` → `ore … patch_radius:`）、遊びの数に **1 件追加**（`patches`）、
不変量に **1 件追加**（`MOST_TILES_ACROSS`）。**既定値は 1 つも動かしていない。**

**F3 が足したもの**: 遊びの数に 1 件（`inserter :arm, seconds_per_item:`、導出）、動かす側に 4 件
（`stress_arms`・`inserter_stagger`・`script_budget`・`script_frame_time_ms`。**後ろの 2 つは
rubevy の既定を継ぐのをやめてこのゲームが言うようにしたもの**で、§9.8 がその測定）、
判定の器に 1 件（`PANEL_WAIT_FRAMES`）、不変量に 5 個（絵の添字と z）。
**F3 が動かした既定値は `headless_seconds` の 1 件だけ**（20 → 30。判定が伸びたぶん、上限の和から）。

**F2 で「動かす側」から「遊びの数」へ 4 件移った**（`belt_tiles_per_second`・`items_per_tile`・
`mine_seconds`・`chest_capacity`）。**F2a でもう 1 件**（`ore_per_tile` → `ore :iron_ore, per_tile:`）。
`ore_patch_radius` だけが残っている（§9.2 の最後）。
移ったぶん **Rust の側には遊びの数の既定値が 1 つも無くなった** — `data.rb` を読むまでゲームは
始まらない。**F2a で書ける値が狭くなった数が 1 つある**: `items_per_tile` は 16 の約数だけで、
それ以外は行つきで断られる。狭めたのは「隙間が割り切れないと隙間でなくなる」からで、
断り文が**書ける値を並べて言う**（1, 2, 4, 8, 16）ところまでが 1 組である。

F1 の終わりの見直しで**消えた数が 3 つ**ある: `Rules::spacing` と `Rules::belt_frame_seconds` と
`draw::snapped` の `.max(0.001)`。どれも「0 で割らないため」だけの数で出どころが無く、
0 以下を読み込み時に断る形に移した。F2 で**もう 1 つ消えた**: `main()` の
`rules.items_per_tile.max(0.001)`（stress の地図の大きさ）。読み込み時に断る側が `data.rb` の
検査になったので、0 以下がここへ来る道がそもそも無い。

### 9.8 スクリプトの予算を測って決めた（F3）

計画書 §3.7 の「命令予算（`budget`）と `frame_time`」の行。手順は rubevy 自身が書いているもの
（`docs/host-api.md`「Choosing `budget` and `frame_time`」）で、**誰かが気に入った倍率は出てこない**。

**器**: `--arms N`（§9.2）。1 台ぶんが「中身入りの箱・腕・空の箱」なので**全部の腕に仕事がある**。
`--headless 12` を 2 巡、最初の 1 回の報告（asset が届くフレームが入る）を捨てて残り 2 回、
命令数は**同じ巡の最小**、p95 は**同じ巡の最大**で取った（共有された機械なので。F2a の作法）。
release、窓なし、2026-09-21。

| 腕 | 散らす | 命令 p50 | 命令 p95 | tick µs p50 | tick µs p95 | 1 フレームの答え p95 | 持ち越し p95 | 落とした |
|---|---|---|---|---|---|---|---|---|
| 100 | する | 216 | 864 | 51 | 125 | 8 | 0 | 0 |
| 100 | **しない** | **0** | **0** | 2 | 3 | 0 | 0 | 0 |
| 300 | する | 1,080 | 1,944 | 95 | 178 | 18 | 0 | 0 |
| 300 | **しない** | **0** | **0** | 2 | 4 | 0 | 0 | 0 |
| 1,000 | する | 3,672 | 4,752 | 263 | 534 | 44 | 0 | 0 |
| 1,000 | **しない** | **0** | **0** | 2 | 9 | 0 | 0 | 0 |
| 3,000 | する | 10,584 | 14,040 | 1,102 | 2,169 | 130 | **0** | 0 |
| 3,000 | **しない** | **0** | **92,589** | 2 | **8,018** | **680** | **1,603** | 0 |

**散らさないと何が起きるかが、この表の全部である。** 1 秒に 1 回、全台が同じフレームで起きる:
中央値は 0 命令（何も起きていない）で、95 番目のフレームが **92,589 命令・8.0 ms**。8.0 ms は
`frame_time` そのもので、**tick が時間切れで打ち切られている**ということであり、
**持ち越し 1,603 件**がその証拠である（次のフレームの頭に回された問い）。散らすと同じ仕事が
**14,040 命令・2.2 ms** に均される。rubevy が「3,000 台・同じ `sleep` で p95 19.5 ms」と測ったのと
同じ現象を、**このゲームの形で測り直した**ことになる。

**予算の決め方（rubevy の 3 歩）**:

1. **速さを測る**: 3,000 台・散らす で 10,800 命令 / 1,102 µs = **毎 ms 約 9,800 命令**
   （1,000 台では 13,800。**忙しいほうの数**を使う）。箱庭の規則が 9,300 だったので、同じ種類の
   Ruby が同じ種類のことをしている、という読みと合う。
2. **フレームの取り分を決める**: 60 Hz の 1 フレームは 16.7 ms、VM は 1 本、**1/4 = 4.2 ms**。
3. **その時間が買う命令数**: 4 ms × 9,800 = **39,000**。

**`script_budget = 39,000`、`script_frame_time_ms = 8.0`**（rubevy の既定のまま）。
**予算が先に噛み、時計は余裕のある番人**という向きで、理由は rubevy の文書どおり:
命令数は*スクリプトについての事実*で、数倍遅い機械でもブラウザでも同じ数になる。
39,000 は 9,800/ms で約 4.0 ms、8 ms は約 78,000 命令なので、**時計は予算の 2 倍**の所にいる。

**利用者に残る余地**: 既定の地図が持てる 340 台ぶんで最悪のフレームは約 2,000 命令なので、
**出荷するスクリプトの 20 倍**が利用者のものである。3,000 台（どの地図も持てない数）でも 2.8 倍。
**噛んだのを 1 回だけ見た**: 3,000 台の走行の**最初の報告**に 39,045 命令のフレームが 1 つあった
（全台が起動するフレーム）。予算が仕事をして、残りは次のフレームに回った — 正しい振る舞いである。

---

## 10. S11（2026-09-22）— 設定が断る数と、断らずに縛っている数

計画書 §6 の S11 (2)。**既定値は 1 つも動いていない。** 動いたのは「店が書いた値を黙って別の数に
すり替えていた所」で、そこは既定値が立ち、断りが log に出るようになった
（`games_shell::Settings::positive` / `counted`、`SettingsRefusalsPlugin`）。
拾い方と全一覧は `docs/worklog/2026-09-22-s11.md` §2。

### 10.1 床そのものは数である — どこに置いたか

`counted(key, least, default)` の `least` は**呼ぶ側の数**である。同じ「個数」でも床の理由が違う:

| 床 | どこ | 理由（コードのコメント） |
|---|---|---|
| 0 | `start_plants` / `start_beetles` / `start_rabbits` / `start_trees` / `start_rocks`、`edge_trees`、`separate_passes`、`look_gait_blend_ms`、`hud_bar_ticks`、`script_budget`（3 本）、`world_script_budget`、`stress_items` / `stress_arms` | **無いことは頼める**（岩の無い庭、目盛りの無いバー、何も走らせない VM）。頼めないのは 1.5 個と −3 個 |
| 1 | `light_shadow_cascades`、`start_tries`、`look_floor_pattern`、`window_width` / `window_height`（Battle と Factory） | 0 だと物そのものが無くなる（影の無い影、`Vec2::ZERO` に置かれる生き物、0 で割る床の模様、幅 0 の窓）|
| 2 | `sky_rings` | ドームの輪 |
| 3 | `sky_sides` | 多角形の辺。**S11 まではここまで黙って持ち上げていた** |

`positive(key, default)`（0 より大きい）: `checks_instructions_a_frame`、`camera_half_height`、
3 本の `headless_seconds` と `shot_seconds`。

### 10.2 断らずに縛っている数（S11 は意味を変えていない）

| 数 | どこ | 縛り | 出どころ・理由 |
|---|---|---|---|
| `night` の範囲 | `garden/src/main.rs` | `.clamp(light_dial_min, light_dial_max)` | **理由あり**: ダイヤルの範囲そのもの。ダイヤルが表示できない値を店から入れても意味が無い |
| `--eye` の範囲 | `garden/src/main.rs` | `.clamp(eye_at.zoom_min, eye_at.zoom_max)` | **理由あり**: カメラ自身の範囲。店ではなく引数 |
| `inserter_stagger` の床 0 | `factory/src/main.rs` | `.max(0.0)` | **理由あり**（S11 でコメントに書いた）: 負の散らしと散らし無しは同じ走行なので、すり替えたものが無い |
| `script_frame_time_ms` ≤ 0 | 3 本 | `None`（時計を持たない） | **理由あり**: rubevy の意味のある指定。断ってはいけない |
| `vm_regs_frames` / `vm_value_chars` / `code_rows` / `code_chars` の床 | `crates/rubevy-egui` | `.max(0.0)` / `.max(1.0)` | **口が届かない**: `rubevy-egui` は設計上 `Settings` を知らない（S4a）。S11 は触っていない |

### 10.3 出どころ不明のまま残った数（S11 が触った行の隣にあるもの）

| 数 | どこ | 何 | 出どころ |
|---|---|---|---|
| 2.0 秒 | `garden/src/main.rs` の `take_shot` | 絵を頼んでからファイルが書かれるまで待つ猶予 | **不明**。3 本で 2.0 / 1.0 / 1.0 と違うが、違う理由はどこにも書かれていない。S11 は動かしていない |
| 1.0 秒 | `sabibots/src/main.rs`、`factory/src/main.rs` の `take_shot` | 同上 | **不明** |
