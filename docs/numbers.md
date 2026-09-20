# 数の一覧 — rubevy_games が今持っている数と、その出どころ

計画書 `docs/plans/shared-crate-plan.md` の段階 **S5a**。コードは 1 行も変えていない。
対象は `crates/rubevy-egui/src/` と `crates/games-shell/src/`（S5a を取った時点ではどちらも `crates/rubevy-arena/src/` だった。
S4a で 2 つに割れた）、`garden/src/`、`sabibots/src/`、`garden/ruby/**/*.rb`、`sabibots/ruby/**/*.rb`。
行番号は `numbers` ブランチ（main `b1ce042` から分岐）の時点のもの。
拾い方と選び方の過程は `docs/worklog/2026-09-20-numbers-inventory.md`。

**これは提案であって決定ではない。** 著者が見てから S5b で移す。

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

> **S4a（2026-09-20）で `crates/rubevy-arena` は 2 つに割れた。** この節の表の `位置` 欄は S5a を取った
> 時点のもので、ファイル名はそのままだが置き場所と行番号は動いている。`editor.rs` / `inspect.rs` /
> `code.rs` は `crates/rubevy-egui/src/`、`lib.rs`（`ArenaPlugin`）/ `camera.rs` / `guide.rs` /
> `settings.rs` / `platform.rs` / `args.rs` / `checks.rs` / `hud.rs` は `crates/games-shell/src/`。
> S5b で数を動かすときに行番号ごと取り直す。

### 1.1 カメラと画面

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `ArenaSize::default` | `lib.rs:43` | 32.0 | | `ArenaPlugin { size }` で app が渡せる | (e) → ただし sabibots は `default()` のまま（`sabibots/src/main.rs:302`） | **不明** |
| 壁の外に見せる床 | `lib.rs:93, 115` | `+ 3.0` | | `const` ですらない（式に直書き） | (c) `ArenaPlugin` の設定値へ | *理由のみ* 「a little floor past the wall, so what stands at the edge is not cut off」`lib.rs:92` |
| 窓が無いときの縦横比 | `lib.rs:114` | 16/9 | | 直書き | (c) | *理由のみ*（`docs/sabiruby-battle.md:646`「The window is 16:9 and the arena square」） |
| エディタを開いたときカメラをずらす幅 | `lib.rs:118` | 画面幅の 0.16 | | 直書き | (c) — 計画 S3 が「塞がれ px の Resource」に置き換える | *理由のみ* 「the editor is about a third of the window on the right」`lib.rs:117`。**1/3 と 0.16 が合っていない**（§7-4） |

### 1.2 エディタ（`editor.rs`）

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `MARGIN` | `editor.rs:426` | 8.0 | | `pub const` | (c) | 名前付きである理由は書いてある（窓のチェックが矩形を計算できるように、`editor.rs:419-425`）。値は **不明** |
| `WIDTH` | `editor.rs:427` | 520.0 | | `pub const` | (c) | 同上・**不明** |
| `HEIGHT` | `editor.rs:428` | 640.0 | | `pub const` | (c) | 同上・**不明** |
| `FONT` | `editor.rs:417` | 13.0 | | `const` | (c) | **不明** |
| `KINDS`（9 色） | `editor.rs:454-464` | `(210,214,222)` ほか 8 色 | ● | `const` | (c) 表示。ただし出どころは強い | **測った**: WCAG コントラスト地 ≥4.96 / 帯 ≥3.09、CIE76 ΔE ≥25.9。`editor.rs:430-453` と `docs/worklog/2026-09-18-editor-highlight.md` |
| 熱の帯の色 | `editor.rs:507` | `(150,110,20)` | ● | 直書き | (c) | **引用**: 合成後の (103,77,17) が 9 色を測るときの前提になっている（`editor.rs:440-441`） |
| 熱の帯の最大 α | `editor.rs:506` | 170 | ● | 直書き | (c) | **不明**（170 という数自体の根拠。色の方はこの 170 を前提に測っている） |
| 熱を描き始める下限 | `editor.rs:505` | 最も熱い行の 0.1 | ● | 直書き | (c) | *理由のみ* 「a line passed through once is not a place」`editor.rs:504` |
| 琥珀（`* edited` と「未適用」） | `editor.rs:324` | `(240,190,90)` | ● | 直書き | (c) | *理由のみ* 「暖色は熱のもの」（`editor.rs:445-447`） |
| 行番号の桁の色 | `editor.rs:401` | `(120,128,140)` | ● | 直書き | (a) 分類 3（コメント）と同じ値であることが意味（`editor.rs:435-436`） | **導出**: `editor.rs:458` と同じ数 |
| ボタンの余白と字の大きさ | `editor.rs:341-342, 349, 367` | 10/5, 6.0, 16.0, 8/4 | ● | 直書き | (c) | *理由のみ* 「big enough to hit without aiming」`editor.rs:340` |

### 1.3 VM パネル（`inspect.rs`）

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `REGS_FRAMES` | `inspect.rs:29` | 6 | ● | `pub const` | (c) | *理由のみ* 「A brain waiting for the game stands about three frames deep in the DSL, so six reaches its own code as well」`inspect.rs:27-28` |
| 命令数/フレームの平滑化 | `inspect.rs:264` | 0.95 / 0.05 | ● | 直書き | (c) | **不明** |
| VM の ms の平滑化 | `inspect.rs:533` | 0.8 / 0.2 | ● | 直書き | (c) | **不明** |
| ログに出すフレーム数 | `inspect.rs:410` | 4 | | 直書き | (c) | **不明** |
| クラス名を切る長さ | `inspect.rs:480, 483` | 40 / 39 | ● | 直書き | (c) | **不明** |
| `LIMIT`（値の表示を切る長さ） | `inspect.rs:564` | 52 | ● | `const` | (c) | **不明** |
| `FONT` | `inspect.rs:555` | 12.0 | | `const` | (c) | **不明** |
| `PANEL_HEIGHT` / `FRAMES_HEIGHT` / `REGS_HEIGHT` | `inspect.rs:557-559` | 440 / 148 / 150 | | `const` | (c) | **不明** |
| パネルの幅 | `inspect.rs:596` | 640.0（窓幅 − 16 が上限） | ● | 直書き | (c) | **不明** |

### 1.4 説明（`guide.rs`）と HUD

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| 説明の窓の大きさ | `guide.rs:283-284` | `[640, 820]`、最大 820→860 | ● | 直書き | (c) | **引用**: G6 の絵で下が切れていたので広げた（`docs/plans/garden-plan.md:502-505`「2 枚目ではキー表が下で切れていたので `default_size([640, 820])` に」）。測った記録ではない |
| キーの色 `KEYCOL` | `guide.rs:244` | `(255,226,150)` | ● | `const` | (c) | **不明** |
| 日本語フォント（部分集合） | `guide.rs:56` | 327 文字・62,780 バイト | | ビルド時（`tools/subset-font.sh`） | (a) 本文と一緒に切らないと字が欠ける | **測った**（`docs/plans/garden-plan.md:498`。9,589,900 → 62,780 バイト） |
| HUD の予算バーの目盛 | `hud.rs:120-121` | 16 | ● | 直書き | (c) | **不明** |
| HUD の字の大きさ | `hud.rs:65, 109` | 15.0 / 13.0 | | 直書き | (c) | **不明** |
| HUD の余白 | `hud.rs:52-56` | 8 / 8 / 8 / 3 | | 直書き | (c) | **不明** |

### 1.5 コードパネル（`code.rs`、**今どのゲームも使っていない**）

| 名前 / 数 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `ROWS` | `code.rs:50` | 28 | ● | `const` | (c) | **不明** |
| `WIDTH`（切る文字数） | `code.rs:149` | 44 | ● | `const` | (c) | *理由のみ*、**かつ数が合っていない**: 「430px of a monospace 12px font: about 58 characters」`code.rs:147` に対して 44（§7-1） |
| パネルの位置と幅 | `code.rs:67-69` | 上 120 / 右 8 / 幅 430 | | 直書き | (c) | *理由のみ* 「below the HUD rows, so the two do not overlap」`code.rs:66` |

---

## 2. Garden（Rust）

### 2.1 場

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `FIELD_W` | `main.rs:62` | 40.0 | | `const` | (c) 起動の引数か `Settings` | **引用**: 計画書 `docs/plans/garden-plan.md:51`「40×30 マスの草地」。1 単位 ≈ ウサギ 1 匹（`main.rs:61`） |
| `FIELD_D` | `main.rs:63` | 30.0 | | `const` | (c) | **引用**: 同上 |
| `HALF_W` / `HALF_D` | `main.rs:64-65` | 20.0 / 15.0 | ● | 導出 | (a) 上の 2 つから導いているので独立の数ではない | **導出**: `FIELD_W / 2.0` |
| 壁の内側の余白 | `main.rs:3277-3283, 3296` | 0.5 | ● | 直書き | (c) | **不明**（`move_creatures` と `separate` の両方が同じ 0.5 で clamp する） |

### 2.2 昼夜と光

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `DAY_LENGTH` | `main.rs:74` | 60.0 秒 | ● | `world.rb:21` の `day_length` が上書きする（`garden.rules`）。この数は「規則が黙っているときの既定」 | (e)＋(c) の既定値 | **引用**: `main.rs:67-73`「It is the default now, not the rule (W1)」 |
| `DAWN_OFFSET` | `main.rs:77` | 0.08 | ● | `const` | (c) | *理由のみ* 「the first thing a run sees is daylight」`main.rs:75-76`。0.08 という数自体は**不明** |
| `MIDNIGHT` | `main.rs:83` | `(0.75 − 0.08) × 60` = 40.2 s | | 導出 | (a) `--at midnight` が指す時刻。位相 0.75 が太陽の最下点なので式が壊れると別の時刻になる | **導出**（`main.rs:79-82` に式ごと書いてある） |
| `MOON_LUX` | `main.rs:111` | 950.0 | ● | `NightDial`（`Settings` の `night`）が掛かる | (c) の既定値（倍率は既に (e)） | **測った**: 真夜中の地面の平均輝度 44/255（目標 40〜50、G6 は 26、午後は 81）。`main.rs:85-104`、`docs/plans/garden-plan.md:515, 529` |
| `NIGHT_AMBIENT` | `main.rs:112` | 190.0 | ● | 同上 | (c) の既定値 | **測った**（同上。「110 to 190 moved the mean by 1.9」`main.rs:106-107`） |
| `NIGHT_SKY` | `main.rs:115` | `[0.14, 0.18, 0.36]` | ● | 同上 | (c) の既定値 | **測った**（`docs/plans/garden-plan.md:530`。G6 の `0.06,0.08,0.17` から比のまま） |
| `NightDial` の既定 | `main.rs:135` | 1.0 | ● | `Settings` の `night`（スライダ） | **(e)** | **引用**: `main.rs:117-129`（著者に数を尋ねるための仕掛け） |
| `NIGHT_DIAL_MIN` / `MAX` | `main.rs:141-142` | 0.5 / 2.0 | | `pub const` | (c) | *理由のみ* 「Half the night is still a night; twice it is the author saying the screen is darker than ours」`main.rs:139-140` |
| 昼の照度 | `main.rs:3152` | `1200 + 9000 × noon` | ● | 直書き | (c) | *理由のみ*（`main.rs:98`「the day's floor（1,200 lux, the sun on the horizon）」）。9000 は**不明** |
| 昼の環境光 | `main.rs:3168` | `120 + 260 × height` | ● | 直書き | (c) | **不明** |
| 太陽・月の距離 | `main.rs:3156` | 60.0 | ● | 直書き | (a) 平行光なので距離は影のカスケードに効くだけ | **不明** |
| 太陽の照度（`DirectionalLight`） | `main.rs:2202` | 8,000.0 | | 直書き（起動時） | (c) | **不明**（毎フレーム `day_night` が上書きするので初期値だけ） |
| 影のカスケード | `main.rs:2204-2206` | 2 / 24.0 / 70.0 | | 直書き | (c) | **不明** |
| 太陽の高さから昼夜を決める境 | `main.rs:3140` | `height <= 0.0` | ● | 直書き | (a) 「地平線の下＝夜」の定義そのもの | **導出** |

### 2.3 地平線と空（G8）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `HORIZON_HALF` | `main.rs:158` | 300.0 | | `const` | (c) | *理由のみ*（カメラに追従するので「常にこの距離」`main.rs:153-157`）。300 自体は**不明** |
| `HORIZON_DROP` | `main.rs:160` | −0.02 | | `const` | (a) 2 枚の平面が画素を取り合わないための隙間。0 にすると z ファイティング | *理由のみ*（`main.rs:159`） |
| `SKY_RADIUS` | `main.rs:167` | 500.0 | | `const` | (c) | **不明** |
| `SKY_SIDES` / `SKY_RINGS` | `main.rs:168-169` | 12 / 4 | | `const` | (c) | **導出の記述あり**: 84 三角形・49 頂点（`main.rs:162-166`）。12×4 を選んだ理由は**不明** |
| `SKY_FLOOR` | `main.rs:172` | −0.30 | | `const` | (a) 地面が描く地平線より下でなければ空の縁が見える | *理由のみ*（`main.rs:170-171`） |
| `NIGHT_ZENITH` | `main.rs:175` | 0.55 | ● | `const` | (c) | *理由のみ*（縁は測った `NIGHT_SKY`、勾配はその上、`main.rs:173-174`）。0.55 は**不明** |
| `FOG_NEAR` | `main.rs:191` | 14.0 | ● | `const` | (c) | **導出**: 40×30 の遠い隅は中心より約 14 単位遠い（`main.rs:177-182`） |
| `FOG_DEPTH` | `main.rs:192` | 45.0 | ● | `const` | (c) | **測った**: 既定のカメラは 42 単位・49° 見下ろしで、画面の最遠は 72 単位、畑の隅は 57 — 奥行きは 15 単位しかない（`main.rs:184-190`） |
| `FOG_NEAR_MAX` | `main.rs:193` | 120.0 | ● | `const` | (a) `HORIZON_HALF` の内側に霧の終わりを収めるための上限 | *理由のみ*（`main.rs:181-182`）。120 という数は**不明** |
| 霧の色 | `main.rs:2898-2899` | `(0.35,0.5,0.7)`、60→220 | | 直書き（起動時） | (c) | **不明**（毎フレーム `main.rs:3236` が上書き） |
| `EDGE_TREES` | `main.rs:199` | 16 | | `const` | (c) | *理由のみ* 「the edge of the field reads as the edge of the scenery rather than as a fence」`main.rs:195-198`。16 は**不明** |
| 縁の木の散らし方 | `main.rs:2096-2104` | ±0.4 / 4〜15 / 1.7〜3.1 | | 直書き | (c) | **不明** |

### 2.4 見た目

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `RABBIT_TINT` / `BEETLE_TINT` | `main.rs:206-207` | `(0.93,0.90,0.78)` / `(0.14,0.48,0.56)` | | `pub const` | (a) モデルと HUD が同じ数を使うことが意味（`window::species_color`） | **引用**: `main.rs:201-205`（G8、著者「ウサギと甲虫が見分けられない」） |
| `WORLD_COLOR` | `window.rs:225` | `(210,214,222)` | ● | `const` | (a) エディタの本文色（`editor.rs:455`）と同じであることが意味 | **引用**: `window.rs:218-224` |
| 地面の色 | `main.rs:2029` | `(0.36,0.46,0.25)` | | 直書き | (c) | **不明** |
| 空腹バーの色と境 | `window.rs:1057-1062` | 20.0 / 55.0 | ● | 直書き | (c)。55.0 は甲虫の `hungry_below`（`beetle.rb:10`）と同じ数だが繋がっていない（§7-6） | **不明** |
| 空腹バーの大きさ | `window.rs:1066` | 64×11 | ● | 直書き | (c) | **不明** |
| 草のモデルの倍率 | `main.rs:2509` | 2.6 / 2.2 | | 直書き | (c) | **不明** |
| 木・岩のモデルの倍率 | `main.rs:2528, 2546` | 2.2 / 3.4×3.0 | | 直書き | (c) | **不明** |
| 生き物のモデルの倍率 | `main.rs:2589-2590` | 0.55 / 0.75 | | 直書き | (c) | **不明** |
| `BEETLE_MODEL` | `main.rs:222` | `animal-crab.glb` | | `const` | (c)（数ではないが「著者が替えるもの」として書いてある） | **引用**: `main.rs:216-221` |
| 歩き/食べのアニメの切り替え速度 | `main.rs:5543` | 0.2 | ● | 直書き | (c) | **不明** |
| アニメの遷移時間 | `main.rs:5557` | 180 ms | ● | 直書き | (c) | **不明** |

### 2.5 世界の家具（生成時に 1 度だけ）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `PLANTS_AT_START` | `main.rs:231` | 55 | | `const` | (c) | **不明**（`main.rs:224-230` は「家具として残した」ことを言うだけで、55 の理由は書いていない） |
| `PLANT_MIN` | `main.rs:232` | 0.18 | | `const` | (c) | **不明** |
| `PLANT_MAX` | `main.rs:236` | 1.4 | | `const`。**`world.rb:31` の `plant_max` が同じ数を別に持つ** | (c) の既定値 | **引用**: `main.rs:233-235`（2 か所に同じ事実がある理由は書いてある） |
| `BEETLES` / `RABBITS` | `main.rs:239-240` | 6 / 4 | | `const` | (c) | **不明** |
| `TREES` / `ROCKS` | `main.rs:313-314` | 7 / 9 | | `const` | (c) | **引用**（数だけ）: `docs/plans/garden-plan.md:89`「木 7・岩 9」。選んだ理由は**不明** |
| 生成時の空腹の散らし | `main.rs:2308` | 45〜90 | | 直書き | (c) | **不明** |
| 初期配置の間隔 | `main.rs:2255, 2278, 2298, 2301-2302` | 1.5 / 1.2 / 2.5 / 0.6 / 2.0 | | 直書き | (c) | **引用**（1.5 のみ）: `docs/plans/garden-plan.md:93`「岩どうしは押し戻さないので初期配置の側で 1.5 以上離している」。残りは**不明** |
| 配置をやり直す回数 | `main.rs:2253, 2296` | 40 | | 直書き | (c) | **不明** |
| 草が丸い株になる確率 | `main.rs:2282, 3529, 5256` | 0.35 | | 直書き（3 か所に同じ数） | (c) | **不明** |
| 岩の大きさ | `main.rs:2266, 5262` | 0.8〜1.25 | | 直書き | (c) | **不明** |
| 新しい芽の最小間隔 | `main.rs:3526` | 1.5 | ● | 直書き | (b) 規則の数なのに Rust に残っている（§8-1） | **不明** |

### 2.6 規則の既定値（`world.rb` が黙っているときに使われる）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `HUNGER_MAX` | `main.rs:244` | 100.0 | ● | `const`。規則の方は `world.rb:63` | (a) HUD のバーの「満」。`world.rb` の `hunger_max` と食い違うとバーが嘘をつく | **引用**: `main.rs:241-243` |
| `REACH` | `main.rs:252` | 1.1 | ● | `world.rb:69` の `reach` が `garden.rules` で上書きする | (e)＋既定値 | **引用**: `main.rs:245-251` |
| `TOUCH_REACH` | `main.rs:253` | 1.3 | ● | `const`。`startle` は Rust に残った規則 | (b) 遊びの数だが Ruby に道が無い | **不明**（1.3 という数の理由） |
| `MATE_REACH` | `main.rs:260` | 2.0 | ● | `world.rb:74` が上書き | (e)＋既定値 | **測った**: 満腹な同種 2 匹の最接近は 1.24、半径の和は 0.80 だったので「1 体分の距離 (2.0) 以内」にした。`docs/plans/garden-plan.md:243-244` |
| `CHILD_HUNGER` | `main.rs:299` | 50.0 | | `world.rb:84` が上書き | (e)＋既定値 | **引用**: `main.rs:289-298` |
| `POP_MAX` | `main.rs:305` | 24 | ● | `world.rb:86` が上書き | (e)＋既定値 | **引用**: `main.rs:300-304`。予算の実測もこの上限に立っている（`main.rs:4292`） |

### 2.7 当たり

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `BEETLE_RADIUS` | `main.rs:309` | 0.40 | ● | `const` | (b) 体の寸法は遊びの数 | **不明** |
| `RABBIT_RADIUS` | `main.rs:310` | 0.50 | ● | `const` | (b) | **不明** |
| `TREE_RADIUS` | `main.rs:311` | 0.70 | ● | `const` | (b) | **不明** |
| `ROCK_RADIUS` | `main.rs:312` | 0.60 | ● | `const` | (b) | **不明** |
| `CELL` | `main.rs:317` | 1.6 | ● | `const` | (a) 最大半径の 2 倍以上でなければ近傍を取りこぼす | **導出**: `main.rs:315-316`（`2 × 0.70 = 1.4 ≤ 1.6`）。`docs/plans/garden-plan.md:90` も「1.6 単位のマスの格子」 |
| `SEPARATE_PASSES` | `main.rs:320` | 4 | ● | `const` | (c) | **測った**: 3 回の 90 秒走行で最接近 0.974 / 0.998 / 1.000、0.9 を下回ったフレーム 0（`docs/plans/garden-plan.md:92`）。理由の文は `main.rs:318-319` |
| 押し分けの取り分 | `main.rs:3400` | 0.5 / 0.5 | ● | 直書き | (a) 生き物どうしは半分ずつ、木と岩は不動 — 規則の定義 | **引用**: `docs/plans/garden-plan.md:64` |
| `Collider` の既定半径 | `docs/garden.md:280` の例 | 0.4 | | — | — | 文書の例。数の実体は上の 4 つ |

### 2.8 カメラ（3D オービット）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `Orbit::default` の pitch | `main.rs:1185` | 0.85 rad（≈49°） | | 直書き | (c) | **引用**（値ではなく効果）: `main.rs:185`「looks down at 49°」— 霧の深さはこの角度を前提に測った |
| `Orbit::default` の distance | `main.rs:1185` | 42.0 | | 直書き。`--eye` で 1 回だけ変えられる | (c) | **引用**: `main.rs:1493`「At the default 42 a beetle is thirty pixels across in a 1600-wide window」 |
| `ZOOM_PER_NOTCH` | `main.rs:1204` | 1.10 | ● | `const` | (c) | **測った**（導出も添えてある）: 30 ノッチで全域（`main.rs:1199-1203`、単体テスト `main.rs:6267` が 29〜32 を確かめる） |
| `PIXELS_PER_NOTCH` | `main.rs:1206` | 100.0 | ● | `const` | (a) ブラウザの 1 ノッチ = 100 px という外の事実 | **引用**: `main.rs:1192-1195`（Chromium は 100、Firefox は 3 行で winit が `Line` に直す） |
| `ZOOM_MIN` / `ZOOM_MAX` | `main.rs:1209-1210` | 6.0 / 110.0 | ● | `const` | (c) | *理由のみ*: 「Six units is a creature filling a third of the window; a hundred and ten has the whole forty-by-thirty field and its walls in view」`main.rs:1207-1208` |
| `PAN_PER_PIXEL` | `main.rs:1213` | 0.0016 | ● | `const` | (c) | *理由のみ*（距離に比例させる理由は書いてある。0.0016 は**不明**） |
| `PAN_PER_SECOND` | `main.rs:1215` | 0.9 | ● | `const` | (c) | **不明** |
| `PAN_LIMIT` | `main.rs:1217` | 8.0 | ● | `const` | (c) | *理由のみ* 「How far past the wall the eye may wander」`main.rs:1216` |
| オービットの回転感度 | `main.rs:2989-2990` | 0.005 | ● | 直書き | (c) | **不明** |
| pitch の範囲 | `main.rs:2990` | 0.12〜1.45 | ● | 直書き | (c) | **不明** |
| `CLICK_REACH` | `window.rs:82` | 1.6 | ● | `const` | (c) | *理由のみ* 「within about a body's width」`window.rs:92-93`。ただし体の半径は 0.40〜0.50 なので「体の幅」とは言いにくい（§7-5） |
| `CLICK_SLOP` | `window.rs:85` | 6.0 | ● | `const` | (c) | *理由のみ* 「a press that let go within a few pixels of where it went down was a click」`window.rs:90-91` |

### 2.9 実行と予算

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| 世界 VM の予算 | `main.rs:4323` | 45,000 命令 | ● | 直書き | (c) | **測った**（この repo でいちばん出どころのはっきりした数）: 上限（90 株・24 匹）の 392 フレームで最悪 25,837 命令・tick 4.86 ms、45,000 はその 1.74 倍。`main.rs:4290-4322`、`docs/worklog/2026-09-17-garden-world.md` |
| 世界 VM の `frame_time` | （rubevy の既定のまま） | 8 ms | ● | 触っていない | (c) | **引用**: `main.rs:4315-4322`（命令数の方が先に効くので据え置いた、と書いてある） |
| 生き物 VM の予算 | （rubevy の既定のまま） | 200,000 命令 | ● | 触っていない | (c) | **引用**: `main.rs:4303` |
| `RESTORE_PATIENCE` | `main.rs:1162` | 5.0 秒 | ● | `const` | (c) | *理由のみ*（`main.rs:1159-1161`）。5.0 自体は**不明**。0.5.0 では復元に 1.48 s かかっていた（`docs/README.md` の G9 の行）ので効いていた |
| `SHORTEST_SLEEP` | `main.rs:4747` | 0.05 秒 | ● | `const` | (c) | **引用**: `prelude.rb:438` の `sleep 0.05` と同じ数で、そちらは理由つき（`prelude.rb:430-437`）。`main.rs:4735-4746` |
| 熱の減衰 | `main.rs:4841` | 0.985 /フレーム | ● | 直書き | (c) | **不明** |
| `--headless` の既定 | `main.rs:1469` | 10.0 秒 | | 引数 | **(e)** | **不明**（既定値の理由） |
| `--shot` の既定 | `main.rs:1474` | 6.0 秒 | | 引数 | **(e)** | **不明**。Battle は 3.0（`sabibots/src/main.rs:271`）で食い違う（§7-7） |
| 窓の大きさ | `main.rs:1574` | 1600×900 | | 直書き | (c) | **引用**（間接）: `main.rs:1493`「a 1600-wide window」がこの数に依っている |
| headless のフレーム間隔 | `main.rs:1527` | 1/60 s | ● | 直書き | (a) 60 Hz を模すという定義 | **導出** |
| `Brains` の枠数 | `main.rs:820` | 3（種 2 ＋ world） | | `const`（`Species::ALL.len()` から） | (a) 種を増やすと配列を伸ばす必要がある | **導出**（`main.rs:826`） |

### 2.10 セーブ

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `SAVE_VERSION` | `main.rs:4873` | 1 | | `const` | **(a)** 形式の版。変えると古いセーブが読めなくなる（10 番目の判定がそれを確かめている） | **引用**: `docs/worklog/2026-09-17-garden-G5.md`（版番号を単独で先に読む） |
| `localStorage` の接頭辞 | `platform.rs:150` | `"garden:"` | | `const` | **(a)** 変えると公開版の利用者のセーブが消える | **引用**: `docs/plans/shared-crate-plan.md:162` |

### 2.11 selftest の閾値（(d)）

| 何を見るか | 位置 | 閾値 | 出どころ |
|---|---|---|---|
| 誰かが食べた | `main.rs:5957` | 10 秒以内 | **不明**（実測は 0.55 s。`docs/garden.md:2109`） |
| 夜が来た | `main.rs:5964` | 60 秒以内 | **導出**: `DAY_LENGTH` 1 周（`main.rs:74`） |
| すり抜けていない | `main.rs:5978, 3473` | 半径の和の 0.9 | **引用**: `docs/plans/garden-plan.md:67`「半径の和の 90% を下回るフレームが無い」 |
| 草に着いた | `main.rs:5618` | `REACH + 0.5` | **不明**（0.5 の余裕の理由） |
| 向きが変わった | `main.rs:5693` | `dot < 0.7`（≈45°） | *理由のみ* 「anything past a quarter turn is a different course」`main.rs:5680-5683` |
| 向きが変わった（窓） | `main.rs:5697` | 0.5 秒 | **引用**: `docs/plans/garden-plan.md:35`（G10 で「0.5 秒**以内に**」の仕様に合わせて窓にした） |
| 寝ている | `main.rs:5765, 6016` | 夜の 1.0 秒後、速さ < 0.05 | **不明**（1.0 と 0.05 の理由） |
| `NEWBORN_GRACE` | `main.rs:269` | 2.0 秒 | **引用**: `main.rs:262-268`。「実際に耳が聞こえないのは 2 フレーム」に対して十分長い、と `main.rs:5752-5754` |
| `NEWBORN_DEAF_FRAMES` | `main.rs:282` | 2 フレーム | **測った**: `main.rs:271-281`。夜をまたいで 1 フレームずつずらして生ませた（`docs/worklog/2026-09-17-selftest-flakes.md` §4.2） |
| `TOUCH_SETTLE` | `main.rs:287` | 1.5 秒 | **導出**: ハンドラが 1 通につき 0.5 秒ハンドルを握る（`beetle.rb:33` の `sleep 0.5`）。`main.rs:284-286` |
| 壁際の除外 | `main.rs:5738` | 壁から 1.5 | **不明** |
| 子の遺伝子 | `main.rs:3955` | `RATE = 0.1` | **引用**: `beetle.rb:81` の `mutate(0.1)` の写し（`main.rs:3953`）。**写しなので連動しない**（§7-3） |
| `FREEZE_AT` | `main.rs:3681` | 20.0 秒 | **導出**: 最初の食事（1 秒未満）・最初の死（1.9 s）・プローブの歩き（1.8 s）の後、夜（25 s）の前（`main.rs:3678-3680`） |
| `FREEZE_WINDOW` | `main.rs:3677` | 5.0 秒 | **引用**（導出も添えてある）: 計画書 `garden-world-plan.md` §3.1 の 5 秒。`1.6 × appetite` /秒なので 8 点減るはず（`main.rs:3673-3676`） |
| 季節が伝わった | `main.rs:6095` | 2.0 秒以内 | **不明** |
| 仕込みの断食甲虫 | `main.rs:2317` | 空腹 3.0 | **不明**（`starve` を必ず踏ませるための値） |
| 仕込みのプローブ | `main.rs:2327` | 空腹 40.0、草まで 5 単位 | *理由のみ*（`hungry_below 55` より下、`Sight 8` の内側。`docs/worklog/2026-09-17-garden-world-survey.md` §5） |
| 仕込みのつがい | `main.rs:2473` | 空腹 45.0 | **不明** |
| `MEADOW_AT` | `main.rs:2358` | `(-14, 9)` | *理由のみ*（他の 2 つの仕込みから離れた場所。`main.rs:2353-2357`） |
| `MEADOW_CLEAR` | `main.rs:2365` | 8.0 | **引用**: G2 から `clear_of_fixtures` が使っていた数に名前を付けただけ（`main.rs:2360-2364`） |
| つがいの隅の幾何 | `main.rs:2451-2462` | 規則の `reach` / `mate_reach` / `PLANT_MAX` から組む | **導出**: 不等式ごと `main.rs:2407-2412` に書いてある（`docs/worklog/2026-09-18-corner-and-selftest.md`） |
| 窓のチェックの開始 | `main.rs:1926` | 3.0 秒 | **不明** |

### 2.12 遺伝子（`genome.rs`）

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| 甲虫の種 | `genome.rs:65` | speed 2.2 / sight 8.0 / appetite 1.0 | | `const` 相当（`match`） | (b) 遊びの数 | **不明** |
| ウサギの種 | `genome.rs:66` | 3.4 / 12.0 / 1.0 | | 同上 | (b) | **不明** |
| `SPREAD` | `genome.rs:56` | 0.18 | | `const` | (b) | **不明** |
| 遺伝子の上下限 | `genome.rs:230-232` | speed 0.2〜8.0、sight 1.0〜24.0、appetite 0.2〜4.0 | | 直書き | (a) `garden.spawn` が受け取る値の検査。外すと Ruby から任意の値が入る | **不明**（範囲の理由） |
| `mutate` の既定 | `genome.rs:179` | 0.5 | | 直書き（定数が引けないときの保険） | (a) | **不明** |

変異率 0.1 は **Rust の定数ではない**（`docs/worklog/2026-09-17-garden-world-survey.md` §2）。唯一の出どころは
`beetle.rb:81` の `mutate(0.1)` で、§3.4 に 1 件として数えた。`main.rs:3955` の `RATE` はその写し（§2.11、§7-3）。

---

## 3. Garden（Ruby）

### 3.1 `ruby/world.rb` — 規則。**この節は全部すでに (e)**

エディタ（`F3`）でその場で書き換えられ、`Ctrl+S` でファイルに残る。移す先としての模範。
毎フレーム `each_frame` が読む（`def` なのでメソッド呼び出しが毎フレーム走る）。

| 名前 | 位置 | 値 | 毎F | 出どころ |
|---|---|---|---|---|
| `day_length` | `world.rb:21` | 60.0 | ● | **引用**: 元は `DAY_LENGTH`（`main.rs:74`、§2.2） |
| `growth` | `world.rb:30` | 0.06 /秒 | ● | 元は `PLANT_GROWTH`。**不明** |
| `plant_max` | `world.rb:31` | 1.4 | ● | **引用**: 元は `PLANT_MAX`（`main.rs:236` に同じ数が残る、§2.5） |
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
| 子が親を殺さない下限 | `world.rb:204` | 1.0 | ● | *理由のみ* 「a child does not kill its parent」 |
| つがいを探す前の人数 | `world.rb:262` | 2 匹以上 | ● | **導出** |

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
| 甲虫 `hungry_below` | `beetle.rb:10` | 55.0 | *理由のみ*（`beetle.rb:9`）。5 番目の判定がこの数に依る（`docs/worklog/2026-09-17-garden-world-survey.md` §5） |
| 甲虫 逃げた後の `sleep` | `beetle.rb:33, 43` | 0.5 / 0.25 | **引用**: 0.5 は `TOUCH_SETTLE` の導出の前提（`main.rs:284-286`） |
| 甲虫 変異率 | `beetle.rb:81` | 0.1 | **引用**: この行が唯一の出どころ（§2.12、`docs/worklog/2026-09-17-garden-world-survey.md` §2） |
| 甲虫 子を置く位置 | `beetle.rb:83` | +1.2, +1.2 | **不明** |
| 甲虫 思い出を口にする周期 | `beetle.rb:65` | 5 食に 1 回 | **不明** |
| 甲虫 本体の `sleep` | `beetle.rb:132` | 0.2 秒 | **不明**（1 判断あたりの費用に直結。`docs/worklog/2026-09-17-garden-world-survey.md` §7） |
| ウサギ `hungry_below` | `rabbit.rb:11` | 60.0 | **不明** |
| ウサギ `nosey_range` | `rabbit.rb:14` | 1.2 | **不明** |
| ウサギ 速さの倍率 | `rabbit.rb:78, 86` | `CRUISE × 1.6` / `× 1.4` | **不明** |
| ウサギ 本体の `sleep` | `rabbit.rb:92` | 0.25 秒 | **不明** |
| 両方 起きたときの `sleep` | `beetle.rb:109,115` / `rabbit.rb:66,72` | 0.5 / 0.1 | *理由のみ*（`beetle.rb:107`「One more `act 0, 0` here is what settles it」） |

---

## 4. SabiRuby Battle（Rust）

### 4.1 機体・弾・エネルギー — **まとめて (b)**

`sabibots/src/main.rs:28-51`。1 つのコミット（`978eb34`「robots become tanks」）で一度に入った。
コミットメッセージは**なぜこの形にしたか**（「thrust/fire in any direction left one obvious brain」）を言うが、
**個々の数を測った記録は無い**。`docs/sabiruby-battle.md:185-206` は同じ数を仕様として書き直しているだけで、出どころではない。
名前の付いた 13 個は「模型の理由はある／数の根拠は無い」＝ *理由のみ*、名前も付いていない 6 個は **不明**。
**全部、毎フレーム読まれる**。

| 名前 | 位置 | 値 | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|
| `ROBOT_RADIUS` | `main.rs:28` | 1.6 | `const` | (b) → `matches/*.rb` | *理由のみ*（`978eb34`） |
| `MAX_SPEED` | `main.rs:31` | 12.0 | `const` | (b) | *理由のみ*（`main.rs:29-30`「turns at a limited rate」） |
| `REVERSE_SPEED` | `main.rs:32` | 7.0 | `const` | (b) | *理由のみ*（同上） |
| `TURN_RATE` | `main.rs:33` | 2.6 rad/s | `const` | (b) | *理由のみ*（同上） |
| `TURRET_RATE` | `main.rs:34` | 4.0 rad/s | `const` | (b) | *理由のみ*（同上） |
| `ENERGY_MAX` | `main.rs:37` | 100.0 | `const` | (b) | *理由のみ*（`main.rs:35-36`「A robot that fires everything it has cannot also run away」） |
| `ENERGY_REGEN` | `main.rs:38` | 12.0 /s | `const` | (b) | *理由のみ*（同上） |
| `DRIVE_COST` | `main.rs:39` | 9.0 /s | `const` | (b) | *理由のみ*（同上） |
| `FIRE_COST` | `main.rs:40` | 16.0 × (0.25 + power) | `const`（0.25 は `main.rs:1975` に直書き） | (b) | *理由のみ*（同上） |
| `BULLET_SPEED_FAST` / `SLOW` | `main.rs:42-43` | 55.0 / 30.0 | `const`。**`prelude.rb:61-62` が同じ数を別に持つ**（§7-2） | (b) | *理由のみ*（`main.rs:41`「more damage, a slower shot, a longer reload」） |
| `BULLET_DAMAGE_MIN` / `MAX` | `main.rs:44-45` | 4.0 / 16.0 | `const` | (b) | *理由のみ*（同上） |
| `COOLDOWN_MIN` / `MAX` | `main.rs:46-47` | 0.3 / 0.8 秒 | `const` | (b) | *理由のみ*（同上） |
| `BASE_SPREAD` | `main.rs:49` | 0.02 rad | `const` | (b) | *理由のみ*（`main.rs:48`「a gun is not a laser」） |
| 威力の下限 | `main.rs:1974` | 0.2 | 直書き | (b) | **不明** |
| 体力の満 | `main.rs:1565` | 100.0 | 直書き。`main.rs:638, 715` が `/ 100.0` で 2 回写している | (b)（§7-8） | **不明** |
| 弾の寿命 | `main.rs:1987` | 2.5 秒 | 直書き | (b) | **不明** |
| 弾の出る位置 | `main.rs:1989` | 砲口から 2.8 | 直書き | (b) | **不明** |
| グリップ（速度が追いつく時定数） | `main.rs:2060` | 0.2 秒 | 直書き | (b) | **不明** |
| エネルギー切れの減速 | `main.rs:2053` | 0.35 倍 | 直書き | (b) | **不明** |
| `UNSET` | `main.rs:51` | −999.0 | `const`。**`prelude.rb:59` が同じ数を別に持つ**（§7-2） | **(a)** 「触らない」の合図。実際に取りうる値と重なると制御が飛ぶ | *理由のみ*（`main.rs:50`「leave this as it is」） |

### 4.2 雑音とレーダー

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| `noise` | `matches/training.rb:4` | 0.3 | ● | **Ruby** | **(e)** | **引用**: `docs/sabiruby-battle.md:202-206` |
| 位置のぶれ | `main.rs:1920` | `noise × 距離 × 0.05` | ● | 直書き | (b) | **引用**（式のみ）: `docs/sabiruby-battle.md:204`「positions by up to noise × distance × 5%」 |
| 速度のぶれ | `main.rs:1922` | `× noise × 2.0` | ● | 直書き | (b) | **不明** |
| 弾のぶれ | `main.rs:1949` | `× noise × 1.5` | ● | 直書き | (b) | **不明** |
| 弾道の散り | `main.rs:1979` | `BASE_SPREAD + noise × 0.1` | ● | 直書き | (b) | **引用**（式のみ）: `docs/sabiruby-battle.md:205` |
| `radar` の既定距離 | `main.rs:1913` / `prelude.rb:73` | 60.0 | ● | **Ruby 側は変えられる**が Rust にも既定がある | (e)／(b) | **不明** |
| `incoming` の既定距離 | `main.rs:1942` / `prelude.rb:80` | 25.0 | ● | 同上 | (e)／(b) | **不明** |
| `seed` の桁 | `main.rs:1908` | `× 1,000,000` | | 直書き | (a) Ruby に整数で渡すための桁 | **不明** |

### 4.3 闘技場と壁

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| 闘技場の半径 | `main.rs:302`（`ArenaSize::default`） | 32.0 | | `const` 相当 | (c)／(b) | **不明** |
| 木箱の間隔 | `main.rs:1763` | 約 2.6 | | 直書き | (c) | **引用**: `docs/sabiruby-battle.md:646`（「25 crates a side at the start」＝ 64/2.6） |
| `MIN_CRATES` | `main.rs:1768` | 7 | | `const` | (b) 試合の規則 | **引用**: コミット `28f3b53`「down to 7 a side」、`docs/sabiruby-battle.md:646`「room for a last fight」。7 の根拠は*理由のみ* |
| 床のタイル | `main.rs:1421-1433` | 8.0 単位、3 枚に 1 枚別柄 | | 直書き | (c) | **不明** |
| ロボットの初期配置 | `match_prelude.rb:61-63` | 半径 20、円周に等分 | | **Ruby** | **(e)** | **不明** |
| `TEAMS` の数 | `main.rs:54` | 4 | | `const` | (a) `TEAM_COLORS` と長さが揃っている必要がある | **導出** |

### 4.4 試合の進行（`matches/training.rb`、**すでに (e)**）

`noise` は §4.2 に数えた。

| 名前 | 位置 | 値 | 出どころ |
|---|---|---|---|
| `sudden_death` の開始 | `training.rb:9` | 20 秒 | **引用**: コミット `28f3b53`「shrink one crate every 2 s from 20 s on instead of two jumps」 |
| 縮む周期 | `training.rb:9` | 2.0 秒ごと 1 箱 | **引用**: 同上（`28f3b53`） |
| 試合ループの `sleep` | `match_prelude.rb:112` | 0.2 秒 | **不明** |
| 決着を見る前の `sleep` | `match_prelude.rb:87` | 0.1 秒 | **不明** |

### 4.5 表示と実行

| 名前 | 位置 | 値 | 毎F | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|---|
| HUD の予算バーの満 | `main.rs:2308` | 3,000 命令 | ● | 直書き（**毎フレーム代入**） | (c) | *理由のみ* 「a brain that thinks for a frame spends tens to hundreds; the bar fills as one approaches a timeslice's worth」`main.rs:2306-2307`。3,000 は**不明** |
| CPU バーの満 | `main.rs:659` | 3000.0 | ● | 直書き | (c) | *理由のみ*: 上と同じ数の写し（§7-8） |
| 熱の減衰 | `main.rs:2291` | 0.8 秒の時定数 | ● | 直書き | (c) | *理由のみ* 「about a second of memory」`main.rs:2290` |
| 体力バーの色の境 | `main.rs:639-644, 720-725` | 0.5 / 0.25 | ● | 直書き（2 か所） | (c) | **不明** |
| `BAR_WIDTH` | `main.rs:686` | 3.6 | ● | `const` | (c) | **不明**（`ROBOT_RADIUS × 2.25` = 3.6 と一致するが、そう書かれてはいない） |
| ロボットの見た目の大きさ | `main.rs:1591` | `ROBOT_RADIUS × 2.4` | | 直書き | (c) | **不明** |
| 名札の高さ | `main.rs:562` | `ROBOT_RADIUS × 2.25` | ● | 直書き | (c) | **不明** |
| 爆発の大きさと時間 | `main.rs:2155-2158` | ×3.5 / ×(0.6 + damage/16) / 0.7 / 0.25 秒 | ● | 直書き | (c) | **不明**。`/16.0` は `BULLET_DAMAGE_MAX` の写し |
| 窓の大きさ | `main.rs:331` | 1600×900 | | 直書き | (c) | **不明** |
| `--headless` の既定 | `main.rs:264` | 10.0 秒 | | 引数 | **(e)** | **不明** |
| `--shot` の既定 | `main.rs:271` | 3.0 秒 | | 引数 | **(e)** | **不明**（箱庭は 6.0。§7-7） |
| スクリプトの優先度 | `main.rs:1587, 1653, 2260` | 本体 100 / 試合 10 / 差し替え 128 | | 直書き | (a) 順序の約束。`prelude.rb:262` の `+20` と組で効く | **不明**（100/10/128 の理由） |
| `localStorage` の接頭辞 | `platform.rs:103` | `"sabibots:"` | | `const` | **(a)** 変えると利用者のセーブが消える | **引用**: `docs/plans/shared-crate-plan.md:162` |

### 4.6 selftest の閾値（(d)）

| 何を見るか | 位置 | 閾値 | 出どころ |
|---|---|---|---|
| ハンドラが走った・向きが変わった | `main.rs:1356, 1368, 1389` | 当たりから 0.3 秒 | **引用**: `docs/sabiruby-battle.md:314, 327`（scout の `sleep 0.3` と対）。`main.rs:1301-1303` |
| どれだけ向きが変わったか | `main.rs:1398` | 0.2 rad | **導出**: 「a quarter of the full turning rate over the 0.3 s」`main.rs:1397`（`TURN_RATE 2.6 × 0.3 ÷ 4 ≈ 0.195`） |
| 落ちたロボットを外す | `main.rs:1264` | 落ちてから 0.5 秒 | **不明** |
| 体力が満で始まる | `main.rs:1086` | 100.0 | **不明**: `ENERGY_MAX` ではなく体力の 100（§7-8） |
| 壁が隅で終わる | `main.rs:1101-1102` | 誤差 0.01 | **導出**: ε（判定の一部として挙げた） |
| チェックの開始 | `main.rs:430` | 2.0 秒 | **不明** |

---

## 5. SabiRuby Battle（Ruby）

| 名前 | 位置 | 値 | 今変えられるか | 分類案 | 出どころ |
|---|---|---|---|---|---|
| `UNSET` | `prelude.rb:59` | −999.0 | Ruby | (a)。`main.rs:51` の写し | **不明**（写しであることも書かれていない。§7-2） |
| `SHOT_FAST` / `SHOT_SLOW` | `prelude.rb:61-62` | 55.0 / 30.0 | Ruby | (a) 相当。`main.rs:42-43` の写し | *理由のみ* 「(the game's numbers, for leading a target)」`prelude.rb:60`。§7-2 |
| `ON_SLOTS` | `prelude.rb:118` | 4 | Ruby | **(a)** `run_handler` に書き出されている名前の数 | **引用**: 箱庭の `ON_SLOTS`（`prelude.rb:340-356`）と同じ理由 |
| ハンドラの優先度差 | `prelude.rb:262-263` | 本体 +20、上限 255 | Ruby | (a) 255 は mruby-task の優先度の上限 | **不明**（+20 の理由） |
| `steer_to` の利得 | `prelude.rb:163` | ×2.0 | Ruby | **(e)** | **不明** |
| `aimed?` の許容 | `prelude.rb:184` | 0.12 rad | Ruby | **(e)** | **不明** |
| `near_wall?` の余白 | `prelude.rb:188` | 6.0 | Ruby | **(e)** | **不明** |
| `on_collision?` の半径 | `prelude.rb:201` | 2.5 | Ruby | **(e)** | **不明**（`ROBOT_RADIUS` 1.6 より大きい。当たり判定ではなく「避けるかどうか」の余裕） |
| `wander_turn` の変わりやすさ | `prelude.rb:212` | 0.04 | Ruby | **(e)** | **不明** |
| `lead` の反復 | `prelude.rb:175` | 2 回 | Ruby | (a) 「one guess … corrected once」（`prelude.rb:170-171`） | *理由のみ* |
| `fire` の既定 | `prelude.rb:95` | 0.5 | Ruby | **(e)** | **不明** |
| scout の数 | `scout.rb:18-64` | `sleep 0.3` / `0.05`、`rand < 0.5` / `< 0.01`、`nearest_enemy(45)`、`incoming(18)`、`near_wall?(6)`、距離 14、`lead(_, 0.3)`、`aimed?(_, 0.2)`、エネルギー 25、威力 0.3 | Ruby | **(e)** | `sleep 0.05` と `sleep 0.3` は *理由のみ*（`scout.rb:4-10`、`docs/sabiruby-battle.md:314`）。残りは**不明** |
| hunter の数 | `hunter.rb:5-19` | `nearest_enemy(60)`、`near_wall?(5)`、距離 18 / 34、`lead(_, 0.9)`、エネルギー 40、威力 0.9 / 0.8、`sleep 0.08` | Ruby | **(e)** | **不明**（`docs/sabiruby-battle.md:246-248` は振る舞いを説明するだけ） |

---

## 6. 集計

数えたのはこの文書の表の行（§1〜§5）で、**260 件**。
`docs/garden.md` の例を引いただけの行と rubevy 側の数を参照しただけの行の 2 つは数えていない。

### 分類ごと

| 分類 | 件数 |
|---|---|
| (a) 不変量 | 33 |
| (b) 遊びの数 → Ruby 側 | 33 |
| (c) 動かす側の数 → `Settings` / 引数 | 101 |
| (d) selftest の閾値 | 28 |
| (e) 既に Ruby か設定から変えられる | 65 |
| **合計** | **260** |

2 つに跨るもの（`DAY_LENGTH` のように「既に Ruby から変えられるが Rust にも既定値がある」、
`radar` の既定距離のように「Ruby からも Rust からも来る」）は、**先に書いた方**で 1 件として数えた。

**毎フレーム読まれる数は 103 件**（全体の 40%）。移すときに読む費用を測る必要があるのはこの 103 件。
特に多いのは `world.rb` の規則 21 件と Battle の機体模型 20 件で、どちらも (b)＝ Ruby へ移す候補。

### 出どころごと

| 出どころ | 件数 |
|---|---|
| **測った**（日付・条件つきの実測がある） | 14 |
| **導出**（既にある制約から出ている） | 18 |
| **引用**（文書・計画書・コミットに書いてある） | 50 |
| *理由のみ*（理由らしきことは書いてあるが、測った記録も導出も無い） | 53 |
| **不明** | **125** |
| **合計** | **260** |

**出どころ不明が 125 件、全体の 48%。** 内訳:

| どこ | 件数 | うち不明 |
|---|---|---|
| 共有 crate の `src`（S4a 後は `rubevy-egui` + `games-shell`） | 33 | 19 |
| `garden/src` | 117 | 49 |
| `garden/ruby` | 40 | 20 |
| `sabibots/src` | 57 | 28 |
| `sabibots/ruby` | 13 | 9 |

不明がいちばん濃いのは**表示まわり**（字の大きさ、パネルの寸法、色、バーの目盛）と
**Battle の機体模型に付いている無名の数**（弾の寿命、砲口の位置、グリップ）。
いちばん薄いのは**箱庭の夜と霧と予算**で、そこは測った記録がコメントに残っている。

### 機械的に拾った数と、選んだ数

| どこ | `const` 宣言 | 数値リテラルを含む行 | 選んだ数 |
|---|---|---|---|
| 共有 crate の `src`（S4a 後は `rubevy-egui` + `games-shell`） | 20（うち数値 17） | 149 | 33 |
| `garden/src` | 88（うち数値 68） | 538 | 117 |
| `sabibots/src` | 28（うち数値 21） | 322 | 57 |
| `garden/ruby` + `sabibots/ruby` | 8（`NAME = 数` の行） | 237 | 53 |
| **合計** | **136（うち数値 106）** | **1,246** | **260** |

`const` 宣言は `grep -nE '^\s*(pub\s+)?const\s'`、リテラルは
`grep -nE '(^|[^A-Za-z0-9_.":])[0-9]+(_[0-9]+)*(\.[0-9]+)?'` で拾い、コメントだけの行を落としてから目で選んだ。
**リテラル 1,246 行から 260 件**（21%）。落とした 8 割の内訳は §0 の「入れなかった基準」。
選んだ数がリテラル行より多い節と少ない節があるのは、1 行に数が 2 つある（`spacing([12.0, 6.0])`）のと
同じ数が何行にも出る（`0.35` が 3 か所）のが混ざっているため。

## 7. 気づいた点（一覧の仕事の外。**直していない**）

1. **`code.rs` のコメントと定数が最初から食い違っている。** `crates/rubevy-egui/src/code.rs:147` は
   「430px of a monospace 12px font: about 58 characters」と書いているのに `code.rs:149` は `WIDTH = 44`。
   両方 1 つのコミット（`23c5ad3`）で入っている。どちらが正しいのか読んでも分からない。
   なお `CodePanel` は**どのゲームからも使われていない**（`HudPlugin`、`read_script` も同じ。計画書 §2 が
   「消さない」と決めている）。属する話: 共有 crate。
2. **Battle の 3 つの数が Rust と Ruby の両方に別々に書かれている。** `UNSET = -999.0`
   （`sabibots/src/main.rs:51` と `sabibots/ruby/prelude.rb:59`）、`BULLET_SPEED_FAST/SLOW` = 55/30
   （`main.rs:42-43` と `prelude.rb:61-62` の `SHOT_FAST/SHOT_SLOW`）。`prelude.rb:60` は
   「the game's numbers, for leading a target」と**写しであることを自覚している**が、繋がってはいないので
   片方を動かすともう片方が黙って嘘をつく（`lead` が外れる、`act` が「触らない」を取り違える）。
   属する話: S5b で Battle の数を Ruby に移すときの設計そのもの。
3. **箱庭の変異率 0.1 が 2 か所にある。** 本物は `garden/ruby/creatures/beetle.rb:81` の `mutate(0.1)`、
   判定側は `garden/src/main.rs:3955` の `const RATE: f32 = 0.1`。`main.rs:3953` は「the rate is the one the
   beetle's script passes to `mutate`, which is 0.1」と写しだと書いているが、**`beetle.rb` を書き換えても
   判定は 0.1 のまま**なので、8 番目の判定は利用者が変異率を上げた瞬間に FAIL になる（エディタで
   その場で書き換えられるのがこのゲームの見せ場なので、踏める）。属する話: (d) の閾値の設計。
4. **`ArenaPlugin` のコメントと数が合っていない。** `crates/games-shell/src/lib.rs:117` は
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
7. **2 本のゲームで `--shot` の既定秒数が違う。** 箱庭 6.0（`garden/src/main.rs:1474`）、
   Battle 3.0（`sabibots/src/main.rs:271`）。同じ意味の引数で、計画 S1 がこの引数解析を共有 crate に上げる。
   上げるときにどちらに寄せるか（あるいはゲームごとの既定を受け取るか）を決める必要がある。
   `--headless` はどちらも 10.0 で揃っている。属する話: S1。
8. **Battle の「100」と「3000」がそれぞれ 3 か所・2 か所に散っている。** 体力の満は
   `sabibots/src/main.rs:1565`（生成）、`:638` と `:715`（バーの `/ 100.0`）、`:1086`（判定）に
   4 回書かれていて `ENERGY_MAX` のような名前が無い。VM の予算バーの満は `:2308` の `3_000` と
   `:659` の `3000.0`。どれも `const` にすらなっていない。属する話: S5b の最初の作業。
9. **`sabibots/src/main.rs:2308` は毎フレーム同じ定数を代入している。** `panel.budget = 3_000;` が
   ロボット 1 体につき毎フレーム走る。害は無いが、`ScriptPanel` の初期化時に 1 度でよい。
   属する話: 共有 crate の `ScriptPanel`。
10. **`docs/README.md` の目次の Battle の説明（31 行 + 当たり 2 行）と `?selftest` の実際の行数は
    今回確かめていない。** 一覧の仕事はコードを動かさないので、突き合わせていない。属する話: S1 の基準取り。

---

## 8. 分類に迷った数と、迷った理由

1. **新しい芽の最小間隔 1.5**（`garden/src/main.rs:3526`）。
   草が生える規則は W1 で全部 `world.rb` に移ったのに、「他の草から 1.5 以内には生やさない」だけが
   Rust の `sprout_plants` に残っている。**(b) 遊びの数**として `world.rb` に足すのが筋に見えるが、
   `world.rb` から見ると「草の位置の総当たり」で、それは Rust に残した理由そのもの（`world.rb:11`）。
   → (b) と書いたが、S5b で「Ruby が数だけ渡し、走査は Rust」という `garden.rules` の道に乗るかを
   著者に確かめたい。

2. **`TOUCH_REACH` 1.3**（`garden/src/main.rs:253`）。
   `startle`（ウサギ→甲虫）は Rust に残った規則で、その距離。**(b)** だが `garden.rules` に
   受け口が無い（今あるのは `day_length` / `child_hunger` / `pop_max` / `reach` / `mate_reach` の 5 つ、
   `main.rs:4446`）。受け口を 1 つ増やすだけなのか、`startle` ごと `world.rb` に移すのかで話が変わる。

3. **生き物の半径 4 つ**（`main.rs:309-312`）。
   体の寸法は **(a) 不変量**（モデルの大きさと結びついている）か、**(b) 遊びの数**（押し分けの強さ）か。
   `Collider` は Ruby から**読める**（`register_type` 済み、`docs/garden.md:280`）が書けない。
   モデルの倍率（`main.rs:2589-2590` の 0.55 / 0.75）と一緒に動かさないと見た目と当たりがずれるので、
   → (b) と書いたが「2 つで 1 組」であることを S5b で保つ必要がある。

4. **`KINDS` の 9 色**（`crates/rubevy-egui/src/editor.rs:454`）。
   **(c) 表示**に入れたが、これは「測って決めた既定値」の模範例で、利用者が変えられるようにすると
   測った制約（コントラスト 4.9 / ΔE 25）を破れてしまう。**(a) 寄りの (c)**。
   → 変えられるようにするなら「変えると測った保証が消える」と rustdoc に書くべき、と考えるが、
   そこまで踏み込むかは著者判断。

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

8. **`--headless` / `--shot` の既定秒数**。
   **(e)**（引数で変えられる）に入れたが、既定値そのものは変えられない。
   「既定値は数か」を厳密に取ると (c) だが、そこまで数えると引数のある数は全部二重になるので (e) にした。

9. **`SHORTEST_SLEEP` 0.05**（`main.rs:4747`）と **`prelude.rb:438` の `sleep 0.05`**。
   同じ数だが仕事が違う（片方は判定の下限、片方は Ruby の待ち）。
   (c) と (e) に分けて数えたが、**片方を動かすともう片方の意味が変わる**ので 1 組かもしれない。

10. **`platform.rs` の接頭辞 `"garden:"` / `"sabibots:"`**。
    数ではないが **(a) 不変量**として入れた（変えると利用者のセーブが消える）。
    「数の一覧」に文字列を入れるかは迷った — 計画書 §5 の罠にこの 2 つが名指しで挙がっているので入れた。
