# 2026-09-20 S3: カメラ —— 塞がれた px と、人が動かす 2D カメラ

計画書 `docs/plans/shared-crate-plan.md` の段階 **S3** のみ。ブランチ `shared-crate`（worktree
`rubevy_games-wt-shared`、S2 の `cce11ba` の上。main には触っていない）。
S1 が `platform.rs` を、S2 が rubevy の口を共有化したのに続いて、ここで動かすのは**カメラ**:

1. S2 が書き漏らしとして挙げた `replace_script` の残り 2 か所（別コミット `f39f1e7`）、
2. `ArenaPlugin` が `Editor` を知っているのをやめる ——「画面の何 px が塞がれているか」の Resource 経由に、
3. 3 本目が要るもの: パン・ズームできる 2D カメラ（`camera` モジュール）と、その example。

やらなかった 2 つ（Ruby に答える側、`Editor::default` の語）は §7 と §8 に理由を書いた。

## 0. 着手前の基準（2026-09-20、i7-13700）

| 何を | どう回したか | 結果 |
|---|---|---|
| `cargo test --workspace` | そのまま | **21 通過 / 0 失敗**（garden 5、rubevy-arena 16）。S2 の報告と同じ |
| 4 本の selftest | S2 の走行の**生ログ**（スクラッチパッド `s2/*.log`）を基準にした | §6。生ログが残っているので、比べ方（フィルタ）を今回のものに揃えて掛け直せる |

窓が要るものは、この機械に GPU ドライバが無いので S1・S2 と同じくコンテナ（lavapipe）で回した。
`docker info` は通ったので**起動はしていない**。`docker/run.sh:23` の `-e SABIBOTS_SELFTEST` 固定も
そのままなので、箱庭は S2 の担当が書いた `drun.sh`（ゲーム名から環境変数を作る 1 行）を引き継いだ。

rubevy は S2 と同じ一時の `[patch]`（worktree 根の `.cargo/config.toml`、未追跡）。**コミットしていない**し、
`Cargo.lock` もコミット前に `git restore` して除いた（S2 と同じ理由。rubevy が push された後に本体が取り直す）。

## 1. まず小さく: `replace_script` の残り 2 か所

S2 の報告 1 番が挙げた 2 か所を、計画書 7 章の指示どおり最初に別コミットで替えた。

- `garden::wear_the_rules`（`main.rs`）は**世界の VM** の版。`replace_script<M>` は VM の名札 `M` で
  総称化されているので `Script::<World>::for_vm(handle)` がそのまま通る。`ScriptTask<World>` と
  `ScriptDone<World>` を外す 3 行が 1 行になり、`WorldPrelude` を入れ直す行はそのまま残る
  （あれは差し替えの一部ではなく、prelude の長さが変わりうることの始末）。
- `sabibots` のファイル監視からの再読み込み（`reload_changed`）は、**`replace_script` を直接呼ぶのではなく
  `restart` を呼ぶ**ことにした。`restart(commands, entity, name, handle)` は S2 が作った同じゲームの
  包みで、中身は `replace_script(.., Script::new(handle).with_name(name).with_priority(128))` そのもの。
  監視の側は名前も優先度も同じ値を書いていたので、`replace_script` を直接呼ぶと**同じ引数の組を 2 か所に
  書いたまま**になる。1 段深いほうが重複が消える。

`ScriptTask` は garden の `main.rs` でほかにも使われているので `use` は減らない。
`cargo check` は警告 0。

## 2. `ArenaPlugin` が `Editor` を知らなくなる —— `ViewInsets`

### 何が問題だったか

`follow_arena`（`lib.rs`）は `Option<Res<Editor>>` を読んで、エディタが開いていればカメラを
`visible_width * 0.16` だけ右へずらしていた。コメントは「the editor is about a third of the window on
the right: centre the arena in the rest」。`docs/numbers.md` §7-4 が指摘しているとおり、
**1/3 と 0.16 は計算としては合っている**（1/3 塞がれているなら中心のずれはその半分 = 1/6 ≈ 0.167）が、
1 行では繋がらない。そしてそもそも、これは「パネルについての推測がカメラの中に書いてある」状態で、
3 本目のカメラが同じ推測をもう一度書くことになる。

### 置いたもの

```rust
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub struct ViewInsets { pub left: f32, pub right: f32, pub top: f32, pub bottom: f32 }  // 論理 px
impl ViewInsets { pub fn shift(&self, world_per_px: f32) -> Vec2 }
```

名前は中立に（`Editor` も `panel` も入れない）。単位は論理 px —— `Window::width()` と egui の point が
同じ単位なので、書く側（egui）と読む側（Bevy）で換算が要らない。`shift` は「塞がれている側へその半分だけ」で、
`y` は画面の上下を世界の上下に直す。

**書くのは `EditorPlugin`**（計画書は「`Editor` を持つ各ゲーム、または `EditorPlugin`」と選ばせていた）。
エディタの窓はこの crate のものなので、ゲームに 1 行も書かせずに済むほうを採った。値は `MARGIN + WIDTH` の
定数からではなく**実際の `egui::Window` の矩形**から取る: egui はボタンの行に合わせて窓を広げることがあるし、
プレイヤーは窓を掴んで動かせる。動かされた場合に備えて「窓の中心が画面の中心より右なら右の帯、左なら左の帯」と
した（4 行）。上下は誰も塞がないので 0 のまま。

閉じているときは 0 を**変化したときだけ**書く。開いている間は毎フレーム書く（`Editor` 自体が
`draw_editor` の中で毎フレーム `is_changed()` になるので、これは前と同じ頻度で `follow_arena` が回るということ）。
`follow_arena` の早期 return は `size.is_changed() || editor.is_changed()` から
`size.is_changed() || insets.is_changed()` になった。

### 見た目はどう変わったか（実測）

一時的に `info!("S3TMP …")` を 2 か所に入れてコンテナで Battle の窓のチェックを 1 回回し、
数を採ってから外した（コミットには入っていない）:

```
S3TMP editor band: content w 1600 panel Some((1072.0, 1592.0)) -> left 0 right 528
S3TMP follow_arena: window Vec2(1600.0, 900.0) visible_width 124.44444 world_per_px 0.07777777
      insets ViewInsets { left: 0.0, right: 528.0, top: 0.0, bottom: 0.0 }
      slide Vec2(20.533333, 0.0) old-formula 19.91111
```

エディタは実際に **528 px**（`MARGIN` 8 + `WIDTH` 520。窓の右端 1592 = 1600 − 8）を塞いでいて、
1600 px の窓の **33%**。0.16 が表していたのは 32% なので、

- 前: 19.911 world unit ずれていた、
- 後: 20.533 world unit ずれる、
- 差: **0.622 world unit = 画面で 8 px**（1600 px の窓の 0.5%）。

**完全に同じ px ではない。** 同じにする方法は「0.16 のままにする」以外に無く、それでは
「カメラがパネルの幅を推測するのをやめる」という S3 の目的が残らない。8 px は窓の 0.5% で、
Battle の窓の selftest は前後で同じ 31 行（§6）。**数として報告し、判断は著者に渡す。**

単体テストに両方の数を書いて固定した（`lib.rs` の `the_slide_is_half_of_whatever_is_covered`）:
「窓の 1/3 が塞がれているなら新しい規則は `visible_width / 6` を返す」——これが 0.16 の言い直し——と、
「本物のエディタでは 20.533 対 19.911 で、差は窓の 0.6% 未満」。

## 3. パン・ズームの 2D カメラ（`camera` モジュール）

### 何を持ってきて、何を持ってこなかったか

計画書 3.3 の「garden の知見をそのまま」は、**知見**（形）と**数**（値）を分けて考える必要があった。
形は全部そのまま移せる:

- `MouseWheel::unit` を**ノッチに直してから**（`notches_of`）、**差ではなく比で**ズーム（`zoom_by`）。
- egui がポインタを持っていれば触らない。質問は `wants_pointer_input() || is_pointer_over_area()` の**両方**
  （2026-09-18 の理由: egui の `wants_pointer_input` は「ボタンを押したままパネルの上にいる」を含まない）。
- **ドラッグはボタンが下りた瞬間に決める**（`Local<bool>` の `grabbed`）。パネルの上で始まったドラッグは
  どこまで引きずってもカメラのものにならず、世界の上で始まったドラッグはパネルを横切っても続く。
- ホイールは egui が持っていても**読み捨てる**（読まないと、ポインタが外れた瞬間に溜まったぶんが効く）。
- 押した位置と離した位置で**クリックとドラッグを分ける**（`is_click`、`CLICK_SLOP`）。

数は 3 通りに分かれた。

**(a) そのまま移せる数**: `ZOOM_PER_NOTCH` 1.10（無次元の比。garden で**導出**されている ——
30 ノッチで全域）、`PIXELS_PER_NOTCH` 100.0（ブラウザの外の事実）、`CLICK_SLOP` 6.0（画面の px）。

**(b) 2D に読み替えた数**: garden の `PAN_PER_SECOND` 0.9 は「カメラの距離 1 単位あたり毎秒 0.9 world unit」。
距離は「窓がどれだけ世界を抱えるか」を決める数で、2D でそれに当たるのは**視野の半分の高さ**なので、
`keys_per_second = 0.9`（毎秒、視野の半分ぶんの 0.9 倍）とした。ズームの範囲 `ZOOM_MIN` 6 / `ZOOM_MAX` 110 も
同じ理屈で、**home の視野に対する比**（`6.0 / 42.0` と `110.0 / 42.0`、42 は garden の既定の距離）として
書いた。式のまま書いたのは、その数がどこから来たかが式自体に書いてあるようにするため。
比にしたおかげで **garden の単体テストの数がそのまま通る**: 端から端まで
`ln(110/6) / ln(1.1) = 30.5` ノッチ、テストの `29.0..32.0` に入る。
**garden 側のこの 3 つは `docs/numbers.md` で「不明」**（`ZOOM_MIN`/`MAX` は理由のみ、`PAN_PER_SECOND` は不明）
なので、rustdoc にも「garden から引き継いだ、元の出どころは不明」と正直に書いた。

**(c) 引き継がずに導出した数が 1 つ**: garden の `PAN_PER_PIXEL` 0.0016（距離 1 単位あたり 1 px で
0.0016 world unit）は出どころ不明で、しかも 3D には**正解が無い**（地面の点は窓の上と下で距離が違う）。
真上から見た 2D には正解がある —— 1 px はちょうど `2 × half_height / window_height` world unit —— ので、
`drag_per_pixel = 1.0`「押した瞬間にカーソルの下にあった点が、離すまでカーソルの下にある」を既定にした。
これは測ったのでも借りたのでもなく**導出**で、そう rustdoc に書いた。
（正確に 1 : 1 なのは窓の scale factor が 1 のときだけ。Bevy の `MouseMotion` はデバイスの px を運ぶので。
そのことも rustdoc に 1 行。）

**新しく決めなければならない数は 1 つも出なかった。** 出ていたら計画書どおり止まって報告するつもりだった。

### 数の置き場所

`const` は**既定値の名前としてだけ**（計画書 2 章・`CLAUDE.md` の禁止 1 項目め）。実物はすべて
`CameraControls`（Resource）のフィールドで、

1. app が組み立て時に渡せる（`CameraPlugin::showing(h).with(CameraControls { .. })`）、
2. 走っている間に書き換えられる（`ResMut<CameraControls>`）、
3. プレイヤーが `Settings`（`key=value`）から変えられる（`camera_*` の 7 キー、`CameraControls::read_from`。
   起動時に `PreStartup` の 1 system が読む）。

キーとボタンも `CameraKeys` で差し替えられる（全部 `Vec`。空にすればその操作が消える）。
`Settings` から読めるのは**数だけ**にした: キー名を文字列から引くにはパーサが要るし、
世界の端（`bounds`）はゲームのものでプレイヤーのものではない。

`bounds` は `Option<Rect>` の `None` が既定。garden の `PAN_LIMIT` 8.0（壁の外に 8 だけ出られる）は
**引き継いでいない** —— 汎用の crate は世界に端があるかどうかを知らないので、埋め込みの既定値を置くと
それこそ根拠の無い数になる。ゲームが言う。

### 公開した口

```rust
pub struct CameraPlugin { pub home: CameraHome, pub controls: CameraControls, pub from_settings: bool }
    CameraPlugin::showing(half_height: f32) -> Self   // 既定は無い。窓が抱える世界の量はゲームしか知らない
        .centred_on(Vec2) .with(CameraControls)
pub struct CameraHome { pub focus: Vec2, pub half_height: f32 }   // Home キーの戻り先、ズーム範囲の基準
pub struct CameraView { pub focus: Vec2, pub half_height: f32 }   // 今どこを見ているか（ゲームが書いてよい）
pub struct ViewInsets { pub left, right, top, bottom: f32 }       // 塞がれた px
pub struct Lens { pub centre: Vec2, pub world_per_px: f32, pub window: Vec2 }
    Lens::world_at(cursor) -> Vec2 / screen_at(world) -> Vec2
pub struct WorldClick { pub at: Vec2, pub button: MouseButton, pub cursor: Vec2 }  // Message
pub enum CameraSet { Drive /* Update */, Place /* PostUpdate */ }
pub struct PanCamera;  // このプラグインが立てた Camera2d の目印
```

`Lens` を値として切り出したのは、**クリックの答えとスクリプトの答えが同じ 1 つの計算**だからで、
§7 の受け口もこれになる。

## 4. 単体テスト（rubevy-arena 16 → 26 本）

- `a_notch_is_a_notch_in_either_unit` —— garden の同名テストと**同じ値**（Line 1.0、Pixel 100.0、Pixel −300.0）。
- `ten_notches_are_ten_steps_and_not_the_end_of_the_range` —— garden と同じく、既定（42）から 10 ノッチを
  px と line の 2 通りで刻んで、毎回 1.10 倍で、10 回目が同じ所に着くこと。
- `the_range_is_reached_but_not_passed` —— 比で書いた上限・下限が garden の 6 と 110 になること、
  端で止まること、端から端が 29〜32 ノッチであること。
- `a_press_that_travelled_is_a_drag_and_not_a_click` —— 5 px はクリック、6.0 ちょうどもクリック、6.1 はドラッグ。
- `a_covered_edge_moves_the_middle_and_nothing_else` —— 1600×900 で右 528 px 塞がれたとき、
  中心が 264 px ぶん右へ動き、`world_per_px` は変わらず、**狙っている点が残りの領域の真ん中に描かれる**こと。
- `a_covered_top_and_bottom_move_it_the_other_way` —— 上下でも同じ（符号の確認）。
- `a_cursor_and_a_world_point_are_the_same_point_read_two_ways` —— `world_at` と `screen_at` の往復。
- `the_store_changes_a_number_and_leaves_the_rest` —— `Settings` に 1 キー書くとそれだけ変わる。
- `lib.rs` に 2 本（§2 の 0.16 の言い直しと、塞がれていなければ動かないこと）。

## 5. example（`crates/rubevy-arena/examples/camera.rs`、187 行）

画像なし。12×12 の色つき四角（角の 1 枚だけ橙）を敷いて、パン・ズーム・`Home`、クリックした
world 座標とそれが何番目の四角かを表示する。egui の窓を 1 つ開いていて、

- その上ではカメラが動かない（ドラッグもホイールも）、
- 窓のチェックボックスで「この窓は px を塞いでいると申告する」を切り替えられるので、
  `ViewInsets` が効いている / いないを目で比べられる。

`cargo run -p rubevy-arena --example camera -- 5` で 5 秒後に自分で閉じる（手の無い機械用。
`--` の後の数を秒として読むだけ）。ライブラリの `[dev-dependencies]` に
`bevy = { features = ["multi_threaded", "x11"] }` を足した —— 窓を開ける機能はゲーム側の `Cargo.toml` に
あってライブラリには無かったため。ゲームのビルドには入らない。

確認: コンテナ（lavapipe + WSLg）で本物の窓が開き、8 秒走って自分で終わった。

```
INFO bevy_winit::system: Creating new window rubevy-arena: the camera (62v0)
INFO camera: camera example: 8.01 s, looking at 0.00, 0.00, half-view 16.00, covered right 300 px
```

`web/` には載せていない（計画書どおり）。wasm のビルドは通る:
`cargo build -p rubevy-arena --example camera --target wasm32-unknown-unknown` が `Finished`。

## 6. 確認

| 何を | S2 の後（基準） | S3 の後 |
|---|---|---|
| `cargo test --workspace` | 21 通過 / 0 失敗 | **31 通過 / 0 失敗**（garden 5、rubevy-arena 26）+ doctest 1 |
| 箱庭ヘッドレス 90 s | ok 13 / FAIL 0 | **ok 13 / FAIL 0**。判定の行は数字を伏せて完全一致（差は走行依存の `--` 3 行だけ） |
| 箱庭の窓（docker、3 走行） | ok 43 / FAIL 0 | 走行 2・3 は **ok 43 / FAIL 0 で 43 行の判定が完全一致**。走行 1 だけ `FAIL Apply restarts every beetle on the edited text` が 1 件（下記） |
| Battle ヘッドレス 25 s | 当たりに依らない 3 行 | **同じ 3 行**（`diff` 一致） |
| Battle の窓（docker） | 当たりに依らない **31** 行 | **31 行、`diff` 完全一致** |
| ブラウザ `sabibots/?selftest` 70 s | pageerror 0 / requestfailed 0 / 固定 **32** 行 | **pageerror 0 / requestfailed 0 / 32 行、`diff` 完全一致**（ok 43 / FAIL 0 / -- 1 / done 1） |
| wasm | — | `cargo build -p garden -p sabibots --target wasm32-unknown-unknown --profile web` 通る。`web/build.sh sabibots` も通り、`game_bg.wasm` 35,086,472 B（gzip 10,725,414）で S2 の 35.1 MB / 10.7 MB と同じ |

### 箱庭の窓の FAIL 1 件について

1 走行目の `FAIL Apply restarts every beetle on the edited text` は、**S1 の報告 5 番が無改変の状態で
記録した揺れ**（3 走行中 2 走行が 1 件ずつ FAIL。どれもエディタの Apply / Revert / 再起動の周り）と
同じ場所。2 走行目と 3 走行目は FAIL 0 で 43 行が基準と一致した。
この段階で箱庭に入った変更は `wear_the_rules`（**世界の VM**、`world.rb` の差し替え）と
`EditorPlugin` が `ViewInsets` を書くことの 2 つだけで、落ちた判定は**種のファイル**の Apply
（`restart_species`。S2 が替えたもので、この段階では触っていない）。とはいえ
**1 走行の再現では原因は割れない**ので、「S1 が記録した揺れと同じ場所・同じ顔で、3 走行中 1 走行」
という事実だけを書いておく。

### 比べ方

Battle は S1・S2 と同じく「当たりに依らない行の集合」（`fixedlines.sh`）。箱庭は `selftest:` の行を
数字を伏せて `sort` してから `diff`。ただし箱庭の生の行には**判定でない行**（`--` の n/a、
`Beetle NvN — N ask round trips …` の要約、`miss …` の probe の記録）が混ざっていて、
これらは走行ごとに出たり出なかったり順番が入れ替わったりする（Battle の「当たりに依らない行」と同じ事情）。
そこで箱庭は **`ok` と `FAIL` の行だけ**を取り出して括弧の中身を落としてから比べた。43 行・13 行はその数。

## 7. やらなかったこと: Ruby に答える側（→ `factory-plan.md` F5）

計画書 3.3 の最後の項目「rubevy の任意の Ruby 層（R9）が投げる `camera.world_at` などに答える system」は、
**R9 が著者判断待ちなので送った**（計画書がそう指示している）。

受け口は用意してある。答える側を足すとき、必要なものは **3 つの Resource と 1 つの関数**だけ:

> `Res<CameraView>` と `Res<ViewInsets>` と `Query<&Window>` を取る system を
> `RubevySet::Answer`（または `answer_in_tick`）に置き、`view.lens(window_size, &insets)` で `Lens` を作って
> `lens.world_at(cursor)` / `lens.screen_at(world)` を返す。カメラを**動かす**問い（`camera.look_at`、
> `camera.zoom`）は `ResMut<CameraView>` に書くだけで、`place_camera` が次の `PostUpdate` に反映する
> （`CameraView` を毎フレーム読んで Transform を作る作りにしたのはこのため。Transform を直接書く作りだと
> Ruby の書き込みと人の操作が競合する）。`ArenaPlugin` を使っているゲーム（sabibots）には `CameraView` が
> 無いので、そちらにも答えたいなら `ArenaView` から同じ `Lens` を作る 3 行が要る。

## 8. 止まって報告する 1 件: `Editor::default` の `noun`

計画書 2 章と指示は「`Editor` の `Default`（`noun: "robot"` ほか）を中立の語に変える。
**2 本とも上書きしているので見た目は変わらないはず。変わるなら止まって報告**」だった。
調べたところ**前提が違う**:

| 既定値 | garden | sabibots |
|---|---|---|
| `noun: "robot"` | `"creature"` / `"world"` に上書き（`window.rs:266,282`） | **上書きしていない（既定のまま使っている）** |
| `apply_label: "▶ Apply (F5)"` | 上書き（`window.rs:281,306`） | **上書きしていない** |
| `apply_key: Some(F5)` | `None` に上書き（`window.rs:264`） | **上書きしていない** |
| `apply_all_label: Some("Apply to all")` | `None` に上書き（`window.rs:265`） | `Some(format!("Apply to all {}", …))` に上書き（`main.rs:772`） |

つまり sabibots は 4 つのうち 3 つを既定のまま使っていて、`noun` を中立の語にすると
**Battle のボタンのホバー文が変わる**（"run this in the robot shown" → "run this in the … shown"）。
`apply_label` の "▶ Apply (F5)" と `apply_key` の `F5` も同じで、これらは「共有 crate の既定値が
たまたま Battle の言葉になっている」状態。直し方は 2 つあり、どちらを採るかは見た目の判断なので**決めずに置いた**:

1. 既定を中立にし、sabibots 側に `editor.noun = "robot"` ほか 3 行を書く（見た目は完全に不変）。
2. 既定を中立にして、Battle のホバー文が中立の語になるのを受け入れる。

## 9. 触っていないもの

- garden の 3D オービット（`orbit_camera`、`Orbit`、`zoom_by`、`notches_of` とその単体テスト）は
  計画書どおり**1 行も触っていない**。`camera` モジュールは garden の数と形を**写した**だけで、
  garden がそれを使うようには**していない**（3D と 2D で同じコードにはならないし、共有化は
  2 本目の 3D の客が来てからという計画書 3.3 の線引きに従う）。
- 使われていないもの（`HudPlugin`、`CodePanel`、`read_script`）は消していない。
- `cargo fmt` は走らせていない。`git stash` も使っていない。
- `docs/numbers.md` は main 側にしか無い（S5a が main に入ったのは S2 の後）。この worktree からは
  触らない。`ArenaPlugin` の 0.16 の行（§7-4 の指摘）が S3 で消えたことは、本体が main で写すときに。

## 気づいた点（S3 の範囲外。直していない）

1. **`+ 3.0`（壁の外に見せる床）がまだ 2 か所に直書きされている。** 何を: `view_of` と `seen_by` が
   どちらも `half + 3.0` と書いている。どこで: `crates/rubevy-arena/src/lib.rs:112,119`。
   なぜ気になるか: この段階で `follow_arena` から 0.16 を消したので、`ArenaPlugin` に残る出どころの
   薄い数はこれだけになった。`docs/numbers.md` の (c) に「`ArenaPlugin` の設定値へ」として既にある。
   関数を 1 つ増やしたぶん**書かれている場所が 1 つ増えた**ので、S5b で設定値にするときはその 2 か所。
   どこに属するか: 共有 crate（S5b）。

2. **`Editor` は開いている間ずっと「変わった」ことになる。** 何を: `draw_editor` が
   `ResMut<Editor>` を `reclassify` のために deref するので、パネルが開いている間は毎フレーム
   `is_changed()` が真。どこで: `crates/rubevy-arena/src/editor.rs`。なぜ気になるか: 古い
   `follow_arena` はこれを早期 return の条件にしていたので、**エディタが開いている間は毎フレーム
   カメラを計算し直していた**（そのおかげで窓を掴んで大きさを変えたときも追随していた）。
   `ViewInsets` も同じ頻度で書くようにしたので振る舞いは変えていないが、これは意図ではなく事故で、
   本当に要るのは「窓の大きさが変わったら計算し直す」。`Changed<Window>` を条件に足すのが筋。
   どこに属するか: 共有 crate（`ArenaPlugin` の変更検知）。

3. **HUD もパネルなのに `ViewInsets` を書かない。** 何を: `Hud`（Bevy UI）は画面の上と左を占めるが、
   塞がれ px を申告していない。どこで: `crates/rubevy-arena/src/hud.rs`。なぜ気になるか: 今は
   カメラが上下を読んでも 0 のままで、`ViewInsets` の上下 2 つは example でしか動かない。
   3 本目が HUD の下に世界を置きたくなったときに初めて要る話なので足していない。
   どこに属するか: 共有 crate（F5 / 3 本目が要るときに）。

4. **`docker/run.sh:23` の `-e SABIBOTS_SELFTEST` 固定と、`web/build.sh` が `CARGO_TARGET_DIR` を
   見ないこと**は S1・S2 の報告どおりで、この段階でも両方踏んだ（`drun.sh` を引き継ぎ、
   `web/build.sh` は `CARGO_TARGET_DIR` を外して呼んだ）。計画書は両方 S4 に置いている。
   どこに属するか: 道具。

5. **コンテナから example を回す道が無い。** 何を: `docker/run.sh` は `/target/<mode>/<game>` しか
   実行できないので、`examples/` の実行ファイルはスクラッチパッドの `drun-example.sh` で回した。
   どこで: `docker/run.sh:26`。なぜ気になるか: この機械では窓が開くのがコンテナの中だけなので、
   これから増える example は毎回同じ迂回をすることになる。1 引数（実行ファイルのパス）で済む。
   どこに属するか: 道具（`docker/`、S4 で `run.sh` を触るときに）。

6. **箱庭の窓のチェックの揺れが、この段階でも 3 走行中 1 走行出た**（§6）。S1 の報告 5 番と同じ場所
   （Apply / Revert / 再起動の周り）で、S1 は無改変で 3 走行中 2 走行、S2 は 1 走行だけで 0 件だった。
   これで「無改変でも、変更後でも出る」という観測が 2 つになった。原因は割れていない。
   どこに属するか: ゲーム固有（箱庭の判定）／バグ候補。**著者判断待ち**（S1 の報告で既に挙がっている）。
