# 箱庭の世界を Ruby にする（`world.rb`）— 実装指示書（第 2 版）

作成 2026-09-17（第 1 版は同日、同期読みが入る前の妥協案。保留のうえ書き直した）。著者の問い「シミュレーション環境も Ruby で動的にしないと面白くないと思わないか」への答え。
前提: rubevy の**同期読み**（`rubevy/docs/plans/sync-access-plan.md` S1〜S3 済み: コンポーネントの読みは同じ tick の中で返る、書きはフレーム末尾、規則の chain は `.before(RubevySet::Tick)`）と **S4 = `answer_in_tick`**（ゲームが tick の中で答える口。実装中）。
調査は `docs/worklog/2026-09-17-garden-world-survey.md`（行番号つき。**着手前に全部読む**。ただし §7 の「読みが 1 回 1 フレーム」は S1 で変わった）と `docs/worklog/2026-09-17-sync-reads.md`（S3。順序と計測と selftest 6 番の経緯）。

**この文書だけで着手できるように書いてある。** 読む順は 1 → 2 → 3 → 4。5 は着手前に必ず目を通す。

---

## 0. はじめの一歩

```bash
cd /home/kishima/book/kishima/rubevy_games-wt-world      # worktree（ブランチ garden-world、main から）。無ければ作る
cargo update -p rubevy                                    # S4 が main に入ってから。sabiruby の rev はそのまま（--precise で戻す）
cargo build --release -p garden && GARDEN_SELFTEST=1 ./target/release/garden --headless 90   # 着手前に 10 判定（4 番・5 番は揺れる。§5）
```

作法は `/home/kishima/book/CLAUDE.md` と `/home/kishima/book/.claude/agents/implementer.md`。段階ごとに 1 コミット、push しない、過程は `docs/worklog/2026-09-17-garden-world.md` に書きながら進める（捨てた案と理由も）。**根拠のない数を置かない。unsafe を書かない。**

---

## 1. 何を作るのか（30 秒版）

今の箱庭は**生き物だけが Ruby** で、世界の規則（昼の長さ、草の生え方、腹の減り方、食べる、つがい、餓死）は `garden/src/main.rs:62-308` の `const` と Rust のシステムに固定されている。
遊んで面白いのは「規則を変えたら生き物がどう適応するか」なので、規則が Ruby でなければ半分しか遊べない。

`garden/ruby/world.rb` を置く。生き物と同じくエディタで書き換え、Ctrl+Enter で**箱庭を止めずに**差し替わる。**規則の論理そのものが Ruby** で、毎フレーム走る:

```ruby
world do
  day_length 60

  each_frame do |dt|                                   # 1 フレームに 1 回。dt は秒
    plants.each do |p|                                 # 草は育つ（上限あり）
      size = p[:Plant][:size]
      p[:Plant] = { size: [size + 0.06 * dt, 1.4].min } if size < 1.4
    end
    creatures.each do |c|                              # 腹は減り、餓死する
      h = c[:Hunger][0] - 1.6 * dt * c[:Creature][:genome][:appetite]
      next Rubevy.despawn(c) if h <= 0
      c[:Hunger] = [h]
      near = garden.within(c, 1.1, :Plant)             # 手の届く草（Rust が同じ tick で答える）
      if (p = near.first) && h < 100
        bite = [1.0 * dt, p[:Plant][:size]].min        # 一口ぶん
        p[:Plant] = { size: p[:Plant][:size] - bite }
        c[:Hunger] = [h + 60.0 * bite]
        tell c, "ate", p[:Plant][:size] if starting_to_eat?(c)
      end
    end
    sprout if rand < 0.7 * dt && plants.size < 90       # 芽
  end

  every 60 do |n|                                      # 季節: 乾季と雨季
    tell :all, "season", (n.odd? ? "dry" : "wet")
  end
end
```

**境界**（この計画の要。第 1 版から動いたのは太字）:

| Ruby（`world.rb`） | Rust（そのまま） |
|---|---|
| **規則の論理と数値**: 草の成長と芽、空腹、食べる、つがいの条件と子、餓死、昼の長さ | 力学: `move_creatures`（`Velocity` → `Transform`、壁）、`separate`（押し合い）、`day_night` の**描画**（太陽・空・光。位相は Ruby の `day_length` から） |
| 周期的な出来事（`every`）と宣言（`tell`） | 配達（`ScriptWorld::publish`）と**接触の開始フレームの検出**（`Contacts` / `Bumps`: `"touched"` / `"bumped"` は Rust のまま） |
| 世界の状態（季節など）は `world.rb` の変数 | **空間の問い**（`garden.within` / `nearest`）: `answer_in_tick` で同じ tick に答える。総当たりは Rust |
| | セーブ形式（変えない）、`Genome` の混合と変異の**検査**（selftest 8 番） |

**世界のスクリプトは 2 本目の VM で走る**（名札 `struct World;`）。生き物の VM（既定、無変更）とヒープも予算も購読も別。**世界 VM の Tick は生き物 VM の Tick の前**に置く（`RubevySet::<World>::tick()` → `apply_component_writes::<World>` → `RubevySet::tick()`）ので、世界がこのフレームに書いた `Hunger` を生き物が同じフレームの Tick で読める。`tell` は Rust が生き物の VM に publish する中継。

**やらないこと**: 総当たりを Ruby で書くこと（`garden.within` が Rust。§3.2）。`separate` と接触検出を Ruby に出すこと。セーブ形式の変更。生き物の VM を 2 本目に移すこと。

---

## 2. 決まっていること・既定

### 決まっている（著者）

1. 世界の規則を Ruby で、実行中に差し替えられるようにする（2026-09-17）。
2. 既存の selftest 10 判定は**同じ数値の `world.rb`** で通ること（規則の意味を変えない）。4 番・5 番の揺れは既知（§5）。
3. unsafe を書かない。根拠のない数を置かない。

### 既定（違和感があれば止めて報告）

| # | 項目 | 既定 |
|---|---|---|
| 1 | 1 フレームに 1 回 | `each_frame` は「ブロック → 次のフレームまで待つ」の loop。待つのは `Rubevy.ask("frame")`（ゲームが `RubevySet::<World>::answer()` の小さなシステムで `Answer::Num(frame)` を返す。1 往復 = 1 フレームなので**ちょうど 1 回**）。`sleep 0` や `Task.pass` は使わない（同じフレームに何度も回る） |
| 2 | 予算 | 世界 VM の `budget` / `frame_time` は**測って決める**: W1 で `each_frame` の 1 回の命令数と時間を HUD/headless に出し、その最大値から余裕をどう取るかを worklog に書いてから数を置く。置くまでは rubevy の既定（200,000 / 8 ms）。`P` で 0 |
| 3 | 空間の問い | `garden.within(entity, r, :Kind)` → `Answer::Rows`（近い順、entity と距離）。`garden.nearest(entity, :Kind)` は `within(entity, sight, kind).first`。`answer_in_tick` で登録（世界 VM）。生き物 VM の `garden.nearest` は**今のまま**（Answer の段、1 フレーム。生き物の切替は別の話） |
| 4 | 書きは末尾反映 | 世界が書いた `Hunger` は世界 VM の Tick の末尾で反映、生き物 VM の Tick で読める（§1 の順序）。世界自身は同じフレームで読み返せない（規則は `dt` 積分なので困らない） |
| 5 | Ruby から見せる | `Breeding`（クールダウン）を `Reflect` + `register_type`（つがいの条件が Ruby にあるため）。`Eating`（食事の開始判定）は Ruby の変数（`@eating` の集合）に置き換え、Rust の `Eaters` は消す。`Contacts` / `Bumps` は Rust のまま |
| 6 | 生き物の生成 | `garden.spawn` は世界 VM でも答える（今の `answer_garden` の spawn を関数にして共用）。`hatch` の「子が生まれた時点で親に `MATE_COST` / `MATE_COOLDOWN`」は Ruby（世界）が書く |
| 7 | 昼夜 | 位相は Rust の `Sky` のまま（描画と一体）。Ruby は `day_length` を `garden.rules(day_length:)` で渡す（数値 1 つ。`Rules` リソースは**これだけ**のために作らない → `Sky.day_length` フィールド）。`"night"` / `"day"` の publish は Rust のまま（世界 VM にも届ける: `on(:night)` を世界で使えるように、2 回 publish） |
| 8 | `tell` | 宛先 `:all` か `Rubevy::Entity`。payload は `nil` / 数 / 文字列 / Entity。世界 VM → Rust → 生き物 VM の `publish` |
| 9 | `world.rb` が読めない・壊れている | 起動時: `panic` ではなく**世界の規則が止まったまま走る**（草は育たず腹も減らない）と HUD に 1 行。Ctrl+Enter で壊したとき: 古い世界タスクを止め、エディタの status にエラー |
| 10 | 世界の状態はセーブしない | 季節の番号などは `world.rb` の変数。F9 で読み戻すと世界スクリプトは起動時の状態から。`docs/garden.md` に明記 |
| 11 | 生き物側 | `beetle.rb` / `rabbit.rb` に `on(:season)` を 1 つずつ足してよい（小さく。規則の意味は変えない） |
| 12 | エディタの単位 | 「種」2 つ + 「世界」1 つ。世界を開くのは **`F3`**。`Brains` の 3 枠目、`EditorChoice.id = 2`、Ctrl+Enter は世界スクリプト 1 体の restart |
| 13 | selftest | 落ちたら閾値を動かさず止まって報告 |

---

## 3. 設計

### 3.1 Rust 側

* **消すシステム**: `grow_plants` / `sprout_plants` / `get_hungry` / `eat` / `court` / `starve` / `hatch`（生成の実体は残す）。**残すシステム**: `day_night`（位相は `Sky.day_length`）、`move_creatures`、`separate`、`startle`（`"touched"`）、`bumped` の publish、`answer_garden`（生き物 VM の質問）、`watch_*`（selftest）。`const` のうち規則の数値は削除（残すのは §1 右列の力学・生成・selftest 用）。
* **世界 VM**: `RubevyPlugin::<World>::for_vm(ruby_dir)`。`configure_sets` で `RubevySet::<World>::tick()` を `RubevySet::Tick` の前に、規則の残り（`move_creatures` … `separate`）は `.before(RubevySet::<World>::tick())`。エンティティ 1 体 `WorldScript` に `Script::<World>::for_vm(handle)`。`P` で世界 VM の `budget = 0`。
* **`answer_world`**（`RubevySet::<World>::answer()`）: `frame`（既定 1）、`garden.rules`、`garden.tell`、`garden.spawn`、`garden.count`。**`answer_in_tick`**（世界 VM）: `garden.within`（グリッド `CELL` を使った近傍。`separate` の格子を共用できるならする）。
* **selftest**: 既存 10（`eat` が積んでいた `test.ate_at` は「`Hunger` が増えたフレーム」で代替、`starve` の `test` 記録は `Rubevy.despawn` の観測で代替、`court` の `test.matings` は `garden.spawn` の引数の親の遺伝子で代替 → 8 番の `judge_child` はそのまま）。新規 11〜13:
  11. 世界の規則が Ruby から動いている（起動 2 秒以内に世界 VM の `each_frame` が走り、どれかの `Plant.size` が増えた）
  12. 規則は生きている: 20 秒時点で世界スクリプトを「腹が減らない版」に差し替え、以後 5 秒で誰の `Hunger` も減らない。戻して減る
  13. 世界の宣言が届く: `tell :all, "season", "wet"` を `beetle.rb` の `on(:season)` が `@memory[:season]` に書き、`read_memory` で読める
* **計測**（既定 2 のため）: headless の総計行に世界 VM の「`each_frame` 1 回あたりの命令数（最大・中央値）」「世界 VM の tick 時間」を出す。

### 3.2 `garden.within`（`answer_in_tick`）

* 引数: `(entity, r, kind)`。答え: `Answer::Rows`（各行 `[entity_bits, distance]`、近い順）。`kind` は `ReflectComponent` で解決（`answer_garden` の `nearest` と同じ、コンポーネントごとの Rust コード無し）。
* 費用: 世界の `each_frame` が 24 匹 × 1 回呼ぶ。答えは `&World` の格子走査で µs の桁。これが「総当たりは Rust」の実体。
* 生き物 VM の `garden.nearest` は変えない。

### 3.3 Ruby 側

* `ruby/world_prelude.rb`（新規、生き物の `prelude.rb` と同じ作り）: `world do … end`、`day_length`、`each_frame { |dt| }`、`every(seconds) { |n| }`、`tell`、`plants` / `creatures`（`Rubevy.find(:Plant)` / `find(:Creature)` を **1 フレーム 1 回**キャッシュ）、`sprout`（`garden.spawn` の草版か、Rust の `sprout_plants` の生成部を質問にしたもの）、`starting_to_eat?`、`run_world`。`garden` は `Rubevy::Proxy.new("garden")`。
* `ruby/world.rb`（同梱版）: §1 の例を**今の const と同じ数値**で。季節は 1 日ごとに乾季/雨季（芽の確率を変える）と `tell`。
* `beetle.rb` / `rabbit.rb`: `on(:season)` を 1 つ。

### 3.4 窓（G4 の拡張）

`F3` で世界のスクリプトをエディタに。Ctrl+Enter / Ctrl+S / `Watch` は種と同じ道。`guide_text.rs` に `F3` の行と段落 1 つ（英日、短く）、`tools/subset-font.sh` を回す（足りない文字は `docs/worklog/2026-09-17-garden-guide-rewrite.md` の 1 行で確かめる）。HUD に世界 VM の命令数/フレーム。

---

## 4. 段階

| 段階 | 内容 | 確認 |
|---|---|---|
| W1 | 世界 VM + `world_prelude.rb` / `world.rb` + `frame` / `rules` / `spawn` / `within`（`answer_in_tick`）+ 草・空腹・食べる・餓死を Ruby に移す（Rust の対応システムを消す）+ 計測 + selftest 11・12 | 既存 10 + 11・12。sabibots 無変更で通ること |
| W2 | つがい・子（`court` / `hatch` を Ruby に、`Breeding` を Reflect）+ `every` + `tell` + 季節 + `on(:season)` + selftest 13 | 10 + 11〜13。8 番（遺伝子）がそのまま通ること |
| W3 | 窓: `F3`・`Brains` 3 枠目・`Watch`・wasm（`build.rs` / `localStorage`）・guide とフォント・`docs/garden.md`（規則の表を「規則は `world.rb`」に）・`garden-plan.md` の状況表 | 窓の判定 + 新 1、`web/build.sh garden`、`--shot` 1 枚 |

各段階の終わりに `cargo build --release -p garden -p sabibots`、garden `--headless 90` ×5、sabibots `--headless 25` ×1、`cargo clippy -p garden`。

---

## 5. 分かっている罠

1. **Ruby から publish できない** → `tell` は質問、Rust が publish。
2. **接触の開始検出は Rust のまま**（`Contacts` / `Bumps`）。`Eaters` だけ Ruby に。
3. **selftest 6・7 は publish の瞬間を Rust が記録**（`startle` の `test.touched`、`day_night` の `test.night_at`）。発火点は Rust のままなので触らない。
4. **selftest 4 番・5 番は今も揺れる**（S3 の記録: 4 番は順序と無関係に 1/24〜1/26、5 番は順序を入れてから 3/26）。規則が Ruby に移ると 5 番の筋道（最初のパスが見る世界）が変わりうる。落ちたら閾値を動かさず、`miss` 行の証拠を出して報告。
5. **書きは末尾反映**: 世界が同じフレームで自分の書きを読み返せない。規則は `dt` 積分にする。食べる判定で「同じ草を 2 匹が同じフレームに齧る」は両方の書きが最後の者勝ちになるので、Ruby 側で草ごとに 1 フレーム 1 口にする（`@bitten` の集合）。
6. **`Rubevy.find` は全エンティティ走査**（窓ありではモデルの子も数に入る）。`plants` / `creatures` は 1 フレーム 1 回にキャッシュし、`each_frame` の外では使わない。
7. **世界 VM のロードパスは `ruby/`** だが `require` は使わない（`compile_source` で 1 本）。
8. **`Cargo.lock`**: S4 が rubevy main に入ってから `cargo update -p rubevy`（sabiruby は `--precise` で据え置き）。

---

## 6. 状況

| 段階 | 状態 |
|---|---|
| W1 | **済み**（ブランチ `garden-world`、`87038d9` `bd8fef7` `113f7d5` `39c60f1`、2026-09-17）。12 判定 ×5 全通過。**実測**: 世界 VM は 1 パス 中央 9〜11.6k 命令・最大 13.3k（上限の 90 株 24 匹で最大 20,128）、tick 中央 1.6 ms・99% 3.8 ms。1 パスの命令数 ≈ 262 + 159 × 草 + 280 × 生き物（残差 0.9%）。`garden.within` は 1 回 3.1 µs、1 フレーム 34 µs = tick の 2%。**予算** `budget = 45_000`（上限の 2 倍の箱庭の外挿 42,300 を丸めた）、`frame_time` は既定 8 ms のまま（45,000 命令 ≈ 8.6 ms で同じ限界を指す。ブラウザ版のため時間は下げない）。`rubevy-arena` に `VmClockSet`（2 VM の tick を時計の窓から外す）。**W2 で直すもの**: `@bitten`（1 株 1 フレーム 1 口）は書きを最後に 1 回にまとめた今は根拠が消え、Rust の `eat` と違う規則になっている → 外す。世界 VM の `rand` は走行間で同じ列 → 起動時に Rust の `Dice` から種を渡す。「食べる相手」は「クエリ順の最初」から「いちばん近い株」に変わった（意味は良くなった、記録のみ）。`"ate"` は `tell` が W2 なので W1 では飛ばず、セーブの `@memory` から `meals` が消えている（W2 で戻る） |
| W2 | **済み**（`ec3fd46` `b3b3137` `6f37d68`、2026-09-17）。`court` / `hatch` を Ruby に（体を作る `children_arrive` だけ Rust。`Breeding` は Reflect、`partner` は `Entity::PLACEHOLDER` で「なし」）、`every` / `tell` / 季節（乾季 0.5・雨季 0.9 = 元の 0.7 から等距離。同梱版の遊びの設定）/ `"ate"` / `on(:season)` / 判定 13（10/10、0.03 s で届く）。8 番は `judge_child` ごとそのまま通る。`@bitten` 削除、世界の乱数はゲームの `Dice` の種。`every` のタスクは規則の差し替え時に自分で止まる必要があった（`$world_being` を見る 1 行）。**実測**: 1 パス中央 12.2〜14.5k（W1 比 +25〜30%）、最大 19.4k、tick 中央 1.8〜2.1 ms。予算 45,000 は据え置き（上限の箱庭での測り直しは未。観測倍率の外挿で約 29,000）。**揺れ**: 10 回で 5 番 1 回（既知）、7 番 1 回（夜が落ちた瞬間に生まれた子が `"night"` を聞いていない。G2 以来ありうる、初観測。判定側の話なので未着手） |
| W3 | 未着手 |

第 1 版（数値だけ Ruby、`Rules` リソース）は同期読みが入る前の妥協で、W1 の途中差分ごと捨てた（2026-09-17）。
