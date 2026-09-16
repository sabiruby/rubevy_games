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
| G2 | `Genome`（マクロ）: Rust の構造体を Ruby のクラスに。混ぜる・変異・子を産む | 未着手 |
| G3 | セーブ/ロード（serde）: 世界と `@memory` を JSON に。`Serde<CreatureSpec>` で Ruby から型付きに生成 | 未着手 |
| G4 | 窓: エディタ・VM パネル（`rubevy-arena`）を載せ、HUD に「1 判断あたりのフレーム数」と予算の消費 | 未着手 |
| G5 | ブラウザ版（`web/` の仕組みを共有、Pages で公開） | 未着手（G4 の後。**著者の希望 2026-09-17: 任意ではなく必須**） |

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
#[derive(Clone, Serialize, Deserialize, RubyClass)]
#[ruby(name = "Genome")]
pub struct Genome { speed: f32, sight: f32, appetite: f32 }

#[ruby_methods]
impl Genome {
    fn new(speed: f64, sight: f64, appetite: f64) -> Self { … }
    fn speed(&self) -> f64 { … }  fn sight(&self) -> f64 { … }  fn appetite(&self) -> f64 { … }
    fn mix(&self, other: &Genome) -> Genome { … }          // 平均
    fn mutate(&self, vm: &mut Vm, rate: f64) -> Genome { … } // VM の乱数（`Random`）を使う
    fn to_h(&self, vm: &mut Vm) -> Value { … }               // Serde<T> を返す形でもよい
}
```

* `Creature` に `Genome` を持たせ、`Sight`/速度の上限は `Genome` から決める（Rust の規則）。Ruby は `me[:Creature][:genome]` で **Hash として**読める（Reflect 経由）が、
  `Rubevy.ask("genome")` で **`Genome` の Data オブジェクト**としても受け取れる（`Answer::Data` 相当が無ければ `answer_value` で `data_new`）。両方の見え方を `docs/garden.md` に。
* 繁殖: `Hunger` が満ちた 2 体が触れると Rust が子を産み、`Genome#mix` → `#mutate` を **Ruby から**呼ぶ形にする（`reflex(:mate) { |partner| child = my_genome.mix(partner_genome).mutate(0.1); garden.spawn(species: …, genome: child.to_h) }`）。
  規則（誰が誰と産めるか）は Rust、値の計算は Ruby から Rust のメソッドを呼ぶ、という分担を見せる。
* `sabiruby-macros` は `sabiruby` の `macros` feature（release-prep の Part C で入る）経由。それまでは `sabiruby-macros = { git = … }` 直接でよい。

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

## 窓（G4）

`rubevy-arena` のエディタ（生き物のファイルを書き換えて即反映）と VM パネル（`F2`、`Vm::task_context` が sabiruby main に入ったので `#<Task n ctx=i>` の文字列読みは外す —
sabibots 側も同時に直してよい）を載せる。HUD: 生き物ごとに「1 判断のフレーム数」（`ask` の発行から答えまでを rubevy の stats から）、全体の VM 時間 / 8 ms。
`--shot` で 1 枚。

## ブラウザ版（G5、必須）

`web/build.sh` は既にゲーム名を引数に取る（`build.sh sabibots`）が、出力先が `web/dist` 1 つで、`index.html` と Pages の workflow が sabibots 前提。
これを **2 ゲームを 1 つの Pages に**する形にする: `web/dist/` 直下に入口の `index.html`（2 つのゲームへのリンクとひとこと）、`web/dist/sabibots/`、`web/dist/garden/`。
`build.sh` は `build.sh <game>` で `dist/<game>/` に書き、`build.sh all` で両方 + 入口。workflow は `all` を回す。
URL は `https://sabiruby.github.io/rubevy_games/`（入口）、`…/sabibots/`、`…/garden/`。**今の `…/rubevy_games/` で Battle が開く URL は変わる**ので、
入口ページに Battle へのリンクを一番上に置き、README・`docs/web.md`・book 側の参照（本体が直す）を更新する。
アセット（G0a の `.glb`）は `dist/garden/assets/` に同梱し `fetch` で読む。`localStorage` のセーブは G3 で `platform.rs` に隠す。
wasm のサイズ（gzip 前後）を `docs/web.md` に Battle と並べて記録。lavapipe の `--shot` とは別に、ブラウザで 1 分動かして selftest 相当（食べる・夜・餓死）が
ログに出ることを確認（`web/serve.sh` + headless Chrome があれば自動、無ければ手で）。

## 確認

`cargo build --release -p garden`、`--headless 90 GARDEN_SELFTEST=1`（上の selftest すべて ok）、`--shot`。`docs/garden.md`（構成、境界の表、Battle との比較 —
`ask` の kind 数、Rust の接着コードの行数、`Genome` のマクロ版と手書き版の行数）、`docs/README.md`、`README.md`。worklog `docs/worklog/2026-09-17-garden-G<n>.md`。
段階ごとに別ブランチ・別コミット、push は本体。

## 依存関係（先に確認して、無ければ報告して止まる）

* rubevy: `Rubevy.set_component`、`Rubevy.find`、`subscribe`/`publish`、`answer_value`、`Task.new` の継承、`Unsubscribed` — main `9104f7c` にすべてある。
  **無いかもしれないもの**: プラグイン初期化で `&mut Vm` を触る入口（`install_json` 用）、`Answer` に Data オブジェクトを載せる形、`answer_requests` の順序を決める `SystemSet`。
* sabiruby: `sabiruby-serde`（main にある）、`sabiruby-macros`（`macros` feature は release-prep の後）。
