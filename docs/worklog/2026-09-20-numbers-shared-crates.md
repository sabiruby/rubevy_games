# 共有 crate の数を、利用者が変えられる場所へ（S5b-1）

計画書 `docs/plans/shared-crate-plan.md` の段階 **S5b-1**。対象は `crates/rubevy-egui` と
`crates/games-shell` の 2 つだけで、ゲームに触ったのは 3 か所（Battle の `ARENA_HALF_WIDTH`、
2 本の `PanelSettingsPlugin` の 1 行、箱庭の窓のチェックがエディタの矩形を設定から読む 2 行）。
一覧は `docs/numbers.md` §1。着手時は main と同じ `aa3c916`、作業は worktree の `shared-crate`。

**著者が決めていたこと**（2026-09-20）: 分類案のとおりに進める。**出どころ不明の数は「不明」と
書いたまま移す。既定値は 1 つも動かさない。** 迷った数は本体の案で進めて結果を報告する。
この段階で新しく決めた数は **0** で、動かした既定値も **0** である。

---

## 1. まず行番号を取り直し、抜けていた 8 件を足した

S5a の一覧は `crates/rubevy-arena` が 1 つだった頃に取ってあり、S4a が 2 つに割ったので `位置`
欄が全部古い（一覧の頭にその注意書きがある）。33 件ぶんの行番号を取り直すのは移したあとで
もう一度ずれるので、**先に移し、最後に取り直した**。

取り直しの途中で、一覧に**入っていない数**が見つかった。S3 が足したパン・ズームのカメラ
（`games-shell/src/camera.rs`）の 8 件——`ZOOM_PER_NOTCH` から `CameraControls::bounds` まで——で、
S5a はこれらが存在する前の木を読んでいる。どれも既に `CameraControls`（Resource、`Settings` の
`camera_*` 7 鍵）に入っていて、既定値ごとに出どころが rustdoc に 1 行ある。**コードは 1 行も
触らず、一覧に §1.2 として足した**（S3 の仕事を一覧が知らないままだと、次に数える人が
「カメラの数はどこへ行った」と探すことになる）。

ほかに一覧に無かった数を 2 つ足した: コードパネルの字の大きさ 2 つ（13.0 / 12.0。§4 の
食い違いがこの 12.0 で計算しているので、食い違いを説明するには名前が要る）と、アリーナの床の色。
S3 で消えた「エディタを開いたときずらす幅 0.16」は 1 件落とした（§5）。差し引き 33 → 42 件。

---

## 2. 置き場所を決めるときに当たった 3 つの壁

「Resource にする」で全部済むと思って始めたが、3 か所で済まなかった。どれも**型の都合ではなく、
既にあるコードの形の都合**で、捨てた案ごと書いておく。

### 2.1 `rubevy-egui` は `Settings` を知ってはいけない

依存は `games-shell` → `rubevy-egui` の一方向（S4a の決定）。`Settings` は `games-shell` にあるので、
エディタや VM パネルの `read_from(&Settings)` は**書けない**。

- 捨てた案 A: `rubevy-egui` に `Settings` を移す。パネルの crate に「PC ではファイル、ブラウザでは
  `localStorage`」という殻の話を持ち込むことになり、S4a が割った線を跨ぐ。
- 捨てた案 B: `trait NumberStore` を `rubevy-egui` に置いて `games-shell` が実装する。
  `Res<dyn Trait>` は書けないので、結局どこかで具体型に降りる必要があり、trait が何も買わない。
- 採った案: **`read_from(&mut self, number: impl Fn(&str) -> Option<f32>)`**。鍵の表は各パネルの
  rustdoc にあり、店は呼ぶ側が持つ。`games-shell` 側に `PanelSettingsPlugin`（`PreStartup` で
  `Option<Res<Settings>>` を読み、6 つの `Option<ResMut<…>>` に書く）を置き、2 本のゲームは
  1 行足すだけ。S3 の `CameraPlugin` が `camera_*` を `PreStartup` で読むのと同じ場所・同じ形で、
  `camera_*` はそちらに任せてある。

### 2.2 エディタの 9 色を Resource にすると、Battle の selftest がコンパイルできない

`drawn_kind(&editor, at)` は「そのバイトが**何色で描かれたか**」を `LayoutJob` から読み返す関数で、
色 → 分類の対応表が要る。色を Resource にすると引数が 1 つ増え、呼ぶ側は 2 本の selftest。
**`sabibots` の `selftest` は既に引数 16 個で、Bevy の上限がちょうど 16。** 1 つ足すと落ちる
（`SystemParam` にまとめれば通るが、それは「色を設定にする」ために判定の形を変えることになる）。

そこで**寸法と色を分けた**:

- **`EditorLayout`**（Resource、`Settings` の `editor_*` 6 鍵）: `MARGIN` / `WIDTH` / `HEIGHT` /
  `FONT` / ボタンの余白と字の大きさ。**Resource でなければならない**——箱庭の窓のチェックの
  `FakePointer` がエディタの矩形を計算するのに読むからで、`FakePointer` は
  `ResMut<Editor>` を持つ system と同居する（同じ `Editor` を二重に借りると Bevy が落ちる）。
- **`EditorColors`**（`Editor::colors` のフィールド）: 9 色、熱の帯の色と α と下限、琥珀。
  `Editor` は既にパネルの語（`noun`、`apply_label`、`save_label`）を持っていて、ゲームは
  `Startup` でそれらを書いている（`sabibots` の `setup_editor`）。色をそこに置けば、設定の仕方が
  既にある道と同じになり、`drawn_kind` の引数は増えない。

**9 色は著者の指示どおり「変えられるが、変えると測った保証が破れる」と rustdoc に書いた。**
コントラスト 4.96 / 帯 3.09 / ΔE 25.9（`docs/worklog/2026-09-18-editor-highlight.md`）は
**この 9 色の集合についての測定**で、1 色差し替えれば 3 つとも成り立たなくなり、それを見張るものは
コードの中に無い。熱の帯の色と α も同じ測定の背景側なので、同じ注意を書いた。

ついでに 1 つ、**数の写しをやめた**: 行番号の桁の色 `(120,128,140)` は「分類 3（コメント）と同じ
であることが意味」（一覧の (a)）なのに、値が 2 か所に書いてあった。`EditorColors::gutter()` が
`kinds[3]` を読む 1 行にしたので、コメントの色を変えれば桁も付いてくる。**(a) の不変量を
コメントではなくコードで保った**形で、これが唯一「同じ数が 2 か所にある」を潰した箇所。

### 2.3 VM パネルの数は Resource にできない

`VmInspector::fill` は**メソッド**で、Battle のヘッドレスは `VmInspector::default()` を手で作って
呼ぶ（`sabibots/src/main.rs`、ログを 1 回吐くため）。その app には `VmInspectorPlugin` が無いので、
2 つ目の Resource は存在しない。→ **`InspectStyle` は `VmInspector::style` のフィールド**にした。

`VmClock` の平滑化 0.8 / 0.2 も同じ形の問題で、`vm_clock_end` は**箱庭のヘッドレスが自分で
`add_systems` する** pub な関数（`garden/src/main.rs`）。引数を増やすと呼ぶ側が変わるので、
**`VmClock::smoothing` のフィールド**にした（`VmClock` は既にその system が `ResMut` で持っている）。

---

## 3. アリーナの半幅 32.0 —— 共有 crate は既定を持たない

著者の判断（2026-09-20）どおり、`ArenaSize` の `Default` を**消した**。理由は
`CameraPlugin::showing(half_height)` が既定を持たないのと同じで、**窓が抱える世界の量はゲームしか
知らない**——3 本目（工場）にアリーナは無い。

- `ArenaPlugin::showing(half)` で渡す。`ArenaSize::default()` を呼んでいた 3 か所
  （ヘッドレスの `insert_resource`、プラグイン、`restart_match` の戻し）が全部 Battle の
  `const ARENA_HALF_WIDTH: f32 = 32.0` を読む。rustdoc には**「出どころ不明」と書いた**
  （32 を選んだ記録はどこにも無い）。これを Ruby（`matches/*.rb`）へ移すかは S5b-2 が決める。
- 壁の外に見せる床 `+ 3.0` は 2 つの式に直書きされていた。`ArenaPlugin::floor_margin` にし、
  `ArenaView` が `follow_arena` まで運ぶ。`seen_by` / `view_of` は引数で受ける。
- 窓が無いときの 16/9 だけは**設定にしなかった**。`follow_arena` が窓を見つけられないフレームの
  代役で、そのフレームには描くものが無い（ゲームは窓が無ければ `ArenaPlugin` を足さない）。
  `const NO_WINDOW` と名前を付け、「比だけが意味で、値は画面に出ない」と rustdoc に書いて
  一覧では (c) から **(a)** に移した。**分類を自分で動かした唯一の行**なので報告に挙げる。

---

## 4. コメントと数の食い違い（S5a が見つけたもの）

**`CodePanel` の「58 文字」と `WIDTH = 44`**: 指示どおり**どちらにも寄せていない**。
既定は 44 のまま `CodeStyle::chars` に移し、食い違いそのものを `CHARS` の rustdoc に書いた
（「430px of a monospace 12px font: about 58 characters」と 44 が同じコミット `23c5ad3` で入っていて、
どちらが意図か読んでも分からない）。一覧の出どころ欄は **「不明（コメントと値が最初から食い違う）」**。
58 が欲しい人は `code_chars=58` と書ける——が、**それが正しいと決めたわけではない**。

**`ArenaPlugin` の「3 分の 1」と 0.16**: 指示のとおり確かめた。`games-shell/src/lib.rs` に
`0.16` も「a third of the window」も**もう無い**（S3 が `ViewInsets` に替えたときに消えている）。
残っているのは `follow_arena` の rustdoc と単体テストの中で「16% は何だったか」を説明する記述だけで、
使われている数ではない。一覧の §7-4 にその旨を書き足した。

---

## 5. 動きを変えていないこと

前後の比較は `tools/fixedlines.sh` と `docs/verification/selftest-lines.md` の 6 通りの走行。
**前の版の走行は着手前に取ってある**（`git stash` は使わない規則があるので、先に測ってから触った）。

| 走行 | 前 | 後 | 判定 |
|---|---|---|---|
| 箱庭 ヘッドレス（`--headless 90`） | 13 行 FAIL 0 | 13 行 FAIL 0 | `diff` 空 |
| Battle ヘッドレス（`--headless 25`） | 4 行 FAIL 0 | 4 行 FAIL 0 | 3 走行のうち 2 走行が `diff` 空、1 走行は**記録済みの「判定が動く 1 行」**（`the handler tasks of every robot that went down ended` が `--`）。`verification/selftest-lines.md` がその 1 行を名指ししている |
| 箱庭 窓（docker/lavapipe） | 44 行 FAIL 0 | 44 行 FAIL 0 | `diff` 空 |
| Battle 窓（docker/lavapipe） | 32 行 FAIL 0 | 32 行 FAIL 0 | `diff` 空 |
| 箱庭 ブラウザ（`garden/?selftest`） | 一覧の 45 行 | 45 行 FAIL 0 | 一覧と完全一致、pageerror 0・requestfailed 0 |
| Battle ブラウザ（`sabibots/?selftest`） | 一覧の 33 行 | 33 行 FAIL 0 | 同上 |

ほかに: `cargo build --workspace --all-targets` **警告 0**、`cargo test --workspace` **34 → 46
通過**（増えた 12 は全部この段階で足したテストと doc テスト）、wasm は
`cargo build -p garden -p sabibots --target wasm32-unknown-unknown --profile web` が通り、
`web/build.sh all` の `wasm-opt` 後で **sabibots 35,111,618 → 35,152,714（+41,096、+0.12%）、
garden 35,828,300 → 35,900,116（+71,816、+0.20%）**。増えたぶんは既定値に名前を付けた `const` と
`impl Default` と、鍵の文字列である。

### 設定を変えると効くこと

**単体テストで 1 種類 1 本**（`read_from` が鍵を読むこと、書いていない鍵は既定のままであること）:
`EditorLayout`、`EditorColors`（色を差し替えると `listing` がその色で塗り、`drawn_kind` は同じ
表を引くので分類は読み返せる。帯は α=0 で消える）、`InspectStyle`（`vm_regs_frames=2.7` は 2、
負数は 0）、`CodeStyle`（`code_chars=58` で切る位置が変わる）、`HudStyle`（バーの目盛）、
`GuideStyle`、`ArenaPlugin::floor_margin`（窓が抱える世界の量が変わる）、そして
`PanelSettingsPlugin` 自体（`App` を建てて `PreStartup` を 1 回回し、店の 1 行が 2 つのパネルに
届くこと。ゲームが持たないパネルは `Option` なので何も起きないこと）。

**本物の走行で 1 本**（`garden.settings.txt` を置いて docker の窓のチェック）。ここは 2 回やっていて、
1 回目の失敗の方が中身がある。

1. `editor_width=760` → 44 行 FAIL 0、`diff` 空。**ただしこれは証明になっていない**: チェックが
   狙う点は「右の余白からエディタの幅の半分」なので、幅を 760 と思って狙った点は、幅が 520 のままでも
   パネルの中に入る。右に張り付いた矩形では、**幅を変えても狙う点は古い矩形の中に留まる**。
2. `editor_width=1400`（1600 px の窓）→ 狙う点は x=892 で、520 のままなら矩形は x=1072〜1592 だから
   **外**。ここで初めて「egui がポインタを持っている」が実際に幅 1400 を意味する。走らせると
   `the wheel over the editor …` は ok、しかし次の
   `and with the panel closed the same wheel in the same place zooms` が **FAIL**。
   原因は探すまでもなく**説明の窓**で、既定の 640×820 は画面中央に置かれるので x 480〜1120 を覆う。
   エディタを閉じても同じ点が説明の下にある。
3. そこで `guide_height=100` / `guide_max_height=120` を足した（説明の窓が y 425〜525 になり、
   狙う点の y=328 から外れる）。→ **44 行 FAIL 0、既定の走行と `diff` 空**。
   つまり 1 回の走行で **エディタの幅と説明の高さが両方効いている**ことが見えた。

2 の FAIL は直すべきものではない（窓を覆うものを 2 つ重ねれば重なるのは当然で、既定では
エディタの矩形は説明の右外にある）。記録しておくのは、**「設定を変えたら判定が落ちた」を
「回帰だ」と読み違えないため**である。

---

## 6. 足した設定の一覧

`docs/numbers.md` §1.7 に表がある。鍵は全部**既存の `camera_*` に揃えて**接頭辞つき
（`editor_` / `vm_` / `hud_` / `guide_` / `code_`）。**色と鍵盤は `Settings` に出していない**——
色を文字列から読むには構文解析が要り、それは S3 が `CameraKeys` を出さなかったのと同じ理由。
app からはどちらも変えられる。

---

## 7. 気づいた点

1. **一覧に入っていない埋め込みの数が、VM パネルにまだ 9 か所ある**（`inspect.rs` の
   琥珀 `(240,190,90)`、淡色 `(200,206,216)`、待ちの黄 `(255,236,150)`、灰 3 種、登録簿の 2 色）。
   エディタの色は一覧に入っていて移したのに、同じ crate の隣のパネルの色は S5a の網に掛かって
   いない（表示の色は「1 組 1 行」に畳む規則が `KINDS` にしか適用されていない）。
   属する話: 共有 crate／一覧の抜け。**直していない**——33 件という範囲を自分で広げないため。
   同じ形で `guide.rs` の余白 3 つ（`add_space(6.0)`、grid の `[14.0, 3.0]`）も一覧に無い。
2. **`HudStyle::padding` と `row_gap` は `Settings` の鍵を持たせなかった**（app からは変えられる）。
   箱の余白まで鍵にすると `hud_*` が 6 つになり、プレイヤーが見たい数（字の大きさ）が埋もれる。
   もし著者が「全部鍵にする」なら 2 行足すだけ。属する話: 共有 crate（設定の粒度）。
3. **`CodePanel` と `Hud` は相変わらずどのゲームからも使われていない。** 今回その 2 つにも
   設定を足したので、「誰も使っていないが公開する」面積がわずかに増えた（S4a の気づきの続き）。
   属する話: 共有 crate（公開の判断）。
4. **`GuidePlugin` / `HudPlugin` / `CodePanelPlugin` が値からユニット構造体でなくなった。**
   設定のフィールドを持たせたので、2 本のゲームの `GuidePlugin,` が `GuidePlugin::default(),` に
   変わっている（動きは同じ）。`EditorPlugin` は元から `with_highlighter(..)` で作られていたので
   呼び方は変わっていない。属する話: 共有 crate（API の一貫性）。
5. **`sabibots` の `selftest` は Bevy の引数の上限ちょうど（16）で止まっている。**
   今回は色を `Editor` の中に置くことで回避したが、次に 1 つでも読むものが増えたら
   `SystemParam` にまとめる必要がある（箱庭は `FakePointer` で既にそれをやっている）。
   属する話: ゲーム固有（Battle の判定の形）。
6. **箱庭の窓のチェックの「狙う点」は右上に固定されている。** 上の §5-1 のとおり、右に張り付いた
   矩形では幅を変えても狙う点が古い矩形に入るので、**幅の設定が効いていなくても通る**。
   本当に効いていることを言いたければ、窓の幅の半分を超える幅で 1 回回すしかない（今回そうした）。
   属する話: 箱庭の判定の作り／本の素材（「設定にした」と「設定が効いている」は別の主張）。
