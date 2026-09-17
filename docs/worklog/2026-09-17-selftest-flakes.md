# 2026-09-17 箱庭の 3 つの揺れる判定 — 頻度を測り、原因を実測で突き止める（調査のみ）

`GARDEN_SELFTEST=1 ./target/release/garden --headless 90` が時々落とす 3 つの判定
（4 番「何も何かを突き抜けていない」、5 番「空腹の個体が視界内の草に着いた」、
7 番「夜の 1 秒後に全員が寝ている」）について、**頻度を測り、原因を実測で特定する**ための調査。
コードは直していない。計測のために入れた仕掛けは全部戻してあり、`git status --short` は
この worklog と `docs/README.md` の 1 行以外に何も出さない。

経緯は `docs/plans/garden-world-plan.md` §5・§6、`docs/worklog/2026-09-17-sync-reads.md` §8〜§9、
`docs/worklog/2026-09-17-garden-world.md` §13.1。

作業場所は worktree `rubevy_games-wt-flakes`（ブランチ `selftest-flakes`、main `9c12a91` から）。

---

## 0. 測り方 — 1 回 94 秒の判定を 98 回

`--headless 90` は実時間で走る（90 秒のシミュレーションに壁時計で 94 秒）。
1 回の CPU 使用は 24 コアのうち 0.22 コア（`time` の user 13.3 s + sys 7.9 s / real 94.3 s）なので、
**8 本並列**で回した。世界の種は `Dice(platform::clock_seed())`（`main.rs:1599`）で走行ごとに違うから、
並列でも 8 本は 8 つの別の箱庭になる。並列が壊しうるのは「フレームの刻み」だけで、
そこは後で単独走行と突き合わせた（§2 の壁クランプは単独走行でも同じ形で出る）。

`another_version_file()` は `std::env::temp_dir()`（`garden/src/platform.rs:102`）なので、
並列の 8 本が同じ `garden-from-another-version.json` を書かないよう、走行ごとに `TMPDIR` を分けた。

```bash
for i in $(seq -w 01 50); do
  d=$(mktemp -d)
  TMPDIR=$d GARDEN_SELFTEST=1 ./target/release/garden --headless 90 > base/run$i.log 2>&1 &
  ...8 本ごとに wait
done
```

### 0.1 素の 50 回

```
run06.log a hungry creature with a plant in sight reached it (it started 5.0 away and got no closer than 1.8)
run09.log nothing walked through anything over 5385 frames (closest pair 0.894 of the radii, 4 frames under 0.9)
run38.log a beetle touched by a rabbit changed heading within 0.5 s (37/38)
run50.log nothing walked through anything over 5381 frames (closest pair 0.854 of the radii, 1 frames under 0.9)
---total runs: 50
```

### 0.2 計測を入れた 48 回

§1 の仕掛けを入れた版で 48 回（下の「原因」は全部この 48 回から出ている）:

```
run04.log a hungry creature with a plant in sight reached it (it started 5.0 away and got no closer than 2.3)
run16.log a hungry creature with a plant in sight reached it (... no closer than 2.2)
        | a child was born whose genome is its parents' mixed and mutated (none was, from 0 pairings)
run31.log nothing walked through anything over 5385 frames (closest pair 0.899 of the radii, 1 frames under 0.9)
run34.log a hungry creature with a plant in sight reached it (it started 5.0 away and got no closer than 1.8)
run36.log nothing walked through anything over 5387 frames (closest pair 0.894 of the radii, 1 frames under 0.9)
```

### 0.3 頻度（合計 98 回）

| 判定 | 素の 50 回 | 計測入り 48 回 | 合計 | 率 |
|---|---:|---:|---:|---|
| 4 番 突き抜け | 2 | 2 | **4 / 98** | 4.1%（1/25） |
| 5 番 プローブ | 1 | 3 | **4 / 98** | 4.1%（1/25） |
| 6 番 向き | 1 | 0 | 1 / 98 | 1.0% |
| 7 番 睡眠 | 0 | 0 | **0 / 98** | — |
| 8 番 子の遺伝子 | 0 | 1 | 1 / 98 | 1.0%（つがいが 90 秒で 0 回。既知でない揺れ、§5.4） |

4 番の 4.1% は S3 の記録（1/24〜1/26）とぴたり合う。
5 番は S3 が「順序を入れてから 26 回に 3 回（11.5%）」と書いたが、ここでは 98 回で 4 回（4.1%）。
**7 番は 98 回で 1 回も落ちていない。** それでも原因は取れた（§4：落ちる条件を人の手で作れる）。

---

## 1. 入れた計測（全部戻した）

`git checkout` で戻すので、置いた印はコメント `// FLAKEPROBE (temporary)` だけにした。入れたのは 5 つ:

1. **`separate` の押し戻しの履歴**（`main.rs:2928`）: パスごとの位置を控え、
   書き戻しの**壁クランプの前と後**それぞれで「半径の和に対する距離の比」を出す。
   比が閾値を割った組について、始まりの比・4 パスそれぞれの比・クランプ前・クランプ後・
   両者の座標・壁の外に出ているか・半径の和 +0.6 以内に何体いるか、を 1 行で印字。
2. **`watch_overlap`**（`main.rs:3034`）: 0.9 を割った組の entity と座標を印字。
3. **プローブの毎フレームの姿**（`watch_probe`、`main.rs:4837`）: 20 秒まで毎フレーム、
   位置・速度・空腹・自分の皿までの距離と皿の大きさ・視界 8 以内の草 3 株・6 以内の他の生き物。
4. **`garden.nearest` の答え**（`answer_garden`）: 質問したのが `Probe` のときだけ、答えと距離。
5. **新生児の購読**: `run_creature` の `start_handlers` 直後に Ruby から 1 行、
   `day_night` の publish の瞬間に `Mind` を持つ全員と年齢、`watch_sleep` で起きていた個体。
   さらに `GARDEN_TESTBIRTH=t1,t2,…` で、指定した時刻に生き物を 1 体ずつ湧かせる仕掛け（§4.2）。

---

## 2. 4 番「何も何かを突き抜けていない」 — **壁のクランプが、解けた重なりを作り直している**

### 2.1 仮説

`separate`（`main.rs:2928`）は 4 パス（`SEPARATE_PASSES`）で円を押し離したあと、
書き戻しでこうする（`main.rs:3001`、コメントは元からある）:

```rust
// back into the world, and back inside the walls: a push can put a creature through one
transform.translation.x = at[i].x.clamp(-HALF_W + 0.5, HALF_W - 0.5);
```

`watch_overlap` は `after(separate)` なので**クランプ後**を見る。
とすると「パスは押し離したのに、壁の外に出た方を引き戻したせいでまた重なる」が起こりうる。
落ちるときの比 0.854〜0.899 は、半径の和 0.8（カブトムシ 2 匹）に対して 0.08〜0.12 の食い込み。
壁際で 0.1 くらい引き戻されるのはありうる幅なので、まず数を出して確かめた。

### 2.2 実測 — 落ちた 2 回、どちらも同じ形

```
flakeprobe overlap: 168v0 (mover, r 0.40) vs 96v0 (mover, r 0.40)
  — start 0.724, passes [1.000 1.000 1.000 1.000], preclamp 1.000, postclamp 0.899;
  at (18.90,-3.99)/(19.60,-4.39) clamped (18.90,-3.99)/(19.50,-4.39); wall false/true; crowd 2/2
flakeprobe watch: t 83.526 168v0 (creature) vs 96v0 (creature) ratio 0.899 at (18.90,-3.99)/(19.50,-4.39)
selftest: FAIL nothing walked through anything over 5385 frames (closest pair 0.899 of the radii, 1 frames under 0.9)
```

```
flakeprobe overlap: 140v0 (mover, r 0.40) vs 107v0 (mover, r 0.40)
  — start 0.699, passes [1.014 1.014 1.014 1.014], preclamp 1.014, postclamp 0.894;
  at (18.87,14.04)/(19.61,13.69) clamped (18.87,14.04)/(19.50,13.69); wall false/true; crowd 2/3
```

読み方:

* **4 パスは完璧に仕事をしている。** `passes [1.000 1.000 1.000 1.000]` — 1 パス目で
  ちょうど「触れている」まで離し、残り 3 パスは何もしていない。`preclamp` も同じ。
  「4 パスで押し戻し切れないフレームが 1 つある」という当初の見立て（計画書 §5）は**外れ**だった。
* 壊しているのは**クランプ**。押された側（`96v0` / `107v0`）が `x = 19.60` / `19.61` と
  壁の外（上限 `HALF_W - 0.5 = 19.5`）に出て、書き戻しで 19.50 に引き戻される。
  失う 0.10〜0.11 がそのまま食い込みになり、0.8 で割って 0.894〜0.899。
* 始まりの比 `start 0.724 / 0.699` — このフレームの頭で 2 匹は 0.22〜0.24 深く重なっていた。
  `separate` は 2 体で半分ずつ持つので片方が 0.11〜0.12 動き、壁から 0.11 以内にいた方が外に出る。
  **深い重なり + 壁際**の 2 条件が揃ったときだけ起きる。
* `crowd 2/2` `2/3` — 団子ではない。3 体目は関係していない。

### 2.3 近傍も全部同じ形（21 件 / 21 件）

尾を見るために閾値を 0.9 から 0.99 に下げて 1 回（90 秒・単独走行、`closest pair 0.977`）回した。
出た 21 件の**全部**が同じ形:

```
flakeprobe overlap: 106v0 (mover, r 0.40) vs 113v0 (mover, r 0.50) — start 0.972,
  passes [1.000 1.000 1.000 1.000], preclamp 1.000, postclamp 0.989;
  at (19.51,-0.58)/(18.72,-1.01) clamped (19.50,-0.58)/(18.72,-1.01); wall true/false; crowd 1/1
flakeprobe overlap: 108v0 (mover, r 0.40) vs 111v0 (mover, r 0.50) — start 0.950,
  passes [1.000 1.000 1.000 1.000], preclamp 1.000, postclamp 0.981;
  at (7.86,-14.52)/(8.29,-13.73) clamped (7.86,-14.50)/(8.29,-13.73); wall true/false; crowd 1/1
（以下 19 件、すべて preclamp 1.000 / どちらかが wall true）
```

**`preclamp` が 1 を割った例は 1 件もない。** 閾値 0.95 で 1 回回したときは 0 件で、
0.99 で 21 件。つまり「クランプが作る食い込み」は連続な分布で、0.9 を割るのはその尾。
比 0.894 と比 0.989 は程度の違いでしかなく、**機構は 1 つ**。

### 2.4 分かったこと（確認済み）

> **4 番が落ちる原因は `separate` の壁クランプ。**
> 深く重なった 2 匹を 4 パスが正しく押し離し、押された方が壁の外に出て、
> 書き戻しの `clamp` がその分を相手の中へ戻す。判定は `after(separate)` なのでそれを見る。
> 観測 23 件（落ちた 2 件 + 近傍 21 件）すべてがこの形。団子でも新生児でも草の消滅でもない。

---

## 3. 5 番「空腹の個体が視界内の草に着いた」 — **ウサギがプローブの隅に入ってくる**

### 3.1 仕込みの前提

`spawn_world`（`main.rs:2076`）はプローブを `(17, 12)`、皿を `(13, 9)` に置く。
`clear_of_fixtures` は**起動時の**木・岩・草・最初の 10 匹を `probe_at` から 7.0 以上離すだけで、
**走り出した後の生き物を隅から締め出す仕掛けは無い**。

### 3.2 実測 — 落ちた 3 回、どれも同じ形

**run04（`no closer than 2.3`）**。`folk` は 6.0 以内の他の生き物:

```
t 1.361 at (14.94,10.45) v (-1.60,-1.20) hunger 37.83 dinner 108v0 d 2.425 size 0.660 ... folk 106v0:1.53
t 1.395 at (14.89,10.41) v (-1.60,-1.20) hunger 37.78 dinner 108v0 d 2.358 size 0.629 ... folk 106v0:1.37
t 1.411 at (14.86,10.39) v (9.00,-0.27) hunger 37.75 dinner 108v0 d 2.324 size 0.613 ... folk 106v0:1.30
t 1.428 at (14.90,10.39) v (2.20,-0.07) hunger 37.73 dinner 108v0 d 2.353 size 0.597 ... folk 106v0:1.27
```

`t 1.411` の `v (9.00,-0.27)` が `Creature::DASH`、つまり `on(:touched)` の `flee_from` そのもの。
`106v0` が `TOUCH_REACH = 1.3` まで寄った（1.37 → 1.30）フレームで `startle` が `"touched"` を publish し、
ビートルは皿と反対へ全速力で走った。次のフレームの 2.20 は `move_creatures` が
自分の `genome.speed` まで落とした同じ向き。**最接近 2.324 がこの瞬間の値**で、
判定が印字する 2.3 はこれ。`"touched"` はウサギ → カブトムシしか出ない（`startle`、`main.rs:3326`）ので、
`106v0` はウサギと確定する。

そのあとは戻って来ない。理由も log に出ている——ウサギが隅に居座り、
1.4 秒・2.0 秒・3.4 秒・4.1 秒…とくり返し触る。触られるたびハンドラが 0.5 秒舵を握り
（`beetle.rb:33` の `sleep 0.5`）、`run` は `busy?` で 0.1 秒寝るだけなので、
プローブは自分で歩ける時間をほとんど持てない。空腹は 37 → 8 まで落ちて、
最後は別の株（`93v0`）を食べた。**皿に着かなかったのであって、飢えたのではない。**

**run34（`1.8`）** も同じ:

```
t 1.613 at (14.54,10.15) v (-1.60,-1.20) hunger 37.43 dinner 116v0 d 1.924 ... folk 113v0:1.53
t 1.680 at (14.50,10.09) v (2.17,-0.37) hunger 37.33 dinner 116v0 d 1.849 ... folk 113v0:1.26
```

`113v0` が 1.26 まで寄ったフレームで速さが 2.17（= DASH をクランプした値、CRUISE の 2.0 ではない）に変わり、
距離 1.849 で折り返す。判定の「着いた」は `REACH + 0.5 = 1.6` なので、**0.25 だけ足りない**。

**run16（`2.2`）** はウサギが皿を先に食べる形も混ざる。皿 `104v0` は 1.400 から 0.25 まで齧られ
（毎秒 0.94 = `eat_rate 1.0` − 成長 0.06）、そのウサギ `102v0` が 2.68 まで寄ったところで同じ `"touched"`。

### 3.3 分かったこと（確認済み）

> **5 番が落ちる原因は、走り出した後のウサギがプローブの隅に入ってくること。**
> `TOUCH_REACH` まで寄られた瞬間に `on(:touched)` が全速力で皿と反対へ舵を切り、
> そのフレームの距離（1.8〜2.3）が `probe_closest` として残る。
> ウサギが居座ると 0.5 秒ごとに触られ続け、プローブは二度と皿に近づけない。
> 観測 3 件 / 3 件がこの形。`garden.nearest` は毎回正しく皿を答えており（log に全部残っている）、
> `Sight`・`wander`・草の消滅・つがい・`Rubevy.find` の順序は**どれも関係なかった**。

「通るときは毎回ちょうど 1.78 秒」も同じ話の裏側で、5.0 − 1.6 = 3.4 を CRUISE 2.0 で
割ると 1.70 秒。邪魔が入らなければ直線で歩くので毎回同じ値になり、邪魔が入ると
そのフレームで止まる。**途中の値が無いのは当たり前**で、連続な失敗の仕方が無い。

### 3.4 「順序を入れてから出るようになった」は、この数では言えない

S3 は `.before(RubevySet::Tick)` を入れる前 24 回で 0 回、入れて 26 回で 3 回と記録している。
Fisher の正確確率検定（両側）で **p = 0.236**、有意ではない。
ここで測った 98 回は 4 回（4.1%）で、0/24 とも 3/26 とも矛盾しない——
率が 4.1% なら 24 回で 0 回になる確率は 37%、26 回で 3 回以上になる確率は 8.8%。

機構の側からも繋がりが見えない。5 番が落ちるかどうかは「ウサギがあの隅へ歩いていくか」で決まり、
それは走行ごとに違う種から出る乱数の話で、規則の chain をどこに置くかとは無関係。
**順序のせいだという根拠は無い**と書いておく。

---

## 4. 7 番「夜の 1 秒後に全員が寝ている」 — **生まれた生き物は 2 フレーム耳が無い**

98 回で 1 回も落ちなかったので、落ちる条件を人の手で作って確かめた。

### 4.1 仕組み（読んで分かること）

* 体は `children_arrive`（`main.rs:3416`、`.after(RubevySet::Answer)`）が作り、`give_mind` が `Script` を付ける。
* その `Script` がタスクになるのは次のフレームの `RubevySet::Tick`。タスクは `run_creature`
  （`prelude.rb:410`）を走らせ、その中の `start_handlers`（同 `:454`）が `Rubevy.subscribe` を呼ぶ。
* `"night"` を publish するのは `day_night` で、これはフレームの**頭**（`Tick` より前）。
* rubevy の `publish_value` は「その時点で購読している queue」にしか積まない（`lib.rs:1093`）。

つまり **N フレーム目に生まれた生き物は、N と N+1 の publish を聞けない**。

### 4.2 実測 — 夜のまわりに 1 体ずつ生ませる

`GARDEN_TESTBIRTH` で、夜が落ちるあたりに 1 フレームずつずらして 8 体湧かせた:

```
$ GARDEN_SELFTEST=1 GARDEN_TESTBIRTH=25.10,25.12,25.14,25.16,25.18,25.20,25.22,25.24 \
    ./target/release/garden --headless 30
flakeprobe testbirth: asked for 25.100, spawned at elapsed 25.1023 (135v0)
flakeprobe testbirth: asked for 25.120, spawned at elapsed 25.1357 (136v0)
flakeprobe testbirth: asked for 25.140, spawned at elapsed 25.1527 (137v0)
flakeprobe testbirth: asked for 25.160, spawned at elapsed 25.1693 (138v0)
flakeprobe testbirth: asked for 25.180, spawned at elapsed 25.1860 (139v0)
flakeprobe publish: "night" at elapsed 25.203 to 19 minds: 139v0:0.000 135v0:0.084 136v0:0.050 …
flakeprobe testbirth: asked for 25.200, spawned at elapsed 25.2027 (140v0)
flakeprobe testbirth: asked for 25.220, spawned at elapsed 25.2362 (141v0)
flakeprobe testbirth: asked for 25.240, spawned at elapsed 25.2529 (142v0)
flakeprobe awake: t 26.206 139v0 age 1.020 speed 2.000 (born about 25.186)
flakeprobe awake: t 26.206 140v0 age 1.003 speed 2.000 (born about 25.203)
flakeprobe awake: t 26.206 141v0 age 0.970 speed 2.000 (born about 25.236)
flakeprobe awake: t 26.206 142v0 age 0.953 speed 2.000 (born about 25.253)
selftest: FAIL the creatures were asleep a second after night fell (22 of them, fastest 2.000 at 26.21 s)
```

フレームは 16.7 ms。publish は 25.203。

| 生まれた時刻 | publish との差 | 夜を聞いたか |
|---|---|---|
| 25.1023, 25.1357, 25.1527 | 3〜6 フレーム前 | **聞いた**（寝ている） |
| 25.1693 | 2 フレーム前 | **聞いた** |
| 25.1860 | **1 フレーム前** | **聞いていない**（速さ 2.000） |
| 25.2027 | 同じフレーム | 聞いていない |
| 25.2362, 25.2529 | 後 | そもそも宛先にいない |

`publish` の行が「19 minds」に `139v0:0.000` を数えているのが決め手で、
**体は publish の宛先を数える側にはもう居るのに、購読はまだ無い**。
速さ 2.000 は `Creature::CRUISE` そのもので、「起きている」ではなく「寝ろと言われていない」
（W2 の worklog §13.1 の見立てどおり）。

### 4.3 では、なぜ 98 回で 1 回も落ちないのか — 判定 12 番が作る出産の山

夜のまわりで生まれる子は運任せではない。98 回の log を機械で読むと:

```
runs 98
night times: {25.2: 59, 25.21: 28, 25.22: 11}
thaw times:  {25.03: 42, 25.04: 39, 25.05: 9, 25.02: 8}
births in night-1.0 .. night+1.05: 14  →  すべて 25.1
fatal-window births (>= night-0.02): 0
births/run: 4.22
```

12 番の判定（`swap_the_rules`、`main.rs:3274`）が `FREEZE_AT = 20.0` から
`FREEZE_WINDOW = 5.0` 秒だけ規則を「何もしない `each_frame`」に差し替える。
その間つがいは 1 組も成立しないので、**規則が戻る 25.03 秒に溜まった分が一斉に成立し、
子が 25.1 秒に生まれる**。夜は 25.20〜25.22。つまり出産の山は夜のすぐ手前に固定されている。

壁時計で測った両者の間隔（14 件）:

```
base/run35.log  born  98.2 ms before the night publish  (5.9 frames)
inst/run13.log  born  98.4 ms …                          (5.9 frames)
… （6 件が 5.9 フレーム、8 件が 6.9 フレーム）…
base/run04.log  born 115.9 ms before the night publish  (6.9 frames)
n 14 min 98.2 max 115.9 median 114.0
```

**余裕は 5 フレーム。** 落ちる側の境界は 1 フレームなので、
規則が戻ってから子が生まれるまでの道（差し替え → 次のパスでつがい → `tell "mate"` →
ハンドラが `garden.spawn` → 答え → `children_arrive`）が 5 フレーム延びれば落ちる。
W2 の worklog が記録した 1 例（`a Beetle was born at 25.2 s`、夜は 25.21）はまさにそれで、
**同じ機構の、余裕を使い切った回**。この 98 回では使い切らなかっただけ。

### 4.4 同じ穴を踏む判定が他にないか

* **6 番**（`watch_turning`）: `startle` が `creature.age > NEWBORN_GRACE`（2.0 秒、`main.rs:258`）で
  弾いている。**塞がっている**。この定数がまさにこの穴のために置かれた前例。
* **13 番**（季節）: `tell :all, "season"` を 1 体でも聞けば立つ。最初の宣言は 0.03 秒で、
  そのとき全員が購読済み。**踏まない**。ただし `every 60` の 60 秒・120 秒に生まれた子は
  季節を聞き損ね、`@memory["season"]` が古いままセーブに入る（判定は落ちないが事実として残る）。
* **1 番**（`"ate"`）、**8 番**（子の遺伝子）、**2 番**（夜の到来）: 発火点が Rust 側の観測か、
  宛先が生まれる前に存在しえない。**踏まない**。
* **`on(:mate)`**: 新生児は `Breeding.ready_at = now + 20` を貰うので 20 秒間 `ready` に入らない。**踏まない**。
* **7 番だけが猶予を持っていない。**

**判定の外の話**もひとつ。夜が落ちた直後に生まれた子は、その晩ずっと起きて歩き回る
（`@asleep` は次の `"night"` まで立たない）。これは判定の都合ではなく**箱庭の振る舞いの穴**で、
7 番はそれを正しく見つけている、とも言える。

---

## 5. 直し方の候補（選ばない。実装もしていない）

### 5.1 4 番

| 案 | どこを変える | 閾値 | 仕様の文言 | 振る舞い |
|---|---|---|---|---|
| **A. 壁を「動かないもの」として押し合いに参加させる** | 力学（`separate`） | 動かさない | 変わらない | **変わる**（正しくなる） |
| B. パスの中でクランプする | 力学（`separate`） | 動かさない | 変わらない | 変わる（軽減のみ） |
| C. 判定が壁際の組を飛ばす | 判定（`watch_overlap`） | 動かさない | **変わる** | 変わらない |

* **A**: 押し出す先が壁の外になる分を、相手側に回す。木や岩に対して既にやっている
  「動かないものは何も譲らない」の対称形で、**新しい数を 1 つも置かずに**書ける。
  食い込みは構造的に消える（クランプが動かす量が 0 になる）。いちばん筋がいい。
* **B**: 各パスの頭で壁に入れ直す。最後のパスがまた外へ押しうるので**残る**。
  §2.3 が示すとおり食い込みは連続分布なので、小さくはなるが 0 にはならない。数の根拠も作れない。
* **C**: 6 番が `by_a_wall`（`main.rs:4978`）でやっているのと同じ除外を 4 番にも入れる。
  判定は書きやすいが、「何も何かを突き抜けていない」が「壁際を除いて」に変わる——
  そして §2 の実測では**突き抜けているのは壁際だけ**なので、判定はほぼ何も見なくなる。取りたくない。

### 5.2 5 番

| 案 | どこを変える | 閾値 | 仕様の文言 | 振る舞い |
|---|---|---|---|---|
| **D. 邪魔が入った回は数えない** | 判定（`watch_probe`） | 動かさない | 変わらない（「測れなかった」が増える） | 変わらない |
| E. プローブの隅を囲う | 仕込み（`spawn_world`） | 動かさない | 変わらない | 変わらない |
| F. 邪魔のあとで測り直す | 判定（`watch_probe`） | 動かさない | **変わる** | 変わらない |

* **D**: プローブが皿に着く前に `"touched"` を受け取ったら、その走行では
  「この判定は測れなかった」として **ok 扱いにせず FAIL 扱いにもしない**（6 番の
  `TOUCH_SETTLE` / `looked == 0` と同じ「数えない」の形）。既に `startle` が
  `test.last_touch` にプローブの被接触時刻を持っているので、読む先は増えない。
  ただし 6 番と違って 5 番は 1 走行に 1 回しか測れないので、「その走行では判定 1 つが空になる」。
* **E**: 起動時に木か岩でプローブと皿の周りを囲い、ウサギが歩いて入れないようにする。
  `clear_of_fixtures` の逆向きで、判定にも規則にも触らない。
  ただし `separate` は壁でなく円なので、囲いの隙間から入られる可能性は残る（測って決める話になる）。
* **F**: 「触られたら `probe_from` と `probe_closest` を今の距離から取り直し、
  制限時間を延ばす」。「5.0 離れたところから着いた」が「邪魔されるたびに仕切り直して着いた」に変わる。
  仕様の文言が変わるので、著者の判断。

**閾値（`REACH + 0.5 = 1.6`）を動かす案は出さない。** §3.2 のとおり止まる距離は
1.8〜2.3 に散らばっていて、1.6 を 2.4 に上げれば「着いた」が「近づいた」になるだけで、
上げ幅を導ける根拠がどこにもない。

### 5.3 7 番

| 案 | どこを変える | 閾値 | 仕様の文言 | 振る舞い |
|---|---|---|---|---|
| **G. 生まれたては数えない** | 判定（`watch_sleep`） | **既存の `NEWBORN_GRACE`** | **変わる** | 変わらない |
| **H. 生まれた生き物に今の空を伝える** | ゲーム（`children_arrive` 側） | 動かさない | 変わらない | **変わる**（箱庭の穴も塞がる） |
| I. 凍結の窓を夜から離す | 仕込み（`FREEZE_AT`） | **動かす**（根拠は導ける） | 変わらない | 変わらない |

* **G**: `watch_sleep` で `creature.age <= NEWBORN_GRACE`（2.0）の個体を数えない。
  6 番が `startle` の中でやっているのと同じで、**新しい数を置かずに済む**唯一の案。
  文言は「（生まれたてを除いて）全員が寝ている」になる。
  §4.4 の「夜に生まれた子は朝まで起きている」という箱庭の穴は**残る**。
* **H**: スクリプトが始まった生き物に、その時点の空（夜なら `"night"`）を 1 通だけ届ける。
  判定は 1 文字も変えずに通り、箱庭の穴も塞がる。要るのは「タスクが購読を終えた」を
  ゲーム側が知る手立てで、今の rubevy には無い（`subscriptions` は `HostState` の中で外から見えない）。
  代わりに「生まれて 2 フレーム後に、その entity 宛に今の空を publish する」なら
  ゲーム側だけで書ける——2 という数は §4.2 の実測から出る。
* **I**: 出産の山（25.1 秒）が夜（25.2 秒）の 0.1 秒手前に来るのは、
  `FREEZE_AT = 20.0` + `FREEZE_WINDOW = 5.0` と、夜が来る 25.2 秒
  （`DAY_LENGTH 60 × (0.5 − DAWN_OFFSET 0.08) = 25.2`）が偶然ぶつかっているから。
  `FREEZE_AT` を早めれば山は夜から離れる。**導ける数**である——
  解凍から子が生まれるまでが実測 0.07〜0.09 秒（§4.3 の 5.9〜6.9 フレーム分のうち、
  夜との差から解凍時刻を引いた分）で、落ちる境界は「生まれたのが夜の 1 フレーム前より後」。
  今の余裕は 5 フレーム（83 ms）しかない。`FREEZE_AT = 18.0` なら解凍 23.0 秒・出産 23.1 秒で、
  夜（25.2 秒）まで 2.1 秒 = 126 フレームの余裕になる。
  「12 番の窓が 7 番の夜と重ならない」という言い方で根拠が書ける数。
  ただしこれは**当たりにくくするだけ**で、穴（§4.1）はそのまま。

**G と H は排他ではない。** H が本筋で、G は「判定は判定として正しく書く」話。

### 5.4 範囲外だが記録（8 番）

計測入り 48 回のうち 1 回（run16）で
`a child was born whose genome is its parents' mixed and mutated (none was, from 0 pairings)` が出た。
**90 秒でつがいが 1 組も成立しなかった**という意味で、5 番が落ちたのと同じ走行。
`meadow` の 2 匹が草に着かなかったか、着く前に散ったのだと思われるが、**確かめていない**。
今回の依頼の範囲外なので推測のまま置く。

---

## 6. 戻したこと

`git checkout -- garden/src/main.rs garden/ruby/prelude.rb garden/ruby/creatures/beetle.rb
garden/ruby/creatures/rabbit.rb` で計測を全部外し、`cargo build --release -p garden` が通ることと
`git status --short` が空であることを確かめた（この worklog と `docs/README.md` を書く前の時点）。
`FLAKEPROBE` という語はソースのどこにも残っていない（`grep -rn flakeprobe garden/` が空）。

**閾値は 1 つも動かしていない。** `0.9`（4 番）、`REACH + 0.5`（5 番）、`1.0` 秒（7 番）、
`SEPARATE_PASSES = 4`、`TOUCH_REACH`、`NEWBORN_GRACE`、`FREEZE_AT`、`FREEZE_WINDOW` —— どれも元のまま。
