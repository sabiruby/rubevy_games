# 箱庭（Garden）— 実装指示書

作成 2026-09-17。rubevy_games の 2 本目のサンプル。SabiRuby Battle は「ゲームの規則は Rust、ロボットの頭脳は Ruby、境界は `Rubevy.ask` の文字列」
という形で、ECS のコンポーネントには触らない。箱庭は逆で、**Ruby が ECS のコンポーネントに名前で触る**（`e[:Hunger]`）ことを主役にし、
今日までに入った接続の仕組みが 1 画面で分かるようにする:

| 見せたいもの | 仕組み | どこで見えるか |
|---|---|---|
| コンポーネントに名前で触る（接着コード 0 行） | `Rubevy::Entity#[]` / `#[]=` / `has?` / `components`、`Rubevy.find`（Bevy のリフレクション） | 生き物の頭脳がすべてこれで書かれている。Rust 側にコンポーネントごとの glue が無いことを数える |
| イベント | `Rubevy.subscribe(:eaten)` / `ScriptWorld::publish` | 食べられた・触れられた・夜になった、を別タスクで待つ |
| Rust の型を Ruby のクラスに（マクロ） | `#[derive(RubyClass)]` + `#[ruby_methods]` の `Genome` | 生き物が自分の遺伝子を Ruby から読み・混ぜ・変異させる。手書き `define_closure` との行数比較 |
| セーブ/ロード（serde） | `sabiruby-serde`: Rust の `GardenSave: Serialize` ↔ Ruby の値、`Serde<T>` で `define_fn` に型付きの Hash を渡す、Ruby 側は `JSON` | F5 で保存、F9 で復元。生き物の `@memory`（Ruby の Hash）も一緒に残る |
| 動的プロキシ | `Rubevy::Proxy.new("garden")` | 規則の質問（`garden.nearest(:Plant)`）だけはこれ。`ask` との違いが読める |
| 複数タスク・時間スライス | mruby-task | 生き物 1 体 = 1 タスク + 反射タスク。数十体で 1 フレーム 8 ms の予算に収まることを HUD に |

著者の判断: sabiruby の release-prep（cgu=1、可視性、0.5.0）と並行してよい。**ベンチが走っている間は Bevy をビルドしない**（本体が合図する）。
ゲームの規則は Rust に閉じる方針（rubevy `docs/rust-bridge.ja.md`）は Battle と同じ。

## 状況

| 段階 | 内容 | 状態 |
|---|---|---|
| G0 | 世界（Rust だけ）: 草が生え、生き物が `Velocity` で動き、腹が減り、草を食べ、死に、昼夜が回る。headless + selftest | 済み（`2ece3a6` + 当たり判定） |
| G0a | 軽いフリーの 3D アセットに置き換え（下記「アセット」。G0 は基本形状で始めてよい） | 済み（`58d6940`。Kenney の Nature Kit + Cube Pets、7 ファイル 314 KiB） |
| G1 | 頭脳（Ruby）: `Entity#[]` と `find` と `subscribe` で書いた 2 種の生き物。反射は別タスク | 済み（`ed54e59`） |
| G2 | `Genome`（マクロ）: Rust の構造体を Ruby のクラスに。混ぜる・変異・子を産む | 済み（`223fc0d`） |
| G3 | セーブ/ロード（serde）: 世界と `@memory` を JSON に。`Serde<CreatureSpec>` で Ruby から型付きに生成 | 済み（`09e8a8c`） |
| G4 | 窓: エディタ・VM パネル（`rubevy-arena`）を載せ、HUD に「1 判断あたりのフレーム数」と予算の消費 | 済み（`b520917`。6 つ目の判定は `fd67de4` で原因 2 つを直して 10/10） |
| G5 | ブラウザ版（`web/` の仕組みを共有、Pages で公開） | 済み（`7d2f67d` 版番号・`10acb53` 2 ゲーム 1 サイト・`625ea59` ブラウザで直した鍵とボタン） |
| G6 | 遊び心地（著者のブラウザ試遊 2026-09-17）: 夜が暗すぎる、ホイールのズームが 2 段階しか効かない、パン（スライド）が無い、ゲーム内の説明（英語 + 日本語） | 済み（`e4164e1` 夜・`f27d941` 視点・`1b8ffa8` 説明） |
| G6b | 著者の 2 回目の試遊（2026-09-17）: 夜がまだ暗すぎる、説明は英日交互ではなくクリックで切り替え | 済み（`499355d` 設定の置き場所・`bbd89d6` 夜とスライダ・`c2b277f` 言語の切り替え） |
| G7 | 名前の整理（著者判断 2026-09-17）: DSL `reflex` → `on`、説明文の「頭脳」→「行動アルゴリズム」、英語の brain/mind → behaviour、キー表は短縮形。両ゲーム | 未着手 |

## 世界（G0）

`garden/` を workspace に足す（`sabibots/` と同じ形: `Cargo.toml`、`src/main.rs`、`src/platform.rs` は sabibots のものを共有できるなら `rubevy-arena` に移す、`ruby/`、`assets/`）。
**3D**（著者の希望 2026-09-17: Bevy らしさを見せる）。`bevy_pbr` を workspace の features に足し、メッシュは Bevy の基本形状だけ
（地面 `Plane3d`、草 `Cone`/`Sphere`、Beetle `Capsule3d`、Rabbit `Cuboid` + 耳）、色は `StandardMaterial`。G0 の時点ではアセットを持たない（G0a で `.glb` に置き換える）。
**昼夜は `DirectionalLight` の回転 + 影 + 環境光の色**で見せる（夜に生き物が寝るのが画面で分かる）。カメラは斜め上から、マウスでオービット。
動くのは XZ 平面（y が上）。Ruby から見える違いは `Transform.translation` が `[x, y, z]` になることと `act(vx, vz)` だけで、
**3D にしても頭脳のコードは 2 行しか変わらない**ことを `docs/garden.md` に書く。成長・繁殖は `Transform.scale` に出す（`Plant.size` → scale。Ruby からも `e[:Transform][:scale]` で読める）。
当たり判定は距離（球）。WSL の `--shot` は lavapipe なので影付き PBR は 1 枚数秒かかるが、headless は描画しないので selftest に影響しない。
wasm は Battle の 30 MB から 35〜40 MB に増える見込み（G5 で実測）。

40×30 マスの草地。エンティティと**コンポーネント**（すべて `#[derive(Component, Reflect)] #[reflect(Component)]` + `app.register_type::<T>()`。
これが Ruby から名前で見える条件で、ここ以外に接着コードは書かない）:

* `Plant { size: f32 }` — 草。毎秒わずかに育ち、食べられると減る。空きマスに確率で芽が出る。`Tree`（数本、固定、`Collider` 付き）と `Rock` は障害物。
* `Creature { species: Species, age: f32 }`、`Hunger(f32)`（0 で餓死）、`Velocity(Vec2)`（Rust が `Transform` に積分、壁で止める）、`Sight(f32)`（見える半径）、
  `Memory`（G3 まで空）。`Species` は `enum { Beetle, Rabbit }`（enum も Reflect なら Ruby にはシンボルで見える —
  rubevy `src/reflect.rs` の変換に従う）。
* 規則（Rust のシステム）: 成長、移動、腹減り、**接触で食べる**（Creature と Plant が重なったら `Hunger`（満腹度: 0 で餓死、100 で満腹）が増え `Plant.size` が減り、
  `ScriptWorld::publish(Some(creature), "ate", …)` と `publish(Some(plant_owner?), …)` は無し — 草は script を持たない）、
  餓死（`ScriptTask` の除去 → 購読解除まで rubevy がやる）、昼夜（60 秒周期。夜になった瞬間に `publish(None, "night", …)`、朝に `"day"`）。
  Rabbit が Beetle に触れると Beetle は `"touched"` を受ける（逃げる反射の材料）。
* **当たり判定**（著者の希望 2026-09-17）: 生き物どうし、生き物と木・岩は**すり抜けない**。物理エンジン（avian/rapier）は入れず（wasm と依存の重さ、
  規則が見えなくなる）、XZ 平面の円で書く: `Collider { radius: f32 }`（`Reflect`、Ruby からも `e[:Collider]` で読める）を生き物・木・岩に付け、
  移動の後に「重なった 2 円を半径の和まで押し戻す」（生き物どうしは半分ずつ、木・岩は動かない）。40×30 マスで数十体なので、マスの格子（`HashMap<(i32,i32), Vec<Entity>>`）で
  近傍だけ調べれば十分。草（`Plant`）は**食べる対象なので通り抜けられる**（接触＝食事）。木は食べられない障害物で、`Tree`（`Plant` とは別のコンポーネント）。
  接触が始まったフレームにだけ `publish(Some(creature), "bumped", …)`（相手のエンティティ）を出し（毎フレーム出すとキュー 64 が 1 つの出来事で埋まる。G0 で確認）、G1 で `reflex(:bumped)` が向きを変える材料にする。
  selftest: 「90 秒間、生き物どうし・生き物と木の中心距離が半径の和の 90% を下回るフレームが無い」。
* `--headless N` と `SABIBOTS_SELFTEST` に当たる `GARDEN_SELFTEST`: 「10 秒以内に誰かが食べる」「60 秒で夜が来る」「餓死したエンティティが消える」「すり抜けが無い」。

**実装で分かったこと**（`docs/worklog/2026-09-17-garden-G0.md`、成果は `docs/garden.md`、画は `docs/garden.png`）:

* **3D の代金は features 3 つ。** `bevy_pbr` だけでは足りない。`Camera3d` の既定のトーンマッパーは曲線を KTX2 のテーブルから読むので
  `tonemapping_luts` が要り、それは `bevy_image/zstd` を要求するが、その `zstd` は**中身の無いマーカー feature**で、実装は
  `zstd_c`（C）か `zstd_rust`（ruzstd）を別に選ぶ形（`bevy_image-0.19.1/Cargo.toml:63`）。G5 のブラウザ版を考えて `zstd_rust` にした。
  workspace の `bevy` は 1 つなので sabibots もビルド時間だけ払う。bevy 0.19 では `DirectionalLight` のフィールドは `shadows_enabled` ではなく
  `shadow_maps_enabled`、`AmbientLight` はカメラに付けるコンポーネントで、既定値の方が `GlobalAmbientLight` というリソース。
* **headless で 3D はそのまま走る。** sabibots の `.init_asset::<Image>()` に当たるものとして `init_asset::<Mesh>()` と
  `init_asset::<StandardMaterial>()` を足すだけで、レンダラ抜きで同じ startup が通る。**窓と headless で世界の作り方が完全に同じ**になった。
  render 側が入れる `GlobalAmbientLight` と `ClearColor` だけが headless に無いので、そこは `Option<ResMut<_>>`。
* **登録しないことがアクセス制御。** 仮の脳 `Wander`、`Sun`、selftest 用の `Fasting` は `Reflect` を derive せず `register_type` もしない。
  rubevy の `docs/host-api.md` の通り、登録されていない型は Ruby から `nil`・`has?` は `false`・`components` に出ない。
  おかげで Ruby から見えるコンポーネントは計画書が挙げたものちょうど（`Plant`/`Tree`/`Rock`/`Collider`/`Creature`/`Hunger`/`Velocity`/`Sight`/`Memory` + `Transform`）になっている。
* **`Transform` は自分で登録する。** `MinimalPlugins` は登録しない（同文書）。headless が確認の場なので `app.register_type::<Transform>()` を書いた。
* **メッシュは子エンティティに置いた。** 親の `Transform` を「位置・向き・大きさ」だけにしておくと、甲虫のカプセルを寝かせる 90 度回転のような
  見た目の都合が Ruby の読むものに混ざらない。G0a の `.glb` 差し替えでコンポーネントが 1 つも変わらない形でもある。
* **selftest の 3 つ目は運任せにしない。** 「餓死したエンティティが消える」は、仮の脳がランダムに歩く以上たまたま起きないことがある。
  `GARDEN_SELFTEST=1` のときだけ、隅に空腹 3.0 の Beetle を 1 匹、**`Wander` を付けずに**置いた（動かすのは `Wander` だけなので動かない）。
  世界の規則には手を入れていない。90 秒の実測では、これとは別に「運の悪い Beetle」が自然に餓死している回もあった。
* **当たり判定（途中で増えた要求、`2299365`）**: `Collider { radius }` を Reflect 登録して生き物・木 7・岩 9 に付け、移動の後に
  1.6 単位のマスの格子で近傍を拾って円を押し戻す（生き物どうし半分ずつ、木・岩は不動、**1 フレーム 4 パス**）。
  草に `Collider` を付けないことが「食えるもの／避けるもの」の定義になった。
  4 つ目の selftest は 3 回の 90 秒走行で最接近 0.974 / 0.998 / 1.000（半径の和に対する比）、0.9 を下回ったフレームは 0。
  **岩どうしは押し戻さない**ので、木と岩は初期配置の側で 1.5 以上離している（規則が直せないものは配置で作らない）。
* **決めなかったこと 2 つ（著者の確認待ち）。**
  1. `Hunger` の向き。計画書の規則の欄は「食べたら `Hunger` が減り」だが、`Hunger(f32)`（0 で餓死）と G1 の
     `if me[:Hunger] < 30 → 草を探す` とは逆。後者 2 つに合わせて**満腹度（0 が死、食べると増える）**として実装した。
  2. `"bumped"` の頻度。計画書は「押し戻されたフレームに publish」。字義通りだと木に寄りかかった 1.5 秒で ≒90 回出て、
     rubevy のキュー（64、古いものから落ちる）が 1 つの出来事で埋まる。`"touched"` と同じく**接触が始まったフレームだけ**にした。戻すのは 1 行。

**実装で分かったこと（G0a・G1、`docs/worklog/2026-09-17-garden-G0a-G1.md`）**:

* **「crates.io に無い」と cargo が言ったら、まずキャッシュを疑う。** `bevy_gltf` / `bevy_scene` /
  `bevy_animation` の 0.19.1 が「存在しない」と言われた。実際には全部ある。ローカルの sparse index の
  キャッシュ（`~/.cargo/registry/index/*/.cache/be/vy/*`）が公開前のもので、`cargo update` は
  **既にグラフに入っている crate しか引き直さない**。3 ファイル消して解決。バージョンは 1 つも動いていない。
* **`bevy_animation` だけではクリップが読めない。** glTF ローダがアニメーションを読むのは
  `gltf_animation` feature（`bevy_gltf?/bevy_animation`）。無いと `#Animation1` が「存在しない
  アセット」になり、ERROR が 1 行出るだけで静かに止まる。
* **`WorldAsset` の spawner は未登録の型があると panic する。** bevy 0.19 は自動登録を
  `reflect_auto_register` feature に移したので、素の状態では `Transform` すら登録されていない。
  `reflect_auto_register`（`Reflect` を derive した型を全部登録する）を入れれば 1 行だが、
  bevy の型が何百と Ruby の前に並び「registry はゲームが決める」という主題を捨てるので入れず、
  **窓のビルドだけ** 23 個（`GlobalTransform`、`TransformTreeChanged`、`VisibilityClass`、`Aabb`、
  `Name`、`ChildOf`/`Children`、`Mesh3d`、`MeshMaterial3d<StandardMaterial>`、アニメーション 3、`Gltf*` 7）を
  手で登録した。窓では Ruby からこれらも読めるが、ゲームの私物は `Reflect` を derive していないので
  登録しようがなく、確認の場（headless）はモデルを読まないので表のまま。
* **`make_look` → `spawn_world` の順序が要る。** `Option<Res<Look>>` の `None` はエラーではなく
  「モデル無し」なので、逆になると地面も木も生き物も出ず、後から生える草だけがモデルを持つ。
  `make_look` は headless に無いので、空の `SystemSet` を作って `after` した。
* **bevy 0.19 では `.glb` は `Scene` ではなく `WorldAsset` になる**（置くのは `WorldAssetRoot`）。
  `bevy_scene` は次世代の BSN シーンに名前を取られ、旧来のものは `bevy_world_serialization` に移った。
  `GltfAssetLabel::Scene(0)` だけは `Scene` のまま。
* **Quaternius「Ultimate Animated Animals」は使えなかった**（CC0 だが 12 種にウサギも虫もおらず、
  `.glb` でなく `.gltf` + `.bin`）。Kenney の Cube Pets 2.0 に替えた。**Kenney の 3D 49 キットに甲虫は無い**ので、
  Beetle にはカニを当てた（同じパレット・同じクリップ名で済む）。替えるなら caterpillar か bee が同じパックにある。
* **headless はモデルを読まない。** `Look` を窓のときだけ作り、spawn 系は `Option<&Look>` を取る
  （`day_night` の `Option<ResMut<GlobalAmbientLight>>` と同じ形）。glTF を headless で読むには
  プラグインを 4 つ足すことになり、selftest が Bevy のローダの試験になる。確かめたいコンポーネントは
  全部親にあり、子は 1 つも見ていない。
* **`Velocity(Vec2)` は Ruby から `[[vx, vz]]`。** タプル構造体 1 個の中に Vec2 なので、書くのも
  `me[:Velocity] = [[vx, vz]]`。計画書の擬似コード（`[vx, vz]`）はフィールド数が合わない。`act` の中に隠した。
  `Hunger(f32)` も同じ理由で `me[:Hunger][0]`。
* **`Rubevy::Proxy` は rubevy の prelude に入っていない**（`assets/scripts/proxy.rb` というサンプル）。
  ゲームが持つもの、という rubevy 自身の説明どおり、garden の prelude に 20 行を写した。
* **`garden.nearest` / `count` はコンポーネント名で `match` しない。** 型名 →
  `AppTypeRegistry::get_with_short_type_path` → `ReflectComponent::contains` で解く（rubevy の
  `entities.with` と同じ）。そうしないと「コンポーネント 1 つあたりの接着コード 0 行」が嘘になる。
* **`"ate"` は 1 食に 1 回。** 毎フレーム publish すると 1 回の食事でキュー（64）が埋まる。
  payload も「一口の価値」（常に 1.0）から「座った草の大きさ」に変えた。
* **反射と脳のハンドルの取り合い。** 反射は 1〜2 フレームで `act` するが、脳は読みを重ねて 4〜5 フレーム
  かかるので、**反射の直後に脳の `act` が上書きする**（last-writer-wins）。`Creature#act` に
  「今ハンドルを持っているタスク以外は書かない」を入れた（フラグではなく `Task.current` で持つ）。
  「夜に velocity 0」が落ちたのも同じ形で、脳の `@asleep` 分岐にも `stop` が要る（計画書の擬似コードが
  `act 0, 0; sleep 0.5; next` になっているのはこのため）。
* **`Velocity` は壁で成分が 0 になる**ので、壁際の生き物の「向き」は `Velocity` から読めない。
  「触られたら向きを変える」の selftest は壁際を数えない。

## アセット（G0a）

著者の希望（2026-09-17）: 基本形状だけでなく、**軽いフリーの 3D アセット**を使う。条件は **CC0**（帰属不要でも `CREDITS.md` に出典・ライセンス・取得日を書く）、
**glTF（`.glb`）**、テクスチャは無いか小さい（頂点色かパレット 1 枚）、**合計 2 MB 以内・10 ファイル以内**（wasm と Pages の重さ）。候補:

* 草・木・岩: **Kenney「Nature Kit」**（CC0、330 点、glTF あり。https://kenney.nl/assets/nature-kit ）から低木・草・岩を 3〜4 点。
  代替: **KayKit「Forest Nature Pack」**（CC0、glTF。https://kaylousberg.itch.io/kaykit-forest ）。
* 生き物: **Quaternius「Ultimate Animated Animal Pack」**（CC0、12 種、歩く・食べる等のアニメーション付き glTF。https://quaternius.com/packs/ultimateanimatedanimals.html ）
  から Rabbit 相当 1 点と、Beetle に当たる小さい生き物 1 点（無ければ同パックの別の小動物か、Poly Pizza で CC0 の甲虫を 1 点。https://poly.pizza ）。
  アニメーションが付くので `bevy_animation` + `bevy_gltf` を features に足し、**歩く/止まる/食べる** を `Velocity` と食事の状態から切り替える（Bevy の `AnimationPlayer`。
  これも「Bevy らしさ」の一部で、Ruby は一切関知しない）。
* 実装者がやること: ダウンロードして各ファイルのライセンス表記を確認し、選んだ `.glb` を `garden/assets/models/` に置き、サイズ表（ファイル・三角形数・バイト）を
  `docs/garden.md` に、出典を `CREDITS.md` に。**2 MB を超えるなら報告して止まる**（Blender で間引く判断は著者）。
  wasm ではアセットは `fetch` で読まれるので、`web/` の同梱リストに足す。

## 頭脳（G1）

`ruby/prelude.rb`（Rust に埋め込む。sabibots の `build.rs` と同じ）と `ruby/creatures/{beetle,rabbit}.rb`。**到達点**:

```ruby
creature "Beetle" do
  reflex(:touched) { |by| flee_from(by); sleep 0.5 }   # 別タスク（Battle の reflex と同じ土台）
  reflex(:night)   { @asleep = true }
  reflex(:day)     { @asleep = false }

  def run
    me = Rubevy.entity
    loop do
      if @asleep then act 0, 0; sleep 0.5; next end
      if me[:Hunger] < 30
        plant = garden.nearest(:Plant)              # Proxy: 規則の質問だけ
        head_to(plant[:Transform][:translation]) if plant
      else
        wander
      end
      sleep 0.2
    end
  end
end
```

* `me[:Hunger]`、`plant[:Transform]` は rubevy の `Entity#[]`（1 往復 ≈ 1 フレーム。**判断 4 の `PreUpdate` の順序をこのゲームには最初から入れる** —
  rubevy に `SystemSet` が入るまでは `answer_requests` を `PreUpdate` に置き、入ったらそれに乗り換える）。
* `act(vx, vy)` は `me[:Velocity] = [vx, vy]`（`Rubevy.set_component` のコマンド。Battle の `act` と同じ「後勝ち」）。
* `garden` は `Rubevy::Proxy.new("garden")`。答える kind は `nearest`（種類、半径は `Sight`）と `count`（種類）だけ。それ以外は全部 `Entity#[]` で済むことを、
  **`sabibots/src/main.rs` の `ask` の kind 数と並べて** `docs/garden.md` に書く（Battle: 6 種、箱庭: 2 種）。
* `wander`/`head_to`/`flee_from` は prelude の Ruby。座標計算は Ruby でよい（規則ではない）。
* HUD（G4 まではログ）: 各生き物の insn/フレーム、待っている行（`task_location`）。

## `Genome`（G2）

Rust の構造体を Ruby から使う。**マクロを使う版**を本番にし、比較のために手書き版（`define_closure` × 6）を `docs/garden.md` に行数付きで並べる（コードは入れない）:

```rust
// 実装したもの（`garden/src/genome.rs`）。Serialize/Deserialize は G3 で足す
#[derive(Clone, Copy, Debug, Reflect, RubyClass)]
#[ruby(name = "Genome")]
pub struct Genome { pub speed: f32, pub sight: f32, pub appetite: f32 }

#[ruby_methods]
impl Genome {
    fn new(speed: f64, sight: f64, appetite: f64) -> Self { … }
    fn speed(&self) -> f64 { … }  fn sight(&self) -> f64 { … }  fn appetite(&self) -> f64 { … }
    fn mix(&self, vm: &mut Vm, other: Value) -> VmResult<Genome> { … }   // 平均。`&Genome` は取れない
    fn mutate(&self, vm: &mut Vm, rate: f64) -> VmResult<Genome> { … }   // VM の乱数（`Random`）
    fn to_h(&self, vm: &mut Vm) -> VmResult<Value> { … }
    fn to_s(&self) -> String { … }
}
```

* `Creature` に `Genome` を持たせ、`Sight`/速度の上限は `Genome` から決める（Rust の規則）。Ruby は `me[:Creature][:genome]` で **Hash として**読める（Reflect 経由）が、
  `Rubevy.ask("genome")` で **`Genome` の Data オブジェクト**としても受け取れる（`Answer::Data` 相当が無ければ `answer_value` で `data_new`）。両方の見え方を `docs/garden.md` に。
* 繁殖: `Hunger` が満ちた 2 体が触れると Rust が子を産み、`Genome#mix` → `#mutate` を **Ruby から**呼ぶ形にする（`reflex(:mate) { |partner| child = my_genome.mix(partner_genome).mutate(0.1); garden.spawn(species: …, genome: child.to_h) }`）。
  規則（誰が誰と産めるか）は Rust、値の計算は Ruby から Rust のメソッドを呼ぶ、という分担を見せる。
* `sabiruby-macros` は `sabiruby` の `macros` feature（release-prep の Part C で入る）経由。それまでは `sabiruby-macros = { git = … }` 直接でよい。

**実装で分かったこと（G2、`docs/worklog/2026-09-17-garden-G2.md`、成果は `docs/garden.md` の「The genome (G2)」）**:

* **マクロ版 61 行 / 手書き版 113 行**（どちらもコメントと空行を除いたコード行、同じ 8 メソッド）。
  手書き版は実際にビルドを通してから数えて消した（`garden/src/genome_by_hand.rs`、コミットしていない）。
  差の 52 行は全部配管 — store と tag、クラスと特異クラス、返り値ごとの `data_new`、
  引数ごとの `data_of` と型エラー、メソッドごとの引数個数、引数ごとの `FromRuby`。
  手書き版には**有利に**倒してある（`Genome` は `Copy` なので store から借りずにコピーで済ませ、
  `take_out`/`give_back` が要らない。getter 3 本は 1 つのクロージャで共有）。
* **`mix(&self, other: &Genome)` は書けない。** マクロが引数を変換するのは `FromRuby` で、
  参照に `FromRuby` は無い（`docs/design/macros.md`「What it does not cover」）。
  自分と同じクラスのオブジェクトを取るメソッドは `&mut Vm` をもらって自分で
  `Genome::borrow(vm, other)` する。副作用として `g.mix(g)` は
  `RuntimeError: Genome is already in use by a call on the same object` になる
  （`&mut Vm` を取るメソッドは自分のレシーバを store から出しているため）。計画書の擬似コードは直した。
* **乱数は VM の `Random`。** native が受け取るのは `&mut Vm` だけで Bevy の `World` には触れないので、
  ゲームの `Dice` リソースは届かない（rubevy `docs/host-api.md`）。`clock_seed` の私物 RNG にすると
  Ruby から `srand` で手が入らない 2 本目の運になるので、`vm.funcall(Random, :rand)` にした。
  スクリプトの `rand` と同じ 1 本の流れになる。
* **「触れたら」という規則はほとんど発火しない。** 2 つの円が重なるのは `separate` が押し戻す 1 フレームだけで、
  そのフレームに `"bumped"` が出て両方のスクリプトが互いから逃げる。40 秒の実測で、満腹な同種 2 匹が
  最も近づいたのは **1.24**（半径の和は 0.80）。「1 体分の距離（2.0）以内」に変えた。
  閾値 `MATE_HUNGER` も 85 では 90 秒に 2 回・0 回のこともあり、75（甲虫自身の「草を探す」55 より十分上）にした。
* **子は生まれた瞬間、まだ耳が無い。** `Script` が `ScriptTask` になるのは次以降のフレームで、
  そのタスクが最初にするのが購読なので、生後すぐに publish された出来事は落ちる。
  規則ではない（聞こえなかった生き物はただ歩き続ける）が、G1 の「触られたら向きを変える」判定は
  生後 2 秒未満を数えないようにした（`NEWBORN_GRACE`）。
* **費用は「子が来たとき」に払う。** `"mate"` を publish した時点で腹を減らすと、購読していない
  スクリプト（ウサギには `reflex(:mate)` が無い）にも課金することになる。publish は 2 秒ごとの再送にして、
  親の腹と 20 秒のクールダウンは `hatch`（子が実際に生まれるフレーム）で引く。
* **kwargs は `method_missing(name, *args)` に Hash で届く。** `garden.spawn(species: …, genome: …, at: …)`
  が `Rubevy::Proxy` を通って `Arg::Value` の Hash 1 個になることは実測で確認した（計画書の綴りのまま書ける）。
* **G1 の 6 つ目の判定はもともと不安定。** 「ウサギに触られた甲虫が 0.5 秒で向きを変える」は
  main にマージ済みの G1 バイナリ（`a61a611`、G2 の変更が 1 つも入っていないもの）でも
  90 秒 4 回中 1 回落ちる（32/33）。この枝は 8 回中 2 回（35/37 と 37/39）。落ちる例は
  「39 度だけ曲がった」= 脳の `wander` に見える形なので、反射が壊れたのではなく取りこぼしか遅れ。
  G1 の課題なのでここでは追っていない。**他の 7 つは 8 回とも ok。**

## セーブ/ロード（G3）

* Rust: `#[derive(Serialize, Deserialize)] struct GardenSave { tick, day_phase, plants: Vec<PlantSave>, creatures: Vec<CreatureSave> }`。
  `CreatureSave` に `genome: Genome` と `memory: serde_json::Value`（Ruby の `@memory` をそのまま）。
* 書く: F5。各生き物の `@memory` を `Rubevy.ask("memory.dump")`（Ruby 側が `@memory.to_json` を返す）… ではなく、**Rust から Ruby の値を読む**方向を見せるため
  `ScriptWorld` 経由で `ivar_get(script_self, "@memory")` → `sabiruby_serde::from_value::<serde_json::Value>`。世界全体を `serde_json` で 1 ファイル（PC: `garden.save.json`、web: `localStorage`）。
* 読む: F9。`GardenSave` から世界を作り直し、各生き物のスクリプトを起こしてから `ivar_set("@memory", sabiruby_serde::to_value(&memory))`。
* Ruby から型付きに生成: `garden.spawn(species: "Beetle", genome: {...}, at: [x, y])` を `define_fn(|spec: Serde<CreatureSpec>| …)` で受ける。Hash の形が違えば `TypeError` に何が足りないかが出ることを selftest に。
* Ruby 側の `JSON`: `sabiruby_serde::install_json(&mut vm)` を rubevy のプラグインの初期化に挟む（rubevy に VM を触る入口が無ければ `RubevyPlugin` に `with_vm(FnOnce(&mut Vm))` を 1 本足す — rubevy 側の小さな追加、報告して止まる）。
  生き物は `@memory[:favorite] = plant.to_h` のように Hash を持ち、`log JSON.generate(@memory)` で見せる。
* selftest: 保存 → 読み込みで、位置・`Hunger`・`Genome`・`@memory` が一致（`GardenSave` を 2 回作って `==`）。

**実装で分かったこと（G3、`docs/worklog/2026-09-17-garden-G3.md`、成果は `docs/garden.md` の「Saving and loading (G3)」）**:

* **`script_self` は使えない。** task が走るときの self は VM の `main` オブジェクトで、**全タスクで 1 つ**
  （`sabiruby` `src/builtins/ext_task.rs:390`）。生き物のファイルのトップレベルの `@memory` は
  別の生き物の `@memory` と同じ変数になる。prelude が `Creature` のサブクラスを `new` する形（G1 から）が
  それを避けている理由そのもので、ホストからその**オブジェクトへ行く道は自分で敷く**必要があった:
  `run_creature` に `Task.current.instance_variable_set(:@being, being)` の 1 行、Rust 側は `ivar_get` 2 回。
  rubevy が entity を `@rubevy_entity` に置いているのと同じ手。
* **rubevy には 1 行も足していない。** `install_json` は `ScriptWorld::vm` を触る `Startup` のシステムに 1 行
  （G2 の `Genome::register` の隣）。計画書が保険で書いていた `with_vm` は要らなかった。
* **戻すほうは 1 フレーム遅れる。** `@memory` を置く相手は `run_creature` が作るので、復元は
  「タスクが最初に走ったフレームの終わり」になる。ウサギは `run` の 1 行目で記憶を読むので、
  復元前の空の Hash を見て `Rubevy.find(:Tree)` で庭を歩き直していた（ログで確認）。prelude で
  購読の後・`run` の前に `sleep 0.05` を 1 行入れて直した。
* **読み込み中は世界を止めた。** 規則すべてに run condition を付け、世界の時計（`Sky::shift`）も釘付けにする。
  おかげで判定が「保存 → 読み込み → 保存 → `diff`」になり、11338 バイトが 1 バイト違わない。
  許容誤差つきの比較スクリプトは捨てた（比較が育つと何を確かめているか分からなくなる）。
* **`serde_json` のパーサは既定で 1 ULP ずれる。** 最初の `diff` が 4 行だけ落ち、全部ウサギの覚えている
  木の座標（ファイルの中で唯一 f32 に丸め直されない f64）。同版で 4 行のプログラムを書いて確かめた:
  書くほうは常に正確、読むほうが `9.595357894897461` を `9.59535789489746` にする。`float_roundtrip` feature で解決。
* **`@memory` の鍵は String。** `Options::symbol_keys` が効くのは struct のフィールド名と variant 名だけで、
  map の鍵は String で出る（`serde/src/ser.rs`）。Symbol 鍵の記憶は保存 → 復元で別の Hash になる。
* **`garden.spawn` は手書き 72 行 → serde 7 行**（`read_birth` 47 + `read_genome` 25 が `CreatureSpec` に）。
  clamp（10 行）は残した — 形の話ではなく規則だから。エラーも良くなった:
  ``missing field `sight` (TypeError)`` が**そのままスクリプトへの答え**になる（9 つ目の selftest）。
* **`define_fn(|spec: Serde<CreatureSpec>|)` にはしなかった。** native は `&mut Vm` しか持たないので
  個体数を数えられず、entity を作れず、`true` か理由かを返せない。結局システムにメモを残すことになり、
  それは `Request` そのもの。`ask` のまま `RubevySet::Answer` に置き、`Serde` は引数を読む半分だけ使っている。
* **`Memory` コンポーネントは空のまま。** 記憶の実体は VM の中の Hash で、コンポーネントに写すと同じものが
  2 つになる。ECS にあるのは「この entity は記憶を持つ」という事実だけ。
* 未実施: 窓の F5 / F9 は押していない（この機械に GPU ドライバが無い）。web の `localStorage` 分岐は
  `cargo check --target wasm32-unknown-unknown` が通ることだけ（実際に動かすのは G5）。

## 窓（G4）

`rubevy-arena` のエディタ（生き物のファイルを書き換えて即反映）と VM パネル（`F2`。`Vm::task_context` への置き換えは battle-followups で済み）を載せる。HUD: 生き物ごとに「1 判断のフレーム数」（`ask` の発行から答えまでを rubevy の stats から）、全体の VM 時間 / 8 ms。
`--shot` で 1 枚。

**実装で分かったこと（G4、`docs/worklog/2026-09-17-garden-G4.md`、成果は `docs/garden.md` の「The window (G4)」）**:

* **6 つ目の判定の犯人は 2 つで、どちらも実在のバグだった**（閾値は 1 つも動かしていない）。
  1 つ目は prelude: `@course`（自分が向いていると思っている方向）を `wander` / `head_to` が
  `act` を呼ぶ**前に**書いていて、`act` は反射がハンドルを握っている間は黙って帰る。
  だから「誰も走ったことのない向き」が残り、`flee_from` はそれを基準に**逃げる側**を選ぶ。
  側を間違えると 57 度の逃走が 39 度になり、閾値（45.6 度）を割る。`@course` を書くのは
  `act` の中だけ・書き込みが実際に出たときだけ、にした。これで曲がり角は
  `|逃げる方向までの角| + swerve` ＝ **最低 57 度**で、判定は通るのが保証される。
  2 つ目は判定側: 「1.5 秒以内に触られていたら数えない」の**時計のリセットが他の除外の if の中**にあった。
  止まっている・壁際・生後 2 秒未満の甲虫に送った `"touched"` はキューには入るのに時計には入らないので、
  次の接触が「1.5 秒ぶり」に見える。実測で**数えた 211 件のうち 41 件**がそれ。
  前 9/10（1 回 35/36 で落ちる）→ 後 **10/10**。
* **捨てた仮説を 1 つ測って捨てた。** ルールの chain が `RubevySet` と順序を持たないので
  `startle` が 1 フレーム古い `Velocity` を見ているのでは、と疑って、順序を付けた版と付けない版の
  2 本のバイナリで「控えた速度の角度」と「スクリプトの `@course`」を並べた。
  212 件、1 件もずれず。`.after(RubevySet::Tick)` は入れずに戻した。
* **一度に 10 匹入れ替えると VM のスケジューラが止まる（要修正、VM 側）。** Apply で甲虫全部を
  入れ替えると `Vm::task_pending()` は true のまま `task_run_limits` が 1 命令も走らせず、
  **触っていないウサギも含めて VM 全体が永久に凍る**（エラーログ無し）。境界を測ると
  **9 匹は平気、10 匹で止まる**（甲虫 1 匹＝脳 1 + 反射 6 タスク、購読 6 本）。
  「1 フレーム 3 匹ずつ」でも止まるので、フレームあたりではなく**短い間の総数**。
  時計で 0.4 秒に 1 匹に散らすと 12 匹でも平気だったので、ゲーム側は待ち行列で渡している
  （`window.rs` の `Restarting`）。sabibots が踏んでいないのは 4 体・反射 1 本だから。
  * **2026-09-17 追記: VM が直ったので待ち行列は消した。** 原因は「古い context の解放が追いつかない」
    ではなかった。`ScriptTask` を外すと反射タスクが `Rubevy::Unsubscribed` で起こされ、
    中身のない `rescue` がブロックの値を `nil` にする。`Vm::task_run_limited` はその `nil` を
    「ready が無い」と読んでホストの 1 フレームを丸ごと終わらせていた（反射 1 本＝1 フレーム、
    甲虫 1 匹＝6 フレーム、10 匹＝60 フレーム）。sabiruby 0.5.1（`bd6829b`、
    sabiruby `docs/worklog/2026-09-17-task-end-nil.md`）で 2 つが区別されるようになったので、
    `Restarting` / `RESTARTS_PER_FRAME` / `RESTART_GAP` と `restart_queued` を削除し、
    Apply は押されたフレームで種の全個体を入れ替える。lavapipe の窓で 1/4/9/10/12/全部 を
    測り直して、どれも VM は進み続ける（表は `docs/garden.md`、記録は
    `docs/worklog/2026-09-17-garden-vm-0.5.1.md`）。
* **エディタの単位はゲームで違う。** sabibots はロボット 1 体＝1 ファイルなので Apply が 2 つ
  （この 1 体 / 同じファイル全員）。箱庭はファイル＝**種**なので 2 つは同じ集合で、Apply は 1 つ。
  押すとその種が全部（同じフレームで。0.4 秒に 1 匹ずつ渡していたのは上の追記のとおり VM の
  バグ回避で、いまは無い）再起動し、**そのあと生まれる子にも配られる**（`Brains`）。
  `rubevy-arena` に足したのは `Editor` の 4 フィールド（`apply_label` / `apply_all_label: Option`（None で描かない）/
  `apply_key: Option<KeyCode>`（箱庭の `F5` は保存で埋まっている）/ `noun`）だけで、sabibots の変更は 1 行。
* **「1 判断のフレーム数」は定義が仕事。** タスクが止まる理由は「聞いた」か「寝た」かの 2 つだけ。
  ゲームへの質問は `answer_garden` が答えた相手にフレーム番号を書く（`Mind::asked_frame`）ので
  **分かっている**。コンポーネント読みは rubevy が `answer_components` で自分で答えるのでゲームからは
  見えないが、`ruby/` の中で一番短い `sleep`（0.05 秒）より短い空白は読みだと言える
  — これは許容誤差ではなくスクリプトについての事実。実測: **ゲームへの質問 403 件 1.000 フレーム、
  コンポーネント読み 2352 件 1.000 フレーム**。`answer_garden` が `RubevySet::Answer` に居ることの値段。
* **VM の時間は `RubevySet::Tick` を `Instant` で挟んだ壁時計。** 比べる相手
  （`ScriptWorld::frame_time` = 8 ms）が壁時計なので。15 匹・約 100 タスクで **1.3 / 8.0 ms**。
* **`Editor::show` の鍵は種の番号。** entity にすると同種の別個体をクリックするたびに
  打ちかけの編集が draft に仕舞われる。
* **スクリプトを入れ替えると `@memory` は戻らない。** 記憶は古いスクリプトが作ったオブジェクトの上にある。
  sabibots と同じで、これは正直な挙動。

## ブラウザ版（G5、必須）

`web/build.sh` は既にゲーム名を引数に取る（`build.sh sabibots`）が、出力先が `web/dist` 1 つで、`index.html` と Pages の workflow が sabibots 前提。
これを **2 ゲームを 1 つの Pages に**する形にする: `web/dist/` 直下に入口の `index.html`（2 つのゲームへのリンクとひとこと）、`web/dist/sabibots/`、`web/dist/garden/`。
`build.sh` は `build.sh <game>` で `dist/<game>/` に書き、`build.sh all` で両方 + 入口。workflow は `all` を回す。
URL は `https://sabiruby.github.io/rubevy_games/`（入口）、`…/sabibots/`、`…/garden/`。**今の `…/rubevy_games/` で Battle が開く URL は変わる**ので、
入口ページに Battle へのリンクを一番上に置き、README・`docs/web.md`・book 側の参照（本体が直す）を更新する。
アセット（G0a の `.glb`）は `dist/garden/assets/` に同梱し `fetch` で読む。`localStorage` のセーブは G3 で `platform.rs` に隠してある。
**セーブに版番号を付ける**（著者判断 2026-09-17）: `GardenSave` に `version: u32`（今は 1）を足し、読み込み時に版が違えば読まずに「保存の版 N はこの版の箱庭では読めません」と
HUD/ログに出して新しい世界で始める（`localStorage` に古い保存が残るので必須）。headless の `--load` も同じ扱い。selftest に「版が違う保存は拒む」を 1 本。
`Memory` コンポーネントは空のままでよい（著者判断: 記憶の実体は VM の Hash、ECS には「持つ」事実だけ）。
wasm のサイズ（gzip 前後）を `docs/web.md` に Battle と並べて記録。lavapipe の `--shot` とは別に、ブラウザで 1 分動かして selftest 相当（食べる・夜・餓死）が
ログに出ることを確認（`web/serve.sh` + headless Chrome があれば自動、無ければ手で）。

**実装で分かったこと（G5、`docs/worklog/2026-09-17-garden-G5.md`、成果は `docs/web.md` と `docs/garden.md` の「In a browser (G5)」）**:

* **無いと言われた道具が 2 つあった。** `wasm-opt` は `PATH` に無いだけで `~/.local/binaryen-version_132/bin` に居り、
  CI と同じ binaryen 132。ブラウザも `~/.cache/ms-playwright` の Chromium 153 と、隣の playground の
  `playwright-core` がある。おかげで「未実施」にする予定だった確認が全部できた。
* **`wasm-opt -Os` はファイルを 10% 小さくし、gzip を 7% 大きくする**（sabibots 9.88 → 10.59 MB gz、
  箱庭 10.01 → 10.72 MB gz）。Pages が送るのは gzip の方なので、**`wasm-opt` の言い分は「ダウンロードが速い」ではない**。
  生の大きさ（ブラウザが decode して持つもの）のために CI では回したままにした。やめるかは著者判断。
  箱庭は Battle より 1.2% 大きいだけ（39.25 対 38.77 MB）だが、**Battle 自身が 26.7 → 34.8 MB に育っている**
  （`web` プロファイルは同じ。育てた犯人は分解していない）。
* **版は「版だけ」を先に読む。** `GardenSave` 全体を通して失敗を見ると、出るのは
  ``missing field `sight` at line 214`` のような**たまたま変わったフィールド**の話で、何が起きたか言わない。
  `version: Option<u32>` だけの struct を同じテキストに先に通す（serde_json は知らない鍵を捨てるので何でも通る）。
  版は struct から導かず**定数**にした: default できるフィールドの増減は「読めてしまうが意味が違う」ファイルを残すので、
  版はファイルの意味についての約束であり、人にしか作れない。
* **10 個目の判定は `--load` そのものに通した。** `read_save` を横から呼ぶと、試しているのが `--load` でなくなる。
  版以外は完全に正しい保存（`GardenSave::default()` + `version: 99`）を書いて `--load` の腕に渡すので、
  版を見なくなったら**読めてしまい**判定は落ちる。
* **アセットは何も言わなくてよかった。** bevy の wasm リーダは `assets/…` を**相対 URL** で `fetch` する
  （`bevy_asset-0.19.1/src/io/wasm.rs`）ので、`…/rubevy_games/garden/` のページはそのまま
  `…/garden/assets/…` を取る。404 は 1 本も出ていない。
* **`page.goto(..., {waitUntil:'load'})` は返ってこない。** モジュールスクリプトが `await game.default()` で
  終わり、それは動いている限り返らない。`load` はモジュールを待つので、**正しく動いているページほど来ない**。`'commit'`。
* **ページは遅くない。** rAF 7531 回 / 150 秒 ≒ 50 fps（SwiftShader）。最初に測った「5 fps」は
  35 MB の wasm を落として compile している時間を数えていた。sabibots の記録にある「1 フレーム以内の押し離しは落ちる」も
  ここでは再現しない。**前の記録を引き写さずに測り直してよかった。**
* **`?selftest` を足した。** ページには環境変数が無く、「誰かが食べた」と言う行は判定が書いている。
  これでブラウザが「見る」だけでなく「確かめられる」ようになり、版の判定が効くところも実際に見えた。
* **HUD のボタンは一覧の下に置いてはいけない。** 1280×800・生き物 14 匹で、その行はパネルの外に出て
  VM パネルの下に潜る（クリック 6 か所、全部届かず）。一覧の**上**に移した。
* **F5 / F9 に `EguiWantsInput` の guard を付けたのは間違いだった。** `P` は文字なのでエディタのものだが、
  F5 / F9 は egui が欲しがらない鍵で、guard を付けると「一度編集したらセーブ鍵が死ぬ」になる（テキストボックスは
  庭をクリックしてもフォーカスを返さない）。外した。
* **`?selftest` の窓の判定が終わると、ページがマウスにもキーボードにも答えなくなる**（パネルは描かれ続け、世界は動く）。
  判定を付けないページは 85 秒まで押しても効き、ボタンも効くので、**プレイヤーが踏むものではない**。
  DOM は無実（canvas にフォーカス、`keydown` は `#garden` に届く）、上の guard でもない（外しても直らない）。
  残る疑いは `window_selftest` が `ButtonInput` を手で押していること、または egui がテキストボックスの
  キーボードフォーカスを離さないこと。**確かめていない。**
  * **2026-09-17 追記: どちらでもなかった。直した。** 犯人は `window_selftest` の最後の
    `exit.write(AppExit::Success)`。PC ではそれが正しい（コマンドラインで頼まれた判定なので、
    シェルにプロンプトを返す）が、**ページには終了する先が無い**。winit の wasm ループが
    回らなくなり、全システムが止まり、最後に描いたフレームが canvas に残るので「庭は動いて見えるのに
    何をしても効かない」になる。「世界は動き続けている」という前の読みも間違いで、ログは最後の判定で
    途切れている。`platform::CHECKS_EXIT_WHEN_DONE`（PC は true、ページは false）にして、
    ページでは `selftest: done — the garden keeps running` と言って庭を続ける。
    同じ Chromium で測り直し: F5 `saved 15 creatures …`、F9 `loaded 15 creatures …`、
    `P` は 4 秒間の `[script]` 行で **動作中 2 / 一時停止 0 / 再開 3**。世界も生きている（`night at 145.4 s`）。
* **全部を一度に入れ替えると新しいタスクが `Created` のまま走らない**（G4 の「10 匹で止まる」と同じ形）。
  窓の判定 22 本のうち 1 本（`every restarted beetle's new task has run`）が落ち、
  **F9 のロードでは `11 creatures never started` で記憶が戻らない**（`load_world` は待ち行列を使わず同じフレームで
  11 匹作り直す）。起動時の `--load` は PC で 11 匹とも記憶が戻る（往復がバイト単位で同一）ので、
  「動いている庭に読み込む」側だけが踏んでいる。**VM のスケジューラ側の仕事**として記録した。
  * **2026-09-17 追記: sabiruby 0.5.1（`bd6829b`）で直った。** 原因は G4 の追記のとおり
    「nil で終わったタスクがホストの 1 フレームを食う」で、ブラウザのほうが 1 秒あたりのフレームが
    少ない分だけ先に見えていた（PC の起動時 `--load` が平気だったのは、待ちが
    `RESTORE_PATIENCE` の 5 秒に収まっていたから — 同じコンテナで測ると復元にかかる時間は
    0.5.0 で 1.48 秒、0.5.1 で 6 ms）。ゲーム側に直すところは無かった。
    同じ Chromium で測り直して窓の判定は **21/21**、動いている庭への F9 は
    `loaded 10 creatures, 46 plants, 7 trees, 9 rocks at 45.3 s (78 entities made way)` で
    `never started` は出ず、記憶も 10 匹とも戻る。headless で F9 の道を通すために
    `GARDEN_RELOAD_AT`（`GARDEN_SELFTEST` のときだけ読む）を足した。
* **ブラウザのコンパイラはファイル名を渡せない。** ページの橋は `window.gardenCompile(source)` でソースしか取らず、
  debug info の名前は playground の `playground.rb` になる。HUD の「待っている行」の列がそう出る。
  行番号は正しく、行番号を使うもの（箱庭は prelude の長さを引いて自分の行を出す）は全部正しい。
  名前を渡せるようにするのは sabiruby-playground 側の変更。

## 確認

`cargo build --release -p garden`、`--headless 90 GARDEN_SELFTEST=1`（上の selftest すべて ok）、`--shot`。`docs/garden.md`（構成、境界の表、Battle との比較 —
`ask` の kind 数、Rust の接着コードの行数、`Genome` のマクロ版と手書き版の行数）、`docs/README.md`、`README.md`。worklog `docs/worklog/2026-09-17-garden-G<n>.md`。
段階ごとに別ブランチ・別コミット、push は本体。

## 依存関係（先に確認して、無ければ報告して止まる）

* rubevy: `Rubevy.set_component`、`Rubevy.find`、`subscribe`/`publish`、`answer_value`、`Task.new` の継承、`Unsubscribed` — main `9104f7c` にすべてある。
  **無いかもしれないもの**: プラグイン初期化で `&mut Vm` を触る入口（`install_json` 用）、`Answer` に Data オブジェクトを載せる形、`answer_requests` の順序を決める `SystemSet`。
* sabiruby: `sabiruby-serde`（main にある）、`sabiruby-macros`（`macros` feature は release-prep の後）。

## 遊び心地（G6）

著者がブラウザで試遊して気になった 3 点（2026-09-17）。Battle にも同じものを載せる（共通部分は `rubevy-arena`）。

1. **夜が暗すぎる。** 夜の環境光と `DirectionalLight` の下限を上げ、月明かり相当に（生き物と木が輪郭で分かる。真夜中の `--shot` を撮って決める）。
   昼夜の「差」は残す（寝ているのが分かる程度）。数値は `docs/garden.md` に。
2. **視点。** ホイールが 2 段階しか効かないのは、ブラウザの `MouseWheel` が `MouseScrollUnit::Pixel`（1 回が大きい）で来て、ズーム範囲の端まで飛んでいる可能性が高い（PC の窓は `Line`）。
   単位ごとに正規化し、ズームは距離の対数で滑らかに（1 ノッチ ≒ 10%）。**パン**を足す: 右ドラッグ（`Shift`+左ドラッグでも）と矢印/WASD、`Home` で視点リセット。
   ブラウザで実測（playwright の `mouse.wheel` と drag）して、PC と同じ段数になることを確認。
3. **ゲーム内の説明。** `H` と `?` で開閉する説明パネル（起動直後に一度表示）。内容: 何が起きているか（草・生き物・昼夜・繁殖）、エディタで頭脳を書き換えられること、
   キー一覧（F1/F2/P/F5/F9/Tab/H、マウスの回転・パン・ズーム）。**英語 + 日本語の併記**。egui の既定フォントに日本語は無いので、
   CJK を含む軽いフォント（Noto Sans JP のサブセット。使う文字だけに絞れば数百 KB）を同梱して `FontDefinitions` に足す。
   同梱後の wasm 増分は測って記録する（著者判断 2026-09-17: 日本語フォント分のサイズ増加は許す。サブセット化はする）。
   Battle にも同じパネル（内容は Battle 用）。共通の枠は `rubevy-arena` に。

確認: 夜の `--shot`、ブラウザでホイール 10 ノッチ・ドラッグ・パンの実測、`H` のパネルが両言語で読める `--shot`（日本語が豆腐でない）、既存の判定すべて。

### 実装で分かったこと（G6）

**3 つとも「書いた機械の上では正しく見えていた」という同じ形をしていた。** だから作業の半分は
直すことではなく、**確かめる道具を作ること**だった。夜の数字は真夜中の絵を見ないと決められないのに
真夜中は起動 40 秒後で、窓は lavapipe（1 フレーム 1 秒近い）。ブラウザの中のカメラは外から読めない。
フォントのサブセットは本文と一緒に切らないと壊れる。それぞれに `--at`、`CameraLog`、
`tools/subset-font.sh` が要った。

1. **夜は 2 つの数字で、それぞれ別の仕事をしている。** `DirectionalLight.illuminance`（月、300 →
   **400 lux**）が描くのは**輪郭**で、`GlobalAmbientLight.brightness`（30 → **55**）が埋めるのは
   **影の面**。月だけ上げると「上面だけ明るくて胴体が無い生き物」、環境光だけ上げると「全部灰色で平ら」。
   昼を触っていないので、いちばん暗い昼（1,200 lux・環境光 120）が夜の 3 倍あり、差は一目で読める。
   地面の平均輝度は 18 → 26 / 255（午後は 81）。
2. **ホイールが 2 段階だったのは `MouseWheel::unit` を読んでいなかったから**で、計画書の推測どおり
   だった — が、**推測が当たっていることを実測で確かめられるようにしたのが仕事の中身**。
   Chromium は `Pixel` で 1 ノッチ 100（実測値。規格ではない）、PC は `Line` で 1.0。
   正規化したうえでズームを**比**（1 ノッチ 10%）にした。比にしたのは単位の話とは別の理由で、
   引き算は距離 8 と 80 で同じ感触になりようがないから。端から端まで 30.5 ノッチ。
3. **wasm のキャンバスの中は console しか外に出ていない。** `Orbit` を読む口は無く、HUD の絵から
   読むのは OCR になる。検査のときだけカメラに 1 行喋らせる `CameraLog` を足した（ホイールは
   メッセージごと、ドラッグは 0.25 秒ごと）。**窓の自己テストにホイールの検査は足せない** —
   `MouseWheel` はプラットフォームから来るもので `ButtonInput` のように押せない。
   結果として「窓では単体テスト、ブラウザでは実測」。窓の判定は 21 行のまま。
4. **サブセットしたフォントは本文と心中する。** 日本語を 1 文字足せば白い箱が出る。だから
   「使ったフォントはこれ」というメモではなく、本文 3 ファイルから文字を読み直して切り直す
   `tools/subset-font.sh` にした。9,589,900 → **62,780 バイト**（327 文字、`wght=400` に固定）。
   egui へは **fallback として末尾に追加**したので、既定フォントが持つ文字は今までどおりで、
   エディタとパネルのラテン文字は 1 ピクセルも変わらない。
   wasm の増分は 1 ゲームあたり **約 90 KB（うち 63 KB がフォント）、35 MB に対して 0.25%**。
5. **説明パネルは最前面でないと意味が無い。** 最初の絵ではエディタと VM パネルの後ろに隠れていた
   （両ゲームとも起動時に 3 枚開く）。`egui::Order::Foreground`。2 枚目ではキー表が下で切れていたので
   `default_size([640, 820])` に。探さないと見つからない説明と、スクロールしないと読めないキー一覧は、
   どちらも無いのと同じ。
6. **`--shot` は説明パネルを閉じて始める。** パネルは窓の中央にあり、絵の主役をちょうど覆う。
   `--guide` を付けたときだけ開く（それが「日本語が豆腐でない」を確かめる撮り方）。

**日本語の文面は下書き**で、著者が直す前提。直す場所は `garden/src/guide_text.rs` と
`sabibots/src/guide_text.rs`（共通の 2 文字列だけ `crates/rubevy-arena/src/guide.rs`）、
直したら `tools/subset-font.sh` → `cargo build` → `web/build.sh all`。

## 遊び心地・2 回目（G6b）

1. **夜がまだ暗すぎる。** G6 の値（月 400 lux、環境光 55、地面の平均輝度 26/255）では足りない。目標は真夜中の地面の平均輝度を **40〜50/255**（昼 81 の半分強）。
   月と環境光を両方上げ、色は青みを保つ。真夜中の `--shot` で輝度を測って決め、`docs/garden-night.png` を差し替える。
   加えて、著者が自分で詰められるように **Garden パネルに「night」のスライダ**（0.5〜2.0 の倍率、既定 1.0。PC は設定ファイルかログに値を出す、web は `localStorage`）を置く。
   著者が決めた倍率を後で既定に焼き込む。
2. **説明パネルの言語切り替え。** 英語と日本語を段落ごとに交互に出すのではなく、パネル上部の **「English | 日本語」ボタンで切り替える**（一度に片方だけ）。
   既定はブラウザなら `navigator.language`（`ja*` なら日本語）、PC なら `LANG`/`LC_ALL`（`ja_JP*` なら日本語）、それ以外は英語。選択は `localStorage` / PC の設定に記憶。
   HUD の `H: help / 操作説明` はそのまま（両言語）。`guide_text.rs` の構造（英日を並べて持つ）は変えず、描画だけ切り替える。Battle も同じ。
   `--shot --guide` を `--lang ja` / `--lang en` で両方撮る（`docs/garden-guide.png`、`docs/garden-guide-ja.png`）。

確認: 真夜中の輝度の数値、両言語の絵、既存の判定すべて（headless 10、窓 21、sabibots）、ブラウザで切り替えボタンが効き記憶されること。

**実装で分かったこと**（`docs/worklog/2026-09-17-garden-G6b.md`、成果は `docs/garden.md`、画は
`docs/garden-night.png` と `docs/*-guide{,-ja}.png`）:

* **真夜中の地面は 44.3 / 255 になった**（G6 は 25.9、午後は 79.7。目標 40〜50）。月 400 → **950 lux**、
  環境光 55 → **190**、空 `0.06,0.08,0.17` → **`0.14,0.18,0.36`**。青みは 3 つとも比のまま。
* **3 つの量は同じだけ効かない。** 環境光を 110 → 190 に上げても平均輝度は **1.9** しか動かない
  （月は 100 lux あたり約 3）。平均は 24 万画素の平均で、そのほとんどが地面だから。
  環境光を上げたのは**生き物**のため（小さく丸く、自分の影の中にいる）で、これは平均に出ない。
* **日没の前後を測ったら、昼と夜の差は光源の強さではなく高さだった。** 月は `-up` を向いているので
  日没の瞬間は地平線に寝ていて地面を照らさない: 日没直前 38.7 → 直後 24.5 → 真夜中 44.3。
  G6 が「昼の下限 1,200 lux との間を空ける」ために 400 に留めていた余白は、守る意味が薄かった。
* **倍率のスライダ（`night`、0.5〜2.0）は設定ではなく質問。** 3 つを同じ倍率で掛ける 1 本にしたのは、
  つまみを 3 本渡すと質問した人に設計を押し付けることになるから。値はログに出て記憶される。
  **焼き込む数字は著者待ち**（今の既定は 1.00）。
* **探索はビルドし直さずにできた。** 倍率は設定ファイルから読むので、`night=1.50` と書いて撮るだけ。
  4 枚で決まった。スライダを先に作ったことが、スライダの数字を決める道具になった。
* **設定の置き場所は新しく作らなかった。** 両ゲームの `platform.rs` の `read`/`write` が既に
  「PC ではファイル、ブラウザでは `localStorage`」を意味しているので、`Settings` はその 2 つを
  関数ポインタで受け取るだけ。JSON にしないのは sabibots が JSON パーサを持たないから。
* **egui の窓はタイトルが id。** 言語を切り替えるとタイトルも変わるので、`.id()` を固定しないと
  クリックのたびに別の窓になり、動かした位置と大きさが捨てられて画面中央に戻る。
* **ガイドに `reflex` の段落が無かった**（英日 1 つずつ足した。許可された 1 箇所の文面変更）。
  フォントは 14 文字増えて 62,780 → 66,796 バイト、`·` が 1 つ落ちた。

## 名前の整理（G7）

デモは初見の人がピンとくる名前であるべき、という著者の判断（2026-09-17）。

1. **DSL `reflex` → `on`**: `reflex(:hit) { |by, damage| … }` → `on(:hit) { |by, damage| … }`。両ゲームの prelude、`robots/*.rb`、`creatures/*.rb`、`match_prelude.rb`。
   旧名は残さない（2 つあると混乱する）。内部名（`REFLEX_SLOTS`、`run_reflex`、`start_reflexes`、HUD の `!n`/`xn`、selftest の文言、`Rubevy.ask("reflex", …)`）も
   `on`/`handler` 系に揃える（`ON_SLOTS`、`run_handler`、`start_handlers`、ask kind `"handler"`）。HUD の「reflex 回数」は「イベント処理の回数」に。
2. **説明文の語**: 日本語「頭脳」→「行動アルゴリズム」、英語 brain / mind → **behaviour**（`guide_text.rs` ×2、`docs/garden.md`、`docs/sabiruby-battle.md`、README の利用者向け記述、
   HUD/エディタのラベル）。**キー表は短縮形**（「編集した行動アルゴリズムを適用」「行動アルゴリズムをファイルに保存」。「種の全個体に」は段落側で言う）。
3. 説明文の反射の段落は「`on` のブロックは行動アルゴリズムの `run` とは別のタスクで、出来事が届いた瞬間に走る。中で `sleep` しても `run` は止まらない。
   動いている間は操作を預かる」の 3 点を英日で。**それ以外の日本語の表現は著者が後で調整する**ので、意味を変えない範囲の置換に留める。
4. `tools/subset-font.sh` でフォントを切り直す（「行動アルゴリズム」の文字が増える）。

確認: 両ゲームの headless selftest、窓の 21 判定、`--shot --guide --lang ja|en` の 4 枚差し替え、`grep -rn reflex` が worklog/plans 以外で 0 件（英文中の一般語としての "reflex" は説明の中で `on` の意味を言う 1 か所まで）。

