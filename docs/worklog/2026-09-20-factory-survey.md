# 2026-09-20 3 本目（Factorio 風の 2D 工場）の下調べ

調査のみ。コードは 1 行も変えていない。計画書（`docs/plans/factory-plan.md`）はこの報告を著者が見てから書く。

## 決まっていること（著者、2026-09-20）

- 題材は「Factorio そのもの」ではなく **Factorio の mod 構造の縮小版**: Ruby は (a) data stage（アイテム・レシピ・機械の定義）と
  control stage（イベントへの反応）を骨格に、(b) プレイヤーが Ruby で書く機械を 1 種類。搬送コア（コンベア上のアイテム）は Rust。
- **2D。最初から画像素材を使う。** PC とブラウザ（wasm32-unknown-unknown）の両方。
- **このゲームは rubevy を汎用化するためのサンプルでもある。** 見つかった不足と新しく書くものは「ゲーム固有」と
  「外の利用者も欲しがる汎用のもの」に分け、汎用のものは crate に足す案として出す。ゲームの語彙は crate に持ち込まない。
- カメラ操作も汎用の部品として用意する（置き場所は下の「著者判断待ち」）。

## 1. 規模の実測（詳細は rubevy `docs/worklog/2026-09-20-factory-survey.md`）

i7-13700、release、P コア固定、sabiruby 0.5.2 / bevy 0.19.1、ヘッドレス。

- スクリプト付きエンティティは**毎フレーム動く形で約 1000 台**（フレーム 8.3 ms、1 台 1 回約 6.4 µs = 読み 1 + in-tick の問い 1 + sleep）。
  まばらに起きる形なら 3000 台。飢えは無い（同じ優先度はラウンドロビン、足りなければ全員が等しく遅くなる）。
- **全台が同じ長さ眠ると同じフレームに一斉に起きる**（3000 台・sleep 0.25 で p95 19.5 ms）。機械の sleep はばらす。
- 起動は 1 台約 2.5 µs、メモリは 3000 台で 44 MB。
- **購読 1 本が 1 フレームに受け取れるのは 64 件**（`QUEUE_LIMIT`、あふれは古い方から黙って落ちる）。
  搬送物 1 個ごとのイベントは無理、機械 1 台の「作れた」「詰まった」の粒度なら十分。
- `frame_time`（既定 8 ms）は上限ではなく、実際のフレームは設定値の 2〜4 倍になりうる。
- 同じ `.mrb` を N 台に載せると irep は N 部、差し替えのたびに積み上がって解放されない（sabiruby に `ireps` から消す箇所が無い）。
- 箱庭と同じく、**命令予算は上限の工場に座って測って決める**（箱庭の 45,000 は実測最大 25,837 から）。

→ 分担（搬送は Rust、Ruby は定義・イベント・機械ごとの低頻度の判断）は今の rubevy で成立する。

## 2. data stage（Ruby の宣言 → Rust の表）

- **tick を使わずに書ける。** `Startup` で `ScriptWorld::vm`（`pub`）に `define_fn` で宣言の受け口を足し、
  `Vm::load_and_run` で同期に完走させる（rubevy 自身が prelude をこの形で入れている。`docs/host-api.md` が Startup での使用を許している）。
  キーワード引数はネイティブに末尾の Hash として届くので、`item :iron_plate, stack: 100` は `(Symbol, Serde<ItemDef>)` で受けられる。
  表が揃ってから最初の `Update` が来る。エラーには Ruby の `data.rb:37` が付く。命令予算は通らない（フレームを消費しない）。
- 制約: data stage のスクリプトの中で `Rubevy.ask(...).pop` と `sleep` は使えない（タスクの中だけ）。ネイティブは Bevy の `World` に触れない
  → 表は `Arc<Mutex<…>>` をクロージャに閉じ込めるか `install_host_store::<T>()`。**`Vm::set_host_state` は rubevy が占有しているので使わない**
  （上書きすると `push_command` が黙って何もしなくなる）。
- 箱庭の `garden.rules`（tick の中の 1 往復）は「動いている世界の規則を差し替える」形で、最初の数フレームは表が空になる。data stage には合わない。
- 読み返し（control stage から `recipes[:gear]`）は `define_fn` が `Serde<T>` を返す形が本命（tick の中で 0 フレーム、構造を返せる）。
  `answer_in_tick` は `Vm` を持たないので Hash を読めず、返せるのも平たい `Answer` だけ。
- `require` は `Host` を差し替えれば（`Vm::set_host` は `pub`）ブラウザでも動く。箱庭は `require` を使わず文字列連結している。
  mod のディレクトリの列挙と順序はゲームの仕事。マルチ VM は言語を隔離するが権限は隔離しない。VM の本数はコンパイル時に決まる。
- ブラウザのコンパイルは同期（非同期なのはモジュールのロードだけ）。起動時に何十本もコンパイルするとその間ページが固まる。
- セーブ: スクリプトの ivar は箱庭と同じ道（タスクの `@being` → `@memory` を VM から読む）で保存できる。**タスクの途中は保存できない**
  → 機械のスクリプトは状態を ivar か component に書く設計にする。

## 3. 既存のものの流用

| そのまま使える | 直せば使える | 新しく要る |
|---|---|---|
| `Editor`（bevy + bevy_egui だけに依存、語彙は `noun` に外出し済み）、`VmInspector`（`fill<M>` で 2 本目の VM も）、`Watch`、`Settings`、`Guide`（フォントの再サブセットが要る）、`?selftest` の作法、`answer_in_tick`、bevy の feature（`bevy_sprite` + `png` は既にある）、`pages.yml`（変更不要）、`docker/build.sh` | `platform.rs`（2 本はほぼ同一。garden 側のコメントが「3 本目が来たら共有する価値がある」と予告）、`build.rs`（2 本が 1 バイトも違わない）、エラー行番号の補正（garden だけにあり、**sabibots は prelude 295 行ぶんずれた番号を出すバグのまま** `sabibots/src/main.rs:1685`）、エディタ 4 アクションの骨、`P` の一時停止、`docker/run.sh:23`（`SABIBOTS_SELFTEST` がハードコード） | **タイル格子一式**（座標、設置、セルの向き、受け渡し、マウスでのタイル選択 — 前例 0）、**パン・ズームできる 2D カメラ**、DSL と prelude、`guide_text.rs`、`web/factory.html`（`sabibots.html` から 15 行の差分）、素材と `CREDITS.md`、selftest の中身、`docs/factory.md` |

- `ArenaPlugin` は正方形アリーナの固定カメラで、`Editor` を読んでカメラを画面幅の 16% ずらす（`crates/rubevy-arena/src/lib.rs:103,118`）。工場には使えない。
- `Hud` / `ScriptPanel` のプラグインと `CodePanel`、`read_script` はどちらのゲームからも使われていない。
- `README.md:12` に `factory` が既に planned として載っている（「machines on a line, each with its own script; queues are the conveyors」）。
- 新しい crate を足すとき触る場所: `Cargo.toml` の members、`factory/{Cargo.toml,build.rs,src/platform.rs,src/guide_text.rs,ruby/,assets/}`、
  `web/build.sh:31-33`、`web/factory.html`、`web/index.html`、`tools/subset-font.sh:45`、`docker/run.sh:23`、`README.md`、`docs/README.md`、`docs/web.md`、`CREDITS.md`、`.gitignore`。

## 4. 画像素材と 2D の描画

- 方針は CC0 のみ（`docs/plans/garden-plan.md`）、CC0 でも `CREDITS.md` に出典を書き、パックのライセンス文を素材の隣に置く。
- **第一候補: Kenney「Tiny Factory」1.0**（https://kenney.nl/assets/tiny-factory 、CC0、16×16、132 タイル、`tilemap_packed.png` 192×176 = 4,452 B）。
  工場の床、2 コマのアニメーションつきコンベア、ベルトをまたぐ機械（赤・橙・緑・青）、木箱、パイプ、歯車、クレーン。
- **足りないもの**: コンベアは右向きと上向きの直線だけ（曲がり無し）。インサータ、鉱石、アイテムのアイコン、UI の枠。
  3/4 見下ろしでベルトの手前に側面が描いてあるので、回転では他の向きを作れない（左は反転で作れる）。ベルトの色は 5〜7 色。
- 埋め方: 案 A = Tiny 系で統一（鉱石は Tiny Farm の岩に色、UI は UI Pack Pixel Adventure）し、不足分を同じパレットで自作。
  案 B = Kenney「1-Bit Pack」1 枚（単色透過 17,497 B、矢印・パイプ・機械・枠が揃う）に色を付ける。自作は最小だが記号的。
- 使えなかったもの: itch.io の有料パック（再配布不可）、CC-BY-SA / CC-BY のコンベア、再販禁止つきで「cc0」と称するもの。
- タイルマップは Bevy 本体の `TilemapChunk`（0.17 から。1 チャンク 1 ドローコール、配列テクスチャ、`TileData` に index / color / orientation）。
  `bevy_ecs_tilemap` 0.19.0 も Bevy 0.19 対応（1 タイル 1 エンティティ）。**`TilemapChunk` が WebGL2 で動くかは未確認** → 計画の最初の段階でブラウザ確認。
- スプライトのバッチは「z で整列したとき連続する同じ画像」だけ → アイテムのアイコンは 1 枚のアトラス、アイテムは同じ z の層。数千個の実測はまだ。
- ドット絵は `ImagePlugin::default_nearest()`（既存 2 本は未設定、新しいバイナリだけで足りる）。
- 素材の載せ方は既存と同じ fetch（`web/build.sh` が `assets/` を丸ごとコピー、`AssetMetaCheck::Never`）。`array_layout` は `.meta` に頼れないのでコードで渡す。
- 調査中のダウンロードは scratchpad だけ。リポジトリには何も入れていない。

## 5. 汎用化の候補（どこに置くか）

**rubevy 本体**（外の利用者が `cargo add rubevy` で欲しがる。語彙なし）:

- 宣言を集める口: `T: Deserialize` を名前つきで登録 → スクリプトを同期に完走 → `Vec<(名前, T)>` を取り出す。今の部品（`define_fn`・`Serde<T>`・`load_and_run`）で全部書ける。決めるのは呼び出し規約（名前 + キーワード Hash）。serde 前提の derive にするなら置き場所は `sabiruby-serde` 側。
- Rust の表を Ruby から読み返す口（`expose`）。
- `EmbeddedHost`（`&[(&str, &str)]` から読む `Host`。ブラウザで `require` が動く）と、`.rb` を埋め込む build ヘルパ（2 本の `build.rs` が完全一致）。
- prelude + 本文を 1 プログラムにして `prelude_lines` を返す形と、**エラー行番号の補正**（入力は文字列だけ、依存 0。sabibots のバグも直る）。
- スクリプトの差し替え（`ScriptTask` / `ScriptDone` を外して挿し直す 3 行。host-api.md に節はあるが関数が無い）、一時停止（`budget = 0`。一度見送った案だが 3 本目も同じ 20 行を書く）、wasm 対応の時計の種。
- イベントと予算まわり（rubevy の worklog の A〜H: あふれの通知、上限を選ぶ、購読の索引、共有 publish、`frame_time` を上限に、irep の共有、フレーム単位の統計）。
- 計測の example の常設（`how_many_scripts` / `how_many_subscribers`）。

**共有 crate**（`rubevy-arena` の見直し — `b1ce042` で crates.io 前の懸案になっている — と一緒に）:

- `platform` の `read` / `write`（ファイル ⇄ `localStorage`）、ブラウザのコンパイルの橋（名前を `&str` で渡すなら `js_sys::Reflect`）、`?selftest` の枠、引数解析、起動の Settings / Guide の 10 行、HTML のテンプレート化。
- **パン・ズームできるカメラ**（2D、できれば 3D も）: `MouseWheel::unit` を比で吸収、egui がポインタを持っていれば触らない（`wants_pointer_input` と `is_pointer_over_area` の両方）、クリックとドラッグの分離、`Home`。`ArenaPlugin` → `Editor` の参照は「右が何 px 塞がれているか」の数値リソースに替えれば切れる。
- `Editor` の `Default` が sabibots の語彙（`noun: "robot"`）。

**sabiruby**: 待ちタスクの O(N)、irep の解放。

**ゲーム固有のまま**: タイル座標、エディタの選択肢、何を再起動するか、selftest の中身、レシピ・機械・アイテムの語彙。

## 6. 著者判断待ち

1. 素材: 案 A（Tiny Factory + 自作）か案 B（1-Bit）か。案 A なら、3/4 のベルトを活かして 4 向きを描くか、ベルトだけ真上からの絵で自作して回転で済ませるか。
2. カメラ操作の置き場所: 共有 crate（推奨。rubevy の bevy 依存は `bevy_asset` と `bevy_log` だけで、窓・入力・カメラを持ち込むとヘッドレスの利用者にも付く）か、rubevy の feature か。Ruby からカメラを動かす口（`camera.follow` など）は rubevy の範囲。
3. 汎用化の順番: 3 本目を書く前に crate へ上げる（宣言の口、`EmbeddedHost`、行番号補正、`platform` の共有）か、3 本目をコピーで書いてから上げるか。
4. プレイヤーが Ruby で書く機械を何にするか（インサータ／分配器／グリッドを動く運搬ロボット）。
5. rubevy の A〜H のどれを工場の前にやるか（効果と変更量の比がよいのは C 購読の索引と F irep の共有。E は文書だけでも今より正直）。
6. 残っているもの: rubevy の worktree `rubevy-wt-factory-survey`（計測の example 2 本、未コミット）を常設の example にするか捨てるか。

### 著者の返事（2026-09-20）

- 2 は**共有 crate**（人が操作するパン・ズームのカメラはそちら）。そのうえで「要は Ruby からカメラとか、ゲームに必要な IF を使える口があるとうれしい」。
- これを受けて確かめたこと（rubevy `docs/host-api.md`「Components by name」）: **口の大半はもうある**。
  `Rubevy.find(:Camera2d)` で component を持つエンティティが取れ、`cam[:Transform]` の読み（同じ tick で返る）と
  `cam[:Transform] = {translation: [...]}` の書き（フレームの終わり）はリフレクション経由で、rubevy は `Transform` もカメラも知らない。
  つまり rubevy の Cargo にカメラの依存を足さずに、Ruby からカメラを動かせるはず（型が登録されていること。`DefaultPlugins` なら登録済み）。
- 無いもの: (i) **resource を名前で読む口**（`ReflectResource` は `src/` に 0 件。入力・窓の大きさ・ゲームの設定はここ）、
  (ii) 計算が要る問い（画面座標 → ワールド座標は `Camera::viewport_to_world`。データではないので `ask` で答える側が要る）、
  (iii) 入力をイベントとして流す側、(iv) **使いやすい Ruby の層**（`camera.pan dx, dy` / `camera.zoom 1.2` / `camera.follow e`）。
  `Projection` は enum の component なので、ズーム（`{Orthographic: {scale: …}}` の書き）が今の「現在の variant のフィールドだけ書ける」規則で通るかは**未確認**。
- 見立て: 層を 3 つに分ける。rubevy 本体 = 語彙なしの口（今ある find・component・event・ask に、resource を名前で読む口を足す）。
  rubevy が同梱する**任意の Ruby 層**（`.rb` だけ、Rust の依存 0。find と component の上にカメラなどの書きやすい名前を載せる）。
  共有 crate = 人が操作するカメラ、計算が要る問いに答える側、入力をイベントにする側。計画の最初の段階は「今の rubevy で Ruby から `Camera2d` を動かしてズームできるか」の確認。

- 残りの返事（同日）: 1 = **Tiny Factory + 自作**（ベルトの向きは見本を見て決める）、3 = **先に crate へ上げる**、4 = **インサータ**、
  5 = 挙げた 4 つ全部（購読の索引 + irep の共有、あふれの通知と上限の選択、`frame_time`、フレーム統計 + 計測 example の常設）。
  6（計測の worktree）は 5 の「example の常設」の材料として残す。
- 計画書 3 本を書いた（未着手、着手は著者の指示待ち）: rubevy `docs/plans/generalize-plan.md`（R0〜R9）→ `docs/plans/shared-crate-plan.md`（S1〜S4）
  → `docs/plans/factory-plan.md`（F0〜F6）。宣言を集める口は Bevy が要らないので `sabiruby-serde` に置く案にした。

## 付記

素材調査のエージェントが crates.io の API を最初に 1 回叩いたとき、User-Agent に著者のメールアドレスを入れた（2 回目からは外した。送り先は crates.io のみ）。
以後、エージェントへの依頼文に「個人情報を外部に送らない」を書く。
