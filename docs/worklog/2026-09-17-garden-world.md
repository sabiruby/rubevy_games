# 2026-09-17 箱庭の世界を Ruby にする — W1（世界 VM と、草・腹・食事・餓死の引っ越し）

`docs/plans/garden-world-plan.md`（第 2 版）の **W1**。worktree
`/home/kishima/book/kishima/rubevy_games-wt-world`、ブランチ `garden-world`、main `aadc7bf` から。
rubevy は main `3124dfa`（S4 = `answer_in_tick` が入った版）。

前提の調査は `docs/worklog/2026-09-17-garden-world-survey.md`（ただし §7 の「読み 1 回 1 フレーム」は
S1 で変わった）と `docs/worklog/2026-09-17-sync-reads.md`（順序・計測・6 番の経緯）。

書きながら進める記録。捨てた案と、なぜ捨てたかも残す。

---

## 0. 着手前 — 何が動いていて、何が揺れているか

最初にやったのは `cargo update -p rubevy` と、**その状態で 10 判定を 3 回測ること**。
W1 は規則の半分を別の言語に移す変更なので、「移す前の揺れ」を知らずに始めると、
移したあとの FAIL がどちらのせいか言えなくなる。

```
Updating rubevy v0.0.1 (https://github.com/sabiruby/rubevy#4f77881b) -> #3124dfaf
```

`cargo update -p rubevy` は sabiruby も `bd6829b3` → `8faf26f2` に上げようとした
（S3 のときと同じ挙動、`docs/worklog/2026-09-17-sync-reads.md` §0）。指示どおり
`cargo update -p sabiruby --precise bd6829b3…` で戻した。`Cargo.lock` の sabiruby / -macros /
-compiler / -serde は 4 か所とも `bd6829b3` の 1 つ、rubevy だけが `3124dfaf`。

`cargo build --release -p garden -p sabibots` は 2 分 50 秒で通る。

### 0.1 着手前の 10 判定（`GARDEN_SELFTEST=1 ./target/release/garden --headless 90` ×3）

| 判定 | 1 回目 | 2 回目 | 3 回目 |
|---|---|---|---|
| 1 somebody ate within 10 s | ok (0.75 s) | ok (0.56 s) | ok (0.75 s) |
| 2 night arrived by 60 s | ok (25.21 s) | ok (25.21 s) | ok (25.20 s) |
| 3 the starved creature's entity is gone | ok (1.89 s) | ok (1.89 s) | ok (1.89 s) |
| 4 nothing walked through anything | ok (0.980) | **FAIL** (0.860、1 フレーム) | ok (0.972) |
| 5 a hungry creature … reached it | ok (1.79 s) | ok (1.79 s) | ok (1.79 s) |
| 6 a beetle touched by a rabbit turned | ok (36/36) | ok (28/28) | ok (21/21) |
| 7 asleep a second after night | ok | ok | ok |
| 8 child's genome | ok | ok | ok |
| 9 spawn Hash names the gene | ok | ok | ok |
| 10 a save with the wrong version | ok | ok | ok |

**4 番は 3 回中 1 回落ちた。** S3 の記録（§9.5）が「順序なし 1/24、順序あり 1/26」と書いた
すり抜けそのもので、`closest pair 0.860 of the radii, 1 frames under 0.9` と、落ち方も同じ。
**着手前から落ちる判定**だということを、この 3 回で押さえておく。

総計行（世界 VM が入る前の、生き物 VM だけの数）:

```
hud: insn/decision — 5479 passes of a behaviour's loop, 196.3 instructions each; and 1003 questions the game answered, 1.000 frames each
hud: insn/decision — 4772 passes of a behaviour's loop, 203.5 instructions each; and 1111 questions the game answered, 1.000 frames each
hud: insn/decision — 5407 passes of a behaviour's loop, 198.7 instructions each; and 1048 questions the game answered, 1.000 frames each
hud: … VM 0.75 / 8.0 ms this frame (0.93 ms smoothed) / 0.70 (1.03) / 1.26 (1.22)
```

sabibots（`SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 25` ×1）は**全部 ok**。

---

## 1. 形を決めるまで — 「1 パス = 1 フレーム」は何に支えられているか

計画書 §2 の既定 1 は「`each_frame` は `Rubevy.ask("frame")` の 1 往復で次のフレームを待つ」。
これは**世界の脚本が使う質問のうち、フレームを食うのがそれ 1 つだけ**であることに全部かかっている。
書く前に rubevy 側で 1 つずつ確かめた（`rubevy/src/lib.rs` と `docs/host-api.md`）:

| Ruby | 何になるか | フレーム |
|---|---|---|
| `p[:Plant]` / `c[:Hunger]` | `component.get`（`RESERVED_KINDS`、`lib.rs:1863`） | **0**。tick の中の答えループが答える（`lib.rs:1706`） |
| `Rubevy.find(:Plant)` | `entities.with`。**これも `RESERVED_KINDS`** | **0**。ここが効いた（§1.1） |
| `p[:Plant] = {…}` | `HostCommand::SetComponent` | 0（待たない。反映はフレーム末尾） |
| `Rubevy.despawn(c)` | `HostCommand::Despawn` | 0 |
| `garden.within(c, r, :Plant)` | `answer_in_tick` に登録した閉包 | **0** |
| `garden.sprout` | 質問だが**`pop` しない**（§1.2） | 0 |
| `Rubevy.ask("frame").pop` | `answer_world`（システム） | **1** |
| `$rubevy[:delta]` | tick の頭で書かれるグローバル（`lib.rs:1669`） | 0 |

実測は総計行の 1 行で出る: **5,370〜5,374 フレームに対して `each_frame` が 5,370〜5,372 パス**。
取りこぼしは起動の 2 フレーム（スクリプトが立ち上がって `garden.rules` を訊く往復）だけで、
それ以降は 1 フレーム 1 パスがきれいに続く。設計の根拠がそのまま計測になっている。

### 1.1 `Rubevy.find` がフレームを食わないのは偶然ではない

調査（`2026-09-17-garden-world-survey.md` §8）は `Rubevy.find` を「全世界走査なのでたまに使うもの」と
書いていて、それは今も正しい。ただし**フレームの意味では**タダになっている:
`entities.with` は rubevy が自分で答える 4 つのうちの 1 つで、S1 以降その 4 つは tick の中で答えられる。
だから `plants` / `creatures` の 1 フレーム 1 回キャッシュは「往復を減らすため」ではなく
**「世界を 2 回舐めないため」**のもので、prelude のコメントもそう書いた。

### 1.2 `sprout` は質問ではなく命令にした（既定からの逸脱、報告あり）

計画書 §1 の例は `sprout if rand < 0.7 * dt && plants.size < 90`。
芽を実際に生やすのは Rust（位置・間隔・大きさ・モデル）なので質問になるが、
**`pop` すると そのフレームのパスが 1 フレーム parked になる** ——
つまり芽が出るフレームだけ `each_frame` が 2 フレームかかり、その間の草も腹も止まる。
0.7/秒 なので 90 秒で約 63 フレーム、全体の 1.2%。

そこで `sprout` は `Rubevy.ask("garden.sprout")` を投げて **`pop` しない**ことにした。
`ScriptWorld::answer` はキューに push して `gc_unregister` するだけなので（`lib.rs:1003`）、
誰も `pop` しないキューは次の GC で回収される。意味としてもこちらが正しい——
芽を出すのは `Rubevy.spawn` / `Rubevy.despawn` と同じ**命令**であって、問いではない。
失う情報は「置けたかどうか」だが、置けるかどうかの判断（上限と確率）は既に Ruby 側にある。

## 2. Rust に残したもの、Ruby に移したもの

消したシステムは 5 本（`grow_plants` / `sprout_plants` / `get_hungry` / `eat` / `starve`）。
代わりに置いたのは規則を 1 つも知らない 3 本:

* **`plants_wear_their_size`** — `Plant.size` は `Transform.scale` でもある（「1 つの数を 2 通りに見る」、
  `docs/garden.md` の表）。`grow_plants` が両方書いていたのは、それが草を育てていたからで、
  規則が Ruby に移った今は **数を書くのが Ruby、絵にするのが Rust**。`Changed<Plant>` なので
  誰が大きさを変えたか（成長・一口・`world.rb` の編集）に関係なく正しい。
* **`sprout_plants`（作り直し）** — Ruby が頼んだ数だけ種を置く。位置・1.5 の間隔・`PLANT_MIN`・
  茂みか房かは庭の家具で、規則ではない。`garden.spawn` と `hatch` と同じ分け方。
* **`note_the_rules`** — **規則がしたことを外から読む**。ここが W1 でいちばん設計の話になった（§3）。

Rust に残した規則は `day_night`（`Sky::day_length` は Ruby から）、`move_creatures`、`separate`、
`startle`、`court`（W2）、`hatch`（W2）。chain は `.before(RubevySet::Tick)` から
**`.before(RubevySet::<World>::tick())`** に移した——世界 VM が先に走り、その世界 VM が
`garden.within` で測るのはこの chain が置いた `Transform` だから。世界の tick は生き物の tick の前なので、
1 つ前に置き換えるだけで S3 が入れた順序の意味（`docs/worklog/2026-09-17-sync-reads.md` §1）は全部残る。

### 2.1 Ruby に移した数は 1 つも変えていない

`world.rb` の数値は `main.rs` の `const` と同じもので、どの `const` から来たかを 1 行ずつ書いた。
Rust 側から**消した**のは `PLANT_GROWTH` / `PLANTS_MAX` / `SPROUT_RATE` / `HUNGER_RATE` /
`EAT_RATE` / `FOOD_VALUE` の 6 つ。**残した**のは:

| `const` | なぜ残るか |
|---|---|
| `PLANTS_AT_START` 55 / `PLANT_MIN` 0.18 | 世界の生成（最初の草、芽の大きさ） |
| `PLANT_MAX` 1.4 | 生成と selftest の仕込み。`world.rb` にも同じ 1.4 があるが、**言っている事実が違う**（こちらは「世界を作るときの大きさ」、あちらは「成長が止まる規則」） |
| `HUNGER_MAX` 100 | HUD の満腹バーの満杯（`window.rs:720`） |
| `REACH` 1.1 | `startle` ではなく 5 番目の判定（`watch_probe`）が「着いた」を測る距離。`eat` の 1.1 は `world.rb` に移った |
| `DAY_LENGTH` 60 | `Sky::day_length` の**既定**と `--at midnight` の計算。`world.rb` が `day_length` を言うまで、そして言えないとき（コンパイルエラー）はこれで回る |

`PLANT_MAX` と `HUNGER_MAX` は「同じ数が 2 か所」になる。消す道もあったが（生成側を別の数にする、
規則側から質問する）、どちらも**事実を 1 つ増やす**か**毎フレームの往復を増やす**ので採らなかった。
コメントに「ここは何の 1.4 か」を書いて残している。

## 3. いちばん考えたところ — 規則が Rust でなくなると、判定は何を見ればいいのか

`eat` は自分が成功した瞬間に `test.ate_at` を書いていた。`starve` は自分が despawn した相手を書いていた。
**規則が他人のファイルになると、その書き込み点が無くなる。**
計画書 §3.1 の指示は「`Hunger` が増えたフレーム」「despawn の観測」で、そのとおりに `note_the_rules` を書いた。

これは置き換えというより**判定として良くなっている**。`eat` が自分の成功を記録するのは
「`eat` が走った」ことしか言わない。メーターが前より高いのは「世界が変わった」ことを言い、
それは誰が変えたかに依らない——まるで違う書き方をした `world.rb` でも成り立つ。

同じ観測点が**咀嚼のアニメーション**も動かしている。`Eating { until: now + 0.35 }` は `eat` が
挿していたので、規則が移ると窓の中でビートルが口を動かさなくなる。
「メーターが上がった＝一口食べた」を見ている系が、そのまま `Eating` を挿す場所になった。
境界としても正しい: 見た目は Rust のもの。

### 3.1 12 番目の判定が最初に落ちた——繁殖の代償はメーターを下げる

12 番（規則を取り上げて 5 秒、誰の `Hunger` も減らない）は最初の実装で
`2 did`（2 回下がった）と出て落ちた。原因は `hatch`:

```
selftest: FAIL the rules can be taken away and given back while the world runs
             (from 20.02 s no meter fell for 5.0 s; 2 did, and afterwards hunger came back)
```

凍った規則の間は**誰も腹が減らない**ので、全員が `MATE_HUNGER = 75` より上に居続け、
`court` が今まで以上につがいを見つける。生まれるたびに `hatch` が親から `MATE_COST = 30` を引く——
これは規則が下げたメーターではない。W1 では `court` / `hatch` は Rust のままなので、
**Rust に残った唯一の「メーターを下げるもの」**がこれ。

直し方は 3 つ考えた:

1. 1 フレームに 1 点以上下がったものは腹減りではない、と閾値で除く → **根拠のない数**になる。捨てた。
2. 判定を「草が伸びない」に変える → `Plant.size` を触るのは `world.rb` だけなので確実だが、
   計画書の文（「誰の `Hunger` も減らない」）を変えることになる。捨てた。
3. `hatch` が代償を課した親をその場で記録し、`note_the_rules` がその親の下落を数えない。**これにした。**
   正確で、閾値が要らず、「規則がしたこと」と「規則でないものがしたこと」を名指しで分ける。

`note_the_rules` は `.after(hatch)` に置いた——代償と、それが起こした下落を、同じフレームで見るため。
入れたあと 5 回の 90 秒走行はすべて `0 did`。

### 3.2 凍らせる `world.rb` は「何もしない `each_frame`」

12 番が差し替える規則（`FROZEN_WORLD`）は `each_frame do |dt| end` だけ。
草も伸びず、腹も減らず、誰も食べず、誰も餓死しない一方、Rust に残っているもの（太陽、歩き、押し合い、
つがい）はそのまま動く。だから「5 秒メーターが下がらなかった」は
**世界が止まったこと**ではなく**この規則が止まったこと**を言っている。

差し替えの道は W3 のエディタが使う道と同じで、`restart_species`（`window.rs:316`）が種に対してやることを
世界に対してやる: `ScriptTask<World>` を外す（タスクが終わり、キューが閉じる）→ 新しい `Script` を挿す。
**窓は 5 秒を「新しいタスクが実際に立ち上がったフレーム」から数える**——コンパイルと起動に 1〜2 フレームかかり、
頼んだ瞬間から数えると古い規則を数えてしまう。

## 4. 予算を測って決める（計画書 §2 既定 2）

rubevy の既定は 200,000 命令 / 8 ms で、**予算は VM ごとで合算されない**——2 VM だと
最悪 16 ms、60 Hz の 1 フレームまるごと。だから 2 本目には自分の数が要る。指示は「測って決める」。

### 4.1 捨てた案: 最大値に適当な倍率を掛ける

最初に出た数は「実測の最大パス 13,157 命令 → 余裕を見て 50,000」。これは**根拠になっていない**:
13,157 は計測した 90 秒走行のその時の人口（生き物 12〜17、草 27〜53）での最大であって、
箱庭の**上限**（`POP_MAX = 24`、`world.rb` の 90 株）での値ではない。
そのまま置けば「測ったように見える当て推量」になる。

### 4.2 代わりにやったこと: 使い捨ての計測系で法則を出し、上限で確かめた

`world_clock_end` の後ろに使い捨ての系を 1 本足して、毎フレーム
`(1 パスの命令数, 草の数, 生き物の数, tick の ms)` を出した。90 秒 = 5,369 サンプルを最小二乗で:

```
命令数 ≈ 262 + 159.1 × 草 + 279.9 × 生き物        （残差 rms 86、|最大| 470）
```

60 秒の別の走行でも `594 + 154.3×草 + 263.8×生き物` で、係数は一致する。
残差 86 は 10,000 に対して 0.9% で、**この規則の値段は人口の 1 次式**だと言い切れる。

上限に入れると **90 株 × 24 匹 = 21,300 命令**。これは外挿なので、**上限で測り直した**:
`PLANTS_AT_START` を 90、`BEETLES`/`RABBITS` を 16/8 にした使い捨てビルドで 25 秒走らせ、
生き物 24 匹・草 45〜82 株の 1,492 フレーム:

```
命令数  中央 16,553   99 パーセンタイル 20,126   最大 20,128
tick    中央 2.67 ms  99 パーセンタイル 4.45 ms  最大 7.44 ms   （4 ms 超 4.36%、6 ms 超 0.07%）
```

**外挿 21,300 に対して実測の最大 20,128。** 法則は上限でも保つ。

### 4.3 決めた数

* **`budget = 45_000`。** 同じ法則に**上限の 2 倍の箱庭**（180 株・48 匹）を入れた値が 42,300、
  それを丸めた。今の `world.rb` は上限でも 20,128 なので 2 倍以上の余地があり、
  書き直して仕事が倍になっても届かない。生き物 VM の 200,000 の 1/4 弱で、
  **どちらの VM が客か**が数 1 つで分かる。
* **`frame_time` は rubevy の既定（8 ms）のまま。これも計測が決めたこと。**
  上限で中央 2.67 ms・99 パーセンタイル 4.45 ms なので、8 ms は既に「取り分」ではなく「余裕のある番人」。
  そして 45,000 命令は、実測の速さ（1,000 命令あたり 0.19 ms）で **約 8.6 ms** ——
  **2 つの数がほぼ同じ限界を名指している**ので、どちらが先に噛んでも同じ場所で噛む。
  ここで時間の方だけ下げると、**この機械の時計から選んだ数**を、同じ規則が数倍遅く走る
  ブラウザ版（`docs/web.md`）にも押しつけることになる。命令数は規則についての事実、
  ミリ秒は機械についての事実で、下げたのは前者の方。

計測系は数を決めたあと**消した**（`git log` の差分には出ない）。手順は再現できるように上に書いてある。

### 4.4 ついでに見つけた: 生き物 VM の時計が世界の tick を飲み込んでいた

最初の通し走行で HUD の `VM 0.9 ms` が **2.1 ms** になった。生き物のスクリプトは 1 行も変えていない。

原因は `rubevy-arena` の `VmClock`: `vm_clock_start.before(RubevySet::Tick)` と
`vm_clock_end.after(RubevySet::Tick)` で挟んでいるが、**世界 VM の tick は
`RubevySet::Tick` に対してしか順序が無い**ので、bevy がその窓の中に入れることができる。
1 VM のときは窓の中に入るものが無かったので、この書き方で足りていた。

直し方は `rubevy-arena` に 4 行: 2 つの系に `VmClockSet` という名前を付けた。
ゲーム側は `RubevySet::<World>::tick().before(VmClockSet)` と言えば、もう一方の tick を窓の外に出せる。
sabibots には何も変わらない（名前が増えただけ）。入れたあと生き物 VM は **0.13〜0.26 ms** に戻った。

**指示に無い変更なので報告に挙げる。**（garden の headless だけ直すこともできたが、
窓のある版の HUD が「1 VM の予算に対して 2 VM の仕事」を表示したままになる。数が嘘になるのを残すより、
共有 crate に名前を 1 つ足す方を選んだ。）

## 5. 通した確認

| 何を | 結果 |
|---|---|
| `cargo build --release -p garden -p sabibots` | 通る |
| `cargo build -p garden -p sabibots`（窓あり、dev。**実行はしていない**） | 通る（4 分 32 秒） |
| `cargo clippy -p garden` | **13 件。着手前と同じ**（`git stash` して同じコマンドで取り直して比べた） |
| garden `--headless 90` ×5 | **12 判定すべて ok、5 回とも**（§6） |
| sabibots `--headless 25` ×1 | **54 件すべて ok、FAIL 0**。無変更 |
| セーブの往復（`--headless 20 --save` → `--headless 0 --load --save`） | **バイト一致** |
| 走行中のロード（`GARDEN_RELOAD_AT=10`、F9 の道） | **バイト一致**。世界のスクリプトは `load_world` の despawn 対象外なので生き残る |

## 6. 12 判定、5 回

着手前（§0.1）は 3 回中 1 回 4 番が落ちていた。着手後の 5 回は**全部通った**。

| 判定 | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| 1 somebody ate within 10 s | 0.60 | 1.08 | 0.60 | 0.58 | 0.85 |
| 2 night arrived by 60 s | 25.21 | 25.22 | 25.21 | 25.21 | 25.21 |
| 3 the starved creature's entity is gone | 1.89 | 1.89 | 1.89 | 1.89 | 1.89 |
| 4 nothing walked through anything | 0.975 | 0.974 | 0.977 | 0.975 | 0.985 |
| 5 a hungry creature … reached it | 1.79 | 1.79 | 1.79 | 1.79 | 1.79 |
| 6 a beetle … changed heading | 16/16 | 25/25 | 20/20 | 19/19 | 25/25 |
| 7 asleep a second after night | ok | ok | ok | ok | ok |
| 8 the child's genome | ok | ok | ok | ok | ok |
| 9 spawn Hash names the gene | ok | ok | ok | ok | ok |
| **11 the rules in world.rb are running the world** | 0.03 s | 0.03 s | 0.03 s | 0.03 s | 0.03 s |
| **12 the rules can be taken away and given back** | 0 fell | 0 fell | 0 fell | 0 fell | 0 fell |
| 10 a save with the wrong version | ok | ok | ok | ok | ok |

11 番の判定文（そのまま）:

```
selftest: ok   the rules in world.rb are running the world (the grass grew at 0.03 s, over 5371 passes of `each_frame`)
```

12 番:

```
selftest: ok   the rules can be taken away and given back while the world runs
               (from 20.02 s no meter fell for 5.0 s; 0 did, and afterwards hunger came back)
```

4 番は 5 回とも `closest pair 0.974〜0.985`（0.9 を割ったフレーム 0）。
着手前に落ちた回は 0.860 だった。**5 回では「直った」とは言えない**——S3 の計測でも 24〜26 回に 1 回で、
5 回は元々ほとんど落ちない回数だから。言えるのは「悪くなっていない」まで。

総計行の数（5 回）:

```
hud: the world's rules — 5370〜5372 passes of `each_frame`,
     8,955〜11,629 instructions at the median and 10,669〜13,300 at the most (of 45000);
     the world's tick 1.55〜1.69 ms at the median, 3.50〜3.79 at the 99th frame in a hundred,
     4.51〜5.95 at the most / 8.0 ms
hud: … VM 0.13〜0.26 ms smoothed（生き物 VM。着手前は 0.93〜1.22）
hud: insn/decision — 4283〜4701 passes, 181.7〜218.1 instructions each;
     832〜1196 questions the game answered, 1.000 frames each
```

生き物 VM が着手前（0.93〜1.22 ms）より**軽くなっている**のは、草と腹の書き込みが
生き物 VM の tick の外に出たからではない——それは元から Rust だった。
理由は着手前の数が `vm_clock` の窓の話ではなく、**着手前の測り方は正しかった**ので、
0.9 → 0.2 は素直に受け取れない。**まだ説明できていない**ので、そう書いておく。
考えられるのは、この 5 回の生き物の数（12〜17）が着手前の 3 回（16〜20）より少ないことと、
`insn/decision` の総パス数が 5,172〜5,479 から 4,283〜4,701 に減っていること。
規則が Ruby に移って人口動態がわずかに動いた結果だと**推測**する。確かめていない。

## 7. 置いていくもの・気づいたこと

1. **`"ate"` が W1 では飛ばない。** `eat` が publish していた `"ate"` は `tell`（W2）待ちなので、
   `beetle.rb` / `rabbit.rb` の `on(:ate)` が**この段階では一度も走らない**。
   判定には影響しない（1 番は `Hunger` の上昇で見るようになった）が、
   **`@memory` の中身が変わる**: セーブを開くと、以前はビートルに `meals` と `favorite` が入っていたのが、
   今は `children`（`on(:mate)` が書く）だけになる。実測:

   ```
   Beetle None / Rabbit ['trees'] / Beetle ['children'] / …
   ```

   計画書 §4 が `tell` を W2 に置いているので**指示どおり**だが、1 段階のあいだ
   「ビートルが食事を覚えない」状態になる。`remember_out_loud` の JSON ログも出ない。
2. **草ごとに 1 フレーム 1 口（計画書 §2 既定 5）はそのとおり入れたが、理由はもう無い。**
   既定の根拠は「同じ草を 2 匹が齧ると書きが last-writer-wins になる」。
   `world.rb` は大きさを Ruby の Hash に溜めて**パスの最後に 1 回だけ書く**形にしたので、
   2 匹が齧っても両方の一口が足し込まれる——last-writer-wins は起きない。
   つまり `@bitten` は今や回避策ではなく**規則**（「1 株は 1 フレームに 1 つの口を養う」）で、
   Rust の `eat` は 2 匹が同じ株を齧れた（`for` が順に `plant.size` を減らしていた）ので、
   ここだけ**振る舞いが変わっている**。指示にある既定なので入れたが、著者が外したければ 1 行。
3. **`eat` は「クエリ順で最初に届く株」、`world.rb` は「いちばん近い株」。**
   `garden.within` は近い順に返すので、Ruby 側は `break` で最初に届いたものを取る。
   Rust 側はクエリの走査順（= アーキタイプの順）で、誰の順でもなかった。意味は良くなっているが、**違う**。
4. **餓死の順序は Rust に合わせた。** 計画書 §1 の例は `next Rubevy.despawn(c) if h <= 0` を
   食べる前に置いているが、Rust は `get_hungry` → `eat` → `starve` の順だった——
   メーターが 0 を割った生き物も、足元に草があれば食べて生き返れる。`world.rb` はその順にしてある。
5. **世界 VM の dice は毎回同じ列。** sabiruby の既定の生成器は定数から始まり、`srand` を引数無しで
   呼んでも rubevy は `gc_clock` を渡していないので（`rubevy/src/lib.rs` に `gc_clock` は無い）、
   VM の中から取れる種はどれも走行間で同じ。芽が出る「タイミング」は
   フレーム時間の揺れでしか変わらなくなった（「場所」は今も Rust の `Dice`＝時計種）。
   `world_prelude.rb` の `run_world` にそう書いた。
6. **窓ありビルドは実行していない**（指示どおり）。`F3` も `Brains` の 3 枠目も W3。
   `WorldTrouble` の 1 行は HUD に足したが、**表示は見ていない**。
7. **`answer_world` の `garden.spawn` は W1 では誰も呼ばない。** 計画書 §2 既定 6 に従って
   関数に切り出して両 VM で共用にしてあるが、世界が子を頼むのは W2。
   同じく `starting_to_eat?` は `world_prelude.rb` にあるが `world.rb` は呼んでいない（`tell` 待ち）。
8. **世界 VM の tick は中央 1.6 ms。** 生き物 VM の 0.2 ms の 8 倍で、フレームの約 10%。
   命令数から出る時間（10,000 命令 × 0.19 ms/1000 ≒ 1.9 ms）とほぼ一致するので、
   どこかに無駄があるのではなく**300 回近い往復と 120 回の書き込みの素の値段**。
   速くする余地（`Rubevy.find` が毎フレーム 110 個の `Rubevy::Entity` を作る、
   `garden.within` が毎回 `iter_entities` を舐める）はあるが、W1 の範囲ではないので触っていない。
