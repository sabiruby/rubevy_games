# 箱庭の遊びの数を Ruby 側へ（S5b-4）

計画書 `docs/plans/shared-crate-plan.md` の段階 **S5b-4**。一覧は `docs/numbers.md` の §2 の
分類 (b) と §3。手本は `docs/worklog/2026-09-21-numbers-battle.md`（S5b-2）と
`docs/worklog/2026-09-21-numbers-garden-settings.md`（S5b-3）。
着手時は main と同じ `5333e52`、作業は worktree の `shared-crate`。

**著者が決めていたこと**（2026-09-20）: 分類案のとおり——**遊びの数は Ruby 側**。出どころ不明の
数は「不明」と書いたまま移す。**既定値は 1 つも動かさない。** 原則は「**遊びの数は Ruby、走査は
Rust**」。

---

## 0. 着手前に取った基準

機械は空いていた（`vmstat 1 3` の idle **99%**）。隣の worktree
（`rubevy_games-wt-numbers`）に待ちのシェルが 1 つあったが cargo は動いていない。

`GARDEN_SELFTEST=1 target/release/garden --headless 90` が **13 行・FAIL 0**、
`tools/fixedlines.sh` の出力は `docs/verification/selftest-lines.md` の一覧と一致。
`cargo test --workspace` は **56 通過**、`cargo build --workspace --all-targets` は警告 0。

前の版は `git worktree add --detach`（`git stash` は使わない規則）で `5333e52` を別の
ディレクトリに出し、**別の `CARGO_TARGET_DIR`** で建てた。`md5sum` で別のバイナリであることを
確かめてある（前 `f03b1669…`／後 `b45ca285…`）。前の版は ruby も含めて前の版なので、比較は
「Rust と Ruby を合わせた前後」である。

---

## 1. いちばん大きな判断 — Rust の既定値は消せるか

指示はこう言っている: `RuleBook` が「全フィールド `Option` + `#[serde(default)]`」なのは、
ゲームが先に走り出して規則が後から喋るからである。S5b-2 の Battle は「受け取るまで試合を
始めない」形にできたので Rust 側の既定値が 0 になった。箱庭でも同じことができるか。

**できない。そしてその理由はコードが既に書いている。** `give_the_world_its_rules` の rustdoc:

> **A `world.rb` that will not compile is not a reason to refuse to start.** The file is one the
> player is invited to edit, and a typo in it should leave a garden that runs — the sun turns,
> the creatures walk, nothing grows and nobody gets hungry — with a sentence on the HUD saying
> so.

これは W1 が決めた設計そのもので、Battle との違いもそこにある。Battle は試合が黙っていたら
**試合が無い**（`match_prelude.rb` の `Match#run` の 1 行目が数を渡し、ロボットを出す `start`
はその後にしか来ない）。箱庭は `world.rb` が黙っていても庭が回らなければいけない。

「規則の最初の 1 往復が済むまで世界の system を止める」run condition を作る案も見たが、代償が
二重に大きい:

* **`spawn_world` は `Startup` システム**で、世界の VM が最初の `Update` で走り出す前に畑と
  生き物を建てる。止めるなら庭ごと 1〜2 フレーム遅らせることになり、`--shot`、`--at`、
  `load_world`、窓のチェックの 3.0 秒、ヘッドレスの 60 フレーム/秒の模擬が全部その 2 フレームを
  被る。**「動きを変えない」が条件の段階でやることではない。**
* 止めた世界を**いつ動かし始めるか**の条件が「規則が喋ったら」だけでは足りない。`world.rb` が
  コンパイルできない庭（`WorldTrouble`）と、コンパイルはできるが `world do … end` を使わない
  庭——エディタで書き換えられるファイルなので両方ありうる——では規則は永遠に喋らない。
  「喋ったか、もう喋らないと分かったか」の二条件になり、後者は前者より当てにならない。

**採った形**: Rust 側の値は残し、**代役（stand-in）であることを名前と rustdoc に書く**。
`REACH` / `MATE_REACH` / `CHILD_HUNGER` / `POP_MAX` が既にそうなっていたので、増えた 6 つも
同じ言い方に揃えた:

> what a body is until the rules have spoken, and for ever in a garden whose `world.rb` will not
> compile.

そのうえで **`world.rb` と食い違わないことを確かめるテスト**を足した
（`the_stand_ins_are_what_world_rb_says`）。`include_str!` で `ruby/world.rb` を読み、
`def <名前> = <数>` の行を引いて 10 個の代役と突き合わせる。起動しないテストにしたのは、
ブラウザ版には読む先のディレクトリが無く、テストが走る場所によって意味が変わらないほうが
よいため。**プレイヤーが編集した `world.rb` については何も言わない**——そちらは食い違ってよく、
食い違えるようにするのがこの受け渡しの目的である。

## 2. 何を移したか

| 何 | 何件 | どこへ | 受け口 |
|---|---|---|---|
| `TOUCH_REACH` 1.3 | 1 | `world.rb` の `touch_reach` | `garden.rules(touch_reach:)` |
| 新しい芽の最小間隔 1.5 | 1 | `world.rb` の `sprout_gap` | `garden.rules(sprout_gap:)` |
| 4 つの半径 0.40 / 0.50 / 0.70 / 0.60 | 4 | `world.rb` の `*_radius` | `garden.rules(beetle_radius: …)` |
| 変異率 0.1 | 1 | `beetle.rb` の `mutation_rate 0.1` | 判定が VM から読む |
| `Furniture::plant_max` | — | 名前を分けた（`plant_grown`） | `garden.settings.txt` の `plant_grown` |
| 種ごとの遺伝子 3 × 2 と `SPREAD` | 0 | **移していない**（§6） | — |

### 原則は「数は Ruby、走査は Rust」

移した 6 つに共通するのは、**どれも「畑の全部の組を歩く」ループの中で使われている**ことである
——甲虫全部 × ウサギ全部（`startle`）、新しい種 1 つ × 今生えている草全部（`sprout_plants`）、
固い円全部 × 固い円全部（`separate`）。`world.rb` の冒頭が `garden.within` について書いている
のと同じ話で、そのループを Ruby に書かせるわけにはいかない。だから**数だけが渡り、歩くのは
ゲーム**になる。`TOUCH_REACH` を `startle` ごと `world.rb` に移す案（指示の選択肢の片方）を
採らなかったのはこの理由で、1 行の rustdoc にもそう書いた。

受け口を `garden.rules` にしたのも同じ考えからである。芽の間隔は `garden.sprout` の引数に
付けることもできた（`world.rb` が毎フレーム 1 回まで呼ぶ問い）が、それは**同じ数を毎秒 60 回
境界の向こうへ運ぶ**ことになる。`garden.rules` は世界のスクリプトの 1 行目で 1 回だけ渡る。

### 半径だけは、渡すだけでは効かない

`touch_reach` と `sprout_gap` は「使う所で読む」数で、それで終わりである。半径は違う:
**`Collider` としてエンティティに書かれている**（`separate` が歩くのも 4 番目の判定が測るのも
それ）。規則が喋るのは最初の `Update` で、そのときには `spawn_world` が建てた 10 匹と 16 個の
木と岩が既に立っている。**渡しただけでは、庭に今いるものには届かない。**

そこで `bodies_wear_the_rules` を足した。`Res<Bodies>` が変わったフレームだけ全部の
`Collider` を書き直す——`plants_wear_their_size` が `Plant.size` について毎フレームやっている
ことの、数がめったに変わらない版である。これで「`world.rb` の `beetle_radius` を直して
`Ctrl+Enter`」が庭じゅうの甲虫を太らせる。**「移した」と「効いている」は別の主張**という
S5b-1 の教訓がそのまま当たる所で、渡すだけで済ませていたら「設定できるが何も起きない数」を
1 つ増やしていた。

system の登録は、隣の 3 つ（`plants_wear_their_size` / `note_the_rules` / `sprout_plants`）の
組に入れずに**単独で `.after(RubevySet::<World>::answer())`** にした。要るのはその辺 1 本だけで、
既にある 3 つに新しい辺を足すのは Bevy に同期点を 1 つ置かせることであり、それで別の判定が
落ちた前例が S7 にある。

### `CELL` は「答え」から「下限」になった

`separate` の近傍グリッドの一辺 1.6 には「最大半径の 2 倍以上」という導出がコメントに書いて
あった。半径が規則のものになった以上、この数を据え置くと**規則が木を 3 単位にしたとたんに
近傍を取りこぼす**——触れている 2 つが隣り合わないセルに入る。`Bodies::cell()` は
`CELL.max(2 × 最大半径)` で、出荷時の最大半径 0.70 では 1.4 < 1.6 なので**答えは 1.6 のまま**
である（単体テスト `the_grid_is_wide_enough_for_the_widest_body` がその 2 つを書き留めている）。

## 3. 古い `world.rb` を持っている利用者

**ここが公開版を壊しうる唯一の所だった。** `world_prelude.rb` の `run_world` は

```ruby
answer = being.garden.rules(day_length: klass.day_length, child_hunger: being.child_hunger, …)
```

と、世界のオブジェクトのメソッドを**名指しで呼ぶ**。新しい鍵を足すというのは
`being.touch_reach` を足すということで、ブラウザの `localStorage` に**この段階より前の
`world.rb` を Save してある人**——エディタの Save ボタンを押した人——のファイルにはその
メソッドが無い。`NoMethodError` は `run_world` の `rescue` に落ち、`raise` し直され、
**規則がまるごと死ぬ**。庭は回るが何も育たず誰も腹が減らない。

`respond_to?` で聞いてから足す形にした:

```ruby
numbers[:touch_reach] = being.touch_reach if being.respond_to?(:touch_reach)
```

古い 5 つは今までどおり名指しで呼んでいる（`reach` の無い `world.rb` はどの公開版でも
走ったことがない）。新しい 6 つを知らない `world.rb` は 5 つだけ渡し、残りは代役のままになる
——つまり**今までとまったく同じ庭**である。これが §1 で代役を残した判断のもう 1 つの効き目で、
「受け取るまで始めない」形にしていたら、古い `world.rb` を持つ利用者の庭は開かなかった。

種のファイルも同じ形にした。`mutation_rate` を知らない `beetle.rb`（`mutate(0.1)` を直書き
している、公開版のブラウザにあるやつ）は宣言をしないので、判定は代役の `MUTATION_RATE` で
判断する——**前と同じ**。実際に `mutation_rate` の行を消して `mutate(0.1)` に戻したファイルで
走らせ、8 番目の判定が `ok` を出すことを確かめた。

セーブファイル（`garden.save.json`）の形は**変えていない**。半径はセーブに入っておらず、
`load_world` は `spawn_creature` で体を建てるので、読み戻した庭の体はその庭の規則のものになる。

## 4. 変異率の穴（S5a の 09-20 の指摘）

本物は `beetle.rb` の `mutate(0.1)`、8 番目の判定は `const RATE: f32 = 0.1` で、
判定側のコメント自身が「写しだ」と書いていた。**エディタで変異率を上げると判定が FAIL になる**
——このゲームが見せたいことをやった瞬間に。

直し方は `@handlers` / `@asleep` を読んだのと同じ道にした。`prelude.rb` に
`Creature.mutation_rate` というクラスレベルの宣言を足し（`on` の隣）、`beetle.rb` が
`mutation_rate 0.1` と書く。判定は `mutation_rate_of` でタスクの `@being` → そのクラス →
`@mutation_rate` を `ivar_get` 2 回で読む。**読むのは子が要求されたフレーム**（`answer_spawn`）
で、そこがそのタスクがまだ手元にある唯一の場所であり、ファイルはその後で書き換えられうる。

`def hungry_below = 55.0` のような普通のメソッドにしなかったのは、**ホストがメソッドを呼ぶと
Ruby のコードが走る**（`funcall` は入れ子の run loop）ためで、`ivar_get` は値を読むだけである。
`@handlers` が既にその形をしている。

確かめ方（`beetle.rb` の `mutation_rate` を 0.5 にしただけで、ほかは何も変えない）:

| | 走行 1 | 走行 2 |
|---|---|---|
| 前の版 | **FAIL**（speed 2.276、平均 2.200） | **FAIL**（speed 3.101、平均 2.367） |
| 後の版 | ok（speed 2.035、平均 2.200） | ok（speed 1.452、平均 2.200） |

判定の**文は変えていない**。`docs/verification/selftest-lines.md` は文の一覧で、何も変えない
つもりの段階はそれに対して diff が空でなければならない——率を文に足すと 1 行動く。

## 5. 種ごとに 1 つの `Handle`（09-20 R2）

**S7 で済んでいるはずと書かれていたが、済んでいなかった。** `Brains::wearing` は
**エディタからの引き渡し（`hand_over`）でしか埋まらない**ので、誰も編集していない庭では
`give_mind` が生き物 1 匹ごとに種のファイルをコンパイルし、1 匹ごとに `Handle<MrbAsset>` を
作っていた——起動時に 10 匹、そのあと出産のたびに 1 つずつ。

種の最初の 1 匹がコンパイルしたものを `Brains::first_program` に置き、以後はその handle を
渡す形にした。**世代は 0** で、`catch_up_minds` の「遅れている者」は増えず、最初の編集は
今までどおり世代 1 になる。無効化の心配が無いのは、種が走らせているものを変える道が全部
`hand_over` を通るからである（`brains.set` の 3 か所はどれも直後に `hand_over` を呼ぶ）。

## 6. 移さなかったもの — 種の遺伝子と `SPREAD`（著者判断待ち）

`genome.rs` の `Genome::of`（甲虫 speed 2.2 / sight 8.0 / appetite 1.0、ウサギ 3.4 / 12.0 /
1.0）と `SPREAD` 0.18 は、一覧では (b) の 3 行である。**移していない。** 理由は 1 つで、
これは「移せない」ではなく「移すと嘘になる」たぐいの話である。

**読み手がいつ読むかで、渡せる数と渡せない数が分かれる。** 半径は `Collider` として
エンティティに載っているので、あとから渡しても書き直せる（§2）。種の遺伝子はそうではない:
`Genome::of` を読むのは

1. `spawn_world`（`Startup`。最初の 10 匹を `Genome::roll` で振る）
2. selftest の仕込み 3 匹（同じく `Startup` か、規則を聞いた直後の `plant_the_meadow`）
3. 最後の census の 1 行（「その種の本来の値は」）

の 3 か所だけで、**生き物が生まれたあとはどこからも読まれない**（子の遺伝子は Ruby が
`mix`・`mutate` して `garden.spawn` で渡してくる。セーブから読んだ庭は保存された遺伝子を使う）。
`garden.rules` が届くのは最初の `Update` で、1 は既に終わっている。だから `world.rb` に
`beetle_speed 2.2` と書けるようにしても、**その数はどの走行でも 1 度も使われない**——
「エディタで変えられるのに何も起きない数」を 3 × 2 + 1 個作ることになる。それは
S5b-1 が「設定にしたと設定が効いているは別の主張」と書いた失敗そのものである。

**種のファイル（`beetle.rb`）に置く案**も見た。体の寸法や速さは確かに「その種が何であるか」で、
指示もそこを指している。だが種のファイルが喋るのは**その種の生き物が 1 匹生まれて、その
タスクが最初のフレームを走ったとき**で、体はその前に建っている。ファイルに先に喋らせるには、
種ごとに「プログラムを定義するだけの試作タスク」を起動時に 1 回走らせて `$creature_body` の
ような大域を読む、という**今は無い仕掛け**が要る（Battle の `Rubevy.ask("model")` の逆向き）。
そのうえ世界の生成を 1〜2 フレーム遅らせることになり、§1 で退けたのと同じ代償に戻る。

**著者に選んでもらう形で 3 案**（本体は A を薦める）:

* **A. `Furniture`（`garden.settings.txt`）へ。** 種の遺伝子は「新しい庭を建てるときの stock」
  で、実際に読むのは `spawn_world` だけ——`start_beetles` や `start_hunger_min` と同じ家族で
  ある。鍵は `start_beetle_speed` ほか 7 つ。分類は (b) → (c) に動く。
* **B. `world.rb` へ、世界の生成ごと遅らせて。** 「規則が喋ってから庭を建てる」に作り替える。
  数は全部 Ruby に揃うが、§1 の 2 つの代償（起動の順序が変わる／規則が永遠に喋らない庭の
  扱い）を丸ごと引き受ける。
* **C. 据え置き、`const` のまま。** 一覧の (b) に「移していない、理由は worklog」と書く。

## 7. 動きを変えていないこと

### 6 通りの走行

| 走行 | 前 | 後 | 判定 |
|---|---|---|---|
| 箱庭 ヘッドレス（`--headless 90`） | 13 行 FAIL 0 | 13 行 FAIL 0 | **交互に 8 巡**（下の表）。8 組のうち 7 組が `diff` 空 |
| 箱庭 窓（docker/lavapipe） | — | 44 行 FAIL 0（2 走行） | `verification/selftest-lines.md` の一覧と**完全一致** |
| 箱庭 ブラウザ（`garden/?selftest`） | — | 45 行 FAIL 0 | 一覧と完全一致、pageerror 0・requestfailed 0 |
| Battle ヘッドレス（`--headless 25`） | — | 4 行 FAIL 0 | 一覧と一致（記録済みの「動く 1 行」が `--` の側） |
| Battle 窓（docker/lavapipe） | — | 32 行 FAIL 0 | 一覧と完全一致 |
| Battle ブラウザ（`sabibots/?selftest`） | — | 33 行 FAIL 0 | 一覧と完全一致、pageerror 0・requestfailed 0 |

箱庭のヘッドレスだけ前後を取ったのは、この段階が触ったのが箱庭だけだからである。Battle と
共有 crate には 1 行も触っていない（`git diff --stat` が `garden/` しか出さない）。

**`diff` が空でなかった 1 組**は、**前**の走行のほうが

```
selftest: n/a  a hungry creature with a plant in sight reached it (not measured: a rabbit
               walked into the probe at N s; …)
```

を出していた。これは `docs/verification/selftest-lines.md` の「Verdicts」に
`n/a for a probe something walked into` として書かれている形で、ウサギがプローブに歩いて
ぶつかると測れない、というその走行の運である。後の側ではなく前の側に出たので、こちらの変更の
話ではない。**16 走行すべて FAIL 0。**

### 世界の動きの分布 — 交互に 8 巡

`git worktree add --detach` で建てた前の版と、交互に。機械は `vmstat` の idle **99%**。
最後の 1 行（`hud: N creatures · N plants`）がヘッドレスの終わりに出るので、草の数もそこから
取った。

| 巡 | 前: 出産 / 餓死 / 最後の匹数 / 草 / 触った | 後: 出産 / 餓死 / 匹数 / 草 / 触った |
|---|---|---|
| 0 | 5 / 4 / 15 / 36 / 29 | 5 / 1 / 18 / 43 / 21 |
| 1 | 2 / 3 / 13 / 39 / 14 | 2 / 7 / 9 / 52 / 17 |
| 2 | 6 / 5 / 15 / 37 / 21 | 4 / 3 / 15 / 34 / 23 |
| 3 | 3 / 2 / 15 / 33 / 12 | 3 / 5 / 12 / 45 / 21 |
| 4 | 4 / 4 / 14 / 27 / 21 | 5 / 1 / 18 / 33 / 27 |
| 5 | 2 / 4 / 12 / 40 / 26 | 6 / 2 / 18 / 41 / 27 |
| 6 | 6 / 3 / 17 / 44 / 30 | 5 / 2 / 17 / 28 / 26 |
| 7 | 4 / 4 / 14 / 36 / 10 | 4 / 4 / 14 / 38 / 22 |
| 平均 | **4.0 / 3.6 / 14.4 / 36.5 / 20.4** | **4.3 / 3.1 / 15.1 / 39.3 / 23.0** |

**見分けがつかない。** どの列も前後の範囲が重なっていて、平均の差はどれも 1 走行ぶんの振れより
小さい。ここでも S5b-2 の教訓が効いた: **最初の 3 巡だけ**を見たときには「後」の匹数が
9・15・12 と並び、「前」が 13・15・15 だったので「移したせいで餓死が増えた」と読める形を
していた。4 巡目以降の後は 18・18・17・14 で、1 巡目の 9 はただの不運である。3 巡では何も
言えない。

夜の時刻（25.20〜25.22 s）、最初の食事（0.27〜0.57 s）、フレーム数（5,363〜5,385）は前後で
同じ範囲に並ぶ。

### 世界の VM の命令数 — 上限の庭で

`garden.settings.txt` に S5b-3 の 3 行（`start_plants=130` / `start_beetles=14` /
`start_rabbits=10`）を置いた庭で `--headless 60` を交互に 2 巡。`garden.rules` が太って
1 フレームの命令数が増えていないか、が見たいこと（`rules` は最初の 1 回だけのはず）。

| 巡 | 前: 命令数 中央値 / 最大、tick 最大 | 後: 命令数 中央値 / 最大、tick 最大 |
|---|---|---|
| 1 | 18,112 / 23,133、4.33 ms | 18,436 / 23,454、4.44 ms |
| 2 | 18,495 / 24,360、5.87 ms | 19,172 / 23,267、4.30 ms |

**ぶれの範囲。** 最大は予算 45,000 の 52% で、S5b-3 が測った上限の庭の値（この設定で
`world.rb` の `plant_cap` 90 を上から詰める）と同じ帯にある。`garden.rules` の Hash は鍵が
5 個から 11 個に増えたが、**組み立てるのは世界のスクリプトの 1 行目の 1 回だけ**なので、
毎フレームの `each_frame` には 1 命令も足されていない。

### そのほか

`cargo build --workspace --all-targets` **警告 0**、`cargo test --workspace` **56 → 58 通過**
（足したのは `the_stand_ins_are_what_world_rb_says` と
`the_grid_is_wide_enough_for_the_widest_body`）。`web/build.sh all` の `wasm-opt` 後は
**garden 35,964,879 / sabibots 35,216,606 バイト**。S5b-2 が記録した garden 35,900,926 から
**+63,953（+0.18%）**で、sabibots は 1 バイトも動いていない（触っていないので当然だが、
共有 crate を巻き込んでいないことの裏づけになる）。

## 8. 変えると効くこと

**「移した」と「効いている」は別の主張**（S5b-1）なので、移した数の種類ごとに 1 つ、本物の
走行で確かめた。ヘッドレスの走行で観測できるものを選んである。

### `touch_reach`（走査に渡る距離）

`world.rb` の `touch_reach` を **0.5** にして `--headless 60`:

```
selftest: FAIL a beetle touched by a rabbit changed heading within 0.5 s (0/0)   （2 走行とも）
```

甲虫と兎の体は 0.40 + 0.50 = 0.90 で、`separate` はそれ以上近づけない。だから **0.9 より短い
`touch_reach` は「どうやっても触れない庭」**であり、6 番目の判定は 1 件も数えられずに落ちる
（既定の 1.3 では同じ走行で 12〜19 件）。数が `startle` に届いている証拠がこれである。

逆向き（6.0）も試したが、**判別に使えなかった**ので捨てた: 触った回数は「接触が切れて
また作られた回数」で、届く距離が長いほど接触は切れにくくなる。数えられる触りは 11〜16 件で、
既定の 12〜13 件と区別できない。効きを見るには**効き方を選ばないといけない**、という例。

### `sprout_gap`（走査に渡る距離、その 2）

`sprout_gap` を **12.0** にして `--headless 60`、終わりの HUD の 1 行:

| `sprout_gap` | 走行 1 | 走行 2 |
|---|---|---|
| 12.0 | 14 匹・**16 株** | 14 匹・**12 株** |
| 1.5（出荷時） | 15 匹・**48 株** | 15 匹・**41 株** |

新しい芽が既にある株から 12 単位離れていなければ生えない畑（40×30）では、草が 1 分で
3 分の 1 以下になる。

### 体の半径（規則が渡し、ゲームが書き直す）

`beetle_radius` を **1.20** にすると、selftest の隅の 1 行が

```
前: two hungry beetles 1.34 apart, each 2.73 behind a blade of its own
後: two hungry beetles 2.40 apart, each 2.60 behind a blade of its own
```

になる。`plant_the_meadow` は「2 匹が体ごと重ならない最小の間隔」（`2 × beetle_radius`）から
隅の幅を組むので、規則が言った体の幅がそのまま出る。

### 変異率（判定が VM から読む）

§4 の表。`beetle.rb` の `mutation_rate` を 0.5 にすると前の版は 2 走行とも FAIL、後の版は ok。

### 古い `world.rb` を持っている人（§3 の確かめ）

前の版の `world.rb` をそのまま置いて走らせると、規則は生きている:

```
[script] world: the wet season
selftest: ok   the rules in world.rb are running the world (the grass grew at 0.03 s, over 720 passes)
```

`respond_to?` の行を 1 つだけ外して（`numbers[:touch_reach] = being.touch_reach` にして）
同じファイルで走らせると:

```
[script] world: NoMethodError: undefined method 'touch_reach' for #<Class:0x…>
selftest: FAIL the rules in world.rb are running the world (nothing ever grew, over 1 passes of `each_frame`)
```

**規則がまるごと死ぬ。** これが防ぎたかったものである。

ブラウザでも同じことを確かめた。前の版の `world.rb` と `beetle.rb` を
`addInitScript` でページが動き出す前に `localStorage` に入れて `?selftest`:

```
localStorage keys: ["garden:ruby/creatures/beetle.rb","garden:ruby/world.rb",
                    "garden:garden.settings.txt","garden:garden.from-another-version.json"]
pageerror: 0   requestfailed: 0
selftest: 45 行（一覧と完全一致）  FAIL 0   NoMethodError 0
```

古い `beetle.rb` は `mutation_rate` を宣言せず `mutate(0.1)` を直書きしているが、8 番目の
判定は代役の 0.1 で判断して `ok` を出す。**セーブの形は変えていない**ので、
`garden.save.json` はどちらの版でも読める（10 番目の判定がそれを見ている）。

## 9. 気づいた点

1. **`world.rb` の数が「毎フレーム読まれるか」は、もう 1 つの分類になっている。** 半径は
   エンティティに書かれるので渡せば効くが、種の遺伝子は `Startup` でしか読まれないので
   渡しても効かない（§6）。一覧の `毎F` の欄は「読む費用」のための欄だが、**移せるかどうか**の
   欄でもある。次に (b) を動かす人は、数ごとに「誰がいつ読むか」を先に見るのがよい。
   属する話: 一覧の作り／本の素材。
2. **`touch_reach` を大きくしても触りが増えない**（§8）。`Contacts` は「接触が作られた瞬間」
   だけを publish するので、届く距離を伸ばすと接触が切れにくくなり、**イベントは減る方向にも
   動く**。遊びの数としては直感に反するので、`docs/garden.md` に 1 行あってよい。
   属する話: 箱庭（遊びの設計）。
3. **`Genome::of` と `SPREAD` を移せなかった**（§6）。著者判断待ちの 3 案を挙げた。
   属する話: 分類（(b) と (c) の境目）。
4. **一覧に無い直書きの数が箱庭にまだある**（S5b-3 の報告の続き）。この段階でも
   `dice.between(0.3, plant_grown)` の 0.3 と昼の色の傾きには触っていない。
   `plant_grown` の名前を変えたので、0.3 は「`plant_grown` の相方」という意味が読みやすく
   なった——2 つで 1 組の数である。属する話: 一覧の抜け。
5. **`docs/web.md` の wasm の大きさの表がまた 1 段ずれた**（+0.18%）。S5b-1・S5b-2 に続いて
   3 回目で、段階ごとに直す約束にはなっていない。属する話: 文書と実物のずれ。
6. **`Brains::wearing` が「編集されたときだけ埋まる」形だったことは、S7 の設計の穴である**
   （§5）。S7 は「その種が今着ているプログラム」という名前を付けたのに、着せ替えたときしか
   入らなかった。**名前が言っていることと、実際に入るものが違う**状態が 1 日残っていた。
   属する話: 箱庭（設計）／本の素材（「名前が約束を先に書いてしまう」）。
7. **判定の文を変えると `selftest-lines.md` の一覧が動く。** 変異率を判定の文に入れたくなった
   （読む人には親切）が、それは一覧の 1 行を書き換えることになるので入れなかった。
   **「何も変えないつもりの段階」と「読みやすくする変更」は同じコミットに置けない。**
   属する話: 確認の作法／本の素材。
