# 2026-09-18 判定側の穴 2 つ — 数えるべきでないものを数えていた

前回（`2026-09-18-corner-and-selftest.md`）の §1.6 と §2.4 に「今回は手を付けていない、報告に
上げる」と書いて残した 2 件である。どちらも**直すのは判定側だけ**で、ゲームの振る舞いも閾値も
`world.rb` も `creatures/*.rb` も `robots/*.rb` も動かさない。1 件 1 コミット。

作業場所は worktree `rubevy_games-wt-holes`（ブランチ `check-holes`、main `f7e5f10`）。

## 0. 着手前の状態

```
$ cargo build --release -p garden -p sabibots
    Finished `release` profile [optimized] target(s) in 2m 49s
$ cargo clippy -p garden -p sabibots -p rubevy-arena
warning: `rubevy-arena` (lib) generated 2 warnings
warning: `sabibots` (bin "sabibots") generated 12 warnings
warning: `garden` (bin "garden") generated 13 warnings
```

この 3 つの数（2 / 12 / 13）は最後まで変わっていない。

---

## 1. 箱庭 8 番 — 分母が「つがい」ではなく「子が来うるつがい」になる

### 1.1 穴は 2 つとも「数えた後で黙る」形だった

8 番は「子の遺伝子が両親の平均から変異の範囲に入っているか」を見る判定で、子が 1 匹も来なかった
走行では分母（`courtings`）の数だけを見て FAIL / `n/a` を分ける。その `courtings` は
`answer_world` の 1 行で数えていた:

```rust
test.courtings += said.iter().filter(|(_, name, _)| name == "mate").count() as u32;
```

`said` は `world.rb` が `tell` で言ったものの一覧である。`world.rb` の court は
**種でつがいを作る**（`next if b[1] != a[1]`）だけで、その種が `"mate"` を聞いているかどうかは
見ていない。そう書いてあるのは意図的で、`world.rb` の `mate_cooldown` の脇に

> whether a creature does anything at all with the message is its own script's business …
> The rabbit, for instance, has no `on(:mate)`.

と本人が書いている。つまり**ウサギどうしのつがいは、最初から子が来ない**。それを分母に入れて
いたのが穴 a。

穴 b は `garden/ruby/creatures/beetle.rb` の 3 行目にある:

```ruby
mate = genome_of(partner)
next if mate.nil? # it starved between the rule speaking and this waking up
```

`genome_of` は相手の `Creature` コンポーネントを読む prelude のヘルパで、相手が消えていれば nil。
規則が喋ってからハンドラが起きるまでの 1 フレームで相手が餓死すると、ハンドラはここで黙って
`next` する。`garden.spawn` は呼ばれず、ゲーム側には何も届かず、つがいは分母に残る。

前回 64 回の 20 秒走行で 1 回だけ出た FAIL（`final64/run52`、2 pairings / 0 children）は
この 2 つの合成だった。

### 1.2 「どの種が繁殖するか」を Rust に写さずに済ませる

依頼は「`Species::breeds()` のような 1 か所に。ただし、どの種が繁殖するかの事実は
`creatures/*.rb` にあるので、導けるならそれを使え」だった。導けるかを 3 つ当たった。

1. **rubevy に聞く。** `ScriptWorld::publish` は何も返さない。`subscriptions()` は
   `HostState` の購読の **個数** しか返さない（rubevy `src/lib.rs:1149`）。
   「この entity は `mate` を購読しているか」を聞く口は無い。これは
   `tell_newborns_the_sky` のコメントが既に書いている壁と同じもの。
2. **`world.rb` / `world_prelude.rb` に聞く。** 持っていない。上に引いたとおり、
   「知らないことにしてある」のが規則側の設計である。
3. **prelude に聞く。** `garden/ruby/prelude.rb` の `Creature.on` は

   ```ruby
   def self.on(event, &block)
     slot = handlers.size
     define_method("__handler_#{slot}", &block)
     handlers << [event.to_sym, slot]
   end
   ```

   と書いてあり、`handlers` は `@handlers ||= []`。つまり**種ごとのクラスオブジェクトの
   `@handlers` に `[:mate, 1]` が入っている**。ファイルが読まれた時点で入るので、タスクが
   始まる前から正しい。

3 を採った。読む道はセーブが `@memory` を読むのと同じ道で、1 段深いだけである
（`garden/src/main.rs` の `listens_for`）:

```rust
let being = vm.ivar_get(task, BEING_IVAR);   // run_creature が置いた生き物オブジェクト
being.obj()?;
let class = vm.real_class_of(being);          // creature "Rabbit" do … end が作った無名サブクラス
let handlers = vm.ivar_get(class, HANDLERS_IVAR);
```

`@handlers` は `[[Symbol, Integer], …]` なので `sabiruby_serde::from_value::<Vec<(String, u32)>>`
がそのまま読む（de は Symbol を String として visit する。`serde/src/de.rs:110`）。
`ivar_get` はクラスオブジェクトにも効く——sabiruby の ivar は `ObjKind` を問わず
`Object.ivars` に載る（`src/object.rs:870`）ので、クラス変数ではなくクラスの ivar としてそのまま
読める。**Rust には種の表を持たせていない。** `rabbit.rb` に `on(:mate)` を書き足せば、
翌フレームから数えられるようになる。

`@being` がまだ無い（タスクが `run_creature` に届いていない）ときは `None` を返し、呼び手は
**数える側に倒す**。判定を緩めるのは「言えるとき」だけにする、という立て方である。

### 1.3 「測れなかったつがい」を記録して分母から外す

穴 b のほうは、つがいを 1 つずつ持っておいて後で閉じる形にした。`SelfTest` に

```rust
courtings: u32,                                   // 分母
courtings_lost: u32,                              // 測れなかった数（文言のためだけ）
courtings_open: Vec<(Entity, Entity, f32)>,       // まだ決着していないつがい
```

を置き、`close_courtings`（`answer_garden` の末尾、毎フレーム）が 3 通りに閉じる:

* **言われた側が `garden.spawn` を呼んだ** → 測れた。黙って外す（答えが「庭が満杯」でも同じ:
  道は歩かれた）。呼んだかどうかは `answer_spawn` に `asked_for_a_child` を足して拾っている。
* **どちらかが世界から消えた** → 測れなかった。`selftest: --` を 1 行出して分母から引く。
* それ以外 → そのまま待つ。走行が終わるまで決着しなければ分母に残る——**これは意図**で、
  「`on(:mate)` から `garden.spawn` までの道が歩かれなかった」は 8 番が報告すべきことだから。

判定側の文言も直した。`n/a` の括弧は

```
(not measured: the rules paired nobody in the whole run who could have answered
 — 2 pairing(s) ended before the handler could ask, so nothing asked `Genome#mix` anything)
```

になり、「誰も居なかった」のか「測れないまま終わった」のかが 1 行で分かる。

### 1.4 作った状況で引き当てる

**穴 a。** `plant_the_meadow` の隅を一時的にウサギ 2 匹（満腹 90、即座につがいになる）にし、
さらに `spawn_world` の生き物を全部ウサギにした使い捨ての版で 20 秒走らせた:

```
selftest: --   the rules paired two of a species whose file has no on(:mate): … (×5)
selftest: n/a  a child was born whose genome is its parents' mixed and mutated
         (not measured: the rules paired nobody in the whole run who could have answered, …)
```

同じ版で `listens_for` を `Some(true)` 固定（＝直す前の数え方）に差し替えると、同じ状況が

```
selftest: FAIL a child was born whose genome is its parents' mixed and mutated (none was, from 5 pairings)
```

になる。**前後で変わるのは判定の言葉だけで、庭で起きたことは 1 つも変わっていない。**

**穴 b。** 数えた直後に相手を `world.despawn` する使い捨ての probe を入れて 20 秒:

```
selftest: --   the pairing at 1.06 s measured nothing: the partner was gone before the handler could ask for a child
selftest: --   the pairing at 2.97 s measured nothing: the partner was gone before the handler could ask for a child
selftest: n/a  a child was born whose genome is its parents' mixed and mutated
         (not measured: … — 2 pairing(s) ended before the handler could ask, …)
```

直す前はこの状況が `FAIL … (none was, from 2 pairings)` である。

probe は 3 つとも外した（`grep -c PROBE garden/src/main.rs` が 0）。

### 1.5 素のままの走行

`--headless 90` を 5 回（並列、`TMPDIR` 別）:

| | 判定 | 除外されたウサギのつがい |
|---|---|---|
| run 1 | ok 13 / FAIL 0 / n/a 0 | 1 |
| run 2 | ok 13 / FAIL 0 / n/a 0 | 2 |
| run 3 | ok 13 / FAIL 0 / n/a 0 | 7 |
| run 4 | ok 13 / FAIL 0 / n/a 0 | 5 |
| run 5 | ok 13 / FAIL 0 / n/a 0 | 1 |

**ウサギのつがいは珍しくない**（90 秒で 1〜7 組）。そして 8 番の行は 5 走行とも
`[N pairings, N children]`——**数えたつがいの数と子の数がぴったり同じ**になった。前は
`[9 pairings, 3 children]` のような行が普通で、その差はほとんどがウサギだったということである。

### 1.6 直していない 3 つ目の穴

**夜に `"mate"` を受けた甲虫は寝ている。** `beetle.rb` の `on(:mate)` は `next if @asleep` で
始まる。夜のつがいは子を産まないが、これは規則どおりの振る舞いであって壊れてはいない。
今の実装ではそのつがいは「まだ決着していない」まま分母に残り、その走行でほかに子が 1 匹も
来なければ FAIL になりうる。依頼された除外は 2 つなので手を付けていない（3 つ目の除外を足すか、
`@asleep` をゲームから見る方法を作るか、のどちらかになる）。

---

## 2. Battle の反射チェック — 除外が「その robot」しか見ていなかった

### 2.1 何が起きていたか

`handler_selftest` は「当たりから 0.3 秒以内にハンドラが走ってタンクが曲がったか」を測る。
数えない当たりが 2 種類あり、そのうちの 1 つが

```rust
if tasks.get(watch.robot).ok().map(|t| t.task()) != watch.task { … }
```

——**その robot の brain タスクが差し替わった**当たりである。ところが編集チェックは
Apply（1 台）→ Apply to all（同じファイルの全台）→ Revert（1 台）→ Restart（4 台全部）を
0.5 秒おきに押す。`Apply to all` と `Restart` は**他の robot の brain も差し替える**ので、
その当たりも同じ 0.3 秒を失う。しかも当たった robot は、たいてい編集チェックが映している
robot ではない。

前回ブラウザで 4 回に 1 回出ていた

```
FAIL 3 blue/scout ran a handler within 0.3 s of the hit at 3.78 s
```

の 3.78 秒は、Revert（3.5 s）と Restart（4.0 s）のあいだである。

### 2.2 タイミングを 1 か所に出す

`EditChecks` という Resource を足し、編集チェックの側が

* `edits.asked(now, "Apply")` / `"Apply to all"` / `"Revert"` / `"Restart"`（押した瞬間）
* `edits.landed(now)`（step 5、Restart の 4 台がそろって見えた瞬間）

を書き、`handler_selftest` が `edits.over(hit, hit + 0.3)` で読む。区間は
**最初の差し替えから、最後の差し替えが着地したのを見た瞬間まで**で、どちらも観測した時刻であり、
新しい数は 1 つも置いていない。閾値の 0.3 秒はそのまま——聞いている窓は同じ窓で、変わったのは
「その窓についてこの判定が答えられる当たりかどうか」だけである。

`SelfTest` のフィールドにしなかったのは、2 つの判定が一緒に走らないから。編集チェックは
窓がある走行にしか登録されない（`checks_asked && headless.is_none()`）が、反射チェックは
headless でも走る。`EditChecks` は `checks_asked` なら両方で `init_resource` され、headless では
空のまま——空なら何も除外しない。

`Revert` も差し替えである（`do_editor_actions` の `EditorAction::Revert` が `restart` を呼ぶ）
ので 4 つとも記録している。

### 2.3 作った状況で引き当てる

ブラウザ（swiftshader）は遅く、40 秒の走行で当たりが 1〜6 発しか出ない。編集チェックが
差し替えている 1.6 秒に当たりが落ちるのは 8 回に 1 回ほどで、素の走行では滅多に踏めない。
そこで**踏ませた**: 使い捨ての probe で `Apply to all` を 10 秒から 30 秒まで 0.5 秒おきに
押し（＝差し替えが試合の真っ最中に連続で起きる）、除外した当たりについて
「直す前の数え方なら何と言ったか」も一緒に出した。3 回走らせて:

```
run 1: PROBE the old counting would have said ran=false turned=true
       the editor's checks were handing out behaviours (Apply to all (probe))
       within 0.3 s of the hit on 4 blue/scout at 27.91 s: not counted
run 3: PROBE the old counting would have said ran=false turned=true
       … on 3 blue/scout at 12.37 s: not counted
```

**3 回のうち 2 回、直す前なら `FAIL … ran a handler within 0.3 s` になっていた当たりを
引き当てた。** 片方（run 1）は編集チェックが映していない robot 4 のもので、これが新しい除外が
無ければ拾えない側である。同じ 3 走行で、既にあった「その robot の brain が替わった」除外も
5 回働いている（run 2 に 3 回、run 3 に 2 回）——古い除外が無意味だったのではなく、**届く範囲が
足りなかった**。

**分かっていないこと。** run 1 の robot 4 は `Apply to all` で brain を差し替えられている
はずなのに、古い除外（タスクの同一性）は働かなかった。別の probe で ObjId を出してみると
`task then Some(ObjId(2686)) now Some(ObjId(2686))` という当たりがあり、差し替えの前後で
ObjId が同じになる場合があることは見えた。ただしその行は `ran=true` の当たりのもので、
run 1 の当たりについて同じことが起きていたかは測っていない。VM が解放したスロットを使い回した
のか、差し替えが窓の**前**に終わっていて窓の中で購読が間に合わなかっただけなのかは、
どちらもありうる。今回の直しはどちらであっても効くので、そこは追っていない。

probe は全部外した（`grep -c PROBE sabibots/src/main.rs` が 0）。

### 2.4 素のままの走行

ブラウザ `sabibots/?selftest`（Chromium、swiftshader、40 秒）を **8 回**:

| 回 | selftest 行 | ok | FAIL | pageerror | requestfailed | console.error |
|---|---|---|---|---|---|---|
| 1 | 35 | 34 | 0 | 0 | 0 | 0 |
| 2 | 43 | 42 | 0 | 0 | 0 | 0 |
| 3 | 41 | 40 | 0 | 0 | 0 | 0 |
| 4 | 41 | 40 | 0 | 0 | 0 | 0 |
| 5 | 51 | 50 | 0 | 0 | 0 | 0 |
| 6 | 40 | 38 | 0 | 0 | 0 | 0 |
| 7 | 33 | 32 | 0 | 0 | 0 | 0 |
| 8 | 41 | 40 | 0 | 0 | 0 | 0 |

**FAIL は 8 回とも 0。** 新しい除外が働いたのは run 6 の 1 回:

```
selftest: --   the editor's checks were handing out behaviours (Revert) within 0.3 s of
               the hit on 4 blue/scout at 3.62 s: not counted
```

——やはり映している robot 3 ではなく robot 4 の当たりである。

8 回で 0 回は、前の「4 回に 1 回」から見れば偶然でも 1 割ほどの確率で起きる。**8 回だけでは
決め手にならない**ので、§2.3 の作った状況のほうを証拠として置いている。

PC 側（`SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 30`）を 3 回:

```
run 1: ok 46 / FAIL 0 — a handler ran within 0.3 s of the hit (21/21)
run 2: ok 50 / FAIL 0 — (23/23)
run 3: ok 23 / FAIL 0 — (10/10)
```

headless には編集チェックが無いので `EditChecks` は空のまま、除外は 1 回も働いていない。

---

## 3. 2 件が入ったあとの確認

```
$ cargo build --release -p garden -p sabibots
    Finished `release` profile [optimized] target(s) in 5.99s
$ cargo clippy -p garden -p sabibots -p rubevy-arena
warning: `rubevy-arena` (lib) generated 2 warnings       （着手前と同じ）
warning: `garden` (bin "garden") generated 13 warnings   （着手前と同じ）
warning: `sabibots` (bin "sabibots") generated 12 warnings （着手前と同じ）
$ cargo test -p garden --release
test result: ok. 6 passed; 0 failed
$ grep -rn unsafe garden/src/ sabibots/src/ crates/ | wc -l
0
```

* 箱庭 `--headless 90` ×5: **13 判定すべて ok**、FAIL 0 / `n/a` 0。
* Battle `--headless 30` ×3: FAIL 0。
* `web/build.sh`（両ゲーム）＋ Chromium:
  * `garden/?selftest` 60 秒 — **ok 43 / FAIL 0 / n/a 0**、pageerror 0 / requestfailed 0 /
    console.error 0、canvas 1280×800。ウサギのつがいの除外行がブラウザでも 1 行出ている。
  * `sabibots/?selftest` 40 秒 ×8 — 上の表。

触っていないもの: `ruby/world.rb`、`ruby/creatures/*.rb`、`ruby/robots/*.rb`、
`ruby/prelude.rb`、閾値（0.3 秒、0.2 rad、変異率 0.1、`mate_reach` ほか `world.rb` の数）。
