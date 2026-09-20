# 2026-09-20 S4a: `rubevy-arena` を 2 つに割り、その名前を無くす

計画書 `docs/plans/shared-crate-plan.md` の段階 **S4a**（8 章「名前の検討」と、その末尾の
「決定（著者、2026-09-20）: 案 1」）のみ。ブランチ `shared-crate`（worktree
`rubevy_games-wt-shared`、main の `f6dac4d` の上。**main には触っていない**）。
S1〜S3 と違って `[patch]` は要らない —— rubevy は push 済み（`33d851a`）で、`.cargo/` は消してある。

やったこと:

1. `crates/rubevy-arena` → **`crates/rubevy-egui`**（パネル）と **`crates/games-shell`**（殻）。
   `rubevy-arena` という名前は repo から消えた（`docs/worklog/` の過去の記録と、egui の窓の id を除く）。
2. 計画書 7 章が S4a に回した 2 件: `Editor::default` の語を中立にする、`follow_arena` の再計算の条件を
   本当の条件にする。
3. 未使用で wasm でも動かない `read_script` は移さずに削除。

---

## 0. 着手前の基準（2026-09-20、i7-13700）

| 何を | どう回したか | 結果 |
|---|---|---|
| `cargo test --workspace` | そのまま | **32 通過 / 0 失敗**（garden 5、rubevy-arena 26、doctest 1） |
| 箱庭ヘッドレス・箱庭の窓・Battle ヘッドレス・Battle の窓・ブラウザ 2 本 | **S3 の生ログ**（スクラッチパッド `s3/*.log`）を基準にした。worktree の HEAD は S3 が回したコードと同じ（main に取り込まれ、その後 docs しか動いていない） | §6 |

窓が要るものは、この機械に GPU ドライバが無いので S1〜S3 と同じくコンテナ（lavapipe）で回した。
`docker info` は通ったので**起動はしていない**。`docker/run.sh:23` の `-e SABIBOTS_SELFTEST` 固定は
そのままなので、S2・S3 の担当が書いた `drun.sh`（ゲーム名から環境変数を作る 1 行）を引き継いだ。
`[patch]` が要らなくなったぶん `drun.sh` / `dbuild.sh` から rubevy の worktree の mount を落とした。

ブラウザ版の `web/dist` の比較には、本体が main で同じコミットから回した走行
（スクラッチパッド `merge/*.txt`、`merge/build-s3.log`）も基準に使った。

---

## 1. どこで割ったか —— 継ぎ目は S3 が作っていた

計画書 8 章の表は「A. スクリプトを見せる・書かせる道具」と「B. このサンプル集の殻」を分ける。
実際に割ってみて、**継ぎ目になったのは S3 が入れた `ViewInsets` ただ 1 つ**だった。ほかは
モジュールが素直に片側に寄る:

| ファイル | 行き先 | 中の `crate::` 参照 |
|---|---|---|
| `editor.rs` | `rubevy-egui` | `crate::ViewInsets`（書く側）、`crate::Settings::load` への doc リンク 1 本 |
| `inspect.rs` | `rubevy-egui` | 無し |
| `code.rs` | `rubevy-egui` | 無し |
| `lib.rs` の `Watch` | `rubevy-egui` | 無し |
| `lib.rs` の `ArenaPlugin` / `ArenaSize` / `ArenaView` | `games-shell` | `crate::ViewInsets`（読む側）、テストが `editor::{MARGIN, WIDTH}` |
| `camera.rs` | `games-shell` | `crate::ViewInsets`（定義していた）、`crate::Settings` |
| `guide.rs` / `settings.rs` | `games-shell` | 互いに（`GuideLang::pick` と `Settings`） |
| `platform.rs` / `checks.rs` / `args.rs` / `hud.rs` | `games-shell` | 無し |

### `ViewInsets` をどちらに置いたか

計画書の表は「塞がれ px の Resource（エディタが書く側なのでこちら）」と `rubevy-egui` を指定していて、
**実物もそのとおりだった**ので表を直す必要は無かった。確かめたのは S3 の worklog §2 が言う 3 つ:

- 書くのは `EditorPlugin`（`draw_editor` が egui の窓の実物の矩形から）→ `rubevy-egui` 側。
- 読むのは `follow_arena`（`ArenaPlugin`）と `place_camera` / `report_clicks` / `CameraView::lens`
  （`camera`）→ どちらも `games-shell` 側。**読み手は 2 つあって書き手は 1 つ**なので、書き手と同居させる
  ほうが向きが決まる（読む側は依存を 1 本足すだけで済み、書く側は誰が読むかを知らない）。
- `lib.rs` の `seen_by` と単体テストは `editor::{MARGIN, WIDTH}` を読む。これは `games-shell` →
  `rubevy-egui` の向きなので問題にならない。

定義は `rubevy-egui` の `lib.rs`（`camera.rs` から移した）。`games-shell` は `lib.rs` と `camera.rs` の
両方で `pub use rubevy_egui::ViewInsets;` して、`games_shell::ViewInsets` と
`games_shell::camera::ViewInsets` のどちらでも今までの名前で引けるようにした。

### 逆向きの参照は 1 つだけ出て、文面だった

`cargo build` が通った後で `grep` した残りは `editor.rs` の doc リンク 1 本:

> A plain `fn` pointer rather than a boxed closure, which is what [`crate::Settings::load`] already
> takes for `read` and `write` for the same reason

`Settings` は殻の側なので、`rubevy-egui` からは**型としても名前としても**参照しない形に書き直した
（「a resource holding it stays `Copy`」という、この crate だけで完結する理由に）。
これで `rubevy-egui` の中に `games-shell` の語は 1 つも無い。依存も一方向（`games-shell` →
`rubevy-egui`）で、`Cargo.toml` が 1 行でそう言っている。

### `Cargo.toml` の分け方

| | `rubevy-egui` | `games-shell` |
|---|---|---|
| 常に | bevy、bevy_egui、rubevy、sabiruby | bevy、bevy_egui、**rubevy-egui** |
| PC | `notify`（`Watch`） | `sabiruby-compiler`（`platform::compile` / `highlight`） |
| wasm | 無し | `web-sys` / `wasm-bindgen` / `js-sys` |
| dev（PC） | 無し | `bevy` の `multi_threaded` / `x11`（example の窓） |
| `publish` | **書かない**（今の `rubevy-arena` に合わせた） | `publish = false` |

`publish` の非対称は指示どおり。`rubevy-egui` は「crates.io に出せる形にはしておくが、出すかどうかは
著者が別に決める」なので、今の `rubevy-arena` と同じく `publish` の行を持たない。`games-shell` は
名前からして外に出すものではないので明示した。`description` は 2 つとも書き直した
（`rubevy-arena` の description は「The 2D floor the games in this repository stand on」で、
割った後はどちらにも当てはまらない）。

S1 の報告 6 番が挙げた「`rubevy-arena` が PC で `sabiruby-compiler` に依存する」は、**割ったことで
場所が決まった**: `compile` / `highlight` は `platform` のもので、`platform` は殻。`rubevy-egui` は
コンパイラに依存しない（色を塗る関数は `Highlighter` という `fn` ポインタでゲームから渡る）。

---

## 2. 名前の置換 —— 47 ファイルのうち何を書き換え、何を書き換えなかったか

`grep -rn "rubevy_arena\|rubevy-arena"` は着手時点で repo 内 47 ファイルに当たっていた。

**書き換えた**（指示どおり）: 2 本のゲームの `use` と `rubevy_arena::…`（`garden/src/{main,window,platform,guide_text}.rs`、
`sabibots/src/{main,platform,guide_text}.rs`）、`Cargo.toml` 3 つ、`tools/subset-font.sh`（フォントの出力先と
guide.rs の場所、3 か所）、`CREDITS.md`、`README.md` の Layout、`docs/{garden,sabiruby-battle,web,numbers}.md`、
`docs/plans/{garden,editor-highlight,garden-world}-plan.md`、`docs/README.md`。

置換は機械的にはできなかった。`rubevy-arena` という語は文脈ごとに**どちらの crate を指すかが違う**からで、
1 件ずつ読んで割り振った:

- エディタ・VM パネル・`Watch`・`Editor` のフィールド・`VmClock` の話 → `rubevy-egui`
- 案内（`guide.rs`）・フォント・`platform`・`Settings`・`ArenaPlugin` の話 → `games-shell`
- どちらとも言えない古い文（「共通部分は `rubevy-arena`」など） → 「共有 crate」と書き換えた

**書き換えなかった**:

- `docs/worklog/` の過去の記録 21 ファイル（指示どおり。その時点の名前が正しい）。
- `docs/plans/shared-crate-plan.md` と `factory-plan.md`（main 側で本体が直す）。
- **egui の窓の id `"rubevy-arena-guide"`**（`guide.rs`）。利用者の egui のメモリに窓の位置と大きさが
  この文字列で入っているので、変えると全員の案内の窓が既定の位置と大きさに戻る。旧名が id に残っている
  ことを 4 行のコメントで言った。
- `docs/README.md` の**過去の worklog を要約している 2 行**（S1 の記録と factory-survey）。指示は
  「`docs/README.md` は今の名前に」だったが、この 2 行は「その文書に何が書いてあるか」の要約で、
  指している文書の中では `rubevy-arena` のままなので、書き換えると目次と中身が食い違う。
  代わりに**新しい行（この記録）を足して、そこで名前が変わったことを言った**。
  計画書を要約している 1 行（`plans/shared-crate-plan.md`）は生きている指示書の要約なので今の名前にした。
- **互換の別名 crate は作っていない**（計画書 8 章の指示）。

`docs/numbers.md` は S5a の一覧で、`位置` 欄がファイル名と行番号でできている。ファイルは動いたが
**行番号は取り直していない**（S5b が数を動かすときに取り直す）。§1 の頭に、どのファイルがどちらの
crate に行ったかの 6 行の注意書きを足した。

---

## 3. `Editor` の既定を中立にする（計画書 7 章、S3 の報告 8 番）

S3 が「計画書の前提違い」として止めて報告した件。sabibots は 4 つのうち **`apply_all_label` しか
上書きしていない**ので、既定を中立にすると Battle の見た目が変わる。本体の判断は「既定を中立にし、
sabibots に 3 行書いて見た目を完全に保つ」。

| フィールド | 前の既定 | 後の既定 | sabibots | garden |
|---|---|---|---|---|
| `noun` | `"robot"` | **`"script"`** | 3 行のうち 1 行で `"robot"` に | 既に `"creature"` / `"world"` |
| `apply_label` | `"▶ Apply (F5)"` | **`"▶ Apply"`** | 3 行のうち 1 行で元の文字列に | 既に 2 通りに上書き |
| `apply_key` | `Some(F5)` | **`None`** | 3 行のうち 1 行で `Some(F5)` に | 既に `None` |
| `apply_all_label` | `Some("Apply to all")` | 変えない（既に中立） | 既に `Some(format!(…))` | 既に `None` |

中立語の選び方: `noun` は「このパネルが編集しているもの」で、ゲームが何と呼ぼうと `script` である。
`apply_label` から `(F5)` を落としたのは、**既定が押さえないキーを既定のラベルが名乗るのはおかしい**から。
`apply_key` を `None` にしたのは、どのキーが空いているかはゲームしか知らないから（garden の `F5` は
世界をファイルに書く）。`Ctrl+Enter` は既定のまま常に効く。

**3 行を置く位置**は `show_code` の早い return の**前**にした（`editor.choices` / `editor.selected` を
書いた直後）。garden の同じ 3 行が早い return の前に置かれていて、そこに
「Set before the early returns below, so a frame with nothing selected does not leave `F5` meaning
two things」という理由が書いてある。sabibots も同じ理由で、watch されている機体が無いフレームに
中立の語のボタンが描かれることが無い。

### 見た目が変わっていないことの確かめ方

`noun` はホバー文の中にしか出ない（`run this in the {noun} shown` ほか 2 つ）ので、静止画では写らない。
3 通りで確かめた:

1. **構成から**: 3 つの値は前の既定の**文字列とキーコードそのまま**で、`Editor` のこの 3 フィールドに
   書くのは repo 全体でこの 3 行だけ（`grep -n "\.noun\|\.apply_label\|\.apply_key" garden/src sabibots/src`）。
   描画は毎フレーム `editor.noun.clone()` を読んで `format!` するので、値が同じなら文は同じ。
2. **窓の写真**: コンテナで `--shot` を 6 秒で撮った（スクラッチパッド `s4a/s4a-editor.png`）。
   エディタのボタンが `▶ Apply (F5)` / `Apply to all scout.rb` / `Save to file (Ctrl+S)` / `Revert` で、
   S3 までの見た目と同じ。
3. **窓の selftest**: Battle の窓のチェックは 3 走行とも当たりに依らない 31 行が S3 と完全一致（§6）。
   ただし selftest は `editor.action` を直接書くので **`F5` そのものは押していない** —— キーの確認は
   1 と 2 に拠っている。次に誰かがここを触るなら、窓のチェックに「`F5` を押すと Apply になる」を
   1 段足すのが筋（S4 以降の仕事として挙げておく）。

---

## 4. `follow_arena` が再計算する条件（計画書 7 章、S3 の報告 2 番）

S3 が見つけた事故: `draw_editor` が `ResMut<Editor>` を deref するのでエディタは開いている間ずっと
`is_changed()` が真になり、古い `follow_arena` の早期 return（`size.is_changed() || editor.is_changed()`）は
**そのおかげで**毎フレーム通っていた。だから窓の大きさを変えても追随していた。S3 は
`editor.is_changed()` を `insets.is_changed()` に替えたが、`ViewInsets` もエディタが開いている間は
毎フレーム書かれるので、事故はそのまま引き継がれていた。

### `Changed<Window>` を採らなかった理由

計画書（と S3 の報告）は `Changed<Window>` を挙げている。読んで確かめたところ、**これは条件として
広すぎも狭くもある**:

```
bevy_winit-0.19.1/src/state.rs:284-292
    WindowEvent::CursorMoved { position, .. } => {
        ...
        win.set_physical_cursor_position(Some(physical_position));
```

`win` は `Mut<Window>` なので、**カーソルが 1 px 動くたびに `Changed<Window>` が真**になる。
つまり「ポインタが動いている間は毎フレーム」で、しかも「ポインタが窓の外にある状態で窓の縁を
引っぱって大きさを変えた」ときは真にならない。これでは事故を別の事故に置き換えるだけになる。

### 置いたもの

`follow_arena` が読むのは 3 つだけ —— アリーナの半幅、窓の大きさ、`ViewInsets` —— で、カメラの
置き場所はこの 3 つの関数である。そこで、**最後に置いたときの 3 つを憶えて、変わっていなければ返る**形にした:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed { half: f32, window: Vec2, insets: ViewInsets }

fn follow_arena(..., mut placed: Local<Option<Placed>>) {
    let now = Placed { half, window, insets: *insets };
    if *placed == Some(now) { return; }
    *placed = Some(now);
    ...
}
```

これは「窓が変わった、`ViewInsets` が変わった、アリーナの大きさが変わった」を**そのまま**書いたもので、
`Res::is_changed` の近似でも `Changed<Window>` の近似でもない。止まっている画面での費用は
1 フレームにつき比較 1 回（`f32` 3 つと `Vec2` 1 つ）。

**振る舞いの差は 1 つだけ、しかも良い方向**: エディタが**閉じている**間も窓の大きさの変更に追随する
ようになった。ただし塞がれ px が 0 のときは `slide` が 0 で `scaling_mode` も窓に依らない
（`AutoMin` が窓の形を自分で吸収する）ので、**画面には何も現れない**。塞がれているときだけ効く。

### 追随が前と同じことをどう確かめたか

窓の大きさを手で変える走行は、この機械では窓がコンテナの中にしか開かないので回せない。
代わりに、`follow_arena` が計算する値そのものを単体テスト 2 本で固定した（`games-shell` の `lib.rs`）:

- `a_resize_asks_for_a_new_placement_and_a_still_window_does_not` —— 早期 return の条件を書き出したもの。
  同じ 3 つなら「やることなし」、窓が狭くなった / エディタが閉じた / 壁が寄った、のどれでも別の `Placed`。
- `it_is_the_height_of_the_window_that_moves_the_view` —— 追随が効くのは**窓の高さ**であることの固定。
  `view_of` は高さを枠にするので 1 px は `2 × (half + 3) / height` world unit で、窓が 900 → 600 px に
  なると同じ 528 px のパネルが押しのける world 量は 1.5 倍になる（20.53 → 30.80）。幅は
  1600 → 1200 でも 1e-4 以内で同じ（`AutoMin` が幅を自分で広げる）。

既存の 2 本（`the_slide_is_half_of_whatever_is_covered`、`nothing_covered_leaves_the_arena_where_it_was`）は
無変更で通る。sabibots の窓のチェックも 3 走行とも一致した（§6）。

---

## 5. `read_script` を消した

`crates/rubevy-arena/src/lib.rs` の `pub fn read_script(path: &Path) -> std::io::Result<String>` は
中身が `std::fs::read_to_string` 1 行で、**どのゲームからも呼ばれておらず**、`std::fs` なので
ブラウザ版では動かない（`cfg` も無いので wasm でもコンパイルはされていた）。doc コメントは
「`require` する先も 1 つの文字列にして返す」と書いているが、実装はそれをしていない。
計画書 8 章の指示どおり**移さずに消した**。

`CodePanel`（`CodePanelPlugin` つき）と `Hud` / `HudPlugin` / `ScriptPanel` は、どちらもまだ使われて
いないが使い道が文書にあるので移した（`CodePanel` は egui を使わないゲーム向けで `rubevy-egui` へ、
`Hud` は Battle が文字列の受け皿に使っているので `games-shell` へ）。

---

## 6. 確認

| 何を | 前（S3 / main の走行） | 後（S4a） |
|---|---|---|
| `cargo test --workspace` | 32 通過 / 0 失敗 | **34 通過 / 0 失敗**（games-shell 17、rubevy-egui 11、garden 5、doctest 1）。増えた 2 本は §4 のもの |
| `cargo build --workspace` | 警告 0 | **警告 0** |
| example（PC） | `rubevy-arena` で建つ | `cargo build -p games-shell --example camera` **Finished** |
| example（wasm） | 同上 | `--target wasm32-unknown-unknown` **Finished** |
| ゲームの wasm | 通る | `cargo build -p garden -p sabibots --target wasm32-unknown-unknown --profile web` **Finished** |
| 箱庭ヘッドレス 90 s | ok 13 / FAIL 0 | **ok 13 / FAIL 0**。13 行が数字を伏せて完全一致（差は走行依存の `--` 2 行だけ） |
| 箱庭の窓（docker、3 走行） | S3 は 3 走行中 1 走行 FAIL 1 | **3 走行とも ok 43 / FAIL 0**、43 行が S3 の走行 2・3 と完全一致 |
| Battle ヘッドレス 25 s | 当たりに依らない 3 行 | **同じ 3 行**（`diff` 一致） |
| Battle の窓（docker、3 走行） | 当たりに依らない 31 行 | **3 走行とも 31 行、`diff` 完全一致**。1 走行目だけ**当たり依存**の判定が 1 件 FAIL（下記） |
| ブラウザ `garden/?selftest` 160 s | pageerror 0 / requestfailed 0 / ok 43 | **pageerror 0 / requestfailed 0 / ok 43**、43 行が `diff` 一致 |
| ブラウザ `sabibots/?selftest` 70 s | pageerror 0 / requestfailed 0 / 32 行 | **pageerror 0 / requestfailed 0 / 32 行、`diff` 完全一致** |
| `web/build.sh all` | 通る | 通る。`game_bg.wasm` は sabibots 35,104,574 → **35,111,526 B**（+6,952、+0.02%）、garden 35,823,749 → **35,828,300 B**（+4,551、+0.013%） |

比べ方は S1〜S3 と同じ。Battle は「当たりに依らない行の集合」（`fixedlines.sh`）、箱庭は `ok` と `FAIL` の
行だけを取り出して括弧の中身を落とし、数字を伏せて `sort` してから `diff`。

### Battle の窓、1 走行目の FAIL 1 件について

```
selftest: FAIL 3 blue/scout turned within 0.3 s of the hit at 5.86 s (0.07 rad)
```

これは `fixedlines.sh` が**基準から外している当たり依存の行**そのもの（`of the hit at`）で、
「当たったとき 0.3 s 以内に 0.2 rad 以上向きを変えたか」を当たり 1 回ごとに見る判定。
どの機体がどこでいつ撃たれるかは走行ごとに大きく振れる（S1 の記録: 25 秒で 20 発と 9 発）。
**続けて 2 走行回して FAIL 0**、3 走行とも固定 31 行は完全一致だった。
この段階で Battle に入った変更はエディタの 3 行（描画の語）だけで、物理にも弾にも触っていない。

### wasm が 7 KB ほど増えたこと

crate が 1 つ増えて境界が 1 本増えたぶん。`web` profile は `lto = "thin"` なので crate をまたぐ
インライン化は残るが、シンボル名とパニック時のパス文字列は増える。35 MB の 0.02% で、
`gzip` 後も sabibots 10,731,900 → 10,736,877 B（+0.05%）。**測った事実として書くだけで、
減らす作業はしていない**（S4 の HTML テンプレート化の後に `web/dist` を見るのが計画書の順番）。

---

## 7. 触っていないもの

- `docs/plans/shared-crate-plan.md` と `factory-plan.md`（main 側で本体が直す）。
- `docs/worklog/` の過去の記録。
- egui の窓の id、`localStorage` の接頭辞、`Settings` のキー、ページ側の関数名
  （`window.gardenCompile` ほか）。`web/`・`docker/`・`.github/` には `rubevy-arena` が 1 件も出て
  いなかったので、そもそも直すところが無かった（ビルドはパッケージ名ではなくバイナリ名で呼んでいる）。
- `unsafe` は 1 行も増えていない（`grep -rn "unsafe" crates/ --include="*.rs"` は 2 件で、どちらも
  S1 が書いた「ここは unsafe ではない」というコメント。コードの `unsafe` は 0）。
- `cargo fmt` は走らせていない。`git stash` も使っていない。
- フォントの `include_bytes!("../assets/fonts/…")` は `guide.rs` と `assets/` を一緒に動かしたので
  相対パスがそのまま正しい。`crate_dir!` は `env!("CARGO_MANIFEST_DIR")` を**ゲーム側で**展開する
  マクロなので、crate が動いても答えは変わらない（2 本のブラウザ版とヘッドレス走行が通ったことで確認）。

---

## 8. 2 つの crate の公開 API（3 本目が `use` するもの）

### `rubevy-egui`（`rubevy_egui`）

```
ViewInsets { left, right, top, bottom }  ::NONE  .shift(world_per_px) -> Vec2
Watch  ::new(dir) -> Option<Watch>  .changed() -> Vec<PathBuf>  .dir

mod editor:
  Editor（Resource）… choices / selected / picked / action / key / label / file / in_memory /
      text / current / heat / elsewhere / save_label / message / open /
      apply_label / apply_all_label / apply_key / noun / kinds
      .show(key, running) .changed() .applied(msg) .reset_to(src, msg) .clear() .drafts()
  EditorPlugin  ::default()  ::with_highlighter(Highlighter)
  EditorAction { Apply, ApplyAll, Save, Revert }
  EditorChoice { id, label, color, dim }
  Highlighter = fn(&str) -> Vec<u8>   Highlight(Highlighter)（Resource）
  MARGIN / WIDTH / HEIGHT（エディタの窓の既定の寸法）
  listing(text, heat, kinds) / kind_color(kind) / drawn_kind(editor, at)
mod inspect:
  VmInspector（Resource）… .fill(..) .following() .opened() .own_frames() .insn_per_frame()
      .log_lines() .how() .text()
  VmInspectorPlugin, Waiting, VmClock, VmClockSet, FrameRow, RegRow, REGS_FRAMES,
  vm_clock_start / vm_clock_end
mod code:
  CodePanel, CodePanelPlugin
```

根で再公開しているもの: `CodePanel`, `CodePanelPlugin`, `Editor`, `EditorAction`, `EditorChoice`,
`EditorPlugin`, `Highlighter`, `VmClock`, `VmInspector`, `VmInspectorPlugin`, `Waiting`, `ViewInsets`, `Watch`。

### `games-shell`（`games_shell`）

```
ArenaPlugin { size, floor, follow_shrink }   ArenaSize(f32)   ArenaView { framed, follow_shrink }
ViewInsets（rubevy-egui のものの再公開）
crate_dir!("ruby", "garden/ruby")（マクロ。ゲーム側で展開する）

mod platform:  read / write / compile / highlight / clock_seed / pick_dir / SAVE_LABEL / RubyFiles
mod checks:    selftest_asked(env) / asked_number(env) / CHECKS_EXIT_WHEN_DONE
mod args:      Args ::from_env() .has .value .number .headless .shot
mod settings:  Settings ::load(path, header, read, write) .get .set .number / ReadFn / WriteFn /
               remembered(..) -> (Settings, GuideLang) / environment_language()
mod guide:     Guide（::HINT, ::opening）, GuidePlugin, GuideLang, GuideKey, GuideNote, CJK
mod hud:       Hud, HudPlugin, ScriptPanel
mod camera:    CameraPlugin ::showing(half_height) .centred_on(Vec2) .with(CameraControls)
               CameraHome, CameraView（.lens(window, insets)）, CameraControls（.read_from(&Settings)）,
               CameraKeys, CameraSet { Drive, Place }, PanCamera, Lens（.world_at / .screen_at）,
               WorldClick { at, button, cursor }（Message）,
               notches_of / zoom_by / is_click,
               ZOOM_PER_NOTCH / PIXELS_PER_NOTCH / ZOOM_IN_LIMIT / ZOOM_OUT_LIMIT /
               DRAG_PER_PIXEL / KEYS_PER_SECOND / CLICK_SLOP
```

根で再公開しているもの: `Args`, `CameraControls`, `CameraHome`, `CameraKeys`, `CameraPlugin`,
`CameraSet`, `CameraView`, `Lens`, `PanCamera`, `WorldClick`, `Guide`, `GuideKey`, `GuideLang`,
`GuideNote`, `GuidePlugin`, `Hud`, `HudPlugin`, `ScriptPanel`, `remembered`, `Settings`, `ViewInsets`,
`ArenaPlugin`, `ArenaSize`, `ArenaView`。

3 本目（Factory）が要るのは、おおむね `games_shell::{platform, checks, Args, remembered, Settings,
GuidePlugin, CameraPlugin, WorldClick}` と `rubevy_egui::{EditorPlugin, Editor, EditorAction,
VmInspectorPlugin, VmInspector, Watch}` の 2 行の `use` になる。

---

## 気づいた点（S4a の範囲外。直していない）

1. **`F5` が押されることは、どのチェックでも確かめられていない。** 何を: sabibots の窓のチェックは
   `editor.action = Some(EditorAction::Apply)` を直接書くので、`apply_key` が効いているかは走らせても
   分からない。どこで: `sabibots/src/main.rs:1051,1082,1095`。なぜ気になるか: S4a で `apply_key` の
   既定を `None` にしてゲーム側に移したので、**ここを取り違えても selftest は通る**。窓のチェックに
   「`F5` を押すと Apply になる」を 1 段足せば塞がる（garden 側は `F5` を使っていないので Battle だけ）。
   どこに属するか: ゲーム固有（Battle の判定の穴）／確認の作法。

2. **`ViewInsets` を書くのは今もエディタ 1 つだけ。** 何を: `Hud`（Bevy UI）は画面の上と左を占めるのに
   申告しない。S3 の報告 3 番と同じ話だが、割ったことで**置き場所の問題が 1 つ増えた**: `hud.rs` は
   `games-shell` にあり `ViewInsets` は `rubevy-egui` にあるので、HUD が申告するようになると
   「殻が、パネルの crate の型に書く」形になる（依存の向きとしては正しい）。どこで:
   `crates/games-shell/src/hud.rs`。どこに属するか: 共有 crate（3 本目が HUD の下に世界を置きたく
   なったときに）。

3. **`docs/numbers.md` の `位置` 欄が古くなった。** 何を: S5a の 260 件のうち共有 crate の 33 件は
   `lib.rs:43` のようにファイル名と行番号で書かれていて、ファイルが 2 つの crate に分かれ、
   `lib.rs` は 2 つある。どこで: `docs/numbers.md` §1。なぜ気になるか: S5b がこの表を作業指示として
   使う。注意書きは足したが、行番号は取り直していない。どこに属するか: 計画（S5b の最初にやること）。

4. **`web/dist` が 7 KB 増えた。** 何を: crate 境界が 1 本増えたぶんの、シンボルとパスの文字列。
   どこで: `web/build.sh` の出力。なぜ気になるか: `docs/web.md` が wasm の大きさを記録していて、
   そこに書かれている数（S3 時点）と合わなくなる。0.02% なので直す話ではなく、**S4 で `docs/web.md` を
   触るときに数を取り直す**話。どこに属するか: 文書と実物のずれ。

5. **`code.rs` と `hud.rs` は割った後も 1 度も呼ばれていない。** 何を: `CodePanel` / `CodePanelPlugin` は
   `rubevy-egui` に、`Hud` / `HudPlugin` / `ScriptPanel` は `games-shell` に置いたが、どちらのゲームも
   `HudPlugin` を `add_plugins` していない（Battle は `Hud` を文字列の受け皿として直接書いている）。
   どこで: `crates/rubevy-egui/src/code.rs`、`crates/games-shell/src/hud.rs`。なぜ気になるか:
   `rubevy-egui` を crates.io に出すかどうかの判断材料になる —— 出すなら `CodePanel` は
   「誰も使っていないが公開する」ものになる。どこに属するか: 共有 crate（公開の判断）。

6. **`docker/run.sh:23` の `-e SABIBOTS_SELFTEST` 固定と、`web/build.sh` が `CARGO_TARGET_DIR` を
   見ないこと**は S1・S2・S3 の報告どおりで、この段階でも両方踏んだ（`drun.sh` を引き継ぎ、
   `web/build.sh` は `CARGO_TARGET_DIR` を外して呼んだ）。加えて `docker/run.sh` は
   `/target/<mode>/<game>` 固定なので example も回せない（S3 の報告 5 番）。計画書は全部 S4 に
   置いている。どこに属するか: 道具。
