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

## 8. 追記: `world.rb` が壊れているときを実際に試した（計画書 §2 既定 9）

指示は「起動時: `panic` ではなく世界の規則が止まったまま走って HUD に 1 行」。2 通り試した。

**構文エラー**（`each_frame` の中に Ruby でない行を入れた版）:

```
the world has no rules: world.rb: world.rb:214:3: syntax error, unexpected 'end', assuming it is closing the parent 'do'..'end' block
10 creatures, 42 plants, day at phase 0.21
hud: the world's rules — 0 passes of `each_frame`, – instructions at the median and 0 at the most (of 45000)
```

**ファイルが無い**:

```
the world has no rules: …/garden/ruby/world.rb: No such file or directory (os error 2)
```

どちらも 8 秒／5 秒走り切って、太陽は回り、生き物は歩き、草は育たず誰も腹が減らない。
`WorldTrouble` が HUD に出す 1 行は窓ビルドにあるが、**窓は実行していない**ので目で見ていない。
（行番号の 214 は prelude と 1 本にコンパイルしているため prelude ぶんのずれが乗っている。
生き物側の `Mind::prelude_lines` に相当するオフセットは世界にはまだ無い——W3 のエディタの仕事。）

なお、規則が無いときに草が 55 ではなく 42 なのは W1 と関係ない: `spawn_world` は木と岩の
1.2 以内には草を置かない（`main.rs` の `for _ in 0..PLANTS_AT_START`）ので、最初から 55 未満になる。

## 9. 追記: `garden.within` 単体の値段

「総当たりは Rust」が W1 の要なので、その Rust がいくらなのかを単体で測った
（閉包の中で `Instant` を取り、`AtomicU64` 2 本に足して終わりに出す使い捨て。`unsafe` は無い）。
60 秒 ×2、生き物 11〜12 匹:

```
probe: garden.within called 38076 times, 3063 ns each on average
probe: garden.within called 38230 times, 3097 ns each on average
```

**1 回 3.06〜3.10 µs、1 フレームあたり 10.6 回**（腹が満ちている個体と眠っている個体は呼ばない）。
つまり `within` は 1 フレームで **約 34 µs** ——世界 VM の tick 1.62 ms の **2%** にすぎない。

これが言っているのは、規則を Ruby にした値段は**空間の問いではない**ということ。
1 回 3 µs は `world.iter_entities()`（headless で約 145 エンティティ）と型レジストリの読みロックと
距離の並べ替えを全部含んだ値で、Ruby で同じことをやれば 1 匹あたり 90 回の読み × 約 75 命令 = 6,750 命令、
24 匹で 1 フレーム 162,000 命令になる（計画書 §3.2 の見積もりの実測版）。
残りの 98% は**往復そのもの**——1 パスで約 300 回の読みと 120 回の書き——で、
そこを軽くしたければ減らすのは問いの数ではなく読み書きの数。W1 の範囲ではないので触っていない。

---

# 2026-09-17 W2 — つがいと子、`every`、`tell`、季節

同じ計画書（`docs/plans/garden-world-plan.md` 第 2 版）の **W2**。W1（`39c60f1`）の続きで、同じ worktree・
同じブランチ。計画書 §6 が「W2 で直すもの」として挙げた 2 点から始めて、`every` と `tell` の道を作り、
最後に `court` / `hatch` を Ruby に移した。

## 10. W1 で残した 2 点を先に直す

### 10.1 `@bitten` を外す — 回避策が規則になっていた

W1 の §7.2 に書いたとおり、`@bitten`（1 株は 1 フレームに 1 つの口しか養わない）は計画書 §2 既定 5 の
「同じ草を 2 匹が齧ると書きが last-writer-wins になる」を避けるためのもので、**`world.rb` が大きさを
Ruby の Hash に溜めてパスの最後に 1 回だけ書く形にした時点で理由が消えていた**。残っていたのは
「振る舞いの違い」だけ——Rust の `eat` は `for` が順に `plant.size` を減らすので 2 匹が同じ株を齧れた。

消したのは 3 行（表の宣言、`next if … bitten[bits]`、`bitten[bits] = true`）。`size[bits]` に 2 回
引き算が入るだけなので、両方の一口がちゃんと足し込まれる。`crumb`（0.02）を割った株は最後に
`Rubevy.despawn` されるので、2 匹が同時に食べ尽くしても despawn は 1 回。

### 10.2 世界 VM の dice を、Rust の `Dice` の種から撒く

W1 の §7.5 の穴。sabiruby の既定の生成器は定数から始まり、VM の中に混ぜられる時計が無い
（`srand` を引数無しで呼ぶと `gc_clock` を混ぜるが、rubevy はそれを設定していない）ので、
**W1 の 90 秒はどの走行でも同じ列**だった——芽が出る「タイミング」が毎回同じ。

走行間で違う唯一の生成器はゲームの `Dice`（種は `platform::clock_seed`）なので、そこから 1 つ振って
`garden.seed` として答え、`run_world` が `srand` する。`answer_in_tick` で登録したのでフレームは食わない
（`garden.within` と同じ道）。

**`Res<Dice>` を読んで、そのコピーを振る**ことにした。`ResMut` で本体を振ると `spawn_world` が引く
乱数が 1 つずつずれる——「種を 1 つ読むだけ」のつもりの行が、庭の配置を静かに書き換えることになる。
コピーは同じ状態から同じ生成器を回すので、出てくる数は誰とも衝突せず、誰の数も動かない。

確かめた（使い捨ての 1 行を `run_world` に入れて 3 回走らせ、そのあと外した）:

```
world: seed 799206, first rolls 0.3751 0.5543
world: seed 947322, first rolls 0.946 0.451
world: seed 866451, first rolls 0.6411 0.3198
```

この 2 点を入れた状態で `--headless 90` の 12 判定は全部通る（4 番 `closest pair 0.929`、
8 番 `9 pairings, 5 children`）。

## 11. 世界が口をきく — `tell` と `every`

### 11.1 `tell` は質問で、答えは待たない

計画書 §5 の罠 1 のとおり、Ruby からは publish できない。生き物は**別の VM** で、キューは壁の向こうに
あり、両端に手が届くのはホストだけだから。なので `tell(who, name, payload)` は
`Rubevy.ask("garden.tell", …)` で、`answer_world` が受けて `ScriptWorld::publish`（生き物 VM）に渡す。
届くものは `startle` や `day_night` が publish するものと区別がつかない——**同じ関数**だから。

`sprout` と同じく**命令にした（`pop` しない）**。待つと、そのフレームのパスが 1 フレーム parked になる
——草も伸びず、腹も減らないフレームが 1 つできる。`pop` で得られるのは「引数がおかしい」という
ゲームの意見で、それは規則を書くときに 1 回直せばよいもので、毎秒 60 回聞くものではない。

payload は `Arg` の 3 つ（数・文字列・Entity）とそれ以外＝nil に畳んだ。これは
`prelude.rb` の `run_handler` がハンドラのブロックに渡せる形そのままで、
生き物側には W2 のための行が 1 つも要らない（`on(:ate)` も `on(:mate)` も既にある形で受け取る）。

### 11.2 `RubevySet::<World>::answer()` を生き物の tick の前に置いた

最初に動かしたとき、`"season"` は届いていたが**次のフレーム**だった。rubevy は VM ごとに 3 つの set を
chain するだけで、2 つの VM の間には順序が無い（`docs/host-api.md`）ので、メッセージを運ぶ system
（`answer_world`）が、運ぶ先の VM の tick より後ろに置かれうる。

```rust
.configure_sets(Update, RubevySet::<World>::answer().before(RubevySet::Tick))
```

の 1 行で、W1 が tick に対して作った順序（世界 → 生き物）が**答えと配達にも**広がる。
13 番の判定が `0.03 s` で通るのはこの行のおかげで、入れる前は最初の `each_frame` の宣言が
ビートルに届くのが 1 フレーム遅かった。

### 11.3 `every` は別タスクの `sleep`、そして**止め方**が要った

`every 60 do |n| … end` は世界 VM のタスクを 1 本作り、そのタスクは `sleep` するだけ。
デルタを足す数え上げも、期限の表も要らない——スケジューラは既に「何もせず寝ているタスク」を
持っていて、それを起こす時計は生き物の `sleep 0.2` を意味あるものにしている時計と同じ。
ポーズ（`P`、読み戻し中）は世界 VM の tick ごと飛ぶので、rubevy は
スケジューラの時計を進めない（`ScriptWorld::budget` の doc）。**残り 11 秒の季節は、
止めて再開しても残り 11 秒。**

**最初の実装は季節が二重に来た。** 90 秒の走行の log:

```
world: the wet season     (t≈0   — 1 本目の world.rb)
world: the wet season     (t≈26  — 12 番の判定が規則を戻した 3 本目)
world: the dry season     (t≈60  — 1 本目のタイマー。規則はとっくに取り上げられている)
world: the dry season     (t≈86  — 3 本目のタイマー)
```

原因は rubevy の `ScriptTask` の remove フック（`stop_removed_task`）が
**スクリプト自身のタスクを terminate して購読を落とすだけ**だから。生き物のハンドラのタスクは
`queue.pop` が `Rubevy::Unsubscribed` を上げるので自然に終わるが、`sleep` しかしないタスクには
終わる理由がどこにも無い。`run_world` の `ensure` は、タスクが terminate されるときには走らない。

直し方は 3 つ考えた:

1. `main.status` を見る → terminate 済みのタスクは `live()` が「task is closed」で raise するので、
   毎周 begin/rescue が要る。タスクの生死を例外で聞くことになる。捨てた。
2. Rust 側で世界 VM のタスクを全部終わらせる → rubevy か、ゲームが `Task.list` を舐めることになる。
   生き物側の作法（購読が切れたら終わる）と別の仕組みが 2 つ目にできる。捨てた。
3. **`$world_being`**。VM の中の全タスクはグローバルを共有するので、差し替えられた `world.rb` の
   タイマーが「自分はもう差し替えられた」と知れる唯一の場所がそこにある。**これにした**:
   `run_world` が `$world_being = being` と書き、タイマーは目覚めるたびに
   `break unless $world_being.equal?(being)`。1 行ずつで、例外を使わず、
   「この規則のタイマー」という所有関係をそのまま言っている。

入れたあと、同じ 90 秒で `dry` は 1 回だけ（3 本目のぶん）。

### 11.4 `"ate"` が戻った（W1 §7.1 の穴）

`world.rb` の一口の直後に `tell c, "ate", size[bits] if starting_to_eat?(c)`。
payload は **残った草の大きさ**で、Rust の `eat` が publish していたものと同じ
（`plant.size -= bite` の**あと**の値。1 フレームの一口はいつも同じ数で何も言わないが、
座り込んだ株の大きさは script が覚えたくなる数）。`starting_to_eat?` は W1 で書いて誰も呼んでいなかった
メソッドで、W2 の呼び手はこの 1 行。

セーブの往復で `@memory` が戻ったことを確かめた（20 秒 → 読み戻し → 保存、バイト一致）:

```
Rabbit ['meals','season','trees'] / Beetle ['meals','season'] / Beetle ['favorite','meals','season'] …
```

W1 では `children` しか無かった（`docs/worklog` の §7.1）。`meals` と `favorite` が戻り、`season` が増えた。

### 11.5 季節: 芽の確率だけを動かす

`sprout_rate` は `world.rb` で**唯一 `const` 由来でない数**になった。`SPROUT_RATE` は年中 0.7 で、
乾季 0.5 / 雨季 0.9 は**同梱版の遊びの設定**（計画書 §4 の言い方で「元の 0.7 を中心に」）。
0.7 から同じ距離に置いたので 1 年ならすと W1 が予算を測った世界と同じ量の草になり、
季節が変えるのは「いつ生えるか」であって「最後に何本あるか」ではない。この根拠は `world.rb` の
コメントに 4 行で書いた。

`every 60` は `day_length 60.0` と同じ数——**1 季節 = 1 日**。`n` は 1 から始まるので最初の転換は
乾季で、世界は雨季で開く（`each_frame` の頭の `turn_to("wet") if @season.nil?` 1 行。
`start do … end` を DSL に足すほどのものではない）。

### 11.6 ハンドラの枠を 6 → 7

`prelude.rb` の `ON_SLOTS` はブロックの名前を `run_handler` に書き出す都合で決まっている定数で、
ビートルは既に 6 個（night / day / touched / bumped / ate / mate）使い切っていた。`on(:season)` で 7 個目。
`when 6 then __handler_6(*args)` を 1 行足して 7 にした。測った上限ではなく
「このリポジトリの生き物が要る数」なので、そう書いてある。

### 11.7 世界の時計（`garden.now`）

W2 の繁殖はクールダウンを**コンポーネントに書く**（§12）ので、スクリプトより長生きする時計が要る。
`@elapsed` を自分で足すと Ctrl+Enter のたびに 0 に戻り、既に書いたクールダウンだけが古い時計に
残る。`answer_in_tick` の `garden.now` は `world_now(&time, &sky)` そのもの——`court` と `hatch` が
読んでいたのと同じ、`P` で止まりセーブで続く庭の時計（G9）。フレームは食わない。

## 12. つがいと子 — `Breeding` をどちら向きに見せるか

計画書 §2 の既定 5 は「`Breeding` を `Reflect` + `register_type`」、指示は「`Reflect` する／Ruby の変数に
持ち替える、どちらが素直かを**実物で**判断して理由を書く」。実物を読んで決めた結果は **`Reflect`**。
理由は 3 つあって、どれも「クールダウンは子が生まれた時点で親に課す」という性質から出ている。

1. **規則より長生きする。** クールダウンは 20 秒、`world.rb` は Ctrl+Enter で差し替わる（12 番の判定が
   実際にやっている）。世界オブジェクトの `@ready_at` に持つと、**編集のたびに全員の待ち時間が赦される**。
   コンポーネントなら残る。
2. **生き物と一緒に死ぬ。** entity の bits を鍵にした Ruby の Hash は、餓死した相手の分を誰かが
   掃除しないと増え続ける。コンポーネントは entity が消えれば消える。
3. **`partner` を Rust が読めないと 8 番の判定が書けない。** `court` が消えた時点で、
   「誰と誰がつがいだったか」を知っているのは Ruby だけになる。子の遺伝子を検査する `judge_child` は
   Rust にあり、両親の遺伝子が要る。`garden.spawn` の要求が運んでくるのは**訊いた生き物**だけなので、
   もう一方は「訊いた生き物が持っている `Breeding.partner`」から取るしかない（§12.3）。

### 12.1 `partner` は `Option<Entity>` にできない

最初は `Option<Entity>` のままにするつもりだった。読む方は問題ない——rubevy の reflect は
unit variant を `:None`、tuple variant を `{Some: [<Entity>]}` にする（`src/reflect.rs`、
`enum_to_ruby`）。**書く方が通らない**: `apply_enum` は「Hash でフィールドを書けるのは、値が既に
その variant のときだけ」なので、`None` の値に `{Some: [e]}` を当てても
`the fields of Some cannot be written while the value is None` になる。Ruby が書くフィールドは
`Option` にできない。

なので `partner: Entity` にして、「誰も居ない」は `Entity::PLACEHOLDER`。読み側（Ruby）から見ると
**存在しない entity** なので、そのコンポーネントを読むとどれも `nil` が返る——
規則が欲しい答えそのもの。この理由は `Breeding` のコメントに 5 行で書いた。

### 12.2 新しく生まれた子を、規則はどうやって見つけるか

子は**生き物の VM**が頼む（`on(:mate)` → `garden.spawn`。G2 の「規則は世界、算術は生き物」は
そのまま）。だから世界の VM は「子が生まれた」ことを自力で知る必要がある。使ったのは既にある 2 つの事実:

* `Creature.age` を進めているのは**この規則自身**なので、`age == 0.0` は「どのパスもまだ触っていない」。
* 誰が頼んだかは `Birth.parent`（= `request.entity`）として Rust が既に持っている。

そこで `Creature` に `parent: Option<Entity>` を 1 つ足した（**Ruby は読むだけ**なので `Option` で
よい）。世界の最初の 10 匹とセーブから戻った生き物は `None`。規則の側は

```ruby
born = body[:parent]
if body[:age] == 0.0 && born.is_a?(Hash)
```

の 2 行で新生児を見つける。`body` はどのみち毎フレーム読んでいる `c[:Creature]` なので、
**問いも往復も増えない**。

捨てた案: 新生児に印のコンポーネントを付けて Rust が 1 フレーム後に外す（Ruby はコンポーネントを
外せないので Rust に後始末の system が 1 本増える）。位置から親を推定する（`spawn` は親の
1.2 隣に置くので当たるが、根拠のない半径が 1 つ増える）。どちらも `age == 0` より弱い。

### 12.3 代償は「次のパス」で引く

`hatch` は子が生まれたフレームに親の `Hunger` を直接引いていた。Ruby で同じことをすると
**同じフレームに同じコンポーネントへ 2 回書く**ことになる（親の番が来れば親自身の腹減りを書く）ので、
後から書いた方だけが残り、順番はクエリ任せ——計画書 §5 の罠 5 そのもの。

なので「子を見たパスで借用を表に書き、**次のパス**でその親の番が来たときに `h` から引く」。
`owed` / `@owed` の 2 枚を毎パス入れ替えるので、間に餓死した親の借用は 1 パスで忘れられる。
Rust より 1/60 秒遅いだけで、順序に依存しない。

### 12.4 `child_hunger` と `pop_max` だけは Rust に渡る

規則の数はすべて `world.rb` にある（`mate_hunger` 75 / `mate_reach` 2.0 / `court_retry` 2.0 /
`mate_cost` 30 / `mate_cooldown` 20 / `child_hunger` 50 / `pop_max` 24、どれも `const` と同じ値）。
このうち 2 つは**ゲームが体を作るのに要る**: 生まれた瞬間のメーターと、そもそも何匹まで作るか。
`day_length`（太陽は Rust が描く）と同じ理由・同じ道で `garden.rules` に乗せた
（`RuleBook` が 1 つから 3 つになった）。置き場所は既定 7 の精神に従って**使う場所**——
`Births` リソース（子を作る側）で、`Rules` リソースは作らなかった。

`CHILD_HUNGER` と `POP_MAX` の `const` は `DAY_LENGTH` と同じ立場で残る:
**`world.rb` が何も言わないとき（そしてコンパイルできないとき）の既定**。

最初は「子を `Hunger(0)` で作って、世界が最初のパスで 50 を書く」案だった。捨てた理由は 2 つ:
`note_the_rules` が「メーターが上がった＝食事」を見ているので**誕生が食事に見える**（`Eating` の
咀嚼アニメも付く）し、その 1 フレームの間にセーブが走ると空のメーターがファイルに入る。
「規則の数をぜんぶ Ruby に」を優先して、判定と往復に嘘を持ち込むのは割に合わない。

### 12.5 8 番の判定は `garden.spawn` の側に移った

`court` が書いていた `test.matings`（誰と誰、遺伝子つき）は、`answer_spawn` が
**訊いた生き物とその `Breeding.partner`** から作るようになった。判定の意味は同じで、
出どころが「規則が報告した」から「世界を見て分かった」に変わった——W1 で `ate_at` と `starved` に
起きたのと同じことが、繁殖にも起きた。判定は `judge_child` ごとそのまま通っている。

### 12.6 12 番の判定から `paid` が消えた

W1 §3.1 の一番の手間は「凍った規則の間に `hatch` が親から 30 引くので、メーターが下がる」だった。
**繁殖が規則になった今、凍っている間は誰も引かない**ので、`SelfTest::paid` と
`note_the_rules` の `.after(hatch)` を両方消せた。W1 で足した仕掛けが、W2 の引っ越しで
不要になったことになる。境界が正しい場所に来た証拠として記録しておく。

### 12.7 Rust と違うところ（記録）

1. `court` は `starve` の**前**に走っていた（`get_hungry` → `eat` → `startle` → `court` → `starve`）。
   Ruby では腹減り・食事・餓死のループの**あと**につがいを探す。75 以上の生き物が同じフレームに
   餓死することはないので、意味は変わらない。
2. `hatch` は代償のあと `breeding.partner = None` に戻していた。Ruby は戻さない（`partner` は
   「最後に告げられた相手」で、次に告げられたときに上書きされる）。子を頼めるのは告げられた 1 回だけなので
   振る舞いは変わらない。
3. 新生児は最初の 1 パスだけ腹が減らない（`age == 0` の枝で `next` する）。1/60 秒。
4. `answer_spawn` の人口の上限は `POP_MAX` の `const` ではなく `world.rb` が渡した数を見る。
   規則（告げる前に数える）と作る側（頼まれても作らない）が同じ 1 つの数を指すのは前と同じ。

## 13. 通した確認

| 何を | 結果 |
|---|---|
| `cargo build --release -p garden -p sabibots` | 通る |
| `cargo build -p garden -p sabibots`（窓あり、dev。**実行はしていない**） | 通る |
| `cargo clippy -p garden` | **13 件。W1 と同じ数**（`hatch` が 10 引数で 1 件出していたのが `children_arrive` の 9 引数に替わっただけ） |
| garden `--headless 90` ×10 | **128 ok / 2 FAIL**（§13.1） |
| sabibots `--headless 25` ×1 | **38 ok、FAIL 0**。無変更 |
| セーブの往復（`--headless 25 --save` → `--headless 0 --load --save`） | **バイト一致**。`@memory` は `meals` / `favorite` / `season`（W1 は `children` だけだった） |
| 走行中のロード（`GARDEN_RELOAD_AT=10`、F9 の道） | **バイト一致**（`Creature` に `parent` が増えた道を通る。`load_world` は読み戻した生き物に `parent: None` を入れるので、新生児の枝は踏まない） |

### 13.1 落ちた 2 回

**どちらも閾値は動かしていない。** 計画書 §5 の罠 4 の指示どおり、証拠を出して記録する。

**5 番（走行 2 回目）** — 探針のビートルが自分の草に着かなかった:

```
selftest: FAIL a hungry creature with a plant in sight reached it (it started 5.0 away and got no closer than 2.3)
```

これは**着手前から揺れている判定**で、S3 の記録（`docs/worklog/2026-09-17-sync-reads.md`）が
「順序を入れてから 3/26」と書いたもの。10 回で 1 回なので、その率と区別がつかない。
W2 が率を上げうる変更を 1 つだけしている——雨季の芽の確率 0.9（元は年中 0.7）は、
探針の歩く 1.8 秒のあいだに近くへ新しい芽が出る見込みをその比だけ上げる。
「上げていない」とは言えないので、そう書いておく（乾季の 0.5 が同じだけ下げる）。

**7 番（走行 7 回目）** — 夜の 1 秒後に 1 匹だけ動いていた:

```
selftest: FAIL the creatures were asleep a second after night fell (16 of them, fastest 2.000 at 26.21 s)
```

同じ log にその理由がある:

```
a Beetle was born at 25.2 s (140v0) — speed 2.31, sight 8.2, appetite 0.99
```

**夜が落ちた 25.21 秒に生まれた子**。生まれた生き物の `Script` がタスクになり、そのタスクが
`on(:night)` を購読するまでに 1〜2 フレームかかるので、その瞬間に publish された `"night"` は
誰も聞いていない——`NEWBORN_GRACE`（`main.rs`）が 6 番の判定について書いているのと同じ穴で、
7 番の `watch_sleep` には猶予が無い。速さ 2.000 は `Creature::CRUISE` そのもので、
「起きている」ではなく「寝ろと言われていない」。

**W2 のせいではない**（G2 以来ありうる。繁殖が Rust だったときも子は同じように生まれた）が、
W2 で初めて観測されたので記録する。直すとしたら `watch_sleep` に `NEWBORN_GRACE` を入れる話で、
それは判定を変えることなので**やっていない**。著者の判断待ち。

### 13.3 世界 VM の値段 — つがいが乗った分

`--headless 90` ×10 の総計行から（人口は 13〜18 匹、草 37〜56 株で W1 の 12〜17 匹と同程度）:

| | 1 パスの命令数 中央 | 最大 | tick 中央 |
|---|---|---|---|
| W1（5 回） | 8,955〜11,629 | 10,669〜13,300 | 1.55〜1.69 ms |
| W2（10 回） | 12,188〜14,478 | 14,134〜19,390 | 1.83〜2.07 ms |

**中央で +25〜30%、最大で +35〜45%。** 乗ったのは、生き物ごとの
「腹が減ったら `Breeding` と `Transform` をもう 2 回読む」と、パスの終わりのつがい探しと、
新生児の枝。予算 45,000 に対して最大 19,390 なので今の箱庭では半分以下。

ただし W1 が予算を決めた根拠は**上限の箱庭（90 株・24 匹）で測った 20,128** で、その数は
W2 では測り直していない。観測された倍率（最大 ×1.46）をそのまま掛けると上限で約 29,000——
45,000 の下ではあるが、「上限の 2 倍の箱庭でも届かない」という W1 の言い方は W2 では
そのままでは言えない。**外挿であって測定ではない**ので、予算は動かさず、
測り直すかどうかは著者の判断として残す。
