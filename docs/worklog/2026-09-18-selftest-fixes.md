# 2026-09-18 箱庭の 3 つの揺れる判定を直す — 壁を押し合いに入れ、測れなかった走行を数えず、新生児に空を伝える

前日の調査（`docs/worklog/2026-09-17-selftest-flakes.md`）で原因が取れた 3 つについて、
著者が「推奨で」と決めた案（4 番は A、5 番は D、7 番は H と G）を実装した記録。
作業場所は worktree `rubevy_games-wt-fixes`（ブランチ `selftest-fixes`、main `b1e5ec1` から）。
`main` には触っていない。push もしていない。

コミットは 3 本 + この文書:

| コミット | 何を |
|---|---|
| `18c7559` | 4 番 / 案 A: `separate` で壁を「動かないもの」として押し合いに入れる |
| `8769fde` | 5 番 / 案 D: プローブが皿に着く前に触られた走行は「測れなかった」 |
| `d8d113e` | 7 番 / 案 H + G: 生まれた個体に今の空を 1 通届け、判定は生まれたてを数えない |

**閾値は 1 つも動かしていない。** `0.9`（4 番）、`REACH + 0.5`（5 番）、夜の 1.0 秒（7 番）、
`SEPARATE_PASSES`、`TOUCH_REACH`、`NEWBORN_GRACE`、`TOUCH_SETTLE`、`FREEZE_AT`、`FREEZE_WINDOW` ——
どれも元のまま。新しく置いた数は 1 つだけで（`NEWBORN_DEAF_FRAMES = 2`）、その出どころは
§3 と、コードの上にある 10 行のコメントに書いた。`unsafe` は 1 行も増えていない
（`grep -c unsafe garden/src/main.rs` は 0 のまま）。

---

## 0. 着手前の状態

```
$ cargo build --release -p garden -p sabibots
    Finished `release` profile [optimized] target(s) in 4m 08s
$ cargo clippy -p garden
warning: `garden` (bin "garden") generated 13 warnings
warning: `rubevy-arena` (lib) generated 2 warnings
```

13 + 2 を基準にして、3 本のあいだ何度も測り直した（§4.1）。

---

## 1. 4 番 — 壁を「動かないもの」として押し合いに入れる（案 A）

### 1.1 直したこと

調査が突き止めた形はこうだった（§2）: 深く重なった 2 匹を 4 パスがきっかり「触れている」まで
押し離す（`preclamp` は 1.000）、押された方が壁の外に出る、書き戻しの

```rust
transform.translation.x = at[i].x.clamp(-HALF_W + 0.5, HALF_W - 0.5);
```

がその分を引き戻す、引き戻された分がそのまま相手への食い込みになる。
観測 23 件が全部この形で、**パスが押し戻し切れなかった例は 1 件も無かった**。

案 A は「木や岩は何も譲らない」という既にある規則を、壁について言い直す。
押し合いの中で、壁が許さない分を相手に回す:

```rust
(true, true) => {
    let half = dir * push * 0.5;
    let refused_i = refused_by_the_walls(at[i], -half);
    let refused_j = refused_by_the_walls(at[j], half);
    at[i] = inside_the_walls(at[i] - half - refused_j);
    at[j] = inside_the_walls(at[j] + half - refused_i);
}
```

`refused_by_the_walls` は「その一歩のうち壁が許さない分」を返すだけの関数で、
`inside_the_walls` は `move_creatures` が持っている半単位の内側の同じ境界を 1 箇所にまとめたもの。
**新しい数は 1 つも要らない。** 相手に回す分をベクトルのまま渡すので、2 体の相対位置は
「壁が無かったときにパスが決めた形」とそっくり同じになる（片方が壁に押し付けられるときだけ、
その軸の分だけ相手が余計に動く）。

木・岩に押されて壁にぶつかる側（`(true, false)`）も同じく壁で止める。こちらは相手が
動かないものなので回す先が無く、そのフレームは壁に押し付けられたままになる——
これは案 A を採る前も後も同じ振る舞い（前は書き戻しの `clamp` が同じことをしていた）。

これで `at` に入る値は常に壁の内側になったので、書き戻しの `clamp` は何も動かさなくなった。
`clamp` を残しておくと「動かないことになっている clamp」が 1 つ増えるだけなので、
入口（`movers` から `at` を作るところ）に移して出口からは外した。
入口に置いたのは、手で書き換えたセーブのような「外から来た位置」に対して
`separate` の後は必ず壁の内側、という以前からの保証を落とさないため。

### 1.2 効果の計測

調査の §2.3 と同じ仕掛け——`watch_overlap` で 0.99 を割った組を全部印字する——を
**直す前と直した後の両方**に入れて、同じ機械で続けて 1 回ずつ 90 秒回した。

```
（直す前）
$ GARDEN_SELFTEST=1 ./target/release/garden --headless 90
fixprobe overlap: ... × 17
selftest: ok   nothing walked through anything over 5388 frames (closest pair 0.977 of the radii, 0 frames under 0.9)

（直した後）
fixprobe overlap: （1 件も出ない）
selftest: ok   nothing walked through anything over 5389 frames (closest pair 0.995 of the radii, 0 frames under 0.9)
```

| | 0.99 を割った組 | 走行中の最接近 |
|---|---:|---:|
| 直す前 | 17 | 0.977 |
| 直した後 | **0** | **0.995** |

調査が 1 回の単独走行で 21 件・最接近 0.977 を観測したのと同じ数字の並びで、
それが 0 件になった。**「落ちる 0.9」は連続分布の尾だったので、尾ごと消えている**ことが要る——
そこで、後の方の走行には「壁が何かを拒んだ押し合い」を全部印字する仕掛けも入れた:

```
fixprobe wall: 105v0 vs 108v0 pass 0 — start 0.936, settled 1.000; at (19.04,-13.82)/(19.50,-13.16)
…
```

90 秒で **156 件**。`settled`（押し合いが終わったときの比、書き戻し後と同じ値）は
152 件が 1.000、4 件が 0.999（印字の丸め。`distance / sum` は 0.9995 以上）。
**1 つも 1.0 を割らない。** 壁際で押された組が食い込みを残す道が無くなったということで、
これが案 A の「構造的に消える」の中身。

走行全体の最接近 0.995 は壁とは関係ない組（3 体が絡んだフレーム）で、
これは案 A の前も後もある値。0.9 からは遠い。

計測の仕掛けは全部外した（`grep -c FIXPROBE garden/src/main.rs` は 0）。

---

## 2. 5 番 — 邪魔が入った走行は「測れなかった」（案 D）

### 2.1 直したこと

調査 §3 が示したのは、落ちる 3 回とも「プローブが皿に着く前にウサギが `TOUCH_REACH` まで寄り、
`on(:touched)` が皿と反対へ全速力で舵を切った」形だった。そのフレームの距離（1.8〜2.3）が
`probe_closest` に残り、判定はそれを「スクリプトが皿に着けなかった」と印字する。
着けなかったのはスクリプトのせいではない。

案 D は、皿に着く前に `"touched"` を受けた走行を **ok にも FAIL にもしない**。
6 番が「窓のあいだ一度も読めなかった接触」を `looked == 0` で数えないのと同じ形で、
読む先も増えない——`startle` が既に `test.last_touch` に「その甲虫に `"touched"` を送った時刻」を
持っている（6 番の 3 つの除外より**前**に書かれるので、除外に関わらず必ず入る）。

判定の口は 3 つ目の言い方を持つことになった:

```rust
let unmeasured = |what: String| info!("selftest: n/a  {what}");
```

`"ok  "` `"FAIL"` と同じ 4 文字にしてあるので、行が揃う。

6 番との違いも書いておく: 6 番は 1 走行に 20 件ほど測るので 1 件落としても判定は立つが、
5 番は 1 走行に 1 回しか測れないので、**邪魔が入った走行はこの判定を丸ごと失う**。
それでも「測れなかったものを測れたと言わない」ほうを採る。

閾値 `REACH + 0.5 = 1.6` は動かしていない。調査 §5.2 の言うとおり、止まる距離は
1.8〜2.3 に散らばっていて、上げ幅を導ける根拠が無いため。

### 2.2 効果の計測

素の走行では 25 回に 1 回しか起きないので、**ウサギをプローブの隅に置いて**起こした
（`GARDEN_TESTRABBIT`、使い捨て。`spawn_world` でプローブの 2.0 単位隣にウサギを 1 匹湧かせるだけ）。
20 秒 ×4 回:

```
run 1: selftest: ok   a hungry creature with a plant in sight reached it (from 5.0 away, at 12.92 s)
run 2: selftest: n/a  a hungry creature with a plant in sight reached it
         (not measured: a rabbit walked into the probe at 0.24 s; it started 5.0 away and got no closer than 4.7)
run 3: 同上
run 4: 同上
```

3 回はウサギが 0.24 秒で寄ってきて `n/a`、1 回はウサギが逆に歩き去って判定が立った（12.92 秒で到着）。
どちらの側も出ることを見た。仕掛けは外した。

---

## 3. 7 番 — 生まれた個体に今の空を伝える（案 H）と、判定を判定として書く（案 G）

### 3.1 H: 2 フレームという数の出どころ

調査 §4.1 が読んだ道筋はこうだった:

* `children_arrive`（`RubevySet::Answer` の後）が N フレーム目に体を作り `Script` を付ける。
* rubevy がそれをタスクにして最初に走らせるのは **N+1** フレーム目の `RubevySet::Tick`。
  そのタスクの最初の仕事が `start_handlers` で、`Rubevy.subscribe` はそこで呼ばれる。
* `publish_value` は「その時点で購読している queue」にしか積まない。

したがって**新生児が聞ける最初の publish は N+2 フレーム目のもの**。
調査 §4.2 の実測（夜の前後に 1 フレームずつずらして 8 体湧かせた表）がぴったりこれで、
2 フレーム前に生まれた子は聞き、1 フレーム前の子は聞いていない。

「購読が済んだ」をゲーム側から問う口は rubevy に無い（`subscriptions` は `HostState` の中）ので、
待つのはフレーム数になる。その数が `NEWBORN_DEAF_FRAMES = 2` で、
上の 3 行の道筋と調査の実測がそのまま定数の上のコメントに書いてある。

実装は小さい: `Newborns`（`Vec<(Entity, u32)>`）に `children_arrive` が子を積み、
`tell_newborns_the_sky` が毎フレーム 1 つ数えて、2 になったらその entity 宛に
今の空（`"night"` か `"day"`）を `day_night` と同じ payload（世界の時刻）で publish する。

置き場所は `.after(day_night).before(RubevySet::Tick).run_if(is_still)`。

* `after(day_night)` — ちょうどそのフレームで空がひっくり返ったとき、古い方を伝えないため。
* `before(RubevySet::Tick)` — 生き物が最初に読めるフレームのうちに queue へ入れるため。
* `run_if(is_still)` — `P` で止まっている間はタスクが走らない（予算 0）ので、
  そのフレームは「2 フレーム」に数えてはいけない。`children_arrive` が同じ条件を持っているのと同じ理由。

空がひっくり返ったフレームにちょうど 2 フレーム目が来ると、その子は `day_night` の publish も
聞けるので同じ言葉を 2 回受け取る。`@asleep = true` を 2 回やるのは 1 回やるのと同じなので、
「このフレームで空が動いたか」をもう一本持つより、重複を許すほうを採った（コメントに 1 行）。

### 3.2 G: 判定を判定として書く

`watch_sleep` で `creature.age <= NEWBORN_GRACE`（2.0 秒）の個体を数えない。
6 番が `startle` の中でやっているのと同じ除外で、**新しい数は置いていない**——
`NEWBORN_GRACE` は「新生児にはまだハンドラがいない」ためにこの source に既にある定数で、
実際に耳が無い 2 フレーム（0.033 秒）よりずっと長い。

判定の文言は `(14 of them, fastest …)` から `(14 of them, newborns aside, fastest …)` へ。

H が本筋で、G は「H を入れても、判定は判定として正しくあるべき」の分。
H だけでは「生まれて 2 フレーム目に夜が来た子」が理屈の上では残る（その子は空を 2 回聞くので
実際には寝る）し、G だけでは**箱庭の穴**——夜に生まれた子が朝まで起きている——が残る。

### 3.3 効果の計測

調査 §4.2 の `GARDEN_TESTBIRTH` を作り直して（`t1,t2,…` の各時刻に `Births::waiting` へ 1 件積む、
子が通るのと同じ道）、**直す前と直した後**で 30 秒走行を 1 回ずつ。
夜の publish のあたりに 1 フレームずつずらして 8 体:

```
$ GARDEN_SELFTEST=1 GARDEN_TESTBIRTH=25.10,25.12,25.14,25.16,25.18,25.20,25.22,25.24 \
    ./target/release/garden --headless 30
```

**直す前**（`"night"` は elapsed 25.2103）:

| 生まれた時刻 | publish との差 | 夜の 1 秒後の速さ |
|---|---|---:|
| 25.1101 / 25.1268 / 25.1435 / 25.1602 | 2〜6 フレーム前 | 0.000（寝ている） |
| 25.1936 | **1 フレーム前** | **2.000** |
| 25.2103 | 同じフレーム | **2.000** |
| 25.2270 / 25.2437 | 後 | **2.000** |

```
selftest: FAIL the creatures were asleep a second after night fell (24 of them, fastest 2.000 at 26.21 s)
```

**直した後**（`"night"` は elapsed 25.2155）:

```
fixprobe sleep: age 1.102 speed 0.000
fixprobe sleep: age 1.086 speed 0.000
fixprobe sleep: age 1.069 speed 0.000
fixprobe sleep: age 1.052 speed 0.000
fixprobe sleep: age 1.035 speed 0.000
fixprobe sleep: age 1.002 speed 0.000
fixprobe sleep: age 0.985 speed 0.000
fixprobe sleep: age 0.969 speed 0.000
selftest: ok   the creatures were asleep a second after night fell (16 of them, newborns aside, fastest 0.000 at 26.22 s)
```

8 体全部が速さ 0.000。夜の 1 フレーム前・同じフレーム・後に生まれた子も寝ている。
これが H の効果で、判定の `16 of them` は G が 8 体（年齢 1 秒）を数えていないためにこうなる。
2.000 は `Creature::CRUISE` そのもの——「起きている」ではなく「寝ろと言われていない」——
という調査の読みが、言われれば寝ることで裏から確かめられた形。

仕掛けは両方とも外した。

---

## 4. 確認

### 4.1 ビルドと clippy

3 本のコミットそれぞれの後で測った。

```
$ cargo build --release -p garden -p sabibots
    Finished `release` profile [optimized] target(s)
$ cargo clippy -p garden
warning: `garden` (bin "garden") generated 13 warnings
warning: `rubevy-arena` (lib) generated 2 warnings
```

**着手前と同じ 13 + 2。** 内訳も並べて比べた:

| 警告 | 着手前 | 3 本の後 |
|---|---:|---:|
| `too_many_arguments` | 6（11/7, 10/7, 9/7 ×2, 8/7 ×2） | 6（11/7, 10/7 ×2, 9/7, 8/7 ×2） |
| `collapsible_if` | 2 | 2 |
| その他（`manual_range_contains` ×2、`manual_div_ceil`、`wrong_self_convention` ×2、`explicit_counter_loop`、`type_complexity`） | 7 | 7 |

`10/7` が 1 つ増えて `9/7` が 1 つ減っているのは、`children_arrive` が `Newborns` を 1 つ取ったため
（9 → 10 引数）。数は変わらない。

途中で 1 回 14 になった（5 番の実装で `if { if let }` を書いたところ）。
`collapsible_if` を素直に潰す形（先に `Option` を作って代入する 3 行）に直して 13 に戻した。

`unsafe` は 0 のまま（`grep -rn unsafe garden/src/ sabibots/src/ crates/` が 0 行）。

### 4.2 90 秒 ×40 回（8 本並列）

調査と同じやり方: `TMPDIR` を走行ごとに分けて（`another_version_file()` が
`std::env::temp_dir()` なので、並列の 8 本が同じファイルを書かないように）、8 本ずつ 5 巡。

```bash
for i in $(seq -w 01 40); do
  d=$(mktemp -d)
  TMPDIR=$d GARDEN_SELFTEST=1 ./target/release/garden --headless 90 > runs/run$i.log 2>&1 &
  ...8 本ごとに wait
done
```

| 判定 | FAIL | n/a（測れなかった） | 2026-09-17 の 98 回での率 |
|---|---:|---:|---|
| 1 somebody ate | 0 | — | — |
| 2 night arrived | 0 | — | — |
| 3 starved entity gone | 0 | — | — |
| **4 突き抜け** | **0** | — | 4 / 98（4.1%） |
| **5 プローブ** | **0** | **1** | 4 / 98（4.1%） |
| 6 向き | 0 | — | 1 / 98（1.0%） |
| **7 睡眠** | **0** | — | 0 / 98 |
| 8 子の遺伝子 | 0 | — | 1 / 98（1.0%） |
| 9 spawn Hash | 0 | — | — |
| 10 セーブの版 | 0 | — | — |
| 11 world.rb が世界を動かす | 0 | — | — |
| 12 規則の差し替え | 0 | — | — |
| 13 季節が記憶に届く | 0 | — | — |

**FAIL は 1 行も出ていない。** 40 回のうち 39 回が 13 判定すべて ok、1 回（run15）が 12 ok + 1 n/a。

落ちた回の FAIL 行は無いので、代わりに唯一の n/a 行を全部:

```
run15: selftest: n/a  a hungry creature with a plant in sight reached it
         (not measured: a rabbit walked into the probe at 1.63 s; it started 5.0 away and got no closer than 1.9)
```

**これは案 D が狙ったそのものの回**で、作った状況ではなく素の走行で出た。
1.9 は調査 §3 が記録した 1.8〜2.3 のまん中で、閾値 1.6 に 0.3 足りない——
直す前ならこの回は FAIL と印字していた。40 回に 1 回（2.5%）は調査の 4.1% と矛盾しない。

4 番の側の数字も見ておく（`nothing walked through …` 行の `closest pair`）:

| | 最小 | 中央 | 最大 | 0.9 を割ったフレーム |
|---|---:|---:|---:|---:|
| 40 回 | 0.981 | 0.998 | 1.000 | 0（40 回すべて） |

13 回が 1.000 ちょうど。調査の 98 回では落ちた回が 0.854〜0.899、通った回の最接近も 0.975 前後だったので、
分布が丸ごと 1.0 の側へ寄っている（§1.2 の「尾ごと消える」）。

7 番の側は、40 回すべてで `fastest 0.000`。90 秒で生まれた子は 180 匹、
うち夜の前後 1.5 秒に生まれたのが 14 匹（調査の窓 `night-1.0 .. night+1.05` では 13 匹）。
**致命の窓（夜の 1 フレーム前より後）に生まれた子は 0 匹**で、
これは調査の 98 回と同じ——つまりこの 40 回は 7 番の直しを引き当ててはいない。
H の効果は §3.3 の作った状況で示したとおり。

ついでの観察を 2 つ。5 番の到着時刻は 38 回が 1.78 秒ちょうどで、1 回だけ 19.39 秒（run30、
`from 5.2 away`）。邪魔が入らなければ直線で歩くので毎回同じ値になるという調査 §3.3 の読みどおりで、
19.39 秒の回は触られてはいない（`n/a` になっていない）。6 番は 10/10 〜 22/22 で全通過。

### 4.3 sabibots、窓ありビルド、セーブ往復

**sabibots**（×1、`--headless 30`）: 4 判定すべて ok。

```
selftest: ok   the handler tasks of every robot that went down ended (1/1)
selftest: ok   25 hits on a robot with a handler were checked
selftest: ok   a handler ran within 0.3 s of the hit (25/25)
selftest: ok   the heading changed within 0.3 s of the hit (25/25)
```

**窓ありビルド**: `cargo build --release -p garden` が作るのは窓ありのバイナリで
（`--headless` は実行時の引数、feature ではない。`bevy` の `x11` が入っている）、
`garden/src/window.rs` も含めて通っている。**実行はしていない**（画面が要る）。

**セーブ往復**:

```
$ ./target/release/garden --headless 30 --save s1.json
saved 10 creatures, 54 plants at 30.0 s to s1.json (12473 bytes)
$ ./target/release/garden --load s1.json --headless 0 --save s2.json
loaded 10 creatures, 54 plants, 7 trees, 9 rocks at 30.0 s (0 entities made way)
saved 10 creatures, 54 plants at 30.0 s to s2.json (12473 bytes)
$ diff s1.json s2.json && echo IDENTICAL
IDENTICAL
```

`Newborns` はセーブに入らない（入れる必要が無い——ファイルを読んで作られる生き物は
「生まれた子」ではないし、書き出しの時点で待っている子がいればその子は次の走行では
最初から世界にいる）。往復の同一性は変わらなかった。

---

## 5. 迷った点、やらなかったこと

**`separate` の書き戻しの `clamp` を消すか残すか。** 残しても動かない `clamp` になるだけだが、
「動かないことになっている 1 行」は次に読む人を迷わせる。`at` を作る入口に移して出口から外した——
これで「`at` に入っているものは常に壁の内側」という不変が関数の頭から終わりまで言える。
手で書き換えたセーブのように外から壁の外の位置が来ても、以前と同じく引き戻される。

**案 A で相手に回す分をベクトルのまま渡すか、`dir` に射影するか。** ベクトルのまま渡した。
射影すると壁に沿った向きの成分が消えて、2 体の相対位置が「壁が無かったとき」と変わる。
ベクトルのままなら相対位置はパスが決めた形そのものになる（§1.1）。実測でも
156 件すべてが 0.999 以上で、これは射影では出ない数字。

**5 番の n/a を「ok」に倒す案は採らなかった。** 「邪魔が入ったから合格」は測っていないものを
測ったと言うことになる。逆に FAIL に倒す案も採らない（スクリプトのせいではない）。
判定の口を 3 つにするのがいちばん正直で、6 番が既に同じ考え方を持っている。

**7 番の H を `give_mind` に付けなかった。** `give_mind` は起動時の 10 匹にも呼ばれるので、
そこに置くと全員に起動直後の `"day"` が 1 通ずつ飛ぶ。害は無いが、
今回直すのは「生まれた個体」の話なので `children_arrive` の道だけにした。
**`--load` / F9 で読み込まれた生き物は同じ穴を踏む**——夜のセーブを読むと、
その生き物たちは朝まで起きている。範囲外なので直していないが、事実として記録する
（`load_world` の道に同じ `Newborns` を積めば同じ直し方ができる）。

**案 I（`FREEZE_AT` を早める）は実装していない。** 著者判断が H + G だったのに加えて、
I は「当たりにくくするだけ」で穴（購読の 2 フレーム）はそのまま残る。
調査 §5.3 の評価をそのまま採る。

**8 番の揺れ（調査 §5.4、つがい 0 組）は今回の範囲外。** 40 回では 1 回も出ていない。

**判定 5 の n/a が増えると判定が空になる走行がある。** 40 回で 1 回。
「1 走行に 1 回しか測れない判定」の性質そのもので、
気になるなら案 E（プローブの隅を囲う）が別にある——ただし囲いの隙間の話になるので、
測って決める仕事が要る（調査 §5.2）。今回はやっていない。
