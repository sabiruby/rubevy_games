# 箱庭の「世界の規則」を Ruby に移すための調査（2026-09-17）

調査のみ。コードは 1 行も変えていない。対象は `garden/`（Bevy 0.19 + rubevy）と、
rubevy main `bfa47ed`（「1 アプリに VM を複数」が入った版）。

読んだ場所はすべて `ファイル:行` で示す。**確認**は実際に読んだ事実、**推測**はそう書く。
数値は定数の実物を写した。

---

## 1. 世界の規則を実装している Rust のシステム

すべて `garden/src/main.rs`。登録は `main()` の 1301–1352 行にまとまっている。

### 1.1 毎フレームの本体（1 本の `.chain()`）

`main.rs:1301-1325`：

```rust
.add_systems(Update, (
    day_night, move_creatures, separate, grow_plants, sprout_plants,
    get_hungry, eat, startle, court, starve,
).chain().after(load_world).run_if(is_still))
```

- **順序は `.chain()` で固定**。この 10 本は互いに `before/after` を書かずに、この並びだけで
  決まっている。
- `RubevySet` に対する順序は**書かれていない**。つまりこの 10 本は `Deliver`/`Tick`/`Answer`
  のどこに落ちるか Bevy の executor 次第（rubevy `docs/host-api.md:95` の「どこに置くか」で
  言う「運」の状態）。ただし publish するだけなので問題にならない: publish はキューに積むだけで、
  スクリプトは次フレームの `Tick` で読む。
- `run_if(is_still)`（`main.rs:3595`）: `Restoring` が無く、かつ `Paused::on()` が false のとき
  だけ動く。`P` を押すと 10 本まるごと止まる。

| システム | 行 | 読む | 書く | 周期 | publish |
|---|---|---|---|---|---|
| `day_night` | 2238 | `Time`, `Sky`, `NightDial` | `Sky.phase/.night`, `Sun` の `Transform`/`DirectionalLight`, `GlobalAmbientLight`, `ClearColor` | 毎フレーム | `"night"` / `"day"`（**切り替わった瞬間だけ**、宛先 `None` = 全体、payload は `Answer::Num(now)`）`main.rs:2299` |
| `move_creatures` | 2383 | `Creature.genome.speed`, `Velocity` | `Velocity`（上限クリップ）, `Transform.translation.x/z`, `rotation` | 毎フレーム | — |
| `separate` | 2421 | `Collider`, `Transform` | `Transform`（押し戻し）, `Bumps` | 毎フレーム（内部 4 パス） | `"bumped"` 接触の**開始フレームだけ**、相手を `Answer::Entity` で `main.rs:2515,2517` |
| `grow_plants` | 2553 | `Time` | `Plant.size`, `Transform.scale` | 毎フレーム | — |
| `sprout_plants` | 2562 | `Plant` の数と位置, `Dice` | 新しい `Plant` を spawn | 毎フレーム（確率で発火） | — |
| `get_hungry` | 2583 | `Creature.genome.appetite` | `Creature.age`, `Hunger` | 毎フレーム | — |
| `eat` | 2602 | `Transform`, `Hunger`, `Plant` | `Plant.size`, `Hunger`, `Eating`, `Eaters`, plant の `despawn` | 毎フレーム | `"ate"` 食事の**開始フレームだけ**、payload は**皿（plant）の大きさ** `Answer::Num` `main.rs:2640` |
| `startle` | 2660 | `Creature.species`, `Transform`, `Velocity` | `Contacts` | 毎フレーム | `"touched"` ウサギ→カブトムシ、接触の開始だけ、`Answer::Entity(rabbit)` `main.rs:2683` |
| `court` | 2749 | `Hunger`, `Transform`, `Breeding`, `Sky`(世界時計), `Births` | `Breeding.ready_at`, `.partner` | 毎フレーム | `"mate"` **2 匹のうち片方だけ**（entity id の小さい方）、相手を `Answer::Entity` `main.rs:2792` |
| `starve` | 2910 | `Hunger` | `despawn` | 毎フレーム | — |

### 1.2 チェーンの外

| システム | 行 | 登録 | 備考 |
|---|---|---|---|
| `answer_garden` | 2966 | `main.rs:1350` `.in_set(RubevySet::Answer)` | 唯一の「質問に答える」システム。`is_still` **なし**＝ポーズ中も答える |
| `hatch` | 2820 | `main.rs:1330` `.after(RubevySet::Answer).run_if(is_still)` | `answer_garden` が積んだ `Births` を実際に spawn し、親に代償を課す |
| `watch_minds` | 3143 | `main.rs:1351` `.after(RubevySet::Answer)` | 命令数と行番号（HUD 用）。規則ではない |
| `install_host_api` | 2945 | `main.rs:1292` `Startup` | `Genome::register` + `install_json` |
| `hold_the_clock` | 3616 | `main.rs:1168` `.run_if(is_paused).before(day_night)` | ポーズ中だけ `sky.shift -= delta`。**窓のあるビルドだけ** |
| `load_world` | 3454 | `main.rs:1327` `.before(RubevySet::Deliver)` | |
| `restore_memory` → `save_world` → `finish_restore` | 3538/3320/3577 | `main.rs:1336-1345` `.chain().after(RubevySet::Answer)` | |

### 1.3 餓死体の消滅

`starve`（`main.rs:2910-2929`）は `hunger.0 > 0.0` でないものを `commands.entity(entity).despawn()`
するだけ。スクリプトの後始末は書いていない: エンティティが消えると `ScriptTask` も消え、rubevy の
`on_remove` フックがタスクを terminate し、購読キューを閉じるので、ハンドラタスクは
`Rubevy::Unsubscribed` で巻き戻る（rubevy `docs/host-api.md:363` 「Events」、`garden.md:277` の節も
同じことを書いている）。

---

## 2. それぞれの規則が持つ数値

全部 `main.rs` の `const`。**`settings` に出ているのは `night` ダイヤルと guide の `lang` の 2 つだけ**で、
規則の数値は 1 つも設定に出ていない（`main.rs:1103-1113` が `rubevy_arena::Settings` から読むのは
`lang` と `night` のみ）。

### 昼夜

| 名前 | 値 | 行 |
|---|---|---|
| `DAY_LENGTH` | 60.0 秒（太陽 1 周） | 68 |
| `DAWN_OFFSET` | 0.08（開始位相。最初に来るのは昼） | 71 |
| `MIDNIGHT` | `(0.75 - DAWN_OFFSET) * DAY_LENGTH` = 40.2 秒 | 77 |
| `MOON_LUX` | 950.0 | 105 |
| `NIGHT_AMBIENT` | 190.0 | 106 |
| `NIGHT_SKY` | `[0.14, 0.18, 0.36]` | 109 |
| `NightDial` | 既定 1.0、範囲 0.5–2.0。**唯一 settings にある規則側の数値**（ただし見た目だけ） | 124-136 |

夜の判定は `sun_up(phase).y <= 0.0`（`main.rs:2256-2258`）。昼の照度は
`1200 + 9000 * noon`、夜は `MOON_LUX * dial`（2262-2270）。

### 草

| 名前 | 値 | 行 |
|---|---|---|
| `PLANTS_AT_START` | 55 | 219 |
| `PLANTS_MAX` | 90 | 220 |
| `SPROUT_RATE` | 0.7（毎秒。判定は `dice.roll() > SPROUT_RATE * delta` `main.rs:2570`） | 222 |
| `PLANT_MIN` | 0.18（芽の大きさ） | 223 |
| `PLANT_MAX` | 1.4（成長上限） | 224 |
| `PLANT_GROWTH` | 0.06 /秒（`size = (size + 0.06*dt).min(1.4)`、`Transform.scale` も同じ数 `main.rs:2556-2559`） | 225 |

新芽は他の草から 1.5 以内なら置かない（`main.rs:2573-2576`）。

### 空腹・食事

| 名前 | 値 | 行 |
|---|---|---|
| `HUNGER_MAX` | 100.0（満腹。0 が死） | 230 |
| `HUNGER_RATE` | 1.6 /秒 × `genome.appetite` | 231 |
| `EAT_RATE` | 1.0（毎秒かじる量） | 234 |
| `FOOD_VALUE` | 60.0（草 1 単位が満腹度でいくらか） | 235 |
| `REACH` | 1.1（食事の距離。実際は `REACH + plant.size * 0.5` `main.rs:2626`） | 237 |
| `TOUCH_REACH` | 1.3（ウサギ→カブトムシの `"touched"`） | 238 |

食事の条件（`main.rs:2612-2650`）: `hunger < HUNGER_MAX` で、距離が `REACH + size*0.5` 以内、
`plant.size > 0.02`。1 フレームに 1 匹 1 株（`break`）。`size <= 0.02` で `despawn`。
食べたモデルの咀嚼は `Eating { until: now + 0.35 }`。

### つがい・出産

| 名前 | 値 | 行 |
|---|---|---|
| `MATE_HUNGER` | 75.0（両方これ以上） | 262 |
| `MATE_REACH` | 2.0（この距離以内。「触れたら」ではない — G2 の計測で最接近 1.24 だったため） | 293 |
| `MATE_COST` | 30.0（**子が生まれたとき**に親双方から引く。下限 1.0） | 266 |
| `MATE_COOLDOWN` | 20.0 秒（同じく出産時に課す） | 271 |
| `COURT_RETRY` | 2.0 秒（同じ相手に `"mate"` を再送するまで） | 276 |
| `CHILD_HUNGER` | 50.0（新生児の満腹度） | 278 |
| `POP_MAX` | 24（`court` の手前と `garden.spawn` の答えの両方で確認） | 281 |
| `NEWBORN_GRACE` | 2.0 秒（selftest だけが使う） | 247 |

突然変異率 **0.1 は Rust の定数ではない**。`beetle.rb:81` の `my_genome.mix(mate).mutate(0.1)` が
唯一の出どころで、Rust 側は `judge_child`（`main.rs:2880-2883`）に検査用の
`const RATE: f32 = 0.1;` を持っているだけ。`Genome::mutate` は各遺伝子を `1 ± rate` で掛ける
（`genome.rs:130-140`、乱数は **VM の `Random`**）。

遺伝子の種（`genome.rs:62-87`）: カブトムシ `speed 2.2 / sight 8.0 / appetite 1.0`、
ウサギ `3.4 / 12.0 / 1.0`。世界生成時は `SPREAD = 0.18`（`genome.rs:56`）で ±18% 揺らす。

### 餓死

閾値は `hunger.0 > 0.0` のみ（`main.rs:2915`）。定数は無い。

### ぶつかり

| 名前 | 値 | 行 |
|---|---|---|
| `BEETLE_RADIUS` | 0.40 | 297 |
| `RABBIT_RADIUS` | 0.50 | 298 |
| `TREE_RADIUS` | 0.70 | 299 |
| `ROCK_RADIUS` | 0.60 | 300 |
| `TREES` / `ROCKS` | 7 / 9 | 301/302 |
| `CELL` | 1.6（近傍グリッドの一辺） | 305 |
| `SEPARATE_PASSES` | 4 | 308 |

押し分けは「生き物同士は半分ずつ、木と岩は生き物が全部引き受ける」（`main.rs:2478-2488`）。
壁は `±(HALF_W - 0.5)`, `±(HALF_D - 0.5)` で、`move_creatures` と `separate` の両方が clamp する。

---

## 3. 生き物のスクリプトから見た世界

### 購読しているイベントと payload

| イベント | 宛先 | payload | カブトムシ | ウサギ |
|---|---|---|---|---|
| `"night"` | 全体 (`None`) | `Num(now)` = 世界時刻（秒） | `beetle.rb:15` | `rabbit.rb:16` |
| `"day"` | 全体 | `Num(now)` | `beetle.rb:20` | `rabbit.rb:21` |
| `"touched"` | そのカブトムシ | `Entity`（ウサギ） | `beetle.rb:27` | 購読なし |
| `"bumped"` | ぶつかった当人 | `Entity`（相手。木・岩もある） | `beetle.rb:39` | `rabbit.rb:27` |
| `"ate"` | 食べた当人 | `Num`（**皿の残りの大きさ**。一口の量ではない） | `beetle.rb:55` | `rabbit.rb:35` |
| `"mate"` | 2 匹のうち id の小さい方 | `Entity`（相手） | `beetle.rb:77` | **購読なし**（ウサギは繁殖しない） |

ハンドラの仕組み（`prelude.rb:330-369, 438-463`）: `on(:name)` は `define_method("__handler_N")`
＋ `handlers << [event, slot]`。上限は `ON_SLOTS = 6`（`prelude.rb:342`）。
`run_creature` が `start_handlers` でハンドラ 1 つにつき 1 タスクを立て、
`Rubevy.subscribe(event)` のキューを `pop` で待つ。優先度は本体 `-20`（本体は
`Script::new(..).with_priority(100)`、`main.rs:2000` → ハンドラは 80）。

### `Rubevy.ask` の質問と答え手

答え手は全部 `answer_garden`（`main.rs:2966`、`RubevySet::Answer`）。

| Ruby | `kind` | 引数 | 答え | 行 |
|---|---|---|---|---|
| `garden.nearest(:Plant)` | `"garden.nearest"` | `Arg::Text` | `Answer::Entity` or `Nil`。**探す範囲は自分の `Sight`**、自分自身は返さない | 2994-3021 |
| `garden.count(:Plant)` | `"garden.count"` | `Arg::Text` | `Answer::Num` | 3022-3029 |
| `Rubevy.ask("genome")` | `"genome"` | なし | `answer_value` で `Genome` の Data オブジェクト（コピーでなく Rust の値そのもの） | 3030-3043 |
| `garden.spawn(species:, genome:, at:)` | `"garden.spawn"` | `Arg::Value`（Hash 本体） | `Answer::Bool(true)` か、`Answer::Text` で断り文句 | 3044-3086 |

`garden` は `Rubevy::Proxy.new("garden")`（`prelude.rb:36-58, 84-86`）。
`method_missing` が `Rubevy.ask("garden.#{name}", *args).pop` に化ける。
知らない `kind` は `warn!` して `Answer::Nil`（`main.rs:3087-3090`）。

コンポーネント名の解決は `AppTypeRegistry` → `ReflectComponent` なので、
**コンポーネントごとの Rust コードは 1 行も無い**（`main.rs:2982-2988`）。

加えて `Rubevy.find(:Tree)`（`rabbit.rb:47`）— rubevy が自分で答える `entities.with`
（`rubevy/src/prelude.rb:57-60`）。

### 読み書きしているコンポーネント

`register_type` されているのが Ruby から見える全部（`main.rs:1196-1213`）:
`Transform`, `Plant`, `Creature`, `Species`, `Genome`, `Hunger`, `Velocity`, `Sight`,
`Memory`, `Collider`, `Tree`, `Rock`。

- 読み: `me[:Hunger][0]`（`prelude.rb:137`）, `me[:Sight][0]`（141）,
  `me[:Transform][:translation]`（145）, `me[:Creature]` → `[:species]`, `[:genome]`（149-155, 182-186）,
  `thing[:Transform][:translation]`（230-235）, `other[:Creature]`（`rabbit.rb:78`）
- 書き: **`me[:Velocity] = [[vx, vz]]` ただ 1 つ**（`prelude.rb:118`）。
  Ruby が世界に書き込んでいるのはこれだけ。

Ruby から見えない（`Reflect` を持たない）ものは `Sun`, `Mind`, `Eating`, `Animated`,
`Breeding`, `Fasting`, `Probe`, `Horizon`, `SkyShell`, `Tint`（`main.rs:427-563`）。
**`Breeding` が Ruby から見えないのは、つがいの規則を Ruby に移すときの最初の障害**（→ 8 と最後の節）。

`garden.md` の該当節: 「The rules」`docs/garden.md:277`、「The questions」`docs/garden.md:424`。
（「the four questions」は `nearest` / `count` / `genome` / `spawn` の 4 つ。）

---

## 4. スクリプトの置き場と読み込み

### 置き場

```
garden/ruby/prelude.rb              464 行、DSL 本体
garden/ruby/creatures/beetle.rb     128 行
garden/ruby/creatures/rabbit.rb      89 行
```

`Species::file()`（`main.rs:345`）が `"beetle.rb"` / `"rabbit.rb"` を返し、
`Brains::path`（`main.rs:715`）が `ruby/creatures/<file>` を組み立てる。
`RubyDir` は `platform::ruby_dir()`（PC は `CARGO_MANIFEST_DIR/ruby`、`platform.rs:49-52`）。

### `MrbAsset` になるまで

`compile_source`（`main.rs:2023-2046`）が全部:

```rust
let src = format!("{prelude}\n# ---- {name} ----\n{body}\nrun_creature\n");
let prelude_lines = prelude.lines().count() as u32 + 2;
platform::compile(&src, name) -> mrb.add(MrbAsset { bytes })
```

- prelude と種のファイルを**1 つのプログラムとしてコンパイル**する。だから `require` が要らない。
- 末尾に `run_creature` が足される。つまり**ファイルは `creature "..." do ... end` を定義するだけで、
  実行の入口は prelude 側**（`prelude.rb:394-430`）。
- `prelude_lines` は VM パネルが「自分のファイルの何行目」を言うためのオフセット。

`give_mind`（`main.rs:1964-2007`）が `Script::new(handle).with_name(..).with_priority(100)` と
`Mind` をエンティティに挿す。`Brains::text(species)` が `Some` ならそのテキストを、
`None` ならファイルを読む。

### エディタの Ctrl+Enter

`window.rs:16` のとおり **F5 はセーブに取られているので、Apply は `Ctrl+Enter`**。

`do_editor_actions`（`window.rs:234-313`）:

1. `compile_source(&ruby.0, species.file(), &text, &mut mrb)` — 失敗したら何もしない
2. `brains.set(species, Some(text))` — **ディスクには書かない**
3. `restart_species(...)`（`window.rs:316-350`）が**その種の全個体を同じフレームで**
   `remove::<ScriptTask>()` + `remove::<ScriptDone>()` + `insert(Script::new(handle))`

重要な点（`window.rs:296-315` のコメント）:
- **編集の単位は「種」**。ロボットと違い、生き物はファイルを持たない — 種が持つ。
  以後生まれる子も `give_mind` 経由で適用済みテキストを走る。
- **`@memory` は戻ってこない**。`ScriptTask` を落とすと VM のタスクが終わり、
  新しいスクリプトは新しいオブジェクトを作るから。
- sabiruby 0.5.1 以前は一斉差し替えで VM のスケジューラが止まったので順番待ちの列があった。
  0.5.1 で列は消えた。

Save（`window.rs:262-278`）はファイルに書いて `brains.set(species, None)`、Revert（280-294）は
ファイルから読み直す。外部エディタでの保存は `reload_changed`（`window.rs:352-390`、`Watch`）が
拾って同じ `restart_species` を呼ぶ — ただし**エディタで適用中の種は飛ばす**。

### ブラウザ（wasm）との違い

`platform.rs:115-233` と `build.rs`:

- `build.rs` が `ruby/` 以下の `.rb` を全部 `include_str!` の表 `RUBY_FILES` にしてバイナリに埋める。
  **ディレクトリは無い**。`ruby_dir()` は `PathBuf::from("ruby")` という単なる接頭辞（`platform.rs:154`）。
- `read` は **まず `localStorage` を見て**、無ければ埋め込み表（`platform.rs:172-182`）。
  キーは `garden:` + パス。`write` は `localStorage` だけ（184-187）。
  → **ブラウザで Save したスクリプトはそのブラウザに残り、次回起動時にはファイルより優先される**。
- コンパイラは Rust に無い。ページが定義する `window.gardenCompile(source)` を呼ぶ
  （`platform.rs:145-151`）。エラーメッセージにファイル名が乗らないのが既知の欠け
  （`docs/web.md`、`garden.md:1647`）。
- `Watch`（ファイル監視）は窓ビルドの PC 側だけ（`main.rs:1194-1201`）。

**推測**: `world.rb` を足すなら `ruby/world.rb` に置けば `build.rs` は自動で拾う
（`collect` は再帰で `.rb` を全部集める）。`Species::ALL` を前提にしたエディタの
`choices`（`window.rs:187-205`）は種の列挙なので、世界を選択肢に混ぜるには
`EditorChoice` の id 空間を種の index から広げる必要がある。

---

## 5. selftest の判定

`GARDEN_SELFTEST=1`（PC）または `?selftest`（ページ）で `SelfTest`（`main.rs:922-1014`）が入り、
`watch_overlap`（2527）`watch_probe`（3784）`watch_turning`（3817）`watch_sleep`（3864）が足される
（`main.rs:1374-1378`）。判定文は `stop_when_over`（`main.rs:4009-4118`）が印字する。

**判定は 9 個ではなく 10 個**（`docs/garden.md:1472` も「the ten checks」と書いている）。
10 番目は G5 の版番号チェックで、`--load` を自分で与えられた run では走らない。
`--headless 90` に `--load` が無ければ 10 個すべて走る。

| # | 何を見るか | どこで記録されるか | 規則を Ruby に移したとき |
|---|---|---|---|
| 1 | 誰かが 10 秒以内に食べた | `eat` が `test.ate_at`（`main.rs:2645`） | 食事の規則が Ruby になると、Ruby が「食べた」と宣言しないと立たない |
| 2 | 60 秒以内に夜が来た | `day_night` が `test.night_at`（2302-2304） | 同上（昼夜が Ruby になると計測点も動く） |
| 3 | 餓死した個体のエンティティが消えている | `starve` が `test.starved`（2924）＋ 終了時に `everything.contains(entity)` | Ruby から `Rubevy.despawn` しても成立する（**確認**: `despawn` は任意のエンティティを取る） |
| 4 | 何も何かを突き抜けていない | `watch_overlap`（2527、`after(separate)`）。半径の和の 0.9 未満のフレーム数が 0 | `separate` は移さない前提なら無傷 |
| 5 | 空腹の個体が視界内の草に着いた | `watch_probe`（3784）。`Probe { dinner }` を持つ個体と皿の距離 | Ruby 側の `hungry_below 55` と `Sight 8` に依存。世界側の規則を移しても変わらない |
| 6 | ウサギに触られたカブトムシが 0.5 秒で向きを変えた | `startle` が `test.touched` に積み（2680-2700）、`watch_turning`（3817）が 0.5 秒後に `dot < 0.7` を見る。除外 4 種: 静止・壁際（`by_a_wall` 3855）・`age < NEWBORN_GRACE`・直近 `TOUCH_SETTLE=1.5` 秒に既に通知済み | **`"touched"` の発行時刻に速度を記録する**のがこの判定の核。Ruby が publish するなら、その瞬間の `Velocity` を Rust 側で拾う仕掛けが要る |
| 7 | 夜の 1 秒後に全員が寝ている | `watch_sleep`（3864）。`Mind` を持つ個体の最大速度 < 0.05 | `"night"` の発行時刻が基準。同上 |
| 8 | 子の遺伝子が両親の混合＋変異 | `court` が `test.matings` に両親の `Genome` を記録（2795-2800）、`hatch` が `judge_child`（2880）に掛ける。各遺伝子が平均から `0.1 * |平均| + 1e-3` 以内、かつ最低 1 つは両親どちらとも違う | 「`"mate"` の時点の両親の遺伝子」を誰かが覚えている必要がある |
| 9 | 遺伝子が欠けた spawn Hash がその遺伝子名を言う | `answer_garden` が `test.bad_spawn`（3110-3116）。`TESTER`（`main.rs:1794-1803`、4 行の Ruby）が投げる | `garden.spawn` を Rust に残す限り無傷 |
| 10 | 版の違うセーブが拒否される | `main.rs:1257` で `write_a_save_from_another_version()` を書き、同じ `--load` の道に通す。文言が `"version 99"` を含み `"reads 1"` で終わること | セーブ形式を変えると影響（→ 6） |

判定 5・8 のために `spawn_world`（`main.rs:1602`）は selftest のとき 3 つの隅を空けて仕込む:
断食カブトムシ（スクリプト無し、`(-17, -12)` 付近）、プローブ（空腹 40、5 単位先に草 1 株、
`(17, 12)` 付近）、つがい用の草 4 株＋遺伝子の違うカブトムシ 2 匹（`(-14, 9)` 付近、9 単位離して配置）。

---

## 6. セーブ（JSON）に入っている世界の状態

`GardenSave`（`main.rs:3234-3252`）が**そのまま形式**。`SAVE_VERSION = 1`（3226）。

```
version: u32
tick: f32        // 世界時計の秒（Sky::shift 込み）
day_phase: f32   // 0 日の出 / 0.25 正午 / 0.5 日没
night: bool
plants: [{ at: [f32;2], size: f32 }]
trees: [[f32;2]]
rocks: [[f32;2]]
creatures: [{ species, at, hunger, age, genome, memory: serde_json::Value }]
```

**入っていないもの**（`main.rs:3229-3233` のコメントが明言）:
`Breeding` のクールダウン、`Contacts`、`Bumps`、草が茂みか房か、岩の潰れ具合。
「run についての帳簿であって world についてではない」から。

→ **規則の数値を Ruby に移しても、この形式は何も変えなくてよい**（数値はどこにも書かれていない）。
変わるのは 2 点だけ、と**推測**する:

1. `world.rb` 自身が状態を持つなら（例: 次の芽までのタイマー）、それは Ruby の `@ivar` になるので
   セーブに入らない。生き物の `@memory` と同じ扱いにしたいなら、`read_memory`
   （`main.rs:3299`、`@being` → `@memory` の 2 回の `ivar_get`）と同じ道を世界タスクにも引く必要がある。
2. `tick` / `day_phase` / `night` は今 `Sky` リソース由来（`save_world:3341-3344`）。
   昼夜が Ruby のものになると、真実がどちらにあるかを決めないといけない。

読み戻し（`load_world`、3454）は `sky.shift = save.tick - time.elapsed_secs()` で世界時計を合わせ、
`Restoring` を置く。`restore_memory`（3538）が各タスクの `@being.@memory` に書き戻し、
それが終わるまで `is_still` が false で世界は止まっている。`RESTORE_PATIENCE` は 5.0 秒（`main.rs:849`）。

---

## 7. 性能の目安

### 1 フレームあたりのループ回数

上限は生き物 `POP_MAX = 24`、草 `PLANTS_MAX = 90`、木 7、岩 9。

| システム | 1 フレームの仕事 |
|---|---|
| `day_night` | 太陽 1 個。位相計算のみ |
| `move_creatures` | 生き物 N（≤24） |
| `separate` | **4 パス** × グリッド近傍。要素は 24 + 16 = 40。グリッド（`CELL=1.6`）で近傍だけ見る。最後に `bumps` の差分比較 |
| `grow_plants` | 草 M（≤90） |
| `sprout_plants` | 草の数を数える（M）。サイコロが通ったときだけさらに M 回の距離判定 |
| `get_hungry` | N |
| `eat` | **N × M（最悪 24 × 90 = 2160 の距離判定）**。当たったら `break` するので平均はもっと少ない。ここが一番重い |
| `startle` | ウサギを 1 度集めてから カブトムシ × ウサギ |
| `court` | `ready` を作るのに N、そのあと `ready²/2`。人口が `POP_MAX` なら即 return |
| `starve` | N |
| `answer_garden` | 質問 1 件ごとに `world.iter_entities()` を**全走査**（`main.rs:3006`）。窓ビルドではモデルの子エンティティも全部数に入る |
| `watch_minds` | `Mind` を持つ数 |

`answer_garden` の `garden.nearest` が全エンティティ走査なのは、Ruby 側の呼び出し頻度と直結する。
今は生き物 1 匹あたり `run` のループが `sleep 0.2`（カブトムシ）/ `sleep 0.25`（ウサギ）なので
毎秒 4〜5 回。

### `Time` と `FrameCount`

- 世界時計は `world_now(&time, &sky) = time.elapsed_secs() + sky.shift`（`main.rs:3289`）。
  **`court` と `hatch` はこれを読む**（2761, 2835）。
  一方 `eat`・`startle`・`starve`・selftest は `time.elapsed_secs()` を素で読む
  （2612, 2666, 2914）。つまり「セーブから読んだ世界の時刻」と「プロセスの時刻」が
  混ざっている。ポーズ・ロードの影響を受けるのは前者だけ。
- `FrameCount` は `answer_garden`（3099）と `watch_minds` が「1 決定あたり何フレーム」を
  出すのに使う。`Mind::asked_frame` にそのフレームを刻み、次のバーストまでの差を数える
  （`main.rs:3121-3141` の説明）。`SHORTEST_SLEEP = 0.05`（3119）より短い間隔はコンポーネント読み、
  それより長ければ `sleep`。

### `P`（`is_still`）との関係

- `is_still`（3595）= `Restoring` が無い **かつ** `Paused::on()` でない。
- これが付いているのは §1.1 の 10 本と `hatch` だけ。
  `answer_garden`・`watch_minds`・`save_world`・`draw_hud` は止まらない。
- VM 側は別の道: `P` は `world.budget = 0`（`window.rs:431`）。
  rubevy は budget 0 の間 mruby-task の時計も止めるので、
  `sleep 0.1` の残りはポーズを跨いでも残る（host-api.md「Pausing」）。
- `hold_the_clock`（3616）が `sky.shift -= delta` して世界時計を据え置く。
  **これがあるから、ポーズ中に `"night"` を取り落とすことがない** — 位相が動かないので境界を跨げない。

---

## 8. rubevy 側で今すぐ使える手段

`docs/host-api.md` を読んだ結果（行は host-api.md のもの）。

### 使えるもの

| 手段 | 状態 | 出どころ |
|---|---|---|
| `ScriptWorld::publish(Option<Entity>, name, Answer)` | **Rust 側だけ**。Ruby から publish する方法は無い | `host-api.md:363-400`、`rubevy/src/lib.rs:975` |
| `publish_value(entity, name, |vm| ...)` | 平たくない payload 用 | `lib.rs:982` |
| `Answer` の種類 | `Nil` / `Bool` / `Num(f64)` / `Text` / `List(Vec<f64>)` / `Rows` / `Entity` ＋ `answer_value` で任意の Ruby オブジェクト | `host-api.md:64, 202` |
| `Arg`（質問の引数） | `Num` / `Text` / `Entity` はコピー。それ以外（Hash, Array, nil, 自作オブジェクト）は `Arg::Value` = **Ruby の値そのもの**。`request.value(0)` → `vm.hash_entries` / `sabiruby_serde::from_value` | `host-api.md:137-200` |
| コンポーネント読み `e[:X]` | 1 フレームの往復。`RubevySet::Answer` で rubevy が自分で答える | `host-api.md:290-360` |
| コンポーネント書き `e[:X] = h` | **任意のエンティティに対して書ける**（`set_component` は entity bits を取る、`lib.rs:1957`）。名前のあるフィールドだけ書く。`apply_component_writes`（`lib.rs:1778`、`RubevySet::Tick` の末尾）で反映＝**次フレームから見える** | 同上 |
| `Rubevy.spawn "name", x, y, z` | エンティティは作れるが、付くのは `SpawnedByScript { name }` と `Transform` だけ（`lib.rs:1620`）。**生き物は作れない** | `host-api.md:19` |
| `Rubevy.despawn entity` | 任意のエンティティ | `lib.rs:1624` |
| `Rubevy.find(:Component)` | **コンポーネント 1 つ**でのエンティティ列挙。全世界を走査するので「たまに」使うもの | `rubevy/src/prelude.rb:57-60` |
| `each(:Enemy, :Transform)`（複数条件） | **無い**（確認）。`find` は名前 1 つしか取らない | 同上 |
| `$rubevy[:frame] / [:delta] / [:time]` | 毎フレーム更新される。**スクリプトが `ask` なしでフレームと delta を読める** | `host-api.md:479-486` |
| `ScriptWorld::vm` を `Startup` でいじる | 箱庭は既に `Genome::register` と `install_json` でやっている | `main.rs:2945` |

### 2 本目の VM（`ScriptWorld<Mods>`）

`host-api.md:592-724`。今日入った。要点だけ:

- `RubevyPlugin::<Mods>::for_vm(root)` / `Script::<Mods>::for_vm(h)` / `ScriptWorld<Mods>` /
  `ScriptTask<Mods>` / `RubevySet::<Mods>::answer()`。タグは空 struct、実行時コスト無し。
- **共有しないもの**: ヒープ、グローバル、定数、クラス、シンボル、GC、スケジューラ、
  **購読リスト**、フレーム予算、`require` のロードパス。
- **`publish` はその VM の購読者にしか届かない**。両方に届けたいホストは 2 回 publish する。
  全 VM 宛の綴りは意図的に無い。
- **予算は VM ごと**。既定は 200,000 命令 / 8 ms なので、2 VM で最悪 16 ms。
  2 本目は下げるべき（`mods.budget = 20_000; mods.frame_time = Some(1ms)`）。
- 同じ `.mrb` を 2 つの VM に読ませると irep は 2 つ。VM 1 つあたり約 0.5 MB、起動 0.6 ms。
- **同じエンティティが両方の VM のスクリプトを持てる**（`Script` と `Script<Mods>` は別コンポーネント）。
- サンドボックスではない（ヒープ上限なし、`require "./..."` はロードパスを迂回する）。

**推測**: 世界を 2 本目の VM に置く設計は「世界と生き物を言語レベルで分ける」という意味では効くが、
**`publish` が VM を跨がない**ので、世界（`Mods` VM）が生き物（既定 VM）に `"night"` を届けるには
必ず Rust の中継が要る。世界を既定 VM の 1 タスクとして走らせる方が中継は要らない
（ただし同じ予算を生き物 24 匹と分け合う）。

---

## Ruby に移すときに引っかかりそうな点（5 つ）

1. **Ruby から `publish` できない。** 世界の規則を Ruby に置いても、`"night"` や `"ate"` を
   生き物に届けるには新しい質問（例 `garden.publish(who, "night", payload)`）を作って、
   `answer_garden` の側で `ScriptWorld::publish` を呼ぶしかない。しかも `publish` は
   **その VM の購読者にしか届かない**ので、世界を 2 本目の VM に置くとこの中継が必須になる。

2. **`Breeding` と `Contacts` / `Bumps` / `Eaters` が Ruby から見えない。**
   つがいのクールダウン（`Breeding`、`main.rs:524-533`）は `Reflect` を持たず
   `register_type` もされていないので、`e[:Breeding]` は nil を返す。
   「接触の開始フレームだけ publish する」という 3 つの規則（`bumped` / `ate` / `touched`）の
   前フレームとの差分も、今は Rust のリソース（`Contacts` 604, `Bumps` 610, `Eaters` 723）にある。
   Ruby に移すなら、これらを登録済みコンポーネントにするか、Ruby 側の `@ivar` に持ち替えるか、
   どちらかを決める必要がある。

3. **1 回の読みが 1 フレーム、書きは次フレーム反映。** 生き物 24 匹 × 草 90 株の距離判定を
   Ruby で毎フレームやる道は無い（`eat` は最悪 2160 回の距離判定、`grow_plants` は 90 個の書き込み）。
   `Rubevy.find` も全世界走査で「たまに」用。**推測**: 世界のスクリプトは
   「毎フレームの数値更新」ではなく「周期的な判断」（芽を出す、夜を宣言する、つがいを告げる）に
   限り、位置と距離の総当たりは Rust に残すのが現実的。あるいは `answer_garden` に
   「近い組を全部返す」質問（`Answer::Rows`）を足して、判断だけ Ruby にする。

4. **selftest の 6 番と 7 番は「publish した瞬間の状態」を Rust が記録している。**
   `startle` は `"touched"` を出すその場で `velocity.0` を `test.touched` に積み（`main.rs:2700`）、
   `watch_sleep` は `test.night_at` を基準にする。publish の発火点が Ruby に移ると、
   この記録点も一緒に移すか、Rust 側で「Ruby が publish を頼んできたフレーム」を掴む必要がある。
   `TOUCH_SETTLE`（1.5 秒）の再送クロックも `startle` の中にあり、
   これは G4 で 41/211 の取りこぼしの原因になった場所なので、移し替えのときに壊しやすい。

5. **エディタと `restart_species` は「種」を単位に作られている。**
   `Brains` は `[Option<String>; 2]`（`main.rs:701`）、`EditorChoice.id` は `species.index()`
   （`window.rs:190`）、`do_editor_actions` は `watched.entity` の `Mind.species` から種を決める
   （`window.rs:245-248`）。**世界には `Mind` を持つエンティティが無い**ので、
   世界を編集対象に出すには id 空間と「今どれを見ているか」の決め方を広げる必要がある。
   あわせて、`restart_species` と同じく世界を差し替えると `@memory` 相当は失われる
   （`window.rs:303-307`）。世界に状態を持たせるなら、そこも設計に入れること。
