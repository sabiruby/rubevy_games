# 2026-09-17 コンポーネントの同期読み（S3: サンプルゲーム側）

rubevy の `docs/plans/sync-access-plan.md` の **S3** の記録。ブランチ `sync-reads`、main `7f22fb1` から。
rubevy 側の S1／S2 の記録は向こうの `docs/worklog/2026-09-17-sync-reads.md`（同名）。
調査は `docs/worklog/2026-09-17-sync-access-survey.md`（rubevy）の §7「サンプルゲームへの影響」。

読みが同期になったので、ゲーム側でやることは 4 つと言われていた。順に、**1 番目で止まった**。

| # | やること | 結果 |
|---|---|---|
| 1 | スクリプトが読むコンポーネントを書くシステムを `.before(RubevySet::Tick)` に | **止めた。** 入れるとセルフテストの 6 番目が半分の回で落ちる（§2） |
| 2 | 「1 判断あたりのフレーム数」→「1 判断あたりの命令数」 | 済み（§3） |
| 3 | VM パネルの `Waiting::Component` を消す | 済み（§4）。ただし到達不能は厳密には嘘（§4.2） |
| 4 | 文書 | 済み（§5） |

---

## 0. 着手前 — `cargo update -p rubevy` だけで両ゲームがどうなるか

最初にやったのは `cargo update -p rubevy` と、**その状態で何が落ちるかを測ること**。
同期化の影響は、ゲームのコードを 1 行も変えない状態で既に出ているかもしれない。

```
Updating rubevy v0.0.1 (https://github.com/sabiruby/rubevy#ec6f0ce0) -> #4f77881b
```

`cargo update -p rubevy` は sabiruby も `bd6829b3` → `8faf26f2` に上げようとした。
指示は「sabiruby の rev はそのまま」なので `cargo update -p sabiruby --precise bd6829b3…` で戻した。
`Cargo.toml:27` のコメント（「rubevy と同じソースなので、バイナリの中の VM は 1 つ、型も 1 組」）は
**git の URL が同じであることの話**で、rev を指定していない。だから rubevy と games が同じ
`Cargo.lock` の 1 行を共有する形は変わっていない（`Cargo.lock` の sabiruby は 4 か所とも
`bd6829b3` の 1 つだけ）。rubevy 自身の lock は sabiruby 0.5.0 を指しているが、
git 依存の解決はワークスペース側の lock が決めるので、バイナリに入る VM は 1 つのまま。

`cargo build --release` は 2 分 41 秒で通る（rubevy と rubevy-arena だけが再コンパイル）。
そのうえでセルフテストを回した。

* `GARDEN_SELFTEST=1 ./target/release/garden --headless 90` ×3 → **10 判定すべて ok、3 回とも。**
* `SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 25` ×3 → **すべて ok、3 回とも。**

つまり**同期読みそのものは、ゲームのコードを変えなくても何も壊さない**。
前例（往復を 2→1 フレームにしたら Battle の反射テストが 5 回中 3 回落ちた、
`docs/worklog/2026-09-17-battle-followups.md:249-270`）と同じ落ち方は、ここでは起きなかった。
理由は §1 と §2 で分かる。

### 0.1 その代わり、測っていた数の方が壊れた

garden の headless の総計行が、着手前の 3 回でこうなっていた:

```
hud: frames/decision — 727 questions the game answered, 1.000 frames each; 3 component reads, 1.000 frames each
hud: frames/decision — 1017 questions the game answered, 1.005 frames each; 4 component reads, 1.000 frames each
hud: frames/decision — 957 questions the game answered, 1.000 frames each; 4 component reads, 1.000 frames each
```

**`component reads` が 2352 件（`docs/garden.md:1016` に載っている数）から 3〜4 件になっている。**
調査 §7-A が予告したとおりで、しかも予告より悪い。`watch_minds` は「短い空白＝読み」と数えていて、
読みが park しなくなった今、そこに残る 3〜4 件は**読みでも何でもない**（タイムスライス切れなど、
`SHORTEST_SLEEP` より短い空白）。0 にならず 3〜4 になるのが、この測り方が壊れたことの証拠になっている。
これが S3 の 2 番目の仕事（§3）。

### 0.2 sabibots には直すところが無い

「sabibots も同様に確認して揃える」と言われていたので確かめた。
**SabiRuby Battle の Ruby はコンポーネントを 1 回も読んでいない。**

```
$ grep -rn "\[:[A-Z]\|Rubevy.find\|\.components\|Rubevy::Entity" sabibots/ruby/
(何も出ない)
```

ロボットが世界について知ることは全部 DSL（`radar`、`status`、…）＝ `Rubevy.ask` で、
答えるのはゲームの `answer_requests`。だから同期読みは Battle の意味論に**触れていない**。
`.before(RubevySet::Tick)` に動かすべきシステムも無い（動かす理由が無い）。
ゲームの規則の chain は全部 `in_set(RubevySet::Answer)` にあり（`sabibots/src/main.rs:392-411`）、
これは「質問をその場で答えるための置き場」で、読みとは関係が無い。**sabibots は無変更。**

---

## 1. 今の順序を測る — 規則の chain は tick の前でも後でもない

計画書 §3.4 の 1 番目は「garden の規則 chain（`main.rs:1301-1325`）は今 `RubevySet` に対して順序が無いので、
`.before(RubevySet::Tick)` に置く」。まず「今どうなっているか」を測った。

最初に書いた計測は間違っていた。`.after(RubevySet::Tick).before(RubevySet::Answer)` の位置に
「今のフレーム番号」を書く系を置き、chain の中から読んで、`フレーム N - 1` が見えたら
「chain は tick の前」と読んだ。**これは証明になっていない**: その系は tick の後にあるだけで、
chain がその系より前に走ったことしか分からない（tick と その系 の間にいても同じ結果になる）。

正しい測り方は、**tick でしか増えないもの**を見ること。`ScriptWorld::stats(task).instructions` の
全タスク合計は、VM を回す tick の中でしか増えない。そこで

* `.after(RubevySet::Tick).before(RubevySet::Answer)` に「その時点の合計」を記録する系、
* chain の頭（`.before(day_night)`）と尻尾（`.after(starve)`）に「今の合計が、前に記録された合計と同じか」を出す系

を置いた。同じなら chain はこのフレームの tick より前、増えていれば後。結果（`--headless 1`、frame 31–44）:

```
probe: head frame 31 … -> BEFORE this frame's tick
probe: tail frame 31 … -> AFTER  this frame's tick
probe: head frame 32 … -> BEFORE this frame's tick
probe: tail frame 32 … -> BEFORE this frame's tick
…
probe: tail frame 37 … -> AFTER  this frame's tick
probe: tail frame 38 … -> AFTER  this frame's tick
probe: tail frame 39 … -> BEFORE this frame's tick
probe: tail frame 40 … -> AFTER  this frame's tick
```

**chain は tick をまたいで割れていて、割れ方はフレームごとに変わる。**
頭（`day_night`）は毎フレーム tick より前、尻尾（`starve`、その 2 つ手前が `startle`）は
フレームによって前だったり後だったりする。`.chain()` は互いの順序しか決めないので、
排他システムである `tick_scripts` が間に入ることを妨げない。

これが「順序が無い」の実物で、同期読みの前は**どちらでも同じ**だった（読みの答えは次のフレームの
Tick に返るので、このフレームの規則が tick の前だろうが後だろうが、スクリプトが見る値は
1 フレーム古い世界で変わらない）。同期読みが入った今は、**tick の前なら「このフレームの世界」、
後なら「前のフレームの世界」**を読むことになり、フレームごとに違う。だから直す価値はある。

---

## 2. 直そうとして止まった — `.before(RubevySet::Tick)` はセルフテストの 6 番目を落とす

### 2.1 何をしたか

chain に 1 行足しただけ:

```rust
            )
                .chain()
                .after(load_world)
                .before(RubevySet::Tick)      // ← これ
                .run_if(is_still),
```

chain が書くコンポーネントは、スクリプトが読むものとちょうど一致する:
`Transform`（`move_creatures` / `separate` / `grow_plants`）、`Hunger`（`get_hungry` / `eat`）、
`Creature`（`get_hungry` / `eat` / `court`）。読む側は `prelude.rb` の `hunger` / `sight` / `here` /
`body` と、`place_of` の `thing[:Transform]`。

### 2.2 測った結果

garden の headless セルフテスト（`--headless 90`）を、同じ機械で交互に回した。
落ちるのは 6 番目の「a beetle touched by a rabbit changed heading within 0.5 s」だけ。

| ビルド | 走らせた回数 | 落ちた回数 | 数えた接触 | 外した接触 |
|---|---|---|---|---|
| `cargo update` だけ（順序そのまま） | **8** | **0** | 153 | **0** |
| chain 全体を `.before(RubevySet::Tick)` | **12** | **6** | 261 | 13（5.0%） |
| 書く系だけ前に、`startle` / `court` / `starve` は据え置き | **6** | **3** | 119 | 4（3.4%） |

落ちたときの文言（そのまま）:

```
selftest: FAIL a beetle touched by a rabbit changed heading within 0.5 s (33/34)
selftest: FAIL a beetle touched by a rabbit changed heading within 0.5 s (20/21)
selftest: FAIL a beetle touched by a rabbit changed heading within 0.5 s (23/28)
selftest: FAIL a beetle touched by a rabbit changed heading within 0.5 s (22/23)
selftest: FAIL a beetle touched by a rabbit changed heading within 0.5 s (17/19)
```

順序を入れないほうは 8 回 153 接触で外れ 0。入れると 5% 前後で外す。偶然ではない。

### 2.3 外した接触は何をしていたか

`watch_turning` の判定が外れたところで、`was`（接触時の速度）と `is`（0.5 秒後の速度）を出した:

```
probe: miss beetle 118v0 at 6.687 sampled 7.189 (gap 0.502) dot 0.958 was Vec2(1.94, -0.49) is Vec2(8.99, 0.40)
probe: miss beetle  98v0 at 7.290 sampled 7.792 (gap 0.502) dot 0.993 was Vec2(-0.71, 1.87) is Vec2(-0.49, 1.94)
probe: miss beetle 113v0 at 56.848 sampled 57.350 (gap 0.502) dot 0.902 was Vec2(-0.50, -1.94) is Vec2(1.76, -8.83)
probe: miss beetle 100v0 at 63.701 sampled 64.203 (gap 0.502) dot 0.998 was Vec2(0.97, -1.75) is Vec2(4.89, -7.56)
probe: miss beetle 166v0 at 78.381 sampled 78.883 (gap 0.502) dot 0.992 was Vec2(-1.91, -0.42) is Vec2(-8.97, -0.79)
probe: miss beetle  96v0 at 62.827 sampled 63.329 (gap 0.502) dot 0.858 was Vec2(1.48, 1.35) is Vec2(2.58, 8.62)
```

**8 件のうち 6 件は、速さが 9.0 =`DASH`。つまりハンドラは走っていて、ビートルはちゃんと逃げている。**
外れているのは向きで、逃げる先が元の進路の 16°〜31° 以内に入っている
（`dot 0.858` が 31°、`dot 0.998` が 3.6°）。残り 2 件は速さ 2.0 =`CRUISE` で、
脳の `wander` が舵を取り戻したあとの姿。

だから「ハンドラが動かなくなった」のではなく、**`flee_from` が計算する向きが変わった**。
`flee_from` は接触した相手の位置を読んで「その反対 + `swerve: 1.0`（57°）」に走る。
向きの元になるのは「自分と相手の位置の差」で、**踏まれている最中の 2 体の距離はほとんど 0** だから、
その差ベクトルの向きは 1 フレームの動きで大きく振れる。順序を固定するとハンドラが起きるフレームが
1 つ早くなり（`startle` が publish した同じフレームの tick で起きる）、読む位置が 1 フレーム分
「ウサギがまだ乗り越えていない」側になる。`swerve` の左右も `@course` との角度差の符号で決まるので、
向きが少し変わると**逆側に振れて**、元の進路の近くに出てくる。

前例（`docs/worklog/2026-09-17-battle-followups.md:249-270`）は「往復が 2→1 フレームになって
舵の入る時間が 100 ms → 50 ms に減った」だった。**今回は 1→0 で、形は同じ。**
調査 §7-D の「1→0 は同じ種類の変更で、同じ種類の落ち方をすると見てよい」が当たった。

### 2.4 なので止めた

閾値（`0.5` 秒、`dot < 0.7`、`TOUCH_SETTLE = 1.5`、`NEWBORN_GRACE = 2.0`）は**1 つも動かしていない**
（計画書 §2 既定 11、プロンプトの指示）。`.before(RubevySet::Tick)` の 1 行も**入れていない**。
ブランチには順序の変更は入っていない。著者に決めてもらうべきことが 3 つある:

1. **順序を入れず、今のまま**にする。スクリプトが読む世界は「このフレーム」と「前のフレーム」が
   フレームごとに入れ替わる（§1 の実測）。garden の脳は 0.2 秒に 1 回しか判断しないので実害は
   見えていないが、`garden-world-plan.md` の「毎フレーム全員を見る」世界の脚本はこれに当たる。
2. **順序を入れて、ビートルのハンドラか 6 番目の判定を作り直す**。ハンドラが舵を握る時間は
   `sleep 0.5` で、判定が見るのも 0.5 秒後。**同じ数が両側にある**のが落ちやすさの本体で、
   どちらかを動かせば（ハンドラを 0.6 秒にする、判定を 0.3 秒後に見る、など）直るはず。
   ただしこれは「閾値を動かす」ことなので、私は手を付けていない。
3. **書く系だけ前に置く**（表の 3 行目）。これも 6 回中 3 回落ちたので、**逃げ道になっていない**。
   `startle` の publish だけが原因ではなく、ハンドラが読む位置が動くことが原因だから。

私の見立ては 2 で、直すのはハンドラ側（`sleep 0.5` → 舵を長めに握る）だと思う。
判定の 0.5 秒は「反射が 0.5 秒以内に見える」という**仕様**で、ハンドラの 0.5 秒は**実装**だから。
ただしこれは garden の振る舞いを変える話なので、決めずに置く。

---

## 3. 「1 判断あたりのフレーム数」を「1 判断あたりの命令数」に

（§3 以降は下に書き足す）
