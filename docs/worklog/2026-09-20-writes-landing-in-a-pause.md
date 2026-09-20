# 2026-09-20 一時停止の中に「書き」が着地したように見えた件 — 動いていたのは世界ではなく行の順番

計画書 `plans/shared-crate-plan.md` の段階 **S8**。**調査だけで、ゲームのコードは 1 行も直していない。**
作業場所は worktree `rubevy_games-wt-numbers`（ブランチ `s8-pause`、main `40227eb` から）。
リポジトリに入れるのはこの記録と `docs/README.md` の 1 行だけで、計器のコードはコミットしていない
（パッチとログは `.../scratchpad/s8/`、パッチは `s8-probes.patch`）。

S7 の担当が見たもの（`2026-09-20-window-check-fixes.md` §0.2 と末尾の気づいた点 1）:
`Cargo.lock` の rubevy を `33d851a` → `d347711` に上げただけの状態で、ブラウザの `garden/?selftest`
を 3 走行したうち 1 走行が

* `FAIL 2 s paused: every creature is where it was`
* `FAIL 2 s paused: nobody got hungrier`

の 2 行だけを落とし、**同じ走行で `nothing ran while it was paused` は ok** だった。
VM は 1 命令も走っていないのに component への書きだけが止まりの中で着地した、という形である。
S7 の読み（未確認）は「rubevy の R4（時間切れのフレームで答え切れなかった問いを次のフレームへ回す）
が作る形ではないか」だった。

---

## 0. 結論

**R4 は関係が無い。rubevy も関係が無い。止まっている間に動いたのは世界ではなく、
判定が世界を読むときの `Query` が個体を返す順番である。**

止まりの中で `Transform` も `Hunger` も 1 つも書き換わっていない。変わったのは、ある個体が
**別の archetype へ移った**せいで `bodies.iter()` の並びが入れ替わったことだけで、判定はその並びを
`Vec<(Entity, Vec3)>` の `==` で比べているので、値が全部同じでも FAIL する。

止まりの中で archetype を動かすのは、`is_still` を持っていない 3 つの系統である:

| 何が | どこ | いつ |
|---|---|---|
| rubevy の `start_scripts` が `ScriptTask` を付ける | `RubevySet::Deliver`（run condition 無し） | `Script` が着いた次のフレーム |
| `dress_animations` が `Animated` を付ける | `garden/src/main.rs:5520`（run condition 無し） | モデルの `AnimationPlayer` が現れたフレーム。**ブラウザでは生まれてから数フレーム後**|
| `note_the_rules` が `Eating` を付ける | `garden/src/main.rs:3611`（run condition 無し） | 止める直前のフレームに誰かが食べていたとき、その `Commands` が着地するフレーム |

どれも「世界が動いた」ことではない。**生まれたての個体にスクリプトと見た目が着いただけ**である。
一時停止の直前・直近に**出産があった走行でだけ**この 3 つが止まりの中に食い込み、判定が落ちる。

---

## 1. 判定が比べているもの

`garden/src/window.rs` の窓のチェック、段 2 で標本を取り、段 3 で比べる:

```rust
let places = || -> Vec<(Entity, Vec3)> { bodies.iter().map(|(e, t, _)| (e, t.translation)).collect() };
let hunger = || -> Vec<(Entity, f32)> { bodies.iter().map(|(e, _, h)| (e, h.0)).collect() };
…
ok(places() == test.places, "2 s paused: every creature is where it was");
ok(hunger() == test.hunger,  "2 s paused: nobody got hungrier");
```

`bodies` は `Query<(Entity, &Transform, &Hunger), With<Creature>>`。`Vec` の `==` は**順番まで見る**。
`Query::iter` は archetype ごとにまとめて返すので、1 体が archetype を移れば並びが変わる。
その並びは世界の状態ではない。

判定のコメントは「"nothing moved" is an equality rather than a tolerance」と書いてあり、
**許容誤差を置かないという意図は正しい**。順番まで比べてしまっているのは、その意図の巻き添えである。

## 2. 一時停止が止めているもの、止めていないもの

`P` は `window::inspect_keys`（`garden/src/window.rs:709`）が読む。押すと

* creature の VM の `ScriptWorld::budget = 0`
* world の VM の `ScriptWorld::<World>::budget = 0`
* `Paused::was = Some(…)` → `crate::is_still` が偽

`is_still` を run condition に持っているのは `day_night` / `move_creatures` / `separate` / `startle` の鎖、
`children_arrive`、`tell_newborns_the_sky`、そして **`RubevySet::<World>::tick()` の set 全体**
（`main.rs:1857`）。世界の規則は `ruby/world.rb` なので、草も空腹も繁殖も餓死も set ごと止まる。

止まっていないのは、`RubevySet::Deliver`（両 VM）、`RubevySet::Tick`（creature VM。`budget = 0` で
1 命令も走らないが **`drain_commands` と `apply_component_writes` は走る**）、
`RubevySet::Answer`（両 VM）、`note_the_rules` / `sprout_plants` / `plants_wear_their_size`、
`catch_up_minds`、`dress_animations` / `animate_creatures`、カメラとパネルと保存。

`inspect_keys` は `watch_minds` の後 = `RubevySet::Answer` の後なので、**`P` を押したフレームの
両 VM の tick はもう走り終えている**。だから「止めたフレームの書き」は必ずそのフレームのうちに
着地する。ここは正しく、判定が 0.2 秒（ブラウザで 1 フレーム）待ってから標本を取るのも正しい。

## 3. 計器

使い捨てで 3 つ入れた（`s8-probes.patch`、コミットしていない）。

1. **`s8_trace`** — `inspect_keys` と `RubevySet::Answer` の後に置き、段 1〜4 の各フレームで
   「判定と同じ問い方で並べた個体の一覧」を、`Script` / `ScriptTask` / `Animated` / `Eating` の
   有無と位置と空腹つきで印字する。**前のフレームと 1 文字でも違うときだけ**中身を出し、
   同じなら `(unchanged)` の 1 行にする。止まりが本当に止まっているなら 1 行ずつしか出ない。
2. **`s8_diff`** — 段 3 で落ちたとき、2 つの標本の差を「消えた個体 / 増えた個体 / 値が変わった個体」
   に分けて言い、**3 つとも空なら `order-only=true`** と書く。並びそのものも 2 行で出す。
3. **`GARDEN_S8_TOUCH` / `GARDEN_S8_BIRTH`** — 止まりの n フレーム目に 1 体へ「どこにも使っていない
   component」を付ける／`P` を押すフレームに 1 体生ませる。どちらも
   `birth_in_the_apply_frame`（S7）と同じ作りで、後者は `Births` に 1 件積むだけなので
   `children_arrive` が本物の出産とまったく同じ手順で体を作る。

`GARDEN_S8_*` は wasm では読めない（`std::env` が無い）ので、ブラウザ側は素のまま回した。
`docker/run.sh` は `<GAME>_SELFTEST` しか渡さないので、S7 と同じく `docker run` を手で書いた
（`rungarden.sh`）。**隣の担当（S5b）と同じ target volume を使わないよう、`rubevy-games-target-s8`
と `rubevy-games-cargo-s8` を別に作った。**

## 4. ブラウザで数えた

`web/build.sh garden`（`CARGO_TARGET_DIR` はこの worktree のもの）→ `python3 -m http.server 8231`
→ Playwright で `garden/?selftest` を開き、`selftest: done` が出たら閉じる。1 走行 2〜3 分。

| 版 | 走行 | `2 s paused` の 2 行が落ちた | 止まりの前後に出産があった |
|---|---|---|---|
| S7 が lock だけ上げた版（S7 §0.2） | 3 | **1** | **1**（その走行） |
| S7 の最終版（S7 `final/`） | 14 | 0 | 0 |
| S8（= main `40227eb`、計器つき） | 21 | 0 | 0 |
| 合計 | **38** | **1** | **1** |

**出産があった走行と落ちた走行は同じ 1 走行で、それ以外の 37 走行にはどちらも無い。**
S7 の落ちた走行のログ（`s7/lock/browser-garden-1.txt`）は、`ok F2 opens it` の次の行が

```
a Beetle was born at 3.3 s (532v1) — speed 2.11, sight 7.2, appetite 0.97
```

で、その次が `ok P pauses: the scripts' budget is 0` である。**出産は `P` を押したフレームか
その 1 つ手前に落ちている。**

## 5. 止まりの中で archetype が動く瞬間は、落ちなかった走行でも見えた

S8 の 1 走行目（`run1/g-001.txt`、判定は全部 ok）の計器:

```
s8probe f17 step=2 … 414v0[STA- …] 938v0[STA- -13.060,0.000,9.226 h63.1486] 410v0[…] …
s8probe f18 step=3 … 414v0[STA- …] 412v0[…] 410v0[…] … 938v0[STAE -13.060,0.000,9.226 h63.1486] 430v0[…]
```

`938v0` は f17 で 2 番目、f18 で 13 番目。**位置も空腹も 1 桁まで同じで、変わったのは
`Eating` が付いたこと（`STA-` → `STAE`）だけ**である。`Eating` を付けるのは `note_the_rules` で、
それは止める直前のフレーム f17 に「この個体の空腹が増えた」（48.5 → 63.1、f17 の world の tick で
食べた）のを見て `Commands` に積んだものだ。f17 の終わりに着地している。

この走行が落ちなかったのは、**段 2 の標本も f18 で取られていて、着地の後だった**からである。
つまり同じ出来事が 1 フレームずれるだけで ok と FAIL が入れ替わる。

止まりの他の 8〜11 フレームは、21 走行のうち 20 走行で `(unchanged)` が並んだだけだった。
`carried=…+…`（R4 の持ち越し。`FrameStats::carried_reflect` / `carried_in_tick`）は
**止まりの全フレームで 0+0**、1 度だけ止まる前のフレームに `carried=2+0` が出たきりである。

## 6. 意図的に再現した

PC（docker + lavapipe + WSLg の窓。1 走行 25 秒）で 2 通り。

**(1) 値を 1 つも変えずに archetype だけ動かす。** `GARDEN_S8_TOUCH=5` で、止まりの 5 フレーム目に
1 体へ空の marker component を付ける:

```
selftest: s8probe touched 412v0 on pause frame 5 (no value of the world changed)
selftest: s8probe places: len 14 -> 14  order-only=true
selftest: s8probe   before-order: 410v0,412v0,424v0,416v0,418v0,420v0,426v0,440v0,444v0,422v0,725v0,428v0,414v0,434v0
selftest: s8probe   after-order:  410v0,426v0,424v0,416v0,418v0,420v0,440v0,444v0,422v0,725v0,428v0,414v0,434v0,412v0
selftest: FAIL 2 s paused: every creature is where it was
selftest: FAIL 2 s paused: nobody got hungrier
```

**落ちるのはこの 2 行だけ**で、`nothing ran while it was paused` も `the day did not turn` も ok。
S7 が見た姿とまったく同じである。`GARDEN_S8_TOUCH=1`（標本を取ったフレームの、標本の直後）でも同じ。

**(2) 本物の出産で再現する。** `GARDEN_S8_BIRTH=2` は `P` を押すフレームに 1 体生ませる。
20 走行のうち **2 走行**が落ちた（`pc/b2-11.log`、`pc/b2-15.log`）。落ちた 2 走行はどちらも
`order-only=true` で、**動いたのは生まれたてのその 1 体**である:

```
before-order: 772v1,426v0,424v0,422v0,436v0,…
after-order:  426v0,424v0,422v0,772v1,436v0,…
```

`772v1` は標本のときには先頭（生まれたてだけの archetype）にいて、判定のときには 4 番目
（`Script` + `ScriptTask` + `Animated` が揃った archetype）に移っている。
落ちなかった 18 走行では、この着地が**標本より前**に起きていた。

つまり **1 フレームの勝ち負け**である。`window_selftest` は `.before(inspect_keys)` と
`.before(choose_watched)` しか持たず、`RubevySet::Deliver`（`start_scripts`）とも
`dress_animations` とも順序が付いていないので、どちらが先に走るかはその走行の executor 次第になる。

## 7. R4 の前後

`Cargo.lock` の rubevy を `33d851a`（R4 の前）に落とした版と `d347711`（今）の版を、
**同じソース・同じ計器・別の `CARGO_TARGET_DIR`（docker の volume を 2 つ）**で建て、
`md5sum` が違うことを確かめてから、`GARDEN_S8_BIRTH=2` つきで**交互に**回した。

| 版 | 走行 | 再現 | 備考 |
|---|---|---|---|
| `d347711`（R4 あり） | 20 | 0 | 別に先に回した 20 走行では 2（計 2/40） |
| `33d851a`（R4 の前） | 20 | **1** | `order-only=true`、同じ姿 |

**R4 の前の版でも同じ形で落ちる。** これで R4 は外れる。率はどちらも 5%（2/40 と 1/20）で、
この走行数では区別が付かない——付ける必要も無い。**「R4 が無くても起きる」1 件のほうが、
率の差より強い証拠である。**

なお `33d851a` には `ScriptWorld::last_frame()` がまだ無いので、前後を比べる回にかぎり計器から
`carried` と `run` の 3 つの数を外し、**両方を同じ計器で建て直してから**回した
（片方だけ計器が違う比較は取らない）。最初の 20 走行（2/20）は数つきの計器、後の 40 走行は
数無しの計器なので、この 2 つの率を足して 1 つの率と呼んではいけない。

## 8. 捨てた仮説

* **(a) 一時停止を決めるシステムと `RubevySet::Tick` の順序がずれていて、止めたフレームの書きが
  次に着地する。** 外れ。`inspect_keys` は `watch_minds` → `RubevySet::Answer` → `RubevySet::Tick`
  の後にしか走れず、標本は次のフレームで取られる。計器でも、止めたフレームの値は
  次のフレームで 1 つも動いていない。
* **(b) R4 の持ち越し（`reflect_requests` / `in_tick_requests`）が止まりの中で答えられ、
  答えが書きを生む。** 外れ。`tick_scripts` の答えの輪は `budget - spent == 0` で頭から抜けるので
  `answer_reflect_requests` も `answer_in_tick_requests` も呼ばれない。計器の `carried` は
  止まりの全フレームで 0+0。そして §7 のとおり R4 の無い版でも落ちる。
* **(c) Rust の system が `Hunger` や `Transform` を書いていて、run condition が 1 フレーム遅れる。**
  外れ。`Hunger` を書くのは `world.rb` だけで、その tick は set ごと止まる。止まりの中で
  値が動いた走行は 1 つも無い（`order-only=true` がそれを言っている）。
* **(d) ブラウザの 1 フレーム 250 ms で `Time` の delta が大きく、判定の許容を超える。** 外れ。
  判定に許容は無く、差は値ではなく並びだった。
* **(e) それ以外** = 当たり。§0。

**R11（`Rubevy.next_frame`）は games の lock（`d347711`）にまだ入っていない**ことも確かめた。
`next_frame_waiters` も `wake_next_frame` も `458dfe9` / `6ca5475` で、どちらも `d347711` より後である。
「`budget = 0` のときに tick の頭で何が起きるか」を読む対象からは外れる。

## 9. 直し方の案

原因は 2 段ある。**(A) 判定が並び順を比べていること**と、
**(B) 一時停止の中でも新しい個体に `ScriptTask` と `Animated` が着くこと**。
落ちるのは (A) のせいで、(B) はそれ自体は正しい動きだと思う。

### (A) 判定の側（ここを直せば落ちなくなる）

1. **個体ごとに引き当てて比べる。** `Vec` の `==` をやめ、entity を鍵に突き合わせて
   「居なくなった / 増えた / 値が変わった」を別々に言う。計器の `s8_diff` がすでにその形で、
   落ちたときに何が起きたかまで印字できる。**代償はほぼ無い**（14 体の二重ループ）。
   判定の意図（許容を置かない）はそのまま。**推奨。**
2. **標本と判定の両方を entity で並べ替えてから比べる。** 1 行で済む。代償: 増減が
   「並びが違う」に化けたままなので、落ちたときの文面が 1 と比べて何も言わない。
3. **生まれたての個体を判定が数えない**（`Without<ScriptTask>` を足す等）。S6 の案 A3 と同じ形で、
   著者が一度却下している。止まりの中で本当に個体が動いたときも見逃す。**採らない。**

### (B) 一時停止の作りの側（直さなくてよいが、選択肢として）

1. **何もしない。** archetype の移動は世界の状態ではない。`P` は「世界の規則と時計と VM を止める」
   ものであって「Bevy の世界を凍らせる」ものではない、と判定のコメントに 1 行書く。代償: 無し。
   **(A)1 と組で推奨。**
2. **`dress_animations` と rubevy の `RubevySet::Deliver` も `is_still` で止める。**
   止まりの中では本当に何も着かなくなる。代償が重い: 一時停止中に生まれかけた個体がモデルの無い
   まま止まって見え、`deliver_answers`（非同期の答えの受け取り）まで止まる。
   **止める理由が「判定が落ちるから」しか無い。採らない。**
3. **一時停止中は出産を `Births` に溜める。** `children_arrive` はすでに `is_still` で止まるので、
   `P` を押したフレームに `children_arrive` が `inspect_keys` より後に走れば、実際そうなっている
   （どちらが先かは順序が付いていないので走行次第）。順序を付けて必ず溜まるようにすれば、
   止まりの中に生まれたてが入らなくなる。代償: `P` を押した瞬間の 1 体が 2 秒遅れて生まれる
   （プレイヤーには見えない）。(A) を直さないかぎり `Eating` の経路は残るので、**単独では不足**。

### (C) 標本の取り方

4. **標本を「止まって 1 フレーム経った」フレームで取る**（S7 が入れた `Turn` の仕組みで待つ）。
   `Eating` の経路には効く。**`dress_animations` には効かない**——ブラウザではモデルが数フレーム
   遅れて来るので、何フレーム待っても「その後に来る」可能性は残る。症状を薄めるだけ。

## 10. rubevy にとって何が約束で、何が約束でないか

`docs/host-api.md` の "Pausing" は

> `world.budget = 0` is the pause: the VM checks the budget at the head of its own loop, so not one
> instruction runs. … Everything else in the frame goes on while paused: `$rubevy` is still
> refreshed …, and questions are still taken and answered — they simply reach a script that is not
> running.

と書いている。**「1 命令も走らない」は約束されており、実測でもそのとおりだった**
（`nothing ran while it was paused` は落ちた走行でも ok）。
**「component への書きが止まる」は、どこにも約束されていない。** 今回の件はその約束の破れではない。

外の利用者（`budget = 0` で一時停止を書く人）に向けて、今の文書に足りないと思ったのは 2 つ:

* **止まったフレームでも `RubevySet::Tick` の 4 本は走る。** `tick_scripts` が 1 命令も走らせない
  ので新しい書きは出ないが、`drain_commands` / `apply_component_writes` / `apply_resource_writes`
  は走る。つまり「止める前のフレームの書きが VM の待ち行列に残っていれば、それは止まりの中で
  着地する」。今回は残っていなかった（`carried` が全フレーム 0、値の変化 0）が、
  約束としてどちらなのかは文書から読めない。
* **`budget = 0` と「set に run condition を付ける」は同じではない。** 箱庭は world の VM を
  `RubevySet::<World>::tick().run_if(is_still)` で止めているが、この set には
  `drain_commands` と `apply_component_writes` も入っている。set を止めると**その VM の書きの
  着地も止まる**——1 フレーム前の書きが宙に浮きうる。`budget = 0` のほうは着地は止まらない。
  「どちらで止めるか」で意味が変わることは "Pausing" にも "Two VMs in one app" にも書いていない。
* そして **`start_scripts` は `RubevySet::Deliver` にあり、`budget` を見ない**。
  一時停止中でも新しい `Script` はタスクになる（走りはしない）。「止めるのは命令であって
  タスクを作ることではない」というのは正しい設計だと思うが、書いてはいない。

どれもバグではなく、**文書の穴**として rubevy に投げる話である。

## 11. 分からなかったこと

* **ブラウザで自然に再現することはできなかった**（21 走行、出産が止まりに当たらなかった）。
  当たった 1 走行は S7 のもので、そのログを読んで相関を言っている。出産のタイミングを
  ブラウザ側から操る道が無い（`GARDEN_S8_*` は wasm で読めない）ので、意図的な再現は PC でだけ
  取った。ページの問い合わせ文字列につまみを足せば同じことができるはずだが、S8 の範囲ではない。
* **率は測っていない**（`GARDEN_S8_BIRTH` つきで 5%、素の走行では 38 走行に 1 件）。
  素の走行の率は「止まりの前後に出産が落ちる率」× 「その着地が標本の後に来る率」であり、
  前者は世界の混み具合で動く。数えるなら段を分けて数えるべきで、今は数えていない。

---

## 気づいた点

1. **（この件の本体）窓のチェックの `places()` / `hunger()` が `Vec` の `==` で
   `Query` の並び順まで比べている**（`garden/src/window.rs:1378-1382` と段 3）。
   止まりの中で 1 体が archetype を移るだけで落ちる。移すのは `start_scripts`（rubevy、
   `RubevySet::Deliver`）・`dress_animations`（`main.rs:5520`）・`note_the_rules`（`main.rs:3611`）で、
   どれも `is_still` を持たない。再現手順: `GARDEN_S8_TOUCH=5`（止まりの 5 フレーム目に
   marker を 1 つ付ける）で 1 走行 1 件、`GARDEN_S8_BIRTH=2`（`P` のフレームに 1 体生ませる）で
   20 走行 2 件。→ 箱庭（判定の作り）／バグ。
2. **`window_selftest` が `RubevySet::Deliver` とも `dress_animations` とも順序を持っていない。**
   `.before(inspect_keys)` と `.before(choose_watched)` だけなので、判定が「フレームのどの時点の
   世界」を見るかは executor 次第である。S7 が `.before()` を足すと Bevy が同期点を挿して別の
   判定が落ちた話（S7 §3.3）と同じ棚で、**順序を足すのが正解とは限らない**のが厄介なところ。
   → 箱庭（判定の作り）／本（「いつ測るか」が測るものを変える話）の素材。
3. **`docs/host-api.md` の "Pausing" が、書きの着地について何も言っていない**（§10）。
   `budget = 0` の止め方と、set に run condition を付ける止め方とで、`drain_commands` と
   `apply_component_writes` の扱いが変わる。外の利用者が最初に踏む段差だと思う。
   → rubevy（文書と実物のずれ）。
4. **`GARDEN_S8_*` のようなつまみがブラウザから渡せない。** wasm には `std::env` が無く、
   `docker/run.sh` は `<GAME>_SELFTEST` しか渡さない（S7 の気づいた点 2 と同じ話）。
   ページの `?selftest` と同じ場所に `&probe=…` を読む口があれば、ブラウザでだけ出る揺れを
   ブラウザで追える。今回はそれが無いので PC で代わりを立てた。→ 道具。
5. **docker の target volume を worktree ごとに分ける必要がある。** 隣の担当（S5b）が
   `rubevy-games-target` を使っているので `rubevy-games-target-s8` を作った。
   `docker/build.sh` はどの worktree も `/app` に mount するので、volume を分けないと
   別ブランチの成果物を掴む（`implementer.md` の警告そのまま）。→ 作法。
6. **`Paused::on()` と `ScriptWorld::budget == 0` は同じ事実を 2 か所で言っている。**
   `Paused` の doc コメントはそれを承知で「決めた側が言う」と書いてあり、判定もその 2 つを
   突き合わせている（`P pauses: the scripts' budget is 0`）。世界の VM のほうは判定が見ていない
   （`rules.budget` は計器を入れて初めて見た）。落ちるとしたら片方だけ 0 のときだが、
   `inspect_keys` が両方を同じ行で書くので今は起きない。→ 箱庭（判定の穴、小）。
