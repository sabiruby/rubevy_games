# 3 本目: Factory（Factorio の mod 構造の縮小版、2D）— 実装指示書

作成 2026-09-20。著者「サンプルゲームで Factorio 風のものを作ってみたい」「2D がよい。最初から画像素材は使いたい」
「汎用的にできるものはクレートに追加して、汎用化のためのサンプルでもあることを意識して」。
調査は `docs/worklog/2026-09-20-factory-survey.md`（**着手前に全部読む**）と rubevy `docs/worklog/2026-09-20-factory-survey.md`（規模の実測）。
前提になる計画: rubevy `docs/plans/generalize-plan.md`（R0〜R10）、`shared-crate-plan.md`（S1〜S5b）。**この 2 つが先**（要るのは R0〜R9 と S1〜S4。数の棚卸し R10・S5 は並行でよい）。
対象: rubevy_games main `b1ce042` + 上の 2 つの結果、Bevy 0.19.1。

---

## 0. はじめの一歩

```bash
cd /home/kishima/book/kishima
git -C rubevy_games worktree add ../rubevy_games-wt-factory -b factory main   # S1〜S8 と S5b は main に入っている（2026-09-21）
cd rubevy_games-wt-factory && cargo build --workspace --all-targets && cargo test --workspace   # 警告 0、60 passed
```

作法は `/home/kishima/book/.claude/agents/implementer.md` と `/home/kishima/book/CLAUDE.md`。段階ごとに 1 コミット以上、push しない、main に触らない、
過程は `docs/worklog/` に書きながら。**Docker Desktop を起動しない。**

**2026-09-21 の時点で使えるもの**（計画を書いた後にできたもの。名前はここが正しい）:

- 共有 crate は 2 つ: `rubevy_egui`（`Editor`・`EditorLayout`・`EditorColors`・`VmInspector`・`InspectStyle`・`CodePanel`・`Watch`・`ViewInsets`）と
  `games_shell`（`platform`・`checks`・`Args`・`Settings`・`remembered`・`PanelSettingsPlugin`・`Guide`・`Hud`・`ArenaPlugin`・
  `CameraPlugin::showing(half_height)`・`CameraView`・`CameraControls`・`WorldClick`・`Lens`・マクロ `crate_dir!`）。`rubevy-arena` はもう無い。
- rubevy（main）: `Program`・`in_the_authors_lines`・`replace_script`・`EmbeddedHost`・`ScriptWorld::require_from`・build ヘルパ `rubevy-build`、
  `Rubevy.resource` / `set_resource`、`Rubevy.rejected_writes`、`Rubevy.next_frame` / `each_frame`、任意の層 `rubevy::layers::CAMERA`（`Rubevy::Camera`）、
  `ScriptWorld::{queue_limit, dropped, last_frame, loaded_programs, broken_programs, max_depth}`、`Rubevy.subscribe(name, limit:)`。
  `frame_time` は本当の上限。計測器 `examples/how_many_scripts` / `how_many_subscribers`、`docs/verification/scale.md`、`docs/numbers.md`。
  **games の `Cargo.lock` は rubevy `d347711`（R10 まで）を指している。F0 の最初に rubevy の今の main へ上げる**（S7 がやった手順: `cargo update -p rubevy`、
  6 通りの走行が一覧と一致、`web/build.sh all` + Playwright）。
- sabiruby（main）: `sabiruby_serde::declare`（`Declarations::<T>::install(&mut vm).define(&mut vm, "item")` → `load_and_run` → `take`、`expose`）。
  games の `[patch.crates-io]` は sabiruby の git を見ているが lock の rev は古い（`7be7b86`）。**F2 の最初に sabiruby の rev を上げる**（`pages.yml` が lock から rev を grep して
  Playground のコンパイラを同じ VM から建てるので、上げたら CI まで見る）。
- 道具と確認: `web/games.sh`（ゲームを足すのは 1 語 + 値 1 組）+ `web/page.html.in`、`web/build.sh`（`CARGO_TARGET_DIR` を見る）、`docker/run.sh`（`<GAME>_` の環境変数を全部渡す、`--example`、
  target の volume は worktree ごと）、`tools/fixedlines.sh` と `docs/verification/selftest-lines.md`（**確認の基準は行の一覧**。3 本目の一覧もここに足す）、
  ブラウザのつまみ `?selftest&<name>=<value>`（PC の `<GAME>_<NAME>` と同じ名前で `games_shell::checks` が読む）、`docs/numbers.md`（数を足したら行を足す）。
- 確認の作法で分かったこと: 前後は行の集合で比べる／計測は交互に 2 巡以上・別の target・`md5sum`／混み具合は `vmstat` の idle／判定は秒ではなく条件で待ち、上限は導く／
  `Query` の並び順に依る比較を書かない／「設定にした」と「設定が効いている」は別の主張／`cargo build --workspace --all-targets`（`--all-targets` が無いと example を見ない）。

---

## 1. 何を作るのか（30 秒版）

見下ろし 2D のグリッドに、鉱石 → 採掘機 → コンベア → インサータ → かまど・組立機 → 納品、の線を引くゲーム。**Factorio の構造を小さく写す**:

| Factorio | ここでは |
|---|---|
| C++ のコア（搬送、製造） | Rust。コンベア上のアイテムを Ruby は 1 個ずつ触らない |
| data stage（Lua の `data:extend`） | `ruby/data.rb`: `item` / `recipe` / `machine` の宣言 → Rust の表。tick を使わず Startup で完走（R7 の口） |
| control stage（Lua の `script.on_event`） | `ruby/control.rb`: `on(:built)` / `on(:crafted)` / `on(:delivered)` と目標（何をいくつ納品したら勝ち） |
| 回路ネットワーク | **インサータ 1 台ごとの Ruby**。プレイヤーがゲーム内エディタで書き換える |

rubevy の見本として見せるもの: 宣言を集める口、機械数百台のタスク（irep の共有、`sleep` をばらす）、機械の粒度のイベントとあふれの数、
tick の中で答える格子の問い、フレーム統計の HUD、Ruby から動かすカメラ。

**やらないもの**: 電力、流体、敵、研究ツリー、地下ベルト、列車、ブループリント、マルチプレイ。足したくなったら報告に書く。

---

## 2. 決まっていること・既定

### 決まっている（著者、2026-09-20）

- 2D、最初から画像素材。素材は **Kenney「Tiny Factory」（CC0）を軸に、足りない部品は同じパレットで自作**。
- Ruby の役割は (a) data stage + control stage を骨格に、(b) プレイヤーが書く機械は**インサータ**。
- 汎用にできるものは crate へ。ゲームの語彙を crate に持ち込まない。見つけたら実装せず報告に書く（本体が rubevy / 共有 crate の計画に足す）。
- PC とブラウザ（wasm32-unknown-unknown、WebGL2）の両方。Pages に 3 本目として載せる。
- 根拠のない数を書かない。unsafe はなるべく避ける（このゲームに要る場面は無いはず。要ると思ったら止まって報告）。

### 既定（違和感があれば止めて報告する）

- crate 名 `factory`、Pages は `/rubevy_games/factory/`、設定は `factory.settings.txt`、セーブは `factory.save.json`、`localStorage` の接頭辞 `factory:`、
  selftest は `FACTORY_SELFTEST` と `?selftest`。
- VM は 1 本（data stage・control stage・インサータが同じ VM。data stage で定義した Ruby の定数を control が使える）。mod を別 VM にする話は**しない**。
- ライセンスは CC0 のみ。CC0 でも `CREDITS.md` に出典・取得日・どのファイルを何に使ったかを書き、パックの `License.txt` を素材の隣に置く。
- 素材は fetch で載せる（`web/build.sh` が `assets/` をコピー、`AssetMetaCheck::Never`）。`ImagePlugin::default_nearest()`。
- 数（地図の大きさ、ベルトの速さ、機械の上限、命令予算、インサータの `sleep`）は**どれもこの文書では決めない**。3.7 の決め方に従う。
- **数は利用者が変えられる場所に置く**（著者、2026-09-20「マジックナンバーは基本的に禁止。ユーザが変えられるようにするべき」）。
  遊びの数（速さ、時間、容量、レシピ、目標）は **`data.rb` / `control.rb`**（プレイヤーがエディタで変えられる。data stage がそのためにある）。
  動かす側の数（予算、`frame_time`、上限、`limit:`、表示の倍率）は **`factory.settings.txt`** と起動の引数。
  Rust の `const` にしてよいのは、変えると壊れる不変量（セーブの版、素材の 16 px）だけ。箱庭は規則の数がほぼ全部 Rust の `const` だった
  （`docs/worklog/2026-09-17-garden-world-survey.md`）。同じ形にしない。測って決めた数は「既定値」であって、上限そのものではない。
- **数には理由を残す**（著者、2026-09-20「基本は設定可能なように、理由があるなら理由も残して」）。`docs/numbers.md`（`shared-crate-plan.md` の S5 が作る表）に
  Factory の節を足し、数を足す段階ごとに行を足す: 名前、既定値、どこで変えるか、出どころ（測った日と条件／導出／**不明**）。
  `data.rb` の数はその行のコメントに「何に対してこの値か」。理由の無い数は「遊んで決めた、根拠なし」と正直に書き、もっともらしい理由を作らない。

---

## 3. 設計

### 3.1 格子と搬送（Rust、F1）

- タイル座標（整数の組）⇄ ワールド座標。1 タイルの表示の大きさは素材の 16 px の整数倍。
- 1 タイルに建物 1 つ（大きい機械は複数タイルを占める）。向き 4 つ。設置と撤去はマウス（共有 crate のカメラのクリックのメッセージ → タイル）。
- **コンベア**: アイテムはベルト上の位置を持ち、Rust の system が進める。詰まれば止まる。曲がりと合流の規則は最小（直線、L 字、横からの合流）。
  アイテムはエンティティにするか、ベルトごとの列にして描画だけスプライトにするかを、**数千個で両方測って**決める（3.7）。
- 描画: 地面と建物は `TilemapChunk`（Bevy 本体。F0 で WebGL2 の確認が通れば）。アイテムのアイコンは**1 枚のアトラス**、**同じ z の層**
  （バッチは z で整列したとき連続する同じ画像だけ。`bevy_sprite_render-0.19.1/src/render/mod.rs`）。
- ヘッドレスのテスト: ベルトが運ぶ、詰まる、合流する、機械が作る。描画なしで回る形に（garden / sabibots の headless と同じ作り）。

### 3.2 data stage（F2）

```ruby
item    :iron_ore,   stack: 50, icon: 3
item    :iron_plate, stack: 100, icon: 4
recipe  :iron_plate, in: { iron_ore: 1 }, out: { iron_plate: 1 }, time: 3.2, made_in: :furnace
machine :furnace,    size: [2, 2], speed: 1.0, sprite: 75
```

- 受け口は R7 の口（`sabiruby-serde`）。`Startup` で `ScriptWorld::vm` に登録 → `platform::compile` → `Vm::load_and_run` → 表を Resource へ。
  **最初の `Update` の前に表が揃っている。** data stage のスクリプトの中で `Rubevy.ask(...).pop` と `sleep` は使えない。
- 検査は serde の後の規則として書く（存在しないアイテムを参照するレシピ、`time` が 0 以下、大きさ 0 の機械）。エラーは Ruby の `data.rb:行` つきで画面に出し、
  ゲームは始めない（エディタで直して再読み込みできる）。
- 上の数値は**例**。実際の値はゲームの調整で決め、`data.rb` のコメントに「何に対してこの値か」を書く（かまど 1 台がベルト 1 本を何割埋めるか、など）。
- control と インサータからの読み返し: `recipes[:iron_plate]`、`items[:gear]`（R7 の読み返しのヘルパ。tick の中で 0 フレーム）。
- ブラウザではコンパイルが同期でページを止める。起動時にコンパイルするファイルは少なく保つ（data / control / prelude / インサータの既定）。

### 3.3 インサータの Ruby（F3）

```ruby
inserter "Smart" do
  def run
    loop do
      thing = behind            # 後ろのタイルにある取れるもの（無ければ nil）。tick の中で返る
      if thing && front.accepts?(thing) && wants?(thing)
        move thing               # 腕を振る。振り終わるまでこのタスクは待つ
      else
        idle                     # ばらした長さだけ眠る
      end
    end
  end

  def wants?(thing) = thing.name != :stone
end
```

- 上は**形の案**。DSL の語（`inserter` / `behind` / `front` / `move` / `idle` / `wants?`）と、`on(:arrived)` のようなイベント駆動の形にするかは、
  garden の prelude（`on`、`def run`、`me[:X]`、`Rubevy::Proxy`）に合わせて F3 の最初に決め、worklog に理由を書く。
- **読む問いは tick の中で答える**（`answer_in_tick`。格子の隣のタイルを引くだけ）。クロージャは `Vm` を持てず、返せるのは平たい `Answer` だけ
  → 「何があるか」はアイテムの番号や数で返し、名前への変換は prelude が data stage の表でやる。
- **`move` は時間のかかる動作**。腕の動きは Rust。終わるまで待つ形を、(a) `Rubevy.ask("factory.move", …).pop` にゲームが後のフレームで答える
  （`Request` を持ち続けてよいか、その間に機械が撤去・スクリプトが差し替えられたときどうなるか）、(b) イベントを待つ、(c) `sleep` してから確かめる、
  から**調べて選ぶ**。ここは rubevy の汎用の話（「完了まで待つ動作」）になりうるので、分かったことを報告に書く。
- **全台が同じ長さ眠らない**（実測: 3000 台・同じ `sleep` で p95 が 20 ms）。`idle` は prelude が台ごとにずらす。ずらし方と長さは 3.7。
- 既定のスクリプトは全インサータで 1 本（R2 で irep は 1 つ）。エディタで 1 台だけ書き換える／同じスクリプトの全台に適用する、の 2 つ
  （`Editor` の `apply_label` / `apply_all_label`）。差し替えのたびに古い irep が VM に残る（sabiruby 側の既知の穴）ので、差し替え回数と `ireps` を HUD かログで見えるように。
- 壊れたスクリプト（例外、`Task::Overrun`）の機械は止まり、機械の上に印を出す。ゲームは続く。
- セーブはタスクの途中を持てない。インサータの状態は ivar か component に書く（`run` は頭から始め直せる形）。

### 3.4 control stage（F4）

- `ruby/control.rb` は 1 つのスクリプト（1 タスク + `on` のハンドラ）。イベントは**機械の粒度**だけ: 設置、撤去、製造完了、納品、詰まり。
  **搬送物 1 個ごとのイベントは出さない**（購読 1 本は 1 フレーム 64 件が天井）。
- 目標: `goal deliver: { science: N }` のような宣言と、`on(:delivered)` で数えて勝ちを publish する形。N は遊んで決める。
- あふれは `Subscription#dropped`（R3）で数え、HUD に出す。0 でないなら設計（粒度）を見直す合図。

### 3.5 窓（F5）

- `rubevy-egui`: `Editor`（インサータ、`data.rb`、`control.rb` を選べる）、`VmInspector`、`Watch`。`games-shell`: `platform`、`checks`、`Args`、`Guide`（英日、`factory/src/guide_text.rs` →
  `tools/subset-font.sh:45` に足して**フォントを切り直す**）、`Settings`、`CameraPlugin::showing(half_height)`（パン・ズーム、クリックは `WorldClick`、Ruby からは `CameraView` に書く）。
- HUD は egui。`FrameStats`（R5）: 命令数 / 予算、走ったタスク、持ち越し、落としたメッセージ。機械の数、ベルト上のアイテムの数。
- `P` で世界ごと停止（2 本と同じ作り）。セーブ / ロード（serde、先頭に `version`、違う版は読まない。`docs/web.md:187-194`）。
- **Ruby からカメラ**（R9 の層 + S3 の答える側）: `control.rb` が勝ったときに納品口へカメラを寄せる、ガイドの中の「ここを見て」、のどちらか 1 つを実例として入れる。

### 3.6 素材（F0a）

- Kenney Tiny Factory 1.0 — https://kenney.nl/assets/tiny-factory （CC0、16×16、`Tilemap/tilemap_packed.png` 192×176 = 4,452 B、間隔なし 12×11）。
  床 0–2、土 3・9–11・32–35、コンベア右 24–27 / 36–39（2 コマ）、上 4 / 5 系、橙レール 48–51 / 60–63、機械 75–77（赤）・87–89（橙）・99–101（緑）・111–113（青）、
  木箱 73・85・97、歯車 114。**番号は調査時の読み。取り直して `Tilesheet.txt` と拡大画像で確かめる。**
- 鉱石の候補: Kenney Tiny Farm の岩（タイル 77・89、https://kenney.nl/assets/tiny-farm 、CC0）に色。UI の枠: Kenney UI Pack – Pixel Adventure（CC0）。
  既存のパネルは egui なので、画像の枠を使うのはゲーム内の表示（建設メニューなど）だけでよい。
- **足りないもの**: コンベアの下向き・左向き・曲がり、インサータ（基部 + `Transform` で回す腕の 2 枚）、鉱石、アイテムのアイコン、採掘機・かまどの専用の絵。
  ベルトの色は 5〜7 色（`#8b9bb4` `#c0cbdc` `#5a6988` `#3e4e6e` `#3f2631`、橙レールは `#fdbe53` `#e38628`）。
- **ベルトの向きは著者が見て決める**: (i) 3/4 の絵を活かして 4 向き + 曲がりを個別に描く、(ii) ベルトだけ真上からの絵で自作し、回転と反転で済ませる。
  F0a で両方の小さな見本（直線 4 向きと曲がり 1 つ、機械と並べた画面）を作り、**スクリーンショットを報告して止まる**。
- 自作の絵は生成スクリプト（`tools/` に置く。Python + Pillow）で作り、スクリプトと出力の両方をコミットする。著者が後から手で直せるよう、1 枚のシートと番号の表にする。
  自作ぶんのライセンスはリポジトリと同じ（MIT）で、`CREDITS.md` に「Kenney のパレットに合わせて自作」と書く。
- 容量は箱庭と同じ決め方: 先に上限を決めて表に残す。Kenney 由来の 3 枚は合計 13,974 B（4,452 + 5,866 + 3,656）。上限は F0a で実物を並べてから、
  既存 2 本の実績（sabibots 234,202 B、garden 322,924 B。`docs/web.md`）を超えない範囲で決める。

### 3.7 数の決め方

| 数 | 決め方 |
|---|---|
| アイテムの表現（エンティティか列か）とベルト上の上限 | F1 で数千個を両方の形で、PC と**ブラウザ（SwiftShader ではなく実機の数字が取れるなら実機、取れなければその旨）**で測る |
| スクリプト付きインサータの上限 | 実測の目安は「毎フレーム動く形で約 1000 台、8.3 ms」（rubevy の worklog）。R1〜R4 の後に how_many_scripts で取り直し、フレームの中でスクリプトに渡せる時間から決める |
| 命令予算（`budget`）と `frame_time` | 箱庭と同じ: **上限の工場に座って測る**（`garden/src/main.rs:4285-4323` のコメントが手本）。最大値に対する余裕の倍率も測った分布から |
| インサータの `idle` の長さとずらし方 | 腕の 1 往復の時間（ゲームの調整値）より短く起きても無駄、が上限の根拠。ずらしは「同じフレームに起きる台数」を FrameStats で見て決める |
| イベントの `limit:` | control が 1 フレームに受けうる機械の出来事の最大を測って決める。既定の 64 で足りるならそのまま、と書く |
| 地図の大きさ | 上の上限の機械が無理なく置ける広さ、から |

---

## 4. 段階

各段階の終わりに `cargo test --workspace`、PC で起動、wasm ビルド。**F0、F3、F5、F6 は `web/build.sh` + Playwright（pageerror 0、requestfailed 0）まで。**

| 段階 | 到達点 | 確認 |
|---|---|---|
| **F0** | crate の骨組み（`shared-crate-plan.md` の後なのでコピーは最小）、`TilemapChunk` で Tiny Factory の床を敷き、カメラでパン・ズーム、Pages の入口に 3 本目 | **`TilemapChunk` が WebGL2（ブラウザ）で描ける**ことを Playwright のスクリーンショットと pageerror 0 で。描けなければ代替（`bevy_ecs_tilemap` 0.19.0 / `Sprite` の敷き詰め）を比べて**報告して止まる** |
| **F0a** | 素材一式の取得と `CREDITS.md`、自作シートの生成スクリプト、ベルトの向き 2 案の見本 | 見本のスクリーンショット 2 枚と容量の表を報告して**止まる**（著者が選ぶ） |
| **F1** | 格子、設置と撤去、コンベアの搬送、鉱石と採掘機、箱。Ruby なし | ヘッドレスのテスト（運ぶ・詰まる・合流）。アイテム数千個の計測と、表現の選択の記録 |
| **F2** | data stage。かまどと組立機がレシピどおりに作る | 宣言の誤り（未知のフィールド、無いアイテム、`time` ≤ 0）が `data.rb:行` つきで出る。表が最初の `Update` の前に揃っているテスト。ブラウザでも同じ |
| **F3** | インサータの Ruby、prelude と DSL、tick の中の問い、`move` の待ち方の選択、既定のスクリプト 1 本、エディタで 1 台／全台に適用 | インサータなしでは線がつながらず、置くと流れる。わざと壊したスクリプトでゲームが止まらない。N 台での FrameStats（同時に起きる台数、持ち越し）。Playwright |
| **F4** | control stage とイベント、目標と勝ち、あふれの数の HUD | `control.rb` を差し替えて目標が変わる。`dropped` が 0 のまま上限の工場が回る（回らなければ粒度を報告） |
| **F5** | 窓一式（エディタ 3 種、VM パネル、ガイド英日 + フォント、設定、`P`、セーブ / ロード、Ruby からカメラ） | セーブ → ロード → セーブでテキストが一致。インサータの ivar が戻る。ガイドに豆腐が無い |
| **F6** | `?selftest`（`FACTORY_SELFTEST`）、上限の工場での予算の実測と決定、docs（`docs/factory.md`、`docs/web.md` の表とサイズ、`docs/README.md`、`README.md:12` を planned → playable、`CREDITS.md`、`.gitignore`） | selftest を PC で複数回、ブラウザで pageerror 0 と行数一致。wasm のサイズを `docs/web.md` の表に |

---

## 5. 分かっている罠

- **`Vm::set_host_state` を使わない**（rubevy が占有。上書きすると rubevy のコマンドが黙って消える）。表は R7 の口が host store に置く。
- ネイティブは Bevy の `World` に触れない。`answer_in_tick` のクロージャは `Vm` に触れない（Hash の引数は中を読めない、返せるのは平たい `Answer`）。
- 同じ tick の中で書いた component は読めない（書きはフレームの終わり）。「読む → 決める → 書く」の順で DSL を作る。
- `publish` は購読者ごとに値を作り直す。全インサータが同じイベントを購読する形にしない（宛先つきの publish か、読む問いにする）。
- `Script::with_priority` で優先度を分けると、番号の大きい側は予算が尽きたとき飢える。インサータは全台同じ優先度。control は高く、の理由を書く。
- `TilemapChunk` の `array_layout` は `.meta` に頼れない（`AssetMetaCheck::Never`）。コードの `with_settings` で渡す。
- 非整数倍のズームでスプライトのアトラスの隣のタイルがにじむ。カメラのズームを整数倍に丸めるか、1 px 間隔つきのシート（`tilemap.png`）を使う。
- egui はクリックの前にポインタが動いていないと hover 扱いしない。Playwright では `page.goto` を `waitUntil:'commit'`（`docs/web.md:299-305`）。
- ブラウザでは `AppExit` を書かない（`CHECKS_EXIT_WHEN_DONE`）。`SystemTime` / `Instant` / `std::fs` を wasm の道に置かない。
- 外部のサービスを叩くとき（素材の取得など）、著者のメールアドレスなどの個人情報を User-Agent や問い合わせに入れない。

## 6. 状況

| 段階 | 状況 |
|---|---|
| F0 | **済み**（2026-09-21、`factory` の `427fbc9`・`9a88622`・`a32399b`・`72b6621`、main に取り込み済み）。rubevy を `abfc875`（R11 まで）に上げ、既存 2 本の 6 通りの走行は一覧と一致。**`TilemapChunk` は WebGL2 で描ける — ただしそのままでは真っ黒だった**（1,440,000 画素すべて 0、pageerror 0・requestfailed 0・判定は全部 ok。console だけが `INVALID_ENUM: bindTexture` と言っていた）。原因は `wgpu-hal` 29.0.4 の推測: **層が正方形で層数が 6 の倍数の 2D 配列テクスチャをキューブマップ配列として bind する**（`gles/mod.rs:458`）。16×16 のタイルで、Kenney のシートは 132 = 6×22 層。WebGL2 にキューブマップ配列は無い。PC（Vulkan）は通る。直し方は層の数を選べるようにすること: 12 列のグリッドは何行でも 6 の倍数なので、縦 1 列のストリップにし（`ImageArrayLayout::RowHeight`）、`tools/factory-tileset.py` がパックと同じ順・同じ番号で並べ、6 の倍数から外れるまで空白を足す（今 145 層）。直した後のブラウザは 2 色だけ・どちらもパックの床の色・画面の 100%、PC の絵と 99.94% の画素が一致（違う 887 画素は非整数倍の丸め）。代替（`bevy_ecs_tilemap`、`Sprite` の敷き詰め）は要らなくなった。カメラは `games_shell::CameraPlugin::showing`、クリック → タイル、ヘッドレス、判定 4 行（ブラウザ 5 行）、`web/games.sh` に 1 組、`pages.yml` は無変更。**公開サイトの入口には出していない**（`GAMES_ALL` に入れず、`<li>` はコメント。F6 で外す）。素材は 2 ファイル 10,231 B（上限 64 KB を先に決めた: 1 タイル 67 B の実測から）。テスト 60 → 63。記録は `docs/worklog/2026-09-21-factory-F0.md` |
| F0a | **済み。著者の選択は (ii) 真上から**（2026-09-21。(i) の側のタイルと見本 `belt_sample.rs` は F1 の最初に消す。手順は `docs/worklog/2026-09-21-factory-F0.md` §11）（`5aea5c6`）。見本 2 枚: `/home/kishima/book/temp/factory-belts/belts-i.png`（3/4 を活かす）と `belts-ii.png`（真上から）、拡大は `-left` / `-right`。**描く前にパックのベルトを 1 画素ずつ読んだら仕事が半分になった**: 右向きのタイルはシェブロン以外上下対称（左は反転）、上向きのタイルは左右対称で手前の側面が無く**初めから真上の絵**（下は反転、90 度回せば横向きの真上ベルト）。足りないのは曲がりだけ。(i) は曲がりの絵が 4 枚（回転は側面を横や上に持っていくので使えない）、2 コマ込みで 8 タイル、横のベルトが縦より太い（面 8 + 側面 6 px 対 12 px）、自作の曲がりがパックの縦ベルトとぴったり繋がらず手直しが F1 に入る。(ii) は直線 1 枚 + 曲がり 1 枚を `TileOrientation` の 8 通りで回し、2 コマ込みで 4 タイル、4 辺が同じ太さ、ただしベルトだけ真上なので 3/4 の機械と並ぶと平たい |
| F1 | **済み**（2026-09-21、`factory` の `56ab628`・`e3af41a`・`778e99e`、段階の終わりの見直しが `e592146`（コード）とこの行を書いたコミット（docs））。**工場そのものを Rust で書いた段階で、Ruby はまだ 1 行も無い。** 著者が選んだ (ii) を反映して (i) のタイル 8 枚・それを描くコード・見本 `belt_sample.rs` を消し（絵が変わっていないことを `tobytes()` で確かめてから）、格子・設置と撤去・コンベア・鉱石と採掘機・箱を入れた。`factory/src` は 6 ファイル。**規則は `belts.rs` の 1 か所**で、1 タイル 1 列の `VecDeque<f32>`（位置は `0..=1`、隣と `spacing` 以上空く、という不変条件が模型のすべて）に対する 4 パス（尻尾を控える → 運ぶ → 渡す → 掘る）。Bevy を含まないので単体テスト 8 本が `App` 無しで回る。**曲がりは絵と道の両方**で、`Flow` が近所から「どの向きから入るか」を求め、直線 1 枚 + 曲がり 1 枚を `TileOrientation` で回して 4 向き + 8 通りを作る（著者の選択がここで効いている）。**計画が紙の上で決めないと言った「アイテムの表現」は両方作って測って決めた**: 窓なしの PC で 1,000 / 4,000 / 16,000 個を交互に 2 巡、lanes が 2.2〜2.9 倍（16,000 個で 0.3 ms 対 0.8 ms）。窓あり（lavapipe）は描画が支配してフレームは同じ。**ブラウザは SwiftShader なので「測れない」と正直に書いた**（アイテム 2 個でも 1,800 個でも同じ 5 fps、時計は 100 µs 刻み）。捨てた側は消した（規則が「1 つ前」を要求し ECS の query は順番を持たないので、エンティティ側は毎フレーム並べ直す — それが比の正体）。素材は Kenney Tiny Farm の岩を**同じパレットだと分かった**うえで Tiny Factory のオレンジへ写像（新しい色 0）、8 px のアイテムだけ自作。シートは 138 = 6×23 に当たり、**F0 が仕掛けた空白タイルが設計どおり働いた**（139 層）。ズームを整数倍に丸めるようにし、`--shot` と判定が同時に使えるようにした。**`map_tiles` は決められなかった**ことと「何が決まれば決まるか」（実機の GPU、F3 のインサータの上限）を `numbers.md` §9.6 に書いた。段階の終わりの見直しで**出どころの無い数 3 つ（`.max(0.001)`）を消し**、0 以下の設定を読み込み時に断る形へ移した。テスト 63 → 77、判定 4 行 → 8 行。**Docker が使えなかったので窓ありの走行と `--shot` は未実施**（→ 同日、Docker が戻ってから本体が回した: 8 / 45 / 32 行で一覧と diff 空・FAIL 0、`--shot` の絵も出た。worklog §7）。記録は `docs/worklog/2026-09-21-factory-F1.md` |
| F2 | **済み**（2026-09-21、`factory` の `6be7f73`・`18342a7`・`316dd6a` と docs のコミット）。**このゲームが Ruby を走らせた最初の段階。** (a) sabiruby の rev を `7be7b86` → **`50cae754`**（`declare` の入った main）へ上げ、`pages.yml` の grep が新しい lock でも同じ形で当たることをローカルで確かめ、既存 2 本の 6 通りの走行（headless 3 + 窓 3）が**一覧と diff 空・FAIL 0**。(b) `map_tiles` の下限 `8`（出どころ不明）を `Ore::smallest_map` の導出に置き換えた — **計画書がここに書いた `4 × (radius + 1)` = 16 は 1 タイル多く、正しくは `4×radius+2` より大きい最小の整数＝15**、しかも「円は中心を通る行が実在しないと 1 タイル早く空く」ので**保証であって最小ではない**（最初に書いたテストが落ちて分かった）。(c) 本体は `ruby/data.rb` と `factory/src/data.rs`: **6 語**（`item`/`recipe`/`machine` + `belt`/`miner`/`chest`）を `sabiruby_serde::declare` の口で受け、`Startup` で表にする。6 語なのは、世界に備え付けの 3 つを 1 つの `machine` に押し込むと「かまどの `capacity:`」が serde を通ってしまうから。**表は最初の `Update` の前に揃う**（`App` を立てた単体テストと走行の判定の両方で）。誤りは**その行で**断る（`deny_unknown_fields` と `deserialize_with` が native の中で走る）。**宣言をまたぐ誤りには行が無い** — `Declarations::take` の表に行番号が無い — ので、ソースを引いて行を探す 10 行の迂回を入れた（§7 に報告）。(e) 遊びの数は 6 件中 **4 件が Ruby へ**、`ore_per_tile` / `ore_patch_radius` は設定に残した（`map_tiles` の下限が `main()` で要り、そこにまだ VM が無い）。**Rust の側に遊びの数の既定値は 1 つも無い**（S5b-2 の Battle と同じ）。(f) かまど（1×1、Kenney 109）と組立機（2×2、99 を nine-slice で伸ばした 4 枚）が**レシピどおりの時間で**作る。機械の緩衝は「1 回ぶん入り・1 回ぶん出る」で数を持たない。**複数タイルの機械**は原点に `What::Machine`、残りに `What::Covered` を置き、どの辺からでも流し込める。足跡は**回らない**（向きが言うのは出口だけ）。F3 でインサータに置き換わるのは「ベルト→機械」と「機械→ベルト/箱」の 2 か所（`machines::hand_to` と `deliver`）。(g) シートは 139 → **142 層**（6 の倍数でないので空白は要らず、`pad_to_a_safe_count` は 138 と 142 の両方で動かして確かめた）。アイテムは 8 px 3 枚の 1 枚のストリップ（`TextureAtlasLayout`、1 バッチ）。(i) `expose` で `item_of` / `recipe_of` / `machine_of`。テスト 77 → **94**、判定 8 行 → **13 行**（ブラウザ 9 → 14）。data stage の値段は**測った**（§7）。記録は `docs/worklog/2026-09-21-factory-F2.md` |
| F2a | **済み**（2026-09-21、`factory` の `c1a4082`・`3f4301d`・`2723c23`）。**ベルト上の位置を f32 から整数の刻みへ。** 1 タイル = `TILE_PX` = 16 刻み（= 素材 1 画素）。**位置の型は `i32`**（`belts::Steps`）: 符号が要るのは「前の 1 つから隙間を引く」連鎖と、タイルの後ろから押し込まれている物の位置が負だから。32 ビットなのは渡すときの上限（2 タイル = 32 刻み）ではなく**フレームの歩幅**のほうで、`tiles_per_second` に `data.rb` が好きな数を書ける以上 `i16` はデータファイルでオーバーフローさせられる — f32 → 整数の `as` はクランプし、**運ぶ連鎖は `i64` で計算して `i32` へ戻す**（`i32` 同士の和は必ず収まり、戻す所は 2 タイル以下の `limit` で `min` された後なので丸めは起きない）ので、**プレイヤーが書ける数では溢れない**と言い切れる形にした（`i16` にして `OnBelt` を 4 バイトへ戻す道は、「速さの上限」という出どころの無い数が要るのでやめた）。**端数は工場に 1 つ**（`Lanes::part_of_a_step`）: `belt` は 1 つしか宣言できないので速さは 1 種類で、「同じ速さのベルトが同じフレームに同じだけ進む」を**構造で**満たす。60 fps と 5 fps が同じ 2 秒で同じ距離（64 刻み、1 刻み以内）を進むテストを入れた — 既定では 1 フレーム 0.53 刻みなので、端数を捨てる実装はこのテストで 0 刻みになって落ちる。**`items_per_tile` は 16 の約数だけ**で、`f32` で受けて「整数か・割り切るか」を見る `deserialize_with` が `data.rb:行` 付きで断る（文面は約数を `TILE_PX` から計算して並べる: 「… so items_per_tile is one of 1, 2, 4, 8, 16 — not 3」）。`u32` で受けて型で断る案より先に効くうえ、`2.0` と書いた利用者にも同じ文が出る。既定値は動かさず、綴りだけ `2` にした。F1 の 7 値のテストは **16 の約数 5 つで N+1 が厳密・位置も `16 − k × gap` と一致**する形に取り直し、「3 のとき 3 個」は docs から外した（経緯は F1 §5.2 と F2a §6）。描画は `item_at` から掛け算と丸めが消え、**刻みの数がそのまま世界の長さ**（タイルの中心が整数なので `draw::snapped` の整数倍ズームと合わせて画素にぴったり載る）。**`ore_per_tile` は `data.rb` へ**（新しい語 `ore :patch, per_tile: 60`。`miner` の `digs:` の隣に足さなかったのは、F2 が 6 語に分けた理由「その語のものでない数を持たせない」に照らして。名前 `:patch` はラベルで、地面から出るのは `digs:`）。設定に残るのは `ore_patch_radius` だけになり、`numbers.md` §9.2 の「ファイルをまたぐ導出」の注意書きも直した。**長時間の保存則**: 採掘機 2 台 → 合流 → かまど（詰まり）→ 組立機 → 箱を 5 分（18,000 ステップ）回し、**1 秒ごとに**「掘った数 = 箱 + ベルト + 機械の中（作りかけの材料も）」を鉱石換算で `assert_eq!`。テスト 95 → **98**、判定の行は 3 通りとも **diff 空**（ブラウザ pageerror 0・requestfailed 0、絵に黒 0）、箱庭と Battle のヘッドレスも diff 空。**速さは前と同じ**（`--stress` の step µs、4,000 / 16,000 個で 前 42 / 151 対 後 42 / 149）。ただし**最初の書き方は 10% 遅かった** — `Option<Steps>` の tails（8 バイト + 分岐）と、1 アイテムごとの `saturating_`（飽和は加算 + cmov）。番兵と 64 ビットで両方戻した。機械が静かにならなかったので**中央値ではなく、同じ巡で交互に走らせた 16 標本の最小**で比べている（中央値は同じバイナリで 2 倍ぶれた）。記録は `docs/worklog/2026-09-21-factory-F2a.md` |
| F3 | **済み**（2026-09-21、`factory` の `f7d51c2`・`f906d13`・`d74c7f4`・`38b1acb`・`e000e73` と F3-3 のコミット）。**プレイヤーが Ruby で書く機械が入った段階。** (a) sabiruby の rev を `50cae754` → **`8d0fea2`**（`Options::symbols`・`take_with_lines`・`Declared<T>`・`Vm::backtrace_line`、および `current_line` → `next_line` の改名）。F2 の迂回 `data::line_of`（20 行）が消え、**宣言をまたぐ誤りの行が serde の raise と同じ数になった** — 期待値は 1 件ではなく **2 件**動いた（`GOOD` の 2 行に跨るレシピを指す検査が 2 つあった）。`expose` の map のキーが Symbol になり、prelude が `[:in][:iron_ore]` で引ける。(b) **建てる依頼を `build::Order`（場所・何を・向き）に**。`build::clicks` が `WorldClick` + `Hand` からそれを作り、`build::orders` だけが建てる。判定は `Order` を直接書くので **F2 のちらつきの穴は構造ごと消えた**；順序の 1 行は残ったが理由が変わった（「依頼を出して結果を見る」という判定の性質）。(c) **インサータ**: 1 タイルの建物、後ろから取って前に置く。**腕は Rust、状態は格子**（`held` が手、`work` が swing、新しい `swinging`）なので、セーブも保存則の和も単体テストも自動的に付いてくる。DSL は **9 語**（`inserter "…" do` / `def run` / `behind` / `holding` / `front_takes?` / `move` / `idle` / `swing_seconds`・`items` / `log`）。3 つの問いは `answer_in_tick` で **0 フレーム**、`move` だけが待つ。**`move` の待ち方は (a) request を持ち続ける**を選んだ — (c) `sleep` は swing の長さを Ruby と Rust の両方に置くことになり（数が 2 か所）、(b) イベントは 1 台 1 メッセージに購読 1 本（既定 64 件の器）を配ることになる。request の置き場所を `Arms::waiting`（タイル → request）にしたので、**撤去は「格子からタイルが消える」の 1 本道**になった。(d) **機械への出入りはインサータだけ**（`machines::hand_to` に `Offer`、`deliver` は削除）。ベルト → 箱と採掘機 → ベルト/箱 は**今のまま** — Factorio は箱にもインサータを要求するが、写すと F1 の線（Ruby 無しで建つ最初の線）が壊れ、主題が混ざる。副作用で**機械の向きが何も言わなくなり** `Data::output_of` が消えた（Factorio の組立機にも向きは無い）。(e) `inserter :arm, seconds_per_item: 1.0` = **毎秒 1 個 = ベルトの 1/4 = 採掘機 1 台ぶん**。語は `miner` と同じ（意味が同じ）。**取れる範囲は数にしていない**。(f) `idle` は **1 swing**（別の数ではない: 飽和時は `idle` に来ないので、効くのは応答の粒度だけで、それは腕自身の粒度）。ずらしは `srand(me.to_i)` + `sleep swing * stagger * rand` を**起動時に 1 回**（garden の先例、新しい数 0 個）。(g) 壊れたスクリプトはその 1 台だけ止まり印が出る。**行は prelude が出す** — 終わったタスクには `stats` のフレームが残っていないと実測で分かったので、prelude の `rescue` が `@broke_at` に置き、ゲームが `ivar_get` 1 回で読む。そのために生成ブロックが `prelude_lines` を持ち、`compile` が `Program::new` を 2 回呼ぶ。(h) **エディタ**（`rubevy_egui`）: インサータを手にインサータをクリックすると開く（モードもキーも足さない）、Apply / Apply to all / Revert、Save は F5 と言う。差し替えは走っているテキストのハッシュ比較で**変わった台だけ** `replace_script`。差し替え回数と `loaded_programs` をログに。(i) **計測**（`--arms N`、`FACTORY_STAGGER`）: 散らすと 3,000 台で命令 14,040・tick 2.2 ms（p95）、**散らさないと 92,589 命令・8.0 ms（= `frame_time`）・持ち越し 1,603**。`script_budget` を rubevy の 3 歩で **39,000**（測った 9,800 命令/ms × フレームの 1/4）、`frame_time` は 8 ms のまま。テスト 98 → 103、判定 13 → **19（窓 22・ブラウザ 23）**。記録は `docs/worklog/2026-09-21-factory-F3.md` |
| F3a | **済み**（2026-09-21、`factory` の `1ade37d`・`b05eecb` と docs のコミット）。**地図の大きさが利用者のものになった段階。** 著者「サイズは自由に変えられるようにしたい」に対して、既定値は**動かさず**（32×32、F0 の仮の値のまま）、変えられる形を先に作った。(1) **8 語目 `map :world, size: [32, 32]`**。綴りは `machine` の `size:` と揃えた — 意味が同じ（タイル何枚ぶん、横と縦）なら綴りも同じ、という F3 の `seconds_per_item:` と同じ原則で、計画書の例 `tiles:` は採らなかった。**正方形をやめた**（`Map`・`Grid`・`Lanes`・`Ore`・`Flow`・`TilemapChunk` が横と縦を別々に持つ）。`ore_patch_radius` は `ore :iron_ore, patch_radius:` へ（`ore_` は語が言っているので落とした）。`factory.settings.txt` から 2 鍵が消え、**古いファイルに残っていれば log が 1 度「`data.rb` へ移った」と言う**（`LogPlugin` は `App` の中なので `main()` の `warn!` は消える — 窓の走行で何も出ずに気づき、`Startup` の system にした）。**`main()` から動かせなかったものは無い**: 世界は `lay_the_land` が全部作り、カメラの端・見え方・ズームの外側は新しい `point_the_camera_at_the_map` が `Startup` で言う（共有 crate に足りない口は無かった。`CameraHome`/`CameraView`/`CameraControls` が全部 Resource）。`--stress` の地図決めは `ResMut<Rules>` になり、**世界が組まれる前**に走るので F3 の「古い地図でカメラを作って直す」が消えた。(2) **畑の数と置き方を `ore … patches: [2, 2]`** に。数 + 種ではなく**格子**にしたのは、種が出どころの無い数になること、判定の前提（2 回の走行が同じ所に鉱石を見つける）が壊れること、`[2, 2]` が今までの 4 隅と**タイル 1 枚も変わらない**こと、長方形にも意味があること（96×16 には `[6, 2]`）。副産物: `Ore::smallest_map` の導出 `4r + 2` に**隠れていた 2 が畑の数だった**と分かり、`tiles > 2 × n × (radius + 0.5)` になって横と縦に別々に当たるようになった。(3) **上限は発明せず、実物で調べた**。壊れるのは `TilemapChunk` のタイル情報テクスチャ（**1 タイル 1 テクセル**の `Rgba16Uint`）で、Bevy は**アダプタの limits をそのまま要求する**ので値は機械ごとに違う（実測 **lavapipe 16384 / SwiftShader 8192**）。公開するページなので採ったのは **WebGL2 が保証する 2048**（`wgpu-types` の `downlevel_defaults`）で、タイル添字の型が言う 65,535 はこれに隠れる。**メモリは上限にしない**（機械に依る）— 1 タイル **113 バイト**（wasm では 69）を MB に直して log に 1 行。**chunk 分割はやらない**と決めた（買えるのは機械のメモリより大きい地図だけ。理由は `draw::MOST_TILES_ACROSS` の rustdoc）。下限・上限とも黙って直さず **`data.rb:行` つきで、直し方を 3 通り並べて**断る。(4) **空の地図の値段を測った**: step p50 は 32² で 4 µs、512² で 0.6 ms、2048² で 5.9 ms（1 フレームの 35%）。タイル数に比例する毎フレームの仕事が 2 か所ある（`belts::step` の尻尾の控えと `lanes.count()`）ことが分かり、§7 に気づき点として書いた。確認: テスト 106 → **110**、警告 0、`unsafe` 0 件、**既定の 3 通りは一覧と diff 空**（19 / 22 / 23、ブラウザ pageerror 0・requestfailed 0）、**15×15（下限ちょうど）・96×16・512×512・2048×2048 が起動し、96×16 と 512×512 はヘッドレス・窓・ブラウザの 3 通りとも判定の行が既定と同じ**、絵は黒くない。記録は `docs/worklog/2026-09-21-factory-F3a.md` |
| F4〜F6 | 未着手 |

## 7. 気づいた点（段階の報告から本体が集める）

実装担当は、仕事の範囲の外で気づいたことを直さずに報告と worklog の末尾に書く（`implementer.md` の報告の形式 6）。
本体は段階をレビューするたびにここへ写し、行き先を決める。消さずに「状況」を更新する。

| 日付・段階 | 気づいた点 | どこ | 属する先 | 状況（計画に足した／著者判断待ち／見送り・理由） |
|---|---|---|---|---|
| 09-21 F0 | **`TilemapChunk` の層数の罠は rubevy_games に限らない**: Bevy 0.19 の `TilemapChunk` を配列テクスチャで使う誰にでも起き、Bevy 本体の example は 4 層なので当たらない。例外も 404 も出ず判定も全部 ok なので、**画素を数える以外に気づく道が無い** | `wgpu-hal-29.0.4/src/gles/mod.rs:458` | 上流（wgpu か Bevy）への報告候補／本の素材 | **見送り**（著者、2026-09-21「報告しない」。上流には出さない）。再現は小さい: 16×16 × 6 の倍数の層で WebGL2。`docs/factory.md` と `tools/factory-tileset.py` に理由を書いてある。book の findings に写す |
| 09-21 F0 | **Battle のハンドラの判定が、ブラウザの窓を 1600×900 にすると毎回落ちる**（Playwright の既定 1280×720 では FAIL 0）。canvas が大きいぶん SwiftShader のフレームが遅く、壁時計 0.3 秒に入るフレーム数が減る。S7 が箱庭に入れた「秒で待たず条件で待つ」が Battle のこの判定には入っていない。`fixedlines.sh` はこの 2 行を落とす（当たり依存の行）ので行の集合では見えない | `sabibots/src/main.rs:1908,1912` | バグ（Battle の判定） | 計画に足した: games の S9 に入れる |
| 09-21 F0 | `tools/fixedlines.sh` が CR を落とさない（`docker/run.sh` は `-t` 付きなので窓の走行の各行が `\r` で終わる）。`--shot` と `<GAME>_SELFTEST` は同時に使えない（判定が済むとアプリが終わり、`--shot` の時刻の前に窓が閉じる。3 本とも同じ作り） | `tools/fixedlines.sh`、`games_shell::checks` | 道具 | 計画に足した: games の S9 で（CR は `tr -d` を足し、過去の比較と揃うことを確かめる） |
| 09-21 F0 | 調査の「3/4 なので回転では他の向きを作れない」は横向きのタイルにだけ当てはまる（縦のタイルは初めから真上の絵）。PC とブラウザで絵が 0.06% 違う（非整数倍の丸め。計画書 5 章の「にじみ」） | 計画書 3.6、5 章 | 計画書の前提違い | 3.6 は F0a の行に書いた。ズームを整数倍に丸めるかは F1 で見る |
| 09-21 F1 | **「隙間 N 個に N+1 個」は f32 では数として成り立たない**: `items_per_tile = 3` の詰まったタイルは 3 個（1, 2, 4, 5, 7, 8 は N+1）。正確に表せるかどうかではなく、引き算の連鎖の丸めがどちらへ落ちたかで決まる。不変条件（隣と spacing 以上、何も失わない）は全部の値で成り立つ | `factory/src/belts.rs` の渡す判定、テスト `a_jam_keeps_the_gap_even_when_the_gap_is_not_an_exact_number` | ゲーム固有／本の素材 | **F2a が解決した**: 位置が 1 タイル 16 刻みの整数になり、`items_per_tile` は 16 の約数だけになったので、N+1 は書ける値の全部で厳密（テストは個数と位置の両方を `assert_eq!`）。「3 のとき 3 個」は docs から外し、経緯は F1 §5.2 と F2a §6 に残した。book の findings に写す |
| 09-21 F1 | `map_tiles` の下限 `8` に出どころが無い（`docs/numbers.md` §9.4 に「不明」と書いた）。畑を縁から離す最小は `4 × (ore_patch_radius + 1)` で導ける | `factory/src/main.rs` の `Map` を作る行 | 埋め込みの数 | **本体が原則から決めた: 導出に置き換える**。F2 の最初に |
| 09-21 F1 | **「正でない設定を読み込み時に断る」は 3 本とも要る形**: F1 は `positive` / `counted` を factory の中に書いて `.max(0.001)` を 3 つ消した。箱庭と Battle にも同じ形の数が無いか見てから `games_shell::Settings` へ | `factory/src/main.rs`、`games_shell::Settings` | 共有 crate | 計画に足す（shared-crate-plan の次の段。S9 の後） |
| 09-21 F1 | ズームを整数倍の画素に丸める（`draw::snap_zoom` / `snapped`）は次の 2D ピクセルアートのゲームも写す。`CameraControls` のつまみ 1 つ（画素あたりの世界の長さ）にできる | `factory/src/draw.rs` | 共有 crate | 見送り — 利用者が 1 本のうちは上げない。2 本目が出たら上げる |
| 09-21 F1 | `--shot` と判定の両立を F1 は factory の中だけで直した。S9 が共有 crate に `checks_end_the_run()` を入れるので、main に入ったら factory はそれに乗り換える | `factory/src/main.rs` の selftest、`games_shell::checks` | 共有 crate | S9 が main に入った後、F2 の最初に乗り換える |
| 09-21 F1 | 1 フレームに 1 タイル以上進む速さでは、渡すパスが index の順に依る（既定の 30 倍の速さ）。運ぶパスは順に依らない | `factory/src/belts.rs` の 3 パス目 | ゲーム固有 | 見送り — コードのコメントと worklog §6 に書いた。速さの上限を設定で断るかは F2 で data stage の検査を書くときに |
| 09-21 F1 | `Moves` が「数 3 つ」でなく「順番のある一覧」である理由の片方（entities の側）は `778e99e` で消えた | `factory/src/belts.rs` | ゲーム固有 | 見送り — F5 の HUD が何を要るか分かるまで触らない |
| 09-21 F1 | `wasm-opt` が既定の PATH に無く（`~/.local/binaryen-version_132/bin`）、`web/build.sh` が黙って「縮めなかった」で進む。`web/build.sh all` の後に 1 本だけ建てると feature の解決が変わって wasm の依存を全部建て直す（約 20 分） | `web/build.sh` | 道具 | 計画に足す（S9 の後の道具の段: PATH に無ければ既知の場所を探すか、止まって言う） |
| 09-21 F1 | **窓の走行 3 本と `--shot` は Docker が応えず未確認**（ブラウザの絵で画素を数えて代えた） | worklog F1 §5 | 確認の抜け | **済み**（同日、本体）: 窓 3 本は 8 / 45 / 32 行で一覧と diff 空・FAIL 0、`--shot` の絵も出た。worklog F1 §7 |
| 09-21 F2 | **`Declarations::take` の表に行番号が無い**: 宣言をまたぐ検査（無いアイテム、無い `made_in`、`size` と `sprite` の枚数）に言える行が無く、games がソースを引いて探す 10 行の迂回（`data::line_of`）を入れた。「始まる行」を返すので serde の「終わる行」と食い違いうる | sabiruby `serde/src/declare.rs` | VM（sabiruby-serde） | **済み**（sabiruby `574aeff`: `take_with_lines` と `Declared<T>`。F3 が迂回を消し、**期待値が 2 件動いた** — 1 件と数えられていた） |
| 09-21 F2 | **`Options::symbol_keys` が Hash のキーに効かない**（struct のフィールドにしか通っていない）。Symbol で書いた名前が String で返る: `recipe_of(:x)[:in]["iron_ore"]` | sabiruby-serde `ser.rs` の `Serializer::key` | VM（sabiruby-serde） | **済み**（sabiruby `574aeff`: `Options::symbols`、`expose` はそれを使う。F3 の prelude は最初から Symbol で引いている） |
| 09-21 F2 | ブラウザのコンパイラにファイル名を渡す口が無い（`sabi_compile` は `FILENAME` 固定の `playground.rb`）。F2 は `Trouble` が行だけを持ち名前を表示時に付ける形で避けた | sabiruby-playground、`web/page.html.in`、`games_shell::platform` | playground と共有 crate | 計画に足す（F5 のエディタの前。3 本とも得をする） |
| 09-21 F2 | **クリックに「何を持っていたか」が載っていない** — 判定とクリックの順序のちらつき（窓でだけ FAIL 2、F1 からの穴）の根。F2 は順序 1 行で直した | `factory/src/build.rs`、`WorldClick` | ゲーム固有 | **済み**（F3-0b、`f906d13`）: `build::Order { at, what, dir }`。判定は依頼を直接書き、順序の 1 行は残ったが理由が「判定は依頼を出して結果を見る」に変わった。エディタも同じ口を読む（インサータの上のクリックがパネルを開く） |
| 09-21 F2 | `--shot` と判定の終わり方は 2 方向ある（F1「判定が済んでも絵を待つ」、F2「絵を撮っても判定を待つ」）。S9 の `checks_end_the_run()` は片方だけ | `games_shell::checks`、3 本の `take_shot` | 共有 crate | factory の乗り換えの段（F3 の最初）で両方を共有 crate へ。shared-crate-plan §7 の `done` の行の件と一緒に |
| 09-21 F2 | F1 の「アイテムの表現 2.2〜2.9 倍」は今の数ではない（アイテムが 4 → 8 バイト、`Building` が `Copy` でなくなった）。結論は変わらない | worklog F1 §3 | ゲーム固有 | 見送り — entities の側はもう無いので比は取り直せない。lanes の絶対値は F3 のインサータの計測のときに取り直す |
| 09-21 F2 | 計画書 §7 の `4 × (ore_patch_radius + 1)` は 1 タイル多かった（正しくは `4r + 2` より上の最小整数 = 15、しかも最小ではなく保証）。`docs/factory.md` のタイル 75/76/77 の読みも違っていた（完結した 1 タイルのキャビネット。パックに 1 タイルより大きい機械は無い） | 計画書、`docs/factory.md` | 計画書の前提違い | F2 が直した |
| 09-21 F2 | `ore_per_tile` = 60 の出どころは「箱 1 杯ぶん」で、その箱が `data.rb` へ行ったのでファイルをまたぐ導出になった（追従しない）。(ii) `ore_per_tile` も `data.rb` へ移すのが原則に合う | `factory.settings.txt`、`ruby/data.rb`、`docs/numbers.md` §9.2 | ゲーム固有 | **F2a でやった**（(ii)）: 新しい語 `ore :patch, per_tile: 60`。`miner` に足さなかったのは、F2 が 6 語に分けた理由（その語のものでない数を持たせない＝「かまどの `capacity:`」の形を作らない）に照らして。既定値 60 は動いていない。設定に残るのは `ore_patch_radius` だけ |
| 09-21 F3 | **終わったタスクには `ScriptWorld::stats` のフレームが残っていない**（実測: `frames=[] location=None finished=true`）。だから「スクリプトが止まった所」をゲーム側から読む道が無く、prelude の `rescue` が行を作って `@broke_at` に置き、ゲームが `ivar_get` で読む形にした。`ScriptEnded` が例外の**場所**も持てば（`value` は `inspect_str` だけ）、どのゲームも同じ 30 行を書かずに済む | rubevy `src/lib.rs` の `ScriptEnded`、`ScriptWorld::stats` | rubevy | **著者判断待ち**（rubevy の API に足す話。迂回は動いていて害が無い） |
| 09-21 F3 | **「完了まで待つ動作」は rubevy に無い。** `move` は request を持ち続けて後で `answer` する形で書いたが、ゲーム側が「誰の request か」を自分で索引しなければならない（`Arms::waiting`: タイル → `Request`）。rubevy が `answer_when(request, impl FnMut(&World) -> Option<Answer>)` のような「条件が満たされたら答える」口を持てば、この索引と後始末（撤去・差し替え）がゲームから消える。`answer_with` は**未来**を取るので、Bevy の世界を毎フレーム見る形には使えない | rubevy `ScriptWorld::answer_with` の隣 | rubevy（汎用の口） | **報告のみ**。F4 の control stage も同じ形を要る可能性 |
| 09-21 F3 | エディタの **Save を繋いでいない**（`saving inserter.rb is F5's` と言う）。指示の線が「F5 が窓・セーブ」だったので止めたが、`platform::write` は S1 からあり 3 行で繋がる。押せるボタンが「まだ」と言うのは中途半端でもある | `factory/src/window.rs` の `EditorAction::Save` | 著者判断／F5 | **報告のみ** |
| 09-21 F3 | **`Crew` という SystemParam に `Editor` を入れた**のは、Bevy の system が取れる引数が 16 個で判定がちょうど 16 個だったから。層としては「インサータ」に「パネル」が混ざっている | `factory/src/inserters.rs` の `Crew` | そのゲーム固有 | 見送り — F5 が窓を増やすときに `Panel` を分ける |
| 09-21 F3 | **判定が「1 フレーム後に見る」と書いてあると、窓で通ってブラウザで落ちる。** F3 で 2 回踏んだ（パネルが開くのを待つ所と、VM がプログラムを持つのを待つ所）。S7 の「秒ではなく条件で待つ」は**フレームでも同じ**で、`docs/verification` に 1 行書く価値がある | `factory/src/main.rs` の `PANEL_WAIT_FRAMES` | 確認の作法 | **計画に足す候補**（本体が決める） |
| 09-21 F2 | 位置を整数の刻みで持つ模型: 1 タイル = 16 刻み（= 素材 1 画素）、`items_per_tile` は 16 の約数に限られ data stage が行つきで断れる。N+1 が厳密、描画の丸めも消える。失うのは 2.5 のような値と `belts.rs` の書き換え。セーブ（F5）の前なら安く、F3 の後はインサータのぶん高い | worklog F2 の報告 §9(a) | ゲーム固有 | **F2a でやった**（著者「F3 の前に変える」）。利用者が書ける値は 16 の約数に狭まり、断り文が書ける値を並べて言う。セーブ（F5）の前なので安く済んだ |
| 09-21 F3 | **「完了まで待つ動作」が rubevy に無い**: `move` は request を持ち続けて腕が振り終わったフレームで答える形で書けたが、誰の request かをゲームが索引し（`Arms::waiting`、タイル → request）、撤去と差し替えの後始末もゲームのものになる。`answer_with` は future を取るので「毎フレーム世界を見て、満たされたら答える」には使えない。`answer_when(request, impl FnMut(&World) -> Option<Answer>)` のような口があれば索引も後始末も消える。F4 の「納品されるまで待つ」も同じ形 | rubevy、`factory/src` の `Arms` | rubevy（汎用の口） | **著者判断待ち**（rubevy の公開 API に足す話。F4 の前に入れば F4 が使える） |
| 09-21 F3 | **終わったタスクには `ScriptWorld::stats` のフレームが残っていない**（`frames=[] location=None finished=true`）ので、スクリプトがどこで止まったかをゲームから読む道が無い。prelude の `rescue` が `@broke_at` に行を置きゲームが `ivar_get` で読む 30 行を書いた。`ScriptEnded` が例外の場所も持てば、Ruby を書かせるゲームは全部これを書かずに済む | rubevy の `ScriptEnded`、`factory/ruby/prelude.rb` の `said_at` | rubevy | **著者判断待ち**（上と一緒に rubevy の計画へ） |
| 09-21 F3 | エディタの Save を繋いでいない（ボタンは「Save — not yet (F5)」と言う）。`platform::write` は S1 からあり 3 行で繋がる | `factory/src` のエディタ | ゲーム固有 | **本体が原則から決めた: F5 のまま**。Save は「何をどこへ保存するか」（1 台のスクリプト／既定の `inserter.rb`／セーブデータの中）が F5 のセーブの設計と一体で、先に 3 行で繋ぐと後で意味が変わる |
| 09-21 F3 | 機械の `dir` が何も言わなくなった（出口を読むのはインサータだけになった）のに `R` で回せる | `factory/src/build.rs`、`grid.rs` | ゲーム固有 | **本体が原則から決めた: 機械を持っているとき `R` は何もしない**（何も変えない操作を受け付けない。Factorio の組立機にも向きは無い）。F4 の最初に |
| 09-21 F3 | 判定が「1 フレーム後に見る」と書いてあると、窓で通ってブラウザで落ちる（F3 で 2 回）。S7 の「秒ではなく条件で待つ」はフレームでも同じ | `docs/verification`、`implementer.md` | 確認の作法 | 計画に足す（`selftest-lines.md` の作法の節に 1 行。S10 の「交互だけでなく向きも入れ替える」と一緒に `implementer.md` へ — 著者の文書なので案を出す） |
| 09-21 F3 | `Minds::own` はタイルで引くので、撤去して同じ場所に建て直すと前のスクリプトが戻る（「その場所の腕」） | `factory/src` の `Minds` | ゲーム固有 | F5（セーブが何を保存するか）で決める |
| 09-21 F3 | `map_tiles` の既定（32）の材料が揃った: 鎖 1 本 ≈ 30 タイルで 32×32 は鎖 34 本 = 腕 340 台、スクリプトの上限（3,000 台で tick p95 2.2 ms）はもう地図を縛らない。残る不明は描画だけ（この機械はソフトウェア描画しか無い） | `docs/numbers.md` §9.6 | 埋め込みの数 | **著者判断待ち**（既定値。実機の GPU の数字が要る — 著者の機械のブラウザで `?stress` を 1 度回してもらえれば決まる） |
| 09-21 F3 | `Crew` という SystemParam に `Editor` を入れた（判定の system がちょうど 16 引数だったため）。層として混ざっている | `factory/src/main.rs` | ゲーム固有 | F5 で `Panel` を分ける |
| 09-21 F3a | **`belts::step` と `lanes.count()` が毎フレーム地図全体を舐める**（尻尾の控えは `lanes.tails` 全体を書き、`count()` は全レーンの長さを足す）。空の地図で step p50 が 32² 4 µs / 512² 0.6 ms / **2048² 5.9 ms**（1 フレームの 35%）。地図が自由になって初めて意味のある話になった。尻尾は `lanes.order` だけにできる（撤去時に古い値を消す必要がある）、`count()` は `Lanes` が数を持てば O(1) | `factory/src/belts.rs` の `step` パス 1、`factory/src/items.rs:62` | そのゲーム固有（F5 の HUD がこの数を使う） | **報告のみ**（本体が決める） |
| 09-21 F3a | **`main()` で `warn!` を書いても何も出ない** — `LogPlugin` は `DefaultPlugins`/`MinimalPlugins` の中なので、`App` を建てる前のログは subscriber が無く捨てられる。F3a は「移った設定の鍵」の警告をそこに書いて、窓の走行で何も出ずに気づいた（`Startup` の system へ移した） | 3 本の `main()`、`docs/verification` | 確認の作法 | **報告のみ**（`selftest-lines.md` の作法の節に 1 行の価値） |
| 09-21 F3a | **判定が畑の位置を「地図の 1/4」として知っていた。** `Ore::patch_middle` に聞く形へ直したが、同じ性質のものが残っている: `lay_out_the_machine_lines` は**地図の真ん中が空いている**ことに寄りかかっており、`patches` が奇数×奇数だと真ん中に畑が来る（今の `[2, 2]` では当たらない） | `factory/src/main.rs` の `lay_out_the_machine_lines` | そのゲーム固有／F4 以降の判定 | **報告のみ** |
| 09-21 F3a | **`--shot` と判定を同時に使うと `done` の行が 1 行増える**（`CHECKS_EXIT_WHEN_DONE && shot.is_none()`）。`selftest-lines.md` の「窓 22 行」は `--shot` 無しの数である | `factory/src/main.rs` の selftest の末尾、`docs/verification/selftest-lines.md` | 確認の作法 | **報告のみ**（文書に 1 行） |
