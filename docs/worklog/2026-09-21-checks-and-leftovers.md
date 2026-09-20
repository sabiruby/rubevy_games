# 判定の側 (d) と、S5b-1〜4・S6〜S8 が送った宿題（S5b-5）

計画書 `docs/plans/shared-crate-plan.md` の段階 **S5b-5**——S5b の最後。
先に読んだ記録は `2026-09-20-window-check-flakes.md`（S6）、`2026-09-20-window-check-fixes.md`（S7）、
`2026-09-20-writes-landing-in-a-pause.md`（S8）、`2026-09-21-numbers-garden-settings.md`（S5b-3）、
`2026-09-21-numbers-garden-play.md`（S5b-4）、`docs/verification/selftest-lines.md`、
`tools/fixedlines.sh`、`docs/numbers.md`。
着手時は main と同じ `02d3635`、作業は worktree `rubevy_games-wt-shared` の `shared-crate`。

**著者が決めていたこと**（2026-09-21）: 生き物の VM の予算 `script_budget` の既定を
**案 A — `1.74 × 23,686` ≈ 41,000** に。それ以外の既定値は動かさない。

---

## 0. 着手前に取った基準

機械は空いていた（`vmstat 1 3` の idle **99〜100%**）。隣の worktree に前の担当の待ちのシェルが
2 つ残っていたが、cargo も docker も動いていない。

`cargo build --workspace --all-targets` 警告 0、`cargo test --workspace` **58 通過**
（21 + 10 + 18 + 6 + 3）。

前の版は `git worktree add --detach`（`git stash` は使わない規則）で `c17abe1` を
`rubevy_games-wt-s5b5-before` に出し、**別の `CARGO_TARGET_DIR`**（その worktree の `target/`）で
建てた。`md5sum` は前 `e2bb9ff1…`／後 `87a19c9a…` で別のバイナリである。docker 側も
**worktree ごとの volume**（下の §1）なので、`/target/release/garden` は前 `6c8a675d…`／
後 `8e019b96…` と別物であることを volume の中で確かめた。

---

## 1. 道具を先に直した — docker の target volume と環境変数（C-2）

**順番を変えた理由**: この段階の確認は窓の走行（docker）に寄りかかっている。ところが
`docker/build.sh` と `docker/run.sh` はどの worktree も `/app` に mount し、`/target` は
**全 worktree で 1 つの volume** だった。つまり

* cargo からはどの worktree も同じ絶対パスに見え、別の worktree の build が「何も変わっていない」
  と読まれる（印は異様に速い `Finished`。S7 の記録にある）。
* そして `/target/release/garden` は**最後に建てた worktree のもの**である。窓を開けても、
  画面には別のブランチのゲームが出ているかもしれず、それを言うものが何も無い。

2 つ目の方が悪い。**自分の前後の比較を、その罠の上で取るわけにいかない**ので、A より先に直した。

`docker/volumes.sh` を新しく置き、`build.sh` と `run.sh` の両方が source する
（同じ 1 行を 2 か所に書かないため）。volume 名は checkout のディレクトリ名から決める。
古い `rubevy-games-target` は**消さずに置いた**（`docker volume rm` は使う人が決める）。
cargo の registry は共有のまま——crate の原本はどの worktree でも同じものである。

値段は「worktree ごとに 1 回の全 build」で、この機械では **351 crate を 3 分 41 秒**だった
（24 コア）。`before` の worktree も自分の volume で 3 分 03 秒。別のブランチのゲームを
知らずに走らせる値段より安い。

**環境変数**: `docker/run.sh` は `<GAME>_SELFTEST` **だけ**を渡していた。判定は自分のつまみも
環境から読む（`GARDEN_RELOAD_AT`）ので、渡されないつまみは「効かなかったつまみ」と見分けが
つかない——S7 の担当はそれで 4 走行を捨てている。`<GAME>_` で始まる名前を**探して**全部渡す形に
した（列挙ではないので、ゲームがつまみを増やしてもここは無編集）。

確かめ方は `bash -x docker/run.sh` の最後の行:

```
GARDEN_SELFTEST=1 GARDEN_RELOAD_AT=20 GARDEN_S8_TOUCH=1 SABIBOTS_SELFTEST=1 bash -x docker/run.sh garden release --headless 1
→ … -v rubevy-games-target-rubevy_games-wt-shared:/target …
    -e GARDEN_RELOAD_AT -e GARDEN_S8_TOUCH -e GARDEN_SELFTEST … /target/release/garden --headless 1
```

`GARDEN_` の 3 つが渡り、隣のゲームの `SABIBOTS_SELFTEST` は渡らない。

---

## 2. 著者が決めた既定値 — 生き物の VM の予算（A）

### 端数をどう扱うか

著者の指定は `1.74 × 23,686` ≈ **41,000**。式どおりなら **41,214** である。
**1,000 の位に丸めた**。理由は後から作った理屈ではなく、**同じ式の先例がこの repo にある**ことだ:
世界の VM の 45,000 は `1.74 × 25,837 = 44,956` を同じように丸めた数である
（`WORLD_BUDGET` の rustdoc）。同じ式を同じように丸めた。

丸めた結果、余裕の比は `41,000 ÷ 23,686 = 1.73` になる（1.74 ではない）。
これも rustdoc に書いた——「1.74 倍だ」と書いて 1.73 の数を置くのは、数と文が食い違う例そのもの
（`docs/numbers.md` §7-5 の `CLICK_REACH` が同じ形で残っている）。

### 測り直した — 23,686 は今 23,912 である

出どころの 23,686 は S5b-3 の測定で、それは **S5b-4 の前**の庭である。書く前に同じ席に座り直した:
`garden.settings.txt` に S5b-3 と同じ 3 行（`start_plants=130`・`start_beetles=14`・
`start_rabbits=10` = `world.rb` の `pop_max` 24）を置き、`--headless 60` を前後**交互に 3 巡**、
合わせて 6 走行 21,558 フレーム。仕掛けは S5b-3 と同じで、`ScriptWorld::last_frame()`
（rubevy の `FrameStats`）を毎フレーム 1 行印字する `GARDEN_MEASURE=1` のシステム 1 本。
**コミットには入れていない**（測った後に外した。S5b-2・S5b-3 と同じ扱い）。

| | 前（200,000） | 後（41,000） |
|---|---|---|
| フレーム数（3 走行） | 10,781 | 10,777 |
| 上限の庭（株 90 以上・24 匹）のフレーム | 1,533 | 1,439 |
| 生き物 VM の命令数 中央値（上限の庭） | 362 | 312 |
| 同 99 パーセンタイル（上限の庭） | 4,036 | 4,505 |
| **全フレームの最大** | **23,912** | **23,912** |
| 最初のフレーム（f0）の命令数 | 23,912（3 走行とも） | 23,912（3 走行とも） |
| f60 以降の最大 | 4,865 | 5,926 |
| **持ち越し `carried_reflect` / `carried_in_tick` の最大** | **0 / 0** | **0 / 0** |

読み取れること:

* **起動フレームは割れていない。** 予算を 5 分の 1 にしても、最初のフレームは前と**同じ
  23,912 命令**を使い切っている。予算に張り付いた（= 41,000 で切られた）フレームは 1 つも無く、
  持ち越しも 21,558 フレーム全部で 0。案 B（8,600）が「起動が 3 フレームに割れる」と
  予想されていたのに対し、案 A は割らない——これが著者の選んだ案の実物である。
* **23,686 ではなく 23,912。** S5b-3 の測定より 226 命令多い。差の出どころははっきりしていて、
  **S5b-4 が `beetle.rb` に足した `mutation_rate 0.1` の 1 行**である（`bef577e`）。甲虫 14 匹が
  最初の 1 パスで読む行が 1 つ増えた。226 ÷ 14 ≈ 16 命令/匹。
  つまり**実際に出荷される余裕の比は 1.74 でも 1.73 でもなく `41,000 ÷ 23,912 = 1.71`** である。
  rustdoc と一覧にそう書いた。**数は著者の 41,000 のままにした**——ここで 3 つ目の数
  （`1.74 × 23,912 → 42,000`）を勝手に作らない。著者への報告に回す。
* 定常の最大が 4,865 → 5,926 と動いているのは予算とは関係が無い（庭の乱数。`Dice` が時計から
  種を取るので同じバイナリでも 2 回同じ走行にならない）。どちらも 41,000 の 15% 以下である。

### 一緒に動いたもの

`window::scheduler_frames` は `2 + ceil(script_budget ÷ 45,600)` を**起動時に計算する**形に
S5b-3 がしてある。予算が動いたので**式を 1 文字も触らずに 7 → 3 フレーム**になった。
S5b-3 がこの形にしていなければ、判定は 41,000 の予算の VM を 200,000 の忍耐で待っていたことになる。
単体テスト `a_bigger_budget_buys_the_checks_more_frames` の 7 を 3 に直した（それが仕事の全部）。

`frame_time` は据え置き（著者の指定どおり）。上限の庭で生き物 VM の tick の最悪は 1.72 ms、
2 本合わせて 1 フレームの 30% で、絞る材料が無い。

### 前後で同じであることの確認

| 走行 | 前 | 後 |
|---|---|---|
| ヘッドレス `--headless 90`（交互 2 巡） | 13 行 FAIL 0 | 13 行 FAIL 0、`tools/fixedlines.sh` の diff 空 |
| 窓（docker、交互 2 巡） | 44 行 FAIL 0 | 44 行 FAIL 0、diff 空 |

窓の走行の「待った」の行は前後とも 1〜2 フレームで、新しい上限 3 に収まっている
（前の上限は 7 だった。**上限に近づいたのではなく、上限が実態に寄った**）。

### Battle の `script_budget` は動かしていない

Battle の 200,000 は rubevy の既定のままである。**測っていない**からで、箱庭の数を持ってくる
理由も無い（試合は機体 2 台で上限がはっきりしているが、`each_frame` の中身も `matches/*.rb` の
作りも箱庭とは別物である）。`docs/numbers.md` §4.5 に「箱庭は測って決めた、Battle は未測定」と
書いた。

---

## 3. 一時停止の 2 行 — `Vec` の `==` をやめた（B-1）

S8 が割った原因そのもの: `places() == test.places` は `Vec` 同士の比較なので、**値だけでなく
`Query` が生き物を返す順番まで比べていた**。その順番は archetype ごとなので、止まりの前後に
1 体が別の archetype に移る（rubevy が `ScriptTask` を付ける／`dress_animations` が `Animated` を
付ける／止める直前の `Eating` が着地する）だけで、動いていない世界が動いて見える。

直し方は本体が決めた S8 の (A)1 + (B)1 のとおり:

* **entity を鍵に引き当てて比べる**（`what_changed`。`Changes { gone, fresh, moved }`）。
* **「消えた／増えた／値が変わった」を別々に言う。** 判定の行は 2 → 3 行になった:
  `the same creatures are there`（顔ぶれ）、`every creature is where it was`（場所）、
  `nobody got hungrier`（メーター）。FAIL のときは誰がどう動いたかを文が名指しする。
* **許容は置かない。** 等しいことがこの判定の意味で、ここに閾値を置けば出どころの無い数が 1 つ
  増えるだけである。
* コメントに 1 行: **`P` は世界の規則と時計と 2 本の VM を止めるもので、Bevy の世界を凍らせる
  ものではない。** 止まりの中で `ScriptTask` や `Animated` が付くのは世界が動いたのではない。
* 一時停止の作り（`Paused`、`is_still`）は**変えていない**。順序の辺も足していない（S7 が
  `.chain()` に 1 本足して別の判定を割った前例がある）。

**world の VM も見るようにした**（S8 の気づき §206）。`P` は 2 本の VM の予算を一緒に 0 にするのに、
判定は生き物の VM しか見ていなかった。`window_selftest` は Bevy の引数 15 個だったので、
2 本を `BothVms`（`SystemParam`）にまとめて 15 のままにしてある（`FakePointer` と同じ手）。
行の文面が 2 つ変わった: `P pauses: both VMs' budgets are 0`、`P again gives both budgets back`。

### 仕込みで前後を確かめた

S8 の計器を今の木に当て直した（`GARDEN_S8_TOUCH=<n>`: 止まりの n フレーム目に、生き物 1 体へ
**空の marker component を 1 つ付けるだけ**。値は 1 つも変えない）。**常設していない**——
測った後に外した。

| 仕込み `GARDEN_S8_TOUCH=3` の窓の走行 | 直す前（`1ef1fd3`） | 直した後 |
|---|---|---|
| 1 巡目 | **FAIL 2**（`every creature is where it was`、`nobody got hungrier`） | ok 0 FAIL |
| 2 巡目 | **FAIL 2**（同じ 2 行） | ok 0 FAIL |
| 3 巡目 | **FAIL 2**（同じ 2 行） | ok 0 FAIL |

前後とも docker の window で、交互に走らせた。**3/3 で再現し、3/3 で直っている。**

仕込み無しの窓の走行は **45 行 FAIL 0**（前は 44 行）。`tools/fixedlines.sh` の diff は
**狙った 3 行の増減だけ**:

```
< selftest: ok   P again gives the budget back
< selftest: ok   P pauses: the scripts' budget is N
---
> selftest: ok   N s paused: the same creatures are there
> selftest: ok   P again gives both budgets back
> selftest: ok   P pauses: both VMs' budgets are N
```

`docs/verification/selftest-lines.md` の箱庭の窓（44 → 45 行）とブラウザ（45 → 46 行）を直した。

---

## 4. 残っていた 0.6 秒の待ち 2 か所（B-2）

S7 は割れた 4 つの待ちを条件待ちに変え、`world.rb` の Apply（`Ctrl+Enter`）とその Revert の
**2 か所を 0.6 秒のまま残した**——「同じ形だが、まだ落ちたことが無い」という理由で。同じ形なら
同じ直し方である。

待つものは `Turn::TheDayIs(f32)`: **規則が `garden.rules(day_length:)` で game に渡す数**が
その値になったこと。境界を数として越える唯一の規則で、起動したばかりの世界スクリプトしか
言わないので、「この規則が走っている」と 1 つの等式で言える（判定の文もそう書いてある）。

`Turn` から `Eq` の derive を外した（`f32` を持つため）。比較は `!= Turn::NotWaiting` の 1 か所
だけなので `PartialEq` で足りる。

### 実物が「上限ちょうど」を見せた — `spare` を足した

条件待ちにして最初に走らせたら、`Ctrl+Enter` の待ちが **ちょうど 3 フレーム**かかった。
上限も 3（`scheduler_frames`）である。**余裕が無い。**

数え直すと理由がはっきりした。`scheduler_frames` は「VM に何かを頼んだ瞬間から」の勘定である:
`Commands` で `Script` が付くフレーム + タスクが作られて走るフレーム + 予算 1 フレーム分。
ところがこの段は**キーで頼んでいる**ので、その前に 1 フレームある——`rubevy-egui` のパネルは
`Ctrl+Enter` を egui のパスで読むので、`Editor::action` が立つのは押した次のフレームである。
これは `Turn::EguiHasThePointer` が丸ごと扱っているのと同じ 1 フレームで、**待ちの中ではなく
待ちの前**にあるぶんだけ勘定から漏れていた。

`WindowTest::spare`（この待ちが VM の勘定の上に要するフレーム数。0 か 1 しか取らず、1 はその
egui のフレーム）を足し、上限を `frames + spare` にした。`Ctrl+Enter` の段だけ 1 である
（Revert は `Editor::action` を直接立てるので 0）。**新しく決めた数ではない**——
`EguiHasThePointer` の rustdoc が前から書いている構造のフレームを、もう 1 か所で数えただけ。

| 窓の走行 | 待ちのフレーム数（7 つの待ち） | 行 | FAIL |
|---|---|---|---|
| 1 巡目 | 1, 2, 2, **3**, **2**, 1, 1 | 45 | 0 |
| 2 巡目 | 1, 2, 2, **3**, **2**, 1, 1 | 45 | 0 |
| 3 巡目 | 1, 2, 2, **3**, **2**, 1, 1 | 45 | 0 |

太字が新しい 2 つ（`Ctrl+Enter` の 3、Revert の 2）。**行の集合は B-1 と同じ**——
待ちの直しは判定の文を変えない（「待った」の行は `tools/fixedlines.sh` が落とす stage direction）。

これで `window.rs` に残る「秒」は**秒を意味しているものだけ**になった: 止まりの 2 秒、歩きの
0.5 秒、VM が走り続けていることの 9 秒、そして Bevy の入力が 1 フレーム見るための 0.2 秒が 6 つ。

---

## 5. 判定が持っていた「実値の写し」（B-4・B-8）と、`--` と `n/a`（B-7）

S5b-2 が Battle で 1 つ見つけた形——**判定の閾値が、ゲームの側の値を書き写した数になっている**
——を、同じ目で箱庭に当てた。2 つ見つかった。どちらも「その数を直した人が、直したせいで
判定に落とされる」形である。

### (1) 「夜が来た」の 60 秒

`night arrived by 60 s` の 60.0 は直書きで、一覧（§2.11）には**「導出: `DAY_LENGTH` 1 周」**と
書いてあった。導出ではなく**写し**である: 1 日の長さは今 `world.rb` の `day_length` が言い、
`garden.rules` で `Sky::day_length` に届く。`world.rb` の日を 90 秒に伸ばした人は、規則が
守られたのに FAIL を見ることになる。実値（`sky.day_length`）を読む形にした。文面も
`night arrived within a day of N s` に変わる（N を印字するので、何と比べたかが走行に残る）。

### (2) 窓の「規則」の段が持っていた `60.0` と `"day_length 60.0"`

こちらは 2 か所あって、2 つ目の方が悪い:

* `ok(sky.day_length == 60.0, "the day is what world.rb says it is")` ——
  **文が「`world.rb` が言うとおりか」と言っているのに、比べていたのは書き写した 60.0** である。
* `editor.text.replace("day_length 60.0", "day_length 30.0")` ——
  日を変えた `world.rb` では**置換が何にも当たらず、黙って元の文のまま Apply される**。
  そのあとの `sky.day_length == 30.0` が落ちて、「Apply が効かない」という嘘の FAIL が出る。

`day_length_in(&editor.text)` でテキストから読み、その**半分**を書き戻す形にした
（半分である必要は無い——「違う数なら何でもよい」もので、ログで元の数と並べて読みやすいので
半分にした、と書いてある）。単体テスト 1 本を足した: 出荷される `world.rb` から 60.0 が読め、
書き戻した行が実際に置換に当たり、置換後は 30.0 が読める。

ほかに写しが無いかは §2.11 の 22 行を 1 つずつ見た。**残りは写しではない**:
`REACH + 0.5` は実値を読んでいる、`MUTATION_RATE` は S5b-4 が VM から読む形にした、
空腹バーの 55.0 と `hungry_below` 55.0 は「同じ値だが繋がない」と著者が決めた（§7-6）、
仕込みの空腹 45.0 は `START_HUNGER` の下端と同じ数だが別のこと（庭の初期値と仕掛けの値）で、
どちらの記録にも繋いだ形跡が無い——**一覧に「同じ値だが繋がっていない」と書くに留めた**。

### `INSTRUCTIONS_A_FRAME_BUYS` はどちらの実測を採るか（B-4）

実測は 2 つある。S6 は `frame_time` を **300 µs に絞って** 5.7 命令/µs（8 ms で 45,600）、
S5b-3 は上限の庭を**本来の 8 ms で**回して 6.85 命令/µs（54,800）。差は機械ではなく条件で、
300 µs のフレームは tick の出入りの費用を 27 分の 1 の仕事で払うので「短いフレームの速さ」になる。

**遅い方（45,600）を採った。** 理由は誤差の向きである: この数は予算を割って「何フレーム待って
よいか」を出すので、**大きすぎると待ちが短くなり、遅いだけの VM に FAIL を出す**——S6 と S7 が
消しに来た偽の FAIL そのもの。小さすぎれば判定が少し遅れるだけである。rustdoc に 2 つの条件を
表で書いた。

**実物で確かめた**: 既定の予算 41,000 では `ceil(41,000 ÷ 45,600)` も `ceil(41,000 ÷ 54,800)` も 1 で、
`scheduler_frames` は**どちらでも 3**。差が出るのは `script_budget` が 45,600 を超えたときだけである。

### `--` と `n/a`（B-7）

意味は同じ「測れなかった」。Battle は `--`、箱庭は 1 か所だけ `n/a` で、残りは `--` だった。
**`--` に揃えた**（多数派であり、2 つのゲームが同じ綴りになる）。`tools/fixedlines.sh` の
verdict の正規表現から `n/a ` を外し、`docs/verification/selftest-lines.md` の verdict 表と
`docs/garden.md` の 4 か所を直した。6 通りの走行では `n/a` の行は 1 つも出ていないので、
記録された一覧の行は変わらない。
