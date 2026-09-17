# 箱庭の世界を Ruby にする（`world.rb`）— 実装指示書

作成 2026-09-17。著者の問い「シミュレーション環境も Ruby で動的にしないと面白くないと思わないか」への答え。
調査は `docs/worklog/2026-09-17-garden-world-survey.md`（行番号つき。**着手前に全部読む**）。rubevy は `bfa47ed`（今日入った「1 アプリに VM を複数」。`rubevy/docs/host-api.md` の "Two VMs in one app" を読む）。

**この文書だけで着手できるように書いてある。** 読む順は 1 → 2 → 3 → 4。5 は着手前に必ず目を通す。

---

## 0. はじめの一歩

```bash
cd /home/kishima/book/kishima/rubevy_games
git switch -c garden-world            # main から。main には触らない
cargo update -p rubevy                # Cargo.lock を rubevy main（bfa47ed 以降）に。2 本目の VM が要る
cargo build --release -p garden && GARDEN_SELFTEST=1 ./target/release/garden --headless 90   # 着手前に 10 判定が通ること
```

作法は `/home/kishima/book/.claude/agents/implementer.md`。段階ごとに 1 コミット、push しない、過程は `docs/worklog/2026-09-17-garden-world.md` に書きながら進める（捨てた案と理由も）。決めきれない点は止まって報告する。

---

## 1. 何を作るのか（30 秒版）

今の箱庭は**生き物だけが Ruby** で、世界の規則（昼の長さ、草の生え方、腹の減り方、つがいの条件、餓死）は `garden/src/main.rs:62-308` の `const` に固定されている。
遊んで面白いのは「規則を変えたら生き物がどう適応するか」なので、規則が Ruby でなければ半分しか遊べない。

そこで `garden/ruby/world.rb` を置く。生き物と同じようにエディタで書き換え、Ctrl+Enter で**箱庭を止めずに**差し替わる:

```ruby
world do
  day_length 60                                    # 秒。数値は書いた瞬間に世界に効く
  plants   max: 90, sprout: 0.7, growth: 0.06, min: 0.18, big: 1.4
  hunger   rate: 1.6, food: 60, eat: 1.0, max: 100, reach: 1.1
  mating   hunger: 75, cost: 30, cooldown: 20, reach: 2.0, population: 24, child: 50

  every 60 do |n|                                  # 世界の出来事は周期で（毎フレームではない）
    dry = n.odd?
    plants sprout: (dry ? 0.15 : 1.2)              # 乾季と雨季
    tell :all, "season", (dry ? "dry" : "wet")     # 生き物は on(:season) で受ける
  end
end
```

**境界**（これがこの計画の要）:

| Ruby（`world.rb`） | Rust（そのまま） |
|---|---|
| 規則の**数値**（上の 4 行）。書いた瞬間に効く | 数値を**使う側**: `day_night` / `grow_plants` / `sprout_plants` / `get_hungry` / `eat` / `court` / `starve` の力学 |
| **周期的な出来事**（`every`）: 季節、雨、疫病、恵み | 毎フレームの総当たり（`separate`、`eat` の距離判定、`nearest`） |
| **宣言**（`tell`）: 世界から生き物へのイベント | 配達そのもの（`ScriptWorld::publish`）と、接触の開始フレームの検出（`Contacts` / `Bumps` / `Eaters`） |
| 世界の状態（季節の番号など）は `world.rb` の変数 | セーブ形式（変えない） |

**世界のスクリプトは 2 本目の VM で走る**（名札 `struct World;`）。生き物の VM（既定、無変更）と、ヒープも予算も購読も別。世界が `loop {}` しても生き物のフレームは減らないし、生き物のスクリプトから世界の変数は見えない。`tell` は Rust が生き物の VM に publish する（publish は VM を跨がないので、これが中継。`docs/host-api.md` "publish goes to that VM's subscribers"）。

**やらないこと**: 毎フレームの数値更新を Ruby でやること（1 回の読みが 1 フレーム、24 匹 × 90 株の距離判定は Ruby に載らない。調査 §7）。`Breeding`・`Contacts` を Ruby に出すこと。セーブ形式の変更。生き物の VM を 2 本目に移すこと（既定 VM のまま。触る行が 68 行ある。調査 §5 の 5）。

---

## 2. 決まっていること・既定

### 決まっている（著者）

1. 世界の規則を Ruby で、実行中に差し替えられるようにする。
2. 既存の selftest 10 判定は**同じ数値の `world.rb`** で通ること（規則の意味を変えない）。

### 既定（違和感があれば止めて報告）

| # | 項目 | 既定 |
|---|---|---|
| 1 | 数値の置き場 | Rust の `Rules` リソース（`#[derive(Deserialize)] #[serde(deny_unknown_fields)]`、`CreatureSpec` と同じ道）。`const` は**削除**し、全部 `Rules` から読む。Rust に残す定数は力学・生成・selftest 用（`REACH` 以外の半径、`CELL`、`SEPARATE_PASSES`、`TREES`/`ROCKS`、`PLANTS_AT_START`、`DAWN_OFFSET`、`MOON_LUX` 等、`NEWBORN_GRACE`、`TOUCH_SETTLE`、`COURT_RETRY`） |
| 2 | 数値が届く道 | `garden.rules(hash)` という質問（世界 VM だけが持つ）。Hash は `Arg::Value` で来るので `sabiruby_serde::from_value::<RulesPatch>`。**部分更新**（書いた項目だけ変える。`Option` のフィールド）。知らないキーは断り文句を返し、Ruby 側で `raise` |
| 3 | `world.rb` が無い・壊れている | 起動時: 読めなければ `panic` ではなく **`Rules::default()`（今の const の値）で走り、HUD の status に 1 行**。Ctrl+Enter で壊したとき: 古い `Rules` のまま、エディタの status にエラー（生き物と同じ） |
| 4 | 世界 VM の予算 | `budget = 20_000`、`frame_time = 1 ms`（生き物は既定のまま）。`P` で両方 0 |
| 5 | `tell` の宛先 | `:all` か `Rubevy::Entity`。payload は `nil` / 数 / 文字列 / Entity。世界 VM → Rust → 生き物 VM の `publish`。**Rust の既存 publish（`night` 等）は Rust のまま** |
| 6 | `every` | `Task.new { loop { sleep s; n += 1; block.call(n) } }` の砂糖（世界 VM の中）。ポーズで止まる（VM の時計が止まる）。`on(:night)` 等の世界側ハンドラは**今回は作らない**（Rust の publish を世界 VM にも届けるのは後） |
| 7 | 昼夜の位相 | Rust の `Sky` のまま（描画と一体）。Ruby が持つのは `day_length` だけ。`"night"` / `"day"` の publish も Rust のまま |
| 8 | 世界の状態はセーブしない | 季節の番号などは `world.rb` の変数。F9 で読み戻すと世界スクリプトは起動時の状態から。`docs/garden.md` に明記 |
| 9 | 生き物側の変更 | `beetle.rb` / `rabbit.rb` に `on(:season)` を 1 つずつ足してよい（乾季に視野を広く、など小さく）。**規則の意味は変えない** |
| 10 | エディタの単位 | 「種」2 つ + 「世界」1 つ。世界を開くのは **`F3`**。`Brains` の 3 枠目、`EditorChoice.id = 2`、Ctrl+Enter は世界スクリプト 1 体の `restart` |

---

## 3. 設計

### 3.1 Rust 側

* **`Rules`**（`main.rs`、`#[derive(Resource, Clone, Deserialize)]`）: `day_length, plants{max,sprout,growth,min,big}, hunger{rate,food,eat,max,reach}, mating{hunger,cost,cooldown,reach,population,child}`。`Default` は今の const の値。**`RulesPatch`** は同じ形で全部 `Option`（`deny_unknown_fields`）。`Rules::apply(&mut self, patch)`。
* 使う側の書き換え: `day_night`(2238) の `DAY_LENGTH`、`grow_plants`(2553)、`sprout_plants`(2562)、`get_hungry`(2583)、`eat`(2602)、`court`(2749)、`hatch`(2820) の `MATE_COST`/`MATE_COOLDOWN`/`CHILD_HUNGER`、`starve` は閾値なしのまま。`Res<Rules>` を足すだけで構造は変えない。`MIDNIGHT`(77) のように const から派生している値は関数にする。
* **世界 VM**: `RubevyPlugin::<World>::for_vm(ruby_dir)`（`main.rs:1290` 付近、既定のプラグインの隣）。`Startup` で予算を下げる（既定 4）。エンティティ 1 体 `WorldScript` に `Script::<World>::for_vm(handle)`。`P` の `budget = 0`（`window.rs:431`）を `ScriptWorld<World>` にも。
* **`answer_world`**（`RubevySet::<World>::answer()`）: 質問は `garden.rules` / `garden.tell` / `garden.count` / `garden.spawn`（世界が生き物を生めるように。`answer_garden` の spawn を関数に切り出して共用）。`garden.nearest` と `genome` は世界には無い（答えは `Answer::Nil` + `warn!`）。`tell` は `ScriptWorld`（生き物 VM）の `publish` / `publish_value` を呼ぶ。**`answer_world` は `answer_garden` と別システム**（別リソースなので Bevy が並列にできる）。
* **コンパイル**: `compile_source` は prelude + 種 + `run_creature` を 1 本にする。世界は `ruby/world_prelude.rb`（`world do`, `every`, `tell`, 数値の DSL）+ `world.rb` + `run_world` で同じ形。`Brains` の 3 枠目。`build.rs` は `ruby/` を再帰で拾うので wasm は自動（確認する）。`Watch` に `world.rb` を足す。
* **selftest**（`stop_when_over`、判定 11〜13 を足す。既存 10 は無変更で通ること）:
  11. `Rules` が Ruby から届いた（起動後 2 秒以内に `rules.came_from_ruby` が true。`world.rb` の数値が既定と同じなので値では見分けられない → フラグで）
  12. 規則は生きている: 20 秒時点で selftest が世界スクリプトを `hunger rate: 0` の版に差し替え（`restart` と同じ道）、以後 5 秒で誰の `Hunger` も減らない。その後 `rate: 1.6` に戻して減る
  13. 世界の宣言が生き物に届く: `world.rb` が起動 3 秒後に `tell :all, "season", "wet"` し、`beetle.rb` の `on(:season)` が `@memory[:season]` に書く。`read_memory`（`main.rs:3299` の道）で 1 匹でも `"wet"` が読めること

### 3.2 Ruby 側

* `ruby/world_prelude.rb`（新規、生き物の `prelude.rb` と同じ作り）: `world do ... end`、`day_length`、`plants`、`hunger`、`mating`（内部は `garden.rules(...)` 1 回。`world do` の中で書いた数値は**ブロックの終わりにまとめて 1 回**送る。`every` の中からの呼び出しはその場で送る）、`every(seconds) { |n| }`、`tell(target, name, payload = nil)`、`run_world`。
* `ruby/world.rb`（新規、同梱版）: §1 の例。数値は**今の const と同じ**。季節は 1 日（60 秒）ごとに乾季/雨季で `sprout` を変え `tell :all, "season", ...`。
* `beetle.rb` / `rabbit.rb`: `on(:season) { |s| @memory[:season] = s; ... }` を 1 つ（小さな挙動: 乾季は `sleep` を短く、など。**規則の意味は変えない**）。

### 3.3 窓（G4 の拡張）

* `F3` で世界のスクリプトをエディタに（種の切替と同じ `EditorChoice`、id 2）。Ctrl+Enter / Ctrl+S / 外部保存の `Watch` は種と同じ道。
* `guide_text.rs` に `F3` の行（英日）を足し、`tools/subset-font.sh` を回す（`docs/worklog/2026-09-17-garden-guide-rewrite.md` の 1 行で足りない文字を確かめる）。説明の段落 1 つを足す（「世界の規則も Ruby。F3 で開いて書き換えると箱庭を止めずに効く」、英日、短く）。
* HUD の status に世界 VM の insn/frame を 1 行（任意）。

---

## 4. 段階

| 段階 | 内容 | 確認 |
|---|---|---|
| W1 | `Rules` リソース + 世界 VM + `world_prelude.rb` / `world.rb`（数値だけ。`every` と `tell` はまだ）+ `garden.rules` + selftest 11・12 | 既存 10 判定 + 11・12。sabibots も無変更で通ること（`rubevy-arena` を触ったら） |
| W2 | `every` + `tell` + 季節 + 生き物の `on(:season)` + selftest 13 | 10 + 11〜13 |
| W3 | 窓: `F3`・`Brains` 3 枠目・`Watch`・wasm（`build.rs` / `localStorage` の鍵）・guide とフォント・`docs/garden.md`（規則の表を「数値は `world.rb`」に）・`garden-plan.md` の状況表に W 行 | 窓の判定（garden 21 + 新 1: F3 で開いて Ctrl+Enter で `Rules` が変わる）、`web/build.sh garden` が通ること、`--shot` 1 枚 |

各段階の終わりに `cargo build --release -p garden -p sabibots`、`GARDEN_SELFTEST=1 ./target/release/garden --headless 90` ×3、`SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 25` ×1、`cargo clippy -p garden`。

---

## 5. 分かっている罠（調査 §「引っかかりそうな点」から）

1. **Ruby から publish できない** → `tell` は質問として作り、Rust が publish する（§3.1）。
2. **接触の開始検出は Rust のまま**（`Contacts` / `Bumps` / `Eaters`）。Ruby に出さない。
3. **selftest 6・7 は publish の瞬間を Rust が記録している**（`startle` の `test.touched`、`day_night` の `test.night_at`）。発火点は Rust のままなので触らない。
4. **`Sight` は遺伝子、`REACH` は規則**。`hunger.reach`（食べる距離）と `mating.reach` だけ `Rules` に。衝突半径は力学なので Rust。
5. **`court` と `hatch` は世界時計、`eat`/`startle`/`starve` は素の `Time`**（調査 §7）。今回は揃えない（別の話）。
6. **世界 VM のロードパスは `ruby/`**（`for_vm(ruby_dir)`）だが `require` は使わない（`compile_source` で 1 本にする）。
7. **`P` は世界 VM の予算も 0 に**（`window.rs:431` の隣）。`every` は VM の時計で待つので、ポーズ中に季節が進まない。
8. **`rubevy_games` の `Cargo.lock` は rubevy の古い rev を指している**。`cargo update -p rubevy` が最初。sabiruby の rev は rubevy と同じにする（`Cargo.toml:27` のコメント）。

---

## 6. 状況

**保留（2026-09-17、著者判断）。** W1 に着手した直後に「同期の読み書きを先に考えるべき」となり止めた。
この計画の境界（数値と周期だけ Ruby）は「コンポーネントの読みが 1 回 1 フレーム」を前提にした妥協で、
rubevy に同期アクセス（`rubevy/docs/plans/sync-access-plan.md`）が入れば `world.rb` は毎フレームの規則そのものを書ける。
同期アクセスが入ってからこの計画を書き直す。W1 の途中差分（`Rules` への置き換え途中）は worktree `rubevy_games-wt-world`（ブランチ `garden-world`）に未コミットで残してあり、書き直し時に捨てる。


| 段階 | 状態 |
|---|---|
| W1 | 未着手 |
| W2 | 未着手 |
| W3 | 未着手 |
