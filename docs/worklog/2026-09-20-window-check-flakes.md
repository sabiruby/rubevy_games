# 2026-09-20 箱庭の窓のチェックの揺れ — 数え、原因を 2 つに割る（調査のみ）

`GARDEN_SELFTEST=1 docker/run.sh garden release`（PC の窓）と `garden/?selftest`（ブラウザ）が
時々落とす 3 つの判定

* `Apply restarts every beetle on the edited text`（`window.rs:1354`）
* `every restarted beetle's new task has run`（`window.rs:1366`）
* `Revert puts every beetle back on the file`（`window.rs:1382`）

について、**頻度を測り、原因を実測で特定する**ための調査（計画書 `plans/shared-crate-plan.md` の
段階 S6）。前の同じ形の調査は `docs/worklog/2026-09-17-selftest-flakes.md`（ヘッドレスの 3 件）で、
その直し方が `2026-09-18-selftest-fixes.md`。この記録はその続きで、**窓側の揺れは初めて**。

**コードは直していない。** 計測のために入れた仕掛けは全部 `// S6PROBE (temporary)` の印付きで、
コミットには入れていない（差分はスクラッチパッドの `s6-probes.patch`）。**閾値も文面も 1 つも動かしていない。**

作業場所は worktree `rubevy_games-wt-shared`（ブランチ `shared-crate`、main `04a5f53` と同じ所）。
rubevy は `Cargo.lock` の `33d851a`（R0・R1・R2・R6・R8。**R3・R4・R6b は入っていない**）。

---

## 0. 結論（先に）

揺れは 1 つの原因ではなく **2 つ**で、どちらも「判定が早すぎる」でも「判定の文面が悪い」でもない。

| 判定 | 原因 | 何が起きているか | 実物のバグか |
|---|---|---|---|
| `Apply restarts every beetle …` / `Revert puts every beetle back …` | **原因 A** | Apply / Revert と**同じフレームに生まれた個体**が `restart_species` の `Query` に見えていない（その `Mind` はまだ `Commands` の列の中）。差し替えから漏れ、`in_memory` も古いまま | **バグ**。判定を直しても、その個体は違うプログラムを走らせ続ける |
| `every restarted beetle's new task has run` | **原因 B** | 判定は壁時計で 0.6 秒待つが、必要なのは「VM のスケジューラが新しいタスクに順番を回した」こと。creature の VM の 1 フレームは `frame_time`（rubevy の既定 8 ms、ゲームは設定していない）**と** 200,000 命令で切られていて、機械が混むと 8 ms の壁時計で買える仕事が激減する | **判定の側**。ただし「混むと再起動直後の数フレーム、生き物が固まる」という事実は残る |

どちらも**証拠つきで再現した**（§3、§4）。推測で終わらせていない。
おまけに **4 つ目の揺れ**を見つけ、これも再現した（`the meters move again`、§6）——
原因 B の、creature ではなく **world の VM** 版である。

---

## 1. 数 — 1 本ずつと、同時に何本も

窓の走行は 1 回 22 秒（静かな機械）。**負荷の有無で揺れが変わるかどうかが調べる対象そのもの**なので、
「1 本ずつ」と「同時に何本も」を分けて数えた。表も分けてある。

`uptime` の load average は WSL2 では当てにならない（`vmstat` が 99% idle の時に 15〜20 を指す）ので、
**混み具合は `vmstat` の idle と、走行 1 回の壁時計**で見た。

### 1.1 PC の窓、**1 本ずつ**（静かな機械）

| 走行 | 計測 | 回数 | FAIL |
|---|---|---:|---|
| `base` | 無し | 16 | **0** |
| `seq` | 有り | 40 | **0** |
| 計 | | **56** | **0** |

走行 1 回 22〜24 秒（1 走行だけ 43 秒。他の担当が機械を使った時間帯に当たったもので、
その走行も ok 43）。Apply は実時間 7.0 秒、判定はその 0.6 秒後で、**その間に 5 フレーム**通る
（f57 → f62、約 7.5 fps。窓はコンテナの lavapipe、ソフトウェア描画）。
43 秒かかった走行でも 4 フレームあった。

### 1.2 PC の窓、**同時に何本も**（時間を区切った負荷の実験）

| 走行 | 同時 | 計測 | 回数 | FAIL |
|---|---:|---|---:|---|
| `par4` | 4 | 無し | 16 | **1** — `Apply restarts every beetle on the edited text` |
| `inst-par8` | 8 | 有り | 24 | 0 |
| `inst-par8b` | 8 | 有り | 96 | **1** — `every restarted beetle's new task has run` / **2** — `the meters move again` |
| 計 | | | **136** | 4（うち狙いの 3 件は **2 / 136 = 1.5%**） |

4 本同時で 1 回 43 秒、8 本同時で 85〜91 秒（`vmstat` の idle は 1〜3%）。
Apply から判定までは **3 フレーム**に縮む。

この 3 束は 19:27〜20:03 の 36 分で回し、そこで止めた（他の担当が同じ機械で計測しているため）。

### 1.3 ブラウザ（1 本ずつ）

`web/build.sh garden` → `web/serve.sh`（PORT=8099）→ `garden/?selftest` を Playwright で 110 秒。
1 回 1 分 50 秒。**ブラウザは放っておいても「混んだ PC」と同じ速さ**で、Apply は f30・250 ms/フレーム
（約 4 fps）、判定までは **3 フレーム**。

| 走行 | 回数 | pageerror / requestfailed | FAIL |
|---|---:|---:|---|
| `browser` | 16（160 秒 1・100 秒 1・110 秒 14） | 0 / 0 | **1** — `Apply restarts every beetle on the edited text`（`run014`） |

**その 1 件が原因 A の、仕込みでない実物である**（§3.4）。16 走行中 1 走行。
110 秒にしたのは、100 秒で 43 件すべてが出そろうことを 1 走行で確かめたうえの余裕である
（S4 の担当のスクリプトは 150〜160 秒だった）。
`-- `（測れなかった判定）の数は走行ごとに 0〜6 と大きく振れるが、`fixedlines.sh` が落とす行なので
一覧は 44 行で一定。

### 1.4 走行そのものは基準どおり

落ちていない走行は `verification/selftest-lines.md` の一覧と一致している。
`tools/fixedlines.sh` に通した PC の窓の 43 行は文書の一覧と **`diff` が空**、
ブラウザは 44 行（`done` の 1 行ぶん多い）。計測を入れた版でもこれは変わらない
（`s6probe` の行は `selftest:` で始まらないので `fixedlines.sh` が落とす）。

### 1.5 何回回せば言えるか

最初の 16 走行（1 本ずつ）で 0 件だったので、「1 本ずつでは滅多に出ない」ことは分かった。
そこから先は**頻度を詰めるのではなく、原因を割る**方に振った——理由は §3・§4 のとおり、
**原因が 2 つとも人の手で再現できた**からである。前回（2026-09-17）は 98 走行で頻度を出したが、
あれは「落ちる条件を作れなかった 7 番」があったからで、今回は両方作れた。
率を 1% の精度で測るには数百走行が要り、それは同じ機械を何時間も塞ぐことになる。

---

## 2. 入れた計測（全部戻す。コミットしない）

印は `// S6PROBE (temporary)` だけ。5 つの計測と 1 組のつまみ:

1. **フレーム番号と時計**（`main.rs`、`S6_FRAME` / `S6_MS` / `s6_now()`）。引数を増やさずどの probe からも読める。
2. **`give_mind`**: 生まれた個体の entity・種・`in_memory`・そのときのフレーム。
3. **`restart_species`**: 差し替えた個体を 1 行ずつ（フレーム・entity・`in_memory`）。
4. **判定の前後の全個体**（`s6dump`）: step 7 / 8 / 9 / 10 で、甲虫ごとに `in_memory`・
   `last_instructions`・`ScriptTask` の有無・VM が数えているそのタスクの命令数。
5. **毎フレームの姿**（`s6_trace`）: 2 つの「息継ぎ」の間、甲虫と兎それぞれの `Script` / `ScriptTask` /
   タスクの命令数と VM の budget。
6. **4 つのつまみ**（環境変数。既定では何もしない）:
   * `GARDEN_S6_BREATH` — 0.6 秒の息継ぎを伸び縮みさせる
   * `GARDEN_S6_FORCE_BIRTH` — Apply / Revert の**そのフレーム**に `Births` へ 1 件積む
   * `GARDEN_S6_FRAME_TIME_US` — creature の VM の `frame_time` を縮める
   * `GARDEN_S6_WORLD_FRAME_TIME_US` — world の VM の `frame_time` を縮める

`GARDEN_S6_FORCE_BIRTH` は最初、probe が自分で `spawn_creature` + `give_mind` していた。
**それでは再現しない**——`.before(do_editor_actions)` という順序を付けた時点で Bevy がそこに
`ApplyDeferred` を差し込み、`Mind` が同じフレームで見えるようになってしまう。本物の
`children_arrive` にはその順序が無い。`Births`（Resource、書き込みは遅延しない）に積んで
**本物の `children_arrive` に作らせる**形に直して、初めて再現した。
これは probe を書くときの落とし穴そのものなので残しておく。

---

## 3. 原因 A — Apply / Revert のフレームに生まれた個体は差し替えから漏れる

### 3.1 見つかり方

`par4` の唯一の FAIL（`run009`）で、判定の直前のログがこうなっていた:

```
10:34:58.319622  selftest: ok   typing marks the text edited        ← step 7、Apply を頼む
10:34:58.324055  a Beetle was born at 4.5 s (381v1) …               ← 4.5 ms 後、同じフレーム
10:34:59.805319  selftest: FAIL Apply restarts every beetle on the edited text
10:34:59.805357  selftest: ok   a beetle born from now on is born running it
```

落ちたのは 1 件だけで、「これから生まれる甲虫は新しい本文で生まれる」は通っている。

### 3.2 仕組み（読んで分かること）

* `children_arrive`（`main.rs:3779`、`.after(RubevySet::Answer)`）が体を作り、`give_mind` が
  `commands.entity(entity).insert((Script, Mind { in_memory: applied.is_some(), … }))` を積む。
* `do_editor_actions`（`window.rs:325`、`.after(watch_minds)`）は `brains.set(species, Some(text))` を
  してから `restart_species` を呼ぶ。`restart_species` は `Query<(Entity, &mut Mind)>` を回る。
* 2 つのシステムのあいだに**順序の辺が無い**ので、Bevy は同期点を挟まない。
  `children_arrive` が先に走ったフレームでは、その子の `Mind` はまだ列の中で、`Query` に見えない。

つまりその子は:

* `give_mind` の時点で `brains.text(species)` がまだ `None` なので `in_memory = false` を貰い、
* `restart_species` に拾われないので `false` のまま、
* **ファイルの側のプログラムを走らせ続ける**。

Revert はその鏡で、`in_memory = true` のまま**メモリ上の本文を走らせ続ける**。

### 3.3 実測 — そのフレームに 1 件生ませる

`GARDEN_S6_FORCE_BIRTH=1` で Apply のフレームと Revert のフレームに 1 件ずつ積んだ。
**3 走行 / 3 走行とも、2 件とも落ちた**:

```
s6probe forced birth asked: f54 t6.958 in the step-7 frame
s6probe give_mind: f54 t6.958 661v0 Beetle in_memory=false
a Beetle was born at 4.6 s (661v0) …
s6probe restart: f54 t6.958 404v0 Beetle in_memory=true      ← 差し替えた 10 体に
s6probe restart: f54 t6.958 408v0 Beetle in_memory=true
… （661v0 は無い）…
s6probe step8-check: f59 t7.601 404v0 in_memory=true  last_insn=1280
s6probe step8-check: f59 t7.601 661v0 in_memory=false last_insn=1280   ← これ 1 体
selftest: FAIL Apply restarts every beetle on the edited text
```

```
s6probe forced birth asked: f127 t16.612 in the step-9 frame
selftest: FAIL Revert puts every beetle back on the file
```

`restart` の 10 行に `661v0` が**無い**のが決め手で、体も `Mind` も出来ているのに
`Query` には居ない。判定の文面は正しく、**判定は本当のことを言っている**。

### 3.4 仕込みでない実物 — ブラウザの `run014`

ブラウザの 14 走行のうち 1 走行が、**つまみを何も使わずに同じ形で落ちた**。
出産のフレーム（`f30`）と Apply のフレーム（`f30`）がちょうど重なった走行である:

```
s6probe give_mind: f30 t6.811 552v0 Beetle in_memory=false
s6probe restart:   f30 t6.811 1003v0 Beetle in_memory=true    ← 差し替えた 10 体。552v0 は無い
…（10 行）…
s6probe step8-check: f33 t7.561 552v0 in_memory=false last_insn=1000 task=true
s6probe step8-check: f33 t7.561 1003v0 in_memory=true last_insn=1000 task=true
selftest: FAIL Apply restarts every beetle on the edited text
```

`552v0` のタスクは 1000 命令走っている（生きている）のに `in_memory` が `false` のまま——
**ファイルの側のプログラムを走らせている**。§3.3 の仕込みと 1 行ずつ同じ形である。

### 3.5 なぜ滅多に出ないのに、出るときは出るのか

窓は当たりが 1 フレーム。静かな機械の 7.5 fps で 133 ms、混むと 250〜280 ms、ブラウザも 250 ms。
一方、出産は運任せではない——前回の調査（§4.3）と同じで、**judge の仕込みが山を作る**:

* `plant_the_meadow` が起動時に置く 2 匹（`main.rs:2463`）は必ずつがいになり、
  その子は世界の 1.2〜2.0 秒に生まれる（48 走行のほぼ全部でフレーム 8〜11）。
* 2 人目以降は世界の 4〜17 秒に散る。**Apply は実時間 7.0 秒 = 世界の 4.5〜5.0 秒**（間に `P` の
  2 秒があるため）で、ちょうどその散らばりの頭に重なる。

実際、136 走行のうち「Apply のフレームの 1 つ前後に出産」が数件あった（`inst-par8/run020` は
出産 f28・Apply f29 の**1 フレーム違い**）。当たれば落ち、外れれば通る。
機械が混むとフレームが長くなり、的が 2 倍に広がる。

---

## 4. 原因 B — 新しいタスクに、まだ順番が回っていない

### 4.1 落ちた走行の姿

`inst-par8b/run035`（8 本同時）:

```
s6probe restart:     f28 t6.787  （10 体、in_memory=true）
s6probe step8-check: f31 t7.537 422v0 in_memory=true last_insn=0 task=true task_insn=Some(0)
…（10 体すべて task_insn=Some(0)）…
selftest: FAIL every restarted beetle's new task has run
```

**10 体全部が 0。** 1 体だけ取り残されたのではなく、3 フレーム・750 ms のあいだ
**どの新しいタスクにも 1 命令も回っていない**。同じ 3 フレームで通った走行では 854〜1034 命令。

### 4.2 仕組み

* `restart_species` は `replace_script`（rubevy R6）で `ScriptTask` を外し新しい `Script` を積む。
  `mind.last_instructions = 0` に戻る。
* 次のフレームの `start_scripts` が `task_spawn` してタスクを作り、`RubevySet::Tick` が走らせる。
* その `tick_scripts`（rubevy `lib.rs:2222`）のループは、頭で **budget（命令）と `frame_time`（壁時計）**の
  両方を見て、どちらか尽きたら抜ける。creature の VM は budget も `frame_time` も **rubevy の既定のまま**
  （200,000 命令と 8 ms）。`grep -n frame_time garden/src/main.rs` が指す 4 箇所は全部 world の VM か HUD の話で、
  ゲームが creature の VM について決めている数は 1 つも無い。world の VM だけが budget 45,000 を
  30 行のコメント付きで測って決めてある（`main.rs:4330`、その中に「`frame_time` は rubevy の 8 ms のまま」とある）。
* 8 ms は**壁時計**なので、CPU を取り合っている機械では買える命令数が桁で落ちる。
  新しいタスクは順番の後ろにいるので、そこまで届かないフレームが続きうる。

### 4.3 実測 — `frame_time` を縮めると、静かな機械で同じものが出る

`GARDEN_S6_FRAME_TIME_US` で creature の VM の `frame_time` だけを縮めた（負荷は掛けていない）。

**300 µs** — 途中まで届いて切れる姿がそのまま見える:

```
s6probe trace: f59 … R430v0[i=Some(7399)] R424v0[i=Some(6279)]
               B414v0[i=Some(854)] B416v0[i=Some(854)] B418v0[i=Some(0)] B422v0[i=Some(0)] …
s6probe trace: f60 … B418v0[i=Some(854)] B422v0[i=Some(854)] …
```

兎（`R`）は前の値のまま（寝ている）、甲虫（`B`）は**順に 854 まで進み、途中の 2 体が 0 で切れている**。
次のフレームで残りが進む。判定までには全部間に合うので、この値では落ちない。

**100 µs** — 落ちる。しかも**落ちるのはこの 1 行だけ**:

```
s6probe frame_time set to 100 us
s6probe step8-check: f62 t7.626 412v0 in_memory=true last_insn=74 task=true
s6probe step8-check: f62 t7.626 420v0 in_memory=true last_insn=0  task=true
…（9 体が 0）…
selftest: FAIL every restarted beetle's new task has run
```

`run035` と同じ形が、静かな機械で、つまみ 1 つで出た。**40 µs** まで縮めると
`and the garden moves again: somebody has walked` も一緒に落ちる（世界が止まり始める）。

### 4.4 息継ぎが何フレームなら足りるのか

`GARDEN_S6_BREATH` で 0.6 秒を 0.05 秒（= 1 フレーム）にすると、判定の時点で
**`ScriptTask` がまだ無い**（`task=false`、2 走行 / 2 走行）:

```
s6probe restart:     f56 t7.047
s6probe step8-check: f57 t7.185 414v0 in_memory=true last_insn=0 task=false task_insn=None
```

`Script` は `Commands` 経由なので次のフレームに見え、そこで `start_scripts` がタスクを作る。
**最低 2 フレーム**要る（`NEWBORN_DEAF_FRAMES = 2` がまさに同じ勘定で置かれている、
`main.rs:286`）。0.6 秒という**秒での待ち**が、静かな機械では 5 フレーム、混むと 3 フレーム、
ブラウザでも 3 フレームになる——そして 3 フレームあっても、VM が届かなければ足りない。

---

## 5. 今日入った変更との関係

* games の `Cargo.lock` は rubevy **`33d851a`**（R0・R1・R2・R6・R8）を指していて、
  **R3・R4・R6b は入っていない**。この調査は全部その版で測った。
* 原因 A は S1 より前から在る形で、`restart_species` と `children_arrive` の順序の話である。
  S1 以降の `garden/src/window.rs` の差分（`git diff 8f836d4 HEAD`）に
  `restart` / `in_memory` / `step = ` / `test.at = now` / `Apply` / `Revert` を含む行は**1 行も無い**ので、
  判定も差し替えも S1 当時のままである。**前後で比べる意味が無かった**ので、昨日の版は建てていない。
* **「今日の変更の前後で頻度が変わったか」は比べていない。** games 側は上のとおり判定も差し替えも
  S1 当時のままなので、比べるべき差が無い。rubevy 側は lock が今日の R3・R4・R6b より前の
  `33d851a` を指したままなので、「前」と「今」が同じ版である。**R3・R4・R6b 入りでの頻度は未測定。**
* 原因 B は `frame_time` の話で、R4（「`frame_time` を上限に。時間切れのフレームの末尾の読みは
  1 フレーム遅れる」）が**まさにこの道を触っている**。R4 が入ると時間切れの判定がタスクの実行だけでなく
  答えにも効くので、**混んだ機械での原因 B は今より出やすくなる可能性がある**。R4 入りでは測っていない
  （games の lock がまだ指していないため）。入れるときに測り直すべきである。
* S1 が 3 走行中 2 走行で落としたのは「コンテナのビルドの直後」、つまり cargo が回り終わった直後の
  **混んだ機械**である（`2026-09-20-shared-platform.md` §0「機械が温まっている時間帯」）。
  ここで測った「1 本ずつ 16 走行で 0 件、8 本同時 136 走行で 4 件」と矛盾しない。
  ただし **2/3 という率そのものはここでは再現していない**——§8 に書く。

---

## 6. 4 つ目の揺れ — `the meters move again`（範囲外だが記録）

96 走行（8 本同時）で 2 回落ちた。計画書が挙げた 3 件には無い行である。

判定は「`P` で再開して 0.5 秒後、誰かの `Hunger` が変わっている」（`window.rs:1302`）。
`Hunger` を書いているのは **`world.rb` の `each_frame`**（`garden/ruby/world.rb:197`・`241`）で、
world の VM は budget 45,000・`frame_time` 8 ms を持つ。
つまり**原因 B と同じ形**——数フレームのあいだ world の規則が 1 パスも回らなければ、
メーターは 1 つも動かない。同じ走行で `the day turns again`（Rust の太陽）と
`somebody has walked`（Rust の移動）は通っている、という並びがそれを裏づける。

**確かめた。** `GARDEN_S6_WORLD_FRAME_TIME_US` を足して world の VM の `frame_time` だけを縮めた
（負荷は掛けていない。creature の VM は既定のまま）:

| world の `frame_time` | 結果 |
|---|---|
| 400 µs | ok 43（落ちない） |
| 100 µs | ok 43（落ちない） |
| **20 µs** | **`FAIL the meters move again` だけ**が落ちる |
| 5 µs | 同上、落ちるのはやはりこの 1 行だけ |

つまり `the meters move again` は**原因 B の world VM 版**で、機構は同じである。
混んだ機械で 8 ms の壁時計が買える仕事が落ちると、`world.rb` の `each_frame` が
`Hunger` を書く行（`world.rb:197`・`241`）まで届かないフレームが続き、0.5 秒後の判定が空振りする。

---

## 7. 直し方の候補（選ばない。実装もしていない）

### 7.1 原因 A（`Apply restarts …` / `Revert puts …`）

**これは判定ではなくゲームのバグ**なので、「判定を n/a にする」案は最初から弱い。

| 案 | どこを変える | 数 | 判定の文面 | 振る舞い |
|---|---|---|---|---|
| **A1. 差し替えを「種の世代」で言う** | ゲーム（`Brains` と `give_mind`） | 動かさない | 変わらない | **変わる（正しくなる）** |
| A2. `children_arrive` を `do_editor_actions` の後ろに置く | ゲーム（順序） | 動かさない | 変わらない | 変わる（この 1 フレームだけ） |
| A3. 判定が「生まれたて」を数えない | 判定 | **`NEWBORN_GRACE` が使える** | **変わる** | 変わらない（バグは残る） |

* **A1（推奨）**: `Brains` に「今の本文の世代番号」を持たせ、`Mind` にも持たせて、
  `restart_species` は世代を進め、毎フレーム「世代が古い `Mind` を差し替える」システムが拾う。
  `Query` に見えた時点で拾われるので、同じフレームに生まれようが 10 フレーム後に生まれようが同じ。
  **新しい数を 1 つも置かない**。`give_mind` の `in_memory: applied.is_some()` も世代から出る。
  代償は「差し替えが 1 フレーム遅れうる個体がいる」こと——が、それは今も同じで、今は
  **永久に遅れる**のが違う。
* **A2**: `.before(window::do_editor_actions)` を `children_arrive` に付ける。1 行で、
  Bevy が同期点を差し込むので同じフレームの子も `Query` に見える（§2 でこれを実測している）。
  ただし **`do_editor_actions` は窓のときしか登録されない**ので、順序の辺を窓の側に書くことになり、
  「ヘッドレスと窓でシステムの順序が違う」が増える。ブラウザ／窓だけの直しになる。
* **A3**: `beetles()` から `age <= NEWBORN_GRACE`（2.0 秒、`main.rs:273`）を除く。
  7 番の直し（案 G、`2026-09-18-selftest-fixes.md`）と同じ形で**新しい数は要らない**。
  文面は「（生まれたてを除いて）全部の甲虫が」になる。**バグは残る**ので、A1 か A2 と一緒でなければ
  「揺れを隠しただけ」になる。

### 7.2 原因 B（`every restarted beetle's new task has run`）

| 案 | どこを変える | 数 | 判定の文面 | 振る舞い |
|---|---|---|---|---|
| **B1. 秒で待つのをやめ、フレームで待つ** | 判定 | **導ける**（2 フレーム + 余裕） | 変わらない | 変わらない |
| **B2. 「回った」を待つ（条件で待ち、期限で諦める）** | 判定 | 期限は 1 つ | わずかに変わる（`--` が増えうる） | 変わらない |
| B3. 息継ぎを 0.6 秒から伸ばす | 判定 | **導けない** | 変わらない | 変わらない |
| B4. creature の VM の `frame_time` をゲームが決める | ゲーム | **測って決める** | 変わらない | **変わる** |

* **B1（推奨）**: `test.at` を「時刻」ではなく「フレーム番号」にする。最低 2 フレーム
  （`Script` が見える 1 + `start_scripts` が作る 1）は §4.4 の実測で出ていて、
  `NEWBORN_DEAF_FRAMES = 2` と同じ勘定・同じ出どころ。余裕を何フレーム積むかは測って決める
  （静かな機械の 5 フレーム、混んだ機械とブラウザの 3 フレームが今の実測）。
  **ただしこれは原因 B を全部は消さない**——§4.3 の 100 µs のように VM が届かないフレームが続けば、
  フレームで待っても落ちる。消えるのは「フレームが足りない」ぶんだけ。
* **B2**: 「全部の甲虫が 1 命令でも走ったら次へ。N フレーム待って走らないものが残ったら、
  そこで初めて判定する」。落ちる条件を「VM が本当に止まっている」に寄せられる。
  N は B1 と同じ根拠で出せる。文面は変えずに済むが、`--`（測れなかった）の扱いを決める必要がある。
* **B3**: 0.6 を 1.2 にする類。**その数の出どころが作れない**（混み方の上限が無い）。
  `book/CLAUDE.md` の「根拠のないマジックナンバーは禁止」に正面からぶつかる。取りたくない。
* **B4**: 今 `frame_time` は rubevy の既定 8 ms で、ゲームは creature の VM について何も言っていない。
  world の VM は budget を測って決めてある（`install_world_answers` の 30 行のコメント）のに、
  creature の VM は budget 200,000 だけが既定で、時間の側は既定のまま。
  **`book/CLAUDE.md` の「数は利用者が変えられる場所に」から見ると、ここは穴**である。
  ただし `frame_time` を伸ばせば混んだ機械でフレームが延びるので、これは**ゲームの振る舞いを変える**話で、
  判定の揺れの直し方としては遠い。著者の判断。

### 7.3 どちらでもない案として捨てたもの

* **「3 件を比較から外す」**——S4 までの「数で比べる」に戻ることになる
  （`verification/selftest-lines.md` の冒頭がその反省）。取らない。
* **「窓の判定を再実行して 2 回目を採る」**——今の暫定運用（`selftest-lines.md` の「Three of these
  flake」）。原因が割れた今は暫定のままにしておく理由が無い。

---

## 8. 分からなかったこと

* **S1 の「3 走行中 2 走行」という率は再現していない。** ここで測った率は、1 本ずつ 16 走行で 0 件、
  8 本同時 136 走行で 4 件（狙いの 3 行は 2 件）。S1 の走行は「コンテナのビルドの直後」で、
  cargo・docker のビルドが終わった直後の I/O とページキャッシュの状態まで同じには作れていない。
  **率が 2/3 になる条件は特定できていない。** 原因が 2 つとも再現できているので追わなかった。
* （`the meters move again` は §6 で再現したので、ここからは外した。）
* ブラウザで出たのは**原因 A だけ**（14 走行中 1 走行）。**原因 B はブラウザでは見ていない。**
  ブラウザは 3 フレーム／250 ms と「混んだ PC」と同じ姿なので出うるはずだが、
  14 走行では当たらなかった。なお S4 は 2 走行中 1 走行で
  `Revert puts every beetle back on the file`（＝原因 A の Revert 側）を見ている。

---

## 9. 戻したこと

`// S6PROBE (temporary)` の印が付いた 5 つの計測と 4 つのつまみを全部外し、
`git status --short` がこの worklog と `docs/README.md` の 1 行しか出さないことを確かめた。
差分は残してある（スクラッチパッド `s6/s6-probes.patch`、全ログは `s6/` の下）。

---

## 気づいた点

1. **`the meters move again` も揺れる**（8 本同時 96 走行で 2 件）。計画書の 3 件に入っていない
   4 つ目で、`garden/src/window.rs:1302`。`Hunger` を書くのは `garden/ruby/world.rb:197`・`241` なので
   原因 B の world VM 版で、**つまみで再現した**（world の `frame_time` を 20 µs にするとこの 1 行だけ落ちる、§6）。
   直し方は原因 B の B1 / B2 がそのまま当たる。
   → ゲーム固有（箱庭の判定）／S6 の続き。
2. **creature の VM は budget も `frame_time` も rubevy の既定のままで、ゲームはどこでも言っていない。**
   world の VM は budget 45,000 を 30 行のコメント付きで測って決めてある（`main.rs:4330`）のに、
   creature の側は budget 200,000 も `frame_time` 8 ms も既定のまま。
   `book/CLAUDE.md` の「数は利用者が変えられる場所に、理由を残して」から見ると穴で、
   `docs/numbers.md` にも「rubevy の既定」としか書けない。
   → 出どころの無い（＝既定のままの）数／rubevy と games の境界。
3. **probe に順序を付けると Bevy が同期点を差し込み、再現しようとしていた競合が消える。**
   §2 の失敗。`.before(other_system)` は「順序」だけでなく「`Commands` の反映」も変える。
   → 本（テストの書き方）の素材。Bevy の ECS を説明する章で使える。
4. **`restart_species` が `Query` を回る形は、同じフレームに生まれたものを取りこぼす。**
   `garden/src/window.rs:520`。sabibots の `restart`（同じ形）も同じ穴を持つはずで、
   Battle は走行中に robot が生まれないので今は当たらないだけである。
   → バグ（再現手順は §3.3）／共有の話になりうる。
5. **判定の待ちが全部「秒」で書かれている。** `window.rs` の `test.at = now + …` は 0.2 / 0.5 / 0.6 /
   2.0 / 9.0 の 5 種類で、どれも「何フレーム欲しいか」を秒に換算した数である。
   フレームレートが 4 倍違う環境（静かな PC・混んだ PC・ブラウザ）を 1 つの秒で賄っている。
   → 判定の作り（S6 の直し方 B1）／本の素材。
6. **`uptime` の load average が WSL2 では当てにならない。** `vmstat` が 99% idle を指しているときに
   15〜20、走行を止めた後も 40 分以上 100 を超えたままだった。混み具合を見るなら `vmstat` の idle を見る。
   → 本（計測の環境）の素材。
