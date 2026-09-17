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

### 3.1 何が壊れていたのか（G4 の測り方）

G4 の `frames/decision` は**空白を測っていた**。タスクは短いバーストで走って park し、
バーストが終わる理由は「世界に何か訊いた」か「寝た」のどちらか。
訊いた場合、答えが来るまでの空白のフレーム数が `frames/decision` で、2 種類の質問を
こう見分けていた（`watch_minds` の元のコード）:

* `answer_garden` がこの生き物に答えたフレームから始まる空白 → `Rubevy.ask` の往復（`Mind::asked_frame`）
* それ以外で `SHORTEST_SLEEP` より短い空白 → コンポーネント読み
* それより長い空白 → `sleep`。判断ではない

**読みが park しなくなったので、2 番目が拾うものが無くなった。** 拾うものが無くなったのに
0 にはならず、3〜4 になる（§0.1）。残っているのは**読みではない短い空白**（タイムスライス切れ、
ゲーム側のフレームの揺れ）で、前はそれが 2352 件の本物の読みに紛れて見えなかっただけ。
**消えたものを数えて 0 にならない測り方は、それを測っていない。**

### 3.2 代わりに何を測るか

読みが park しないなら、スクリプトが残す空白は**自分で開けた `sleep`** だけになる。
そして garden の行動アルゴリズムは全部「`loop do … sleep 0.2 end`」の形をしている
（`beetle.rb:96-127`、`rabbit.rb:54-93`、ハンドラも同じ）。だから

> **1 判断 = `sleep` から次の `sleep` までの 1 周。その間に使った命令数が、その判断の値段。**

`ScriptStats::instructions` は累計なので、1 周の値段は両端の差。`Mind` に 3 つ足した:

```rust
pub decisions: u32,              // 終わった周の数
pub decision_instructions: u64,  // その周たちが使った命令の合計
pass_start: Option<u64>,         // 今の周が始まったときの累計。最初の sleep まで None
```

`pass_start` が `Option` なのは、**数え始めたときに途中だった周を数えないため**。
`watch_minds` が最初に見たときの周は、頭の分だけ短く出る。

`SHORTEST_SLEEP`（0.05）は**残した**。この定数が言っているのは
「`ruby/` の中で一番短い `sleep`」で、判定に使うのは**「これ以上の空白は `sleep` でしかありえない」**
という向きだけ。逆向き（「これより短い空白はホスト待ち」）が壊れた方で、そちらは使わなくなった。
定数のコメントにそう書いた。ついでに、調査 §7-G が確かめたこと
——この `sleep 0.05` の理由は読みではなく `restore_memory`（書き戻しがフレームの末尾なので、
次の行で `memory` を読むと空の Hash）——も書いた。**書きは同期になっていないので、この理由は動いていない。**

### 3.3 ゲームが答える往復の方は残した

計画書は `Mind::asked_frame` ごと置き換えると書いているが、**ask の側は残した**。
理由は 3 つ:

1. 壊れていない。`asked_frame` は `answer_garden` が実際に答えたフレームを記録していて、
   推測が 1 つも入っていない。実測で今も 1.000 フレーム。
2. それが今や「フレームを食う質問」の**唯一の**例になった。同じ 1 行に
   「判断 1 回 = N 命令」と「ゲームが答える質問 1 回 = 1 フレーム、読みは 0 フレーム」が並ぶと、
   同期化が何を変えたのかが 1 行で読める。rubevy 側も同じ選び方をしている
   （`tests/scheduling.rs` に 0 と 1 の両方を残して「誰が答えるかで値段が違う」を 1 ファイルで読ませる）。
3. `docs/garden.md:1099` の表がこの数を引用している。消すと根拠の無い行になる。

消したのは `read_trips` / `read_frames` だけ。

### 3.4 出てきた数

`--headless 90` を 3 回:

```
hud: insn/decision — 4281 passes of a behaviour's loop, 208.2 instructions each; and 967 questions the game answered, 1.000 frames each (a component read costs no frame at all)
hud: insn/decision — 4443 passes of a behaviour's loop, 209.2 instructions each; and 893 questions the game answered, 1.000 frames each
hud: insn/decision — 5334 passes of a behaviour's loop, 196.6 instructions each; and 927 questions the game answered, 1.000 frames each
```

1 頭ずつ（HUD の列）はこうなる:

```
hud:   Beetle 98v0     hunger  92.6       6 insn/frame      81 insn/decision  beetle.rb:102
hud:   Beetle 95v0     hunger  61.1      11 insn/frame     169 insn/decision  beetle.rb:102
hud:   Rabbit 100v0    hunger  50.8      24 insn/frame     405 insn/decision  rabbit.rb:60
```

**ビートルが 81〜169、ウサギが 346〜410。** 差はそのまま脚本の差で、ウサギの 1 周は
`here` を読み、`garden.nearest(:Plant)` か `garden.nearest(:Creature)` を訊き、
相手の `[:Creature]` を読み、`wander(avoid: memory["trees"])` で木を避ける
——ビートルの 1 周より 3 つ多い。同じ数字が「空腹でないビートル」（`wander` だけ）で 81 まで下がるのも、
この数がちゃんと周の中身を見ていることの傍証になっている。

HUD の列は `f/dec` → `i/dec`、headless の行は `frames/decision` → `insn/decision`、
窓ありセルフテストの行は `component reads` → `decisions` に置き換えた。

---

## 4. VM パネル: 待つ理由が 6 つから 5 つに

`crates/rubevy-arena/src/inspect.rs` の `Waiting::Component` を消した
（enum の枝、`text()`、`how()`、`why()` の `Rubevy::Entity` の分岐、
単体テスト `a_component_read_names_the_component`）。パネルは tick の**外**（`show_vm` は
`Update` の後ろ）から VM を覗くので、tick の中で答え終わっている読みは見えない。
`rubevy-arena` の単体テストは 7 本から 6 本になり、残り 6 本は無変更で通る。

### 4.1 これは garden にしか無かった枝

Battle の Ruby はコンポーネントを読まない（§0.2）ので、この枝は garden の
`me[:Hunger]` 専用だった。`docs/garden.md` のスクショ（`garden-vm.png`）が
`waiting for a component read — [:Transform]` を写しているのはそのため。

### 4.2 「到達不能」は少し言い過ぎ

計画書は「到達不能になるので削除」と書いているが、**厳密には 1 つ残っている**。
tick が命令数の予算か `frame_time` を使い切って終わると、その周の読みは答えられないまま残り、
次の tick の頭で答えられる（rubevy 側 worklog §1.5）。その間にパネルが覗くと、
タスクは `Task::Queue#pop` の下に `Rubevy::Entity#get` を積んだまま立っている。

枝を消した今、そこは `why()` の最後の受け皿に落ちて **`Waiting::Ask("get")`**
——「ゲームが `get` を答えるのを待っている」——になる。ゲームは答えないので、この文は嘘になる。

指示どおり消したうえで、`Waiting` の rustdoc にこの 1 ケースを書いた。
実際に起きる気配は無い（garden の tick は 8 ms の予算に対して 1 ms を切っている。
着手前の HUD 行が `VM 0.20 / 8.0 ms`、`0.92 / 8.0`、`0.35 / 8.0`）が、
**「到達不能」と書くには強すぎる**ので、そう書かずに残した。
編集画面で `loop { me[:Hunger] }` を Apply した人が F2 を押すと、これが最初に出る画面になる。

---

## 5. 文書

直した場所と、**わざと直さなかった**場所。

### 5.1 `docs/garden.md`

* 「The components」の少し下、生き物の脚本の説明——「`hunger` も `here` も `head_to` も
  コンポーネント読み。**それぞれ 1 フレームかかる**」→「**1 フレームもかからない**」に。
  数字は rubevy 側の実測（2.2 µs、約 75 命令）だけを使った。そのうえで
  「1 パスが 3〜4 個読んで寝るのは変わらない。ただし今は**通行料ではなく選択**だ」と書いた:
  `garden.nearest` はゲームが答えるので今も 1 フレーム、`Rubevy.find` は今も世界を全部舐める。
* **The wheel** の段落。ここが一番気を使った。「書きは last-writer-wins のまま、
  **だから wheel は前とまったく同じだけ必要**」を先に言い、そのうえで
  「1 パスの長さが 4〜5 フレームから `garden.nearest` の 1 フレームに縮んだ」を言う形にした。
  調査 §7-C の［推測］（「脳の 1 パスが同一フレーム内で閉じると力学が変わる」）は**まだ起きていない**
  ——1 パスには `garden.nearest` の 1 フレームが残っているから、同一フレーム内には閉じない。
* **The VM panel**。表から `Rubevy::Entity#get` の行を消し、消えた 6 行目のことを段落で書いた
  （なぜ消えたか、1 ケースだけ残っていること、そこでは最後の行に落ちて「ゲームが答える質問」と
  誤って書かれること）。スクショ `garden-vm.png` は**撮り直していない**（窓が要る）ので、
  絵の下に「これは 2026-09-17 より前の絵で、パネルはもうこの文を言わない」と注を付けた。
* **The HUD** の節を丸ごと書き直し。見出しも `"frames per decision"` → `"instructions per decision"`。
  新しい数は `--headless 25` を 1 回走らせて取った実測（1392 パス、246.3 命令、ビートル 111〜196、
  ウサギ 377〜714、ゲームが答えた質問 407 件が 1.000 フレーム）。
  **古い数（403 と 2352）は「これは G4 のときの測り方で、今は測れない」という説明の中にだけ残した。**
  headless の出力例と `vm:` の例も、今の実物を貼り直した（前の例は
  `waiting for a component read — [:Creature]` で、もう出ない画面だった）。
* 比較表の「a round trip, measured (G4)」の行と、G8 の表の「frames per decision」の行。
  G8 の行は**測り直していない**ので数字はそのままにして、下に「この行が名指している数はもう存在しない」
  と 1 行付けた。勝手に書き換えるより、いつ取られた数かが分かる方がよいと判断した。
* `prelude.rb` の行数（464 → 472）。コメントを足したので。

### 5.2 `docs/sabiruby-battle.md`

VM パネルの文（`:525`）から「`waiting for a component read — [:Hunger]`」を外した。

もう 1 か所、指示に無いが**嘘になっていた**ところがある。
「Why the robots do not use `Entity#[]`」の中の
「**A component read is no quicker than a question any more** — both are one frame」。
読みが 0 フレームになったので、これは逆になった。ここは Battle の設計判断の根拠なので、
書き換えるのではなく**経緯を残す**形にした（「この項目は 2 度動いた: 2 フレーム対 1 → 1 対 1 →
今は 1 対 0」）。そのうえで「**下の箇条書きは覆らない**」と明記した——
1 事実 1 質問なのも、読みにはノイズが乗せられないのも、`Rubevy.find` が世界を舐めるのも変わらない。
変わったのはレイテンシの議論だけ。

同じ節の表（`questions per decision` / `frames`）は、下 2 行の `frames` が古くなった。
**書き換えていない**: 5 と 10 の内訳（`Entity#[]` が何回、`Proxy` が何回）がどこにも書かれていないので、
正しい数を出すには測り直しが要る。表の下に「下 2 行の frames は古い。測り直していないので
直った数はここに無い」と書いた。**根拠の無い数を書かない**（`/home/kishima/book/CLAUDE.md`）。

### 5.3 `ruby/prelude.rb` ×2

garden の方は 4 か所:

* 冒頭の「Every read costs one frame: …」→ 読みは 0 フレーム、ゲームが答える 2 つの質問は 1 フレーム、
  **書きは今もフレームの末尾**（だから同じフレームで書いた値は読めない）。
  「1 パスが 3〜4 個読んで寝る」の理由も書き換えた:
  「読みが高いから」ではなく「**1 秒に 5 回考えるのが生き物で、60 回考えるのは意見を持った物理演算だから**」。
* `act` の上の **wheel** のコメント（`:97-107`）。指示どおり
  「書きは末尾反映のまま＝ wheel は変わらない」を残し、パスの長さだけ直した。
* `hunger` / `sight` / `here` / `body` の上の「Each of these is one frame.」→
  「この tick の中で答えが返る: フレームは食わない、2.2 µs くらい」。
* `place_of` の「an entity has to be asked (one frame)」→「読むだけ（フレームは食わない）」。
  ただし**「答えが nil になることは減っていない」**を足した:
  `garden.nearest` の答えは今も 1 フレーム古いので、その間に草は食べられる。

sabibots の方は `:104-107` も `:250-254` も**直すところが無かった**（どちらも `act` ＝
ゲームが答える質問と、その last-writer-wins の話で、どちらも変わっていない）。
代わりに冒頭に 1 段落足して、**なぜここには直すところが無いのか**を書いた——
Battle はコンポーネントを 1 つも読まない、タンクが知れるのは規則がノイズを乗せた「読み取り値」であって
真値ではないから、と。

### 5.4 `docs/plans/garden-plan.md` と `docs/README.md`

garden-plan の状況表に G10 を 1 行（済みの分と、**順序は戻して著者判断待ち**であること）。
README の目次に worklog を 1 行と、`garden.md` の説明文の「frames per decision」を差し替え。
rubevy 側の `sync-access-plan.md` §6 の状況表は**触っていない**（本体が更新する）。

---

## 6. 確認（すべて実際に走らせた）

| 何を | 結果 |
|---|---|
| `cargo build --release`（両ゲーム） | 通る |
| `cargo build -p garden -p sabibots`（窓あり、dev プロファイル。実行はしていない） | 通る（2 分 58 秒） |
| `cargo test -p rubevy-arena` | **6 件通過**（消した 1 本ぶん、7 → 6）。失敗 0 |
| `cargo clippy -p garden -p sabibots -p rubevy-arena` | garden 13・sabibots 12・rubevy-arena 2。**着手前と同じ**（`git stash` して同じコマンドで取り直して比べた） |
| `GARDEN_SELFTEST=1 ./target/release/garden --headless 90` ×3 | **10 判定すべて ok、3 回とも** |
| `SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 25` ×3 | **すべて ok、3 回とも** |
| `web/build.sh garden` | 通る（`web` プロファイル 3 分 15 秒、`garden/game_bg.wasm` 39,426,931 バイト / gzip 10,115,524）。**`wasm-opt` がこの機械に無い**ので「shrunk されていない」と出る。サイズの比較はしていない |
| 文書の相対リンク | 変えた 5 つの文書から拾って全部存在を確かめた。切れ 0 |

窓ありの**実行**はしていない（指示どおり）。なので `docs/garden.md` の
「The window's own checks」のログ例の最後の行は、新しい列名だけ書いて数字は空けてある（§5.1）。
スクリーンショット `garden-vm.png` も撮り直していない。

閾値は 1 つも動かしていない: `SHORTEST_SLEEP = 0.05`、`TOUCH_SETTLE = 1.5`、`NEWBORN_GRACE = 2.0`、
`watch_turning` の 0.5 秒と `dot < 0.7`、`ScriptWorld::frame_time = 8 ms`。
新しく置いた数も 1 つも無い（`decisions` と `decision_instructions` は数えるだけで、閾値を持たない）。

---

## 7. 置いていくもの

1. **順序（§2）。** ブランチには入っていない。著者の判断が要る。
2. **`garden-vm.png`。** パネルがもう言わない文（`waiting for a component read — [:Transform]`）を
   写している。窓が要るので撮り直せない。絵の下に注を書いた。
3. **窓ありセルフテストのログ例の数字**（`docs/garden.md`）。同じ理由。
4. **`docs/sabiruby-battle.md` の「questions per decision / frames」の表の下 2 行。**
   読みが 0 フレームになったので `frames` が古いが、5 と 10 の内訳が書かれていないので直せない。
   測り直せば直せる（Battle の robots を `Entity#[]` で書き直す実験が要る）。
5. **`wasm-opt`。** この機械に無いので、wasm のサイズが前と比べてどうなったかは言えない。

---

# 8. 追記: 著者判断「選択肢 2」— 順序を入れ、6 つ目の判定の原因を測り直した

§2 で止めた件に著者の判断が出た（**選択肢 2: 順序を入れて、ハンドラか判定を作り直す**）。
指示は 5 つ:
(1) chain を `.before(RubevySet::Tick)` に、
(2) `flee_from` の向きの計算を直す——「踏まれている最中は距離がほぼ 0 なので、位置差ではなく相手の進行方向の反対に逃げる」、
(3) 閾値は動かさない、
(4) garden を 10 回、
(5) VM パネルに「予算切れで残った読み」の枝を戻す。

**(1)(3)(4)(5) はやった。(2) はやっていない**——測ったら前提が違っていたので。以下その記録。

## 8.1 まず `flee_from` が何を見ているかを測った

(2) の前提は「踏まれている最中は自分と相手の距離がほぼ 0」。確かめるために、
`flee_from` の中から `span`（2 体の中心間距離）と `me[:Collider]` / `thing[:Collider]` の半径の和を
`Rubevy.log` で出した（**この計測自体が同期読みの産物**で、フレームを食わずに 3 つ読める）。

```
PROBE span=1.292 sum=0.9 vel=[-1.109, 2.571]     ← on(:touched)  ウサギ→ビートル
PROBE span=1.26  sum=0.9 vel=[-1.903, 2.054]     ← 同上
PROBE span=0.8   sum=0.8 vel=[0.257, 1.942]      ← on(:bumped)  ビートル↔ビートル
PROBE span=0.9   sum=0.9 vel=[-1.835, -1.449]    ← on(:bumped)  ビートル↔ウサギ
PROBE span=1.1   sum=1.1 vel=                    ← on(:bumped)  木（Velocity 無し）
```

**2 つのハンドラで形がまったく違う。**

* `on(:bumped)` は `span == sum` **ちょうど**。`separate` が 2 体を半径の和まで押し戻した
  そのフレームに publish するので、当たり前だが位置差は衝突ソルバの出力そのもの。
  ここは前提どおり「位置差は向きとして頼りない」。
* `on(:touched)` は `span` が **1.25〜1.30**、半径の和は 0.9。
  `startle` が `TOUCH_REACH = 1.3` で publish するからで、**2 体は重なっていない**。
  距離は「ほぼ 0」ではなく、半径の和の 1.4 倍ある。

そして 6 つ目の判定が見ているのは `on(:touched)` の方。つまり
**「距離がほぼ 0 だから位置差が振れる」という前提は、この判定が測っている場面には当てはまらない。**

ついでに、境界に使える量の話。Ruby から根拠をもって導ける距離は
**2 体の `Collider` の半径の和**（`separate` が押し戻す先、そこが「触れている」の定義）だけで、
これは 0.9。`touched` の 1.29 には届かないので、この境界では新しい枝が**そもそも発火しない**。
`TOUCH_REACH` は Rust の const で Ruby からは見えず、見せるには
（新しい質問／新しいコンポーネント／payload の変更のどれか）別の設計判断が要る。
数字を prelude に書き写すのは「同じ数が 2 か所」になるので採らない。

## 8.2 では何が外れているのか — 判断の中身を出した

`flee_from` の中で、判断に効く値を全部出した:
`@course`（今の進路）、`escape`（位置差から出した逃げる向き）、`side`（swerve の左右）、
`out`（最終的に `act` した向き）、`theirs`（相手の進行方向）。
Rust 側では `watch_turning` が外した接触の `was` / `is` / `dot` を出した。時刻で突き合わせる。

**外れ 1**（`dot 0.780`、`is` の速さ 9.0 =`DASH`）:

```
PROBEMISS beetle 95v0 at 15.209 sampled 15.711 dot 0.780 was Vec2(-1.812, -0.390) is Vec2(-8.050, 4.025)
PROBE t=15.209 span=1.292 mine=-167.8 escape=-125.7 side=1.0 out=-68.4  theirs=-109.0
PROBE t=15.711 span=1.478 mine=-68.4  escape=-149.3 side=-1.0 out=-206.6 theirs=78.8
```

読み方: 15.209 のハンドラは `-167.8°` で歩いていたビートルを **`-68.4°`** に向けた。99° の転回で、
判定が求める 45.6°（`dot < 0.7`）を大きく超えている。**ハンドラは完璧に仕事をしている。**
ところが 15.711——**判定がのぞいたまさにそのフレーム**——に **2 通目の `"touched"`** が届き、
そのハンドラは「今の進路 `-68.4°`」を基準に swerve を選んで `-206.6° = +153.4°` に向け直した。
`is` は `atan2(4.025, -8.050) = 153.4°` で、ぴったり一致する。
2 通目は 1 通目の進路からは 138° 回っているのに、**判定が覚えている `was`（2 通前の進路）からは 38.7° しかない。**

**外れ 2**（`dot 0.867`、`is` の速さ 2.0 =`CRUISE`）:

```
PROBEMISS beetle 113v0 at 69.085 sampled 69.588 dot 0.867 was Vec2(1.927, -0.534) is Vec2(1.405, -1.424)
PROBE t=69.085 span=1.289 mine=-15.5 escape=68.2 side=1.0 out=125.5 theirs=87.4
```

こちらはハンドラが `-15.5°` → `125.5°`（141° の転回）にしたあと、窓の中に 2 通目は無い。
`is` の速さが `CRUISE` なので、**`sleep 0.5` が明けて `drop_wheel` し、脳の `wander` が舵を取り戻した**姿。
ハンドラが舵を握るのは 0.5 秒、判定がのぞくのは 0.502 秒後（`gap` は毎回 0.502）——
**同じ 0.5 が両側にある**ので、1 フレーム早まると脳の `act` が窓の中に入る。

**どちらも「逃げる向きが元の進路に近かった」ではない。** 出した向きは 99° と 141° 離れている。
判定が見た向きが元の進路に近いのは、**その後に別の `act` が入った**から。
だから (2) の修正（向きの計算を変える）は、この 2 つのどちらにも効かない。

## 8.3 順序を入れた状態で 10 回

(2) を入れずに、(1) と (5) だけの状態で `--headless 90` を 10 回:

| 回 | 6 つ目の判定 | 数えた接触 | 外した接触 | 他の判定 |
|---:|---|---:|---:|---|
| 1 | **FAIL** | 25 | 1 | ok |
| 2 | ok | 27 | 0 | ok |
| 3 | **FAIL** | 27 | 3 | ok |
| 4 | ok | 29 | 0 | ok |
| 5 | ok | 17 | 0 | ok |
| 6 | ok | 28 | 0 | ok |
| 7 | **FAIL** | 29 | 1 | ok |
| 8 | ok | 28 | 0 | ok |
| 9 | **FAIL** | 28 | 2 | ok |
| 10 | ok | 10 | 0 | **FAIL**「a hungry creature with a plant in sight reached it (it started 5.0 away and got no closer than 2.3)」 |
| 計 | **6 つ目は 10 回中 4 回落ちる** | 248 | 7（2.8%） | 何かが落ちたのは 10 回中 5 回 |

外した 7 件の `was` / `is`（指示 4 のとおり）:

```
run 1  beetle 101v0 at  5.858 sampled  6.361 dot 0.722 was Vec2(-1.777, -0.586) |1.87| is Vec2(-4.221, -7.949) |9.0|
run 3  beetle 100v0 at 22.989 sampled 23.495 dot 1.000 was Vec2( 1.233, -1.575) |2.0|  is Vec2( 5.682, -6.980) |9.0|
run 3  beetle 149v0 at 60.885 sampled 61.389 dot 0.793 was Vec2( 0.392, -1.961) |2.0|  is Vec2( 6.771, -5.929) |9.0|
run 3  beetle 114v0 at 81.339 sampled 81.845 dot 0.791 was Vec2( 1.506,  1.316) |2.0|  is Vec2( 0.386,  1.962) |2.0|
run 7  beetle 114v0 at 55.800 sampled 56.303 dot 0.764 was Vec2( 0.094,  1.998) |2.0|  is Vec2(-5.479,  7.140) |9.0|
run 9  beetle 119v0 at 15.407 sampled 15.911 dot 0.798 was Vec2( 1.414, -1.414) |2.0|  is Vec2( 8.914, -1.241) |9.0|
run 9  beetle  99v0 at 55.916 sampled 56.418 dot 0.995 was Vec2(-0.526, -1.930) |2.0|  is Vec2(-3.188, -8.417) |9.0|
```

**7 件のうち 6 件は `is` の速さが 9.0 =`DASH`**——ビートルは全速力で逃げている最中で、
外れているのは「逃げ先が `was` から 5°〜44°」という点だけ。§8.2 の外れ 1 と同じ形（後から来た 2 通目）。
残る 1 件（run 3 の 114v0）は `CRUISE` で、外れ 2 と同じ形（脳が舵を取り戻した）。
`dot 1.000` と `dot 0.995` の 2 件は、2 通目の逃げ先が**元の進路とほぼ同一**になった例で、
「向きの計算が悪い」では説明できない（1 通目からは大きく回っている）。

run 10 の 5 つ目の判定の失敗は**1 回だけ**。順序を入れていない 14 回では 6 つ目以外が落ちたことは無く、
順序を入れた 22 回で 1 回。1 例なので何も言えないが、記録しておく。

## 8.4 それ以上直していない

指示 4 の「10 回のうち 1 回でも落ちたら、落ちた接触の `was`/`is` を出して報告し、それ以上は直さない」に従う。
閾値は 1 つも動かしていない（`0.5` 秒、`dot < 0.7`、`TOUCH_SETTLE = 1.5`、`NEWBORN_GRACE = 2.0`）。
ハンドラの `sleep 0.5` も動かしていない。`flee_from` は 1 文字も変えていない。

測った範囲で言えることだけ書く。**2 つのモードは、どちらも `flee_from` の外にある。**

* **モード A（7 件中 6 件）**: 測っている 0.5 秒の窓の中に **2 通目の `"touched"`** が届く。
  `TOUCH_SETTLE` は「数える接触の**前** 1.5 秒にメッセージがあったら数えない」という除外で
  （G4 が入れたもの）、**後**から来るメッセージは除外していない。
  2 通目のハンドラは「そのときの進路」から swerve を選ぶので、
  判定が覚えている 0.5 秒前の `was` とどういう角度になるかは誰も面倒を見ていない。
* **モード B（7 件中 1 件）**: ハンドラが舵を握る `sleep 0.5` と、判定がのぞく 0.5 秒後が
  **同じ数**で、順序を固定して反射が 1 フレーム早まったぶん、`drop_wheel` の後の
  脳の `act` が窓の中に入る。

どちらも判定（またはハンドラ）の作り直しで、どちらも著者の領分です（閾値に触るので）。
思いつく方向だけ挙げておく——**選ばずに置きます**:

1. モード A: `watch_turning` が待っている間に同じビートルへ 2 通目が publish されたら、
   その接触は**数えない**（`startle` は既に `test.last_touch` でメッセージの時刻を持っている）。
   これは閾値ではなく**除外条件**の追加で、G4 が「前 1.5 秒」に入れたものの対称形。
2. モード B: ハンドラが舵を握る時間と判定がのぞく時刻が同じ数であることをやめる。
   どちらを動かすかは仕様（0.5 秒以内に反射が見える）と実装（ハンドラの `sleep`）のどちらを動かすかの話。
3. 判定の側を「**接触の 0.5 秒後の向き**」ではなく「**接触から 0.5 秒の間に一度でも向きが変わったか**」にする。
   これは「反射が見える」の意味そのものの作り直しになる。

## 8.5 やったこと

### (1) 順序

`garden/src/main.rs` の規則 chain に `.before(RubevySet::Tick)`。
コメントに「なぜ意味が出たのか」（読みが Tick 時点の世界を見る）と
「前はどうだったか」（§1 の実測: tick をまたいで割れ、割れ方はフレームごとに違った）を書いた。
`hatch`（`.after(RubevySet::Answer)`）と `restore_memory` / `save_world` / `finish_restore` の
chain（同じく Answer の後）は**動かしていない**。

### (5) VM パネルの枝を戻した

`Waiting::Component` を消したのは §4.2 で「厳密には嘘」と書いたとおりで、名前と文を変えて戻した:
**`Waiting::Read`**、文は
`waiting for a component read the tick ran out of budget before answering — [:Hunger]`。
`why()` が `Rubevy::Entity` のフレームを見つけたらこれになる。
`how()`（根拠の文）は「rubevy は読みを**その tick の中で**答えるので、外から見えているということは
その tick が先に止まった——命令数の予算か `frame_time` が尽きた——ということで、
次の tick が何かを走らせる前に答える」。
単体テストも 1 本戻した（`a_component_read_left_over_names_the_component`）。7 本に戻って全部通る。

枝の意味が「普通のこと」から「フレームが足りなかった」に変わったのが要点で、
`docs/garden.md` の表と段落、スクショの注、`docs/sabiruby-battle.md` の文もそう直した。

### 6 つ目の判定が外したときに何を言うか

`watch_turning` に 1 行足した。`FAIL … (24/25)` はどのビートルのことも言わないので、
外した接触ごとに `selftest: miss  <entity> touched at …, looked at …: dot …, was … (2.0), is … (9.0)` を出す。
**`is` の長さが証拠の半分**で、`DASH` ならハンドラ、`CRUISE` なら脳。
§8.2 と §8.3 はこの行が無ければ 1 つも書けなかった。この判定は G4 でも一度 flaky で、
そのときも原因は 2 つの向きからしか分からなかった（`docs/worklog/2026-09-17-garden-G4.md`）。
**指示に無い追加なので報告に挙げる。**

## 8.6 確認

| 何を | 結果 |
|---|---|
| `cargo build --release` | 通る |
| `cargo build -p garden -p sabibots`（窓あり。実行はしていない） | 通る |
| `cargo test -p rubevy-arena` | **7 件通過**（枝を戻したので 6 → 7）、失敗 0 |
| `cargo clippy -p garden -p sabibots -p rubevy-arena` | garden 13・sabibots 12・rubevy-arena 2。**着手前と同じ** |
| garden `--headless 90` ×10 | **6 つ目が 4 回落ちる**（§8.3 の表）。他は run 10 の 5 つ目が 1 回 |
| sabibots `--headless 25` ×3 | **すべて ok、3 回とも** |
