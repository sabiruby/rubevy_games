# 2026-09-20 窓のチェックの揺れを直す — 種の世代と、秒でなくフレームで待つこと

計画書 `plans/shared-crate-plan.md` の段階 **S7**。前の段階 S6
（`2026-09-20-window-check-flakes.md`）が、箱庭の窓のチェックが時々落とす 3 行 + 4 行目の原因を
**2 つ**に割り、どちらも人の手で再現したところまでを記録している。この記録はその続きで、
**その 2 つを直す**。S6 が調べただけで 1 行も直さなかったのに対して、ここは直す側である。

作業場所は worktree `rubevy_games-wt-shared`（ブランチ `shared-crate`）。

* **原因 A**（ゲームのバグ）: Apply / Revert と同じフレームに生まれた個体は、その `Mind` がまだ
  `Commands` の列の中なので `restart_species` の `Query` に見えず、差し替えから漏れて
  **古いプログラムを走らせ続ける**。
* **原因 B**（判定の側）: 判定は壁時計で 0.6 秒待つが、要るのは VM のスケジューラが新しいタスクに
  順番を回すこと。0.6 秒は静かな PC で 5 フレーム、混んだ PC とブラウザで 3 フレームにしかならない。
* **4 件目** `the meters move again` は原因 B の world VM 版。

本体の指示は「A は S6 の案 A1（種の世代）、B は B1 + B2（条件で待ち、上限はフレームで、その上限は
導く）。判定を n/a にして隠す案 A3 は採らない。0.6 → 1.2 秒のような出どころの無い延長はしない」。

---

## 0. 先に rubevy を上げた（別コミット `a693187`）

games の `Cargo.lock` は rubevy `33d851a`（朝の版）を指していて、その日の R3・R4・R5・R6b・R9・R10 が
入っていなかった。とくに **R4**（「`frame_time` がタスクの実行だけでなく tick 全体の上限になる。
時間切れのフレームの末尾の読みは 1 フレーム遅れる」）は**原因 B の道そのもの**を触っている。
直したものを古い rubevy の上で測っても意味が無いので、lock を先に、それだけで上げた。

`cargo update -p rubevy` は rubevy と `rubevy-build` の 2 つだけを動かし、SabiRuby の rev は動かなかった。
ほかに動いた行は `windows-sys` が 0.61.2 → 0.60.2 に解決し直したところだけで、これは 5 つの crate の
Windows 専用の依存であり、この機械では 1 度も建たない。

**上げただけの状態で測った**（コードは 1 行も変えていない）:

| 確認 | 前（`33d851a`） | 後（`d347711`） |
|---|---|---|
| `cargo build --workspace --all-targets` | 警告 0 | 警告 0 |
| `cargo test --workspace` | 34 passed | 34 passed |
| 箱庭 ヘッドレス（`--headless 90`） | 13 行 FAIL 0 | 同じ 13 行 |
| Battle ヘッドレス（`--headless 25`） | 4 行 FAIL 0 | 同じ 4 行 |
| 箱庭 窓（2 走行） | 43 行 FAIL 0 | 同じ 43 行 |
| Battle 窓 | 32 行 FAIL 0 | 同じ 32 行 |
| ブラウザ（`web/build.sh all` + Playwright） | — | 箱庭 44 行・Battle 33 行、pageerror 0・requestfailed 0 |

比べ方は `tools/fixedlines.sh` で、6 通りの走行の一覧は `verification/selftest-lines.md`。
`diff` は 4 本とも空である。

### 0.1 R4 の後でも、S6 の 2 つの原因はそのまま出る

S6 の仕込み（スクラッチパッドの `s6-probes.patch`）をそのまま当て直して、新しい rubevy の上で
1 走行ずつ確かめた。**3 つとも S6 と同じ形で再現した**:

| 仕込み | 結果 |
|---|---|
| `GARDEN_S6_FORCE_BIRTH=1`（Apply / Revert のフレームに出産を重ねる） | `FAIL Apply restarts every beetle on the edited text` と `FAIL Revert puts every beetle back on the file` |
| `GARDEN_S6_FRAME_TIME_US=100`（creature の VM を絞る） | `FAIL every restarted beetle's new task has run` だけ |
| `GARDEN_S6_WORLD_FRAME_TIME_US=20`（world の VM を絞る） | `FAIL the meters move again` だけ |

0.6 秒の息継ぎに入るフレーム数も変わっていない（静かな機械で **5 フレーム**、f52 → f57）。
つまり R4 は原因 B を悪くも良くもしておらず、S6 の読みはそのまま使える。

なお **`docker/run.sh` はゲームの `<GAME>_SELFTEST` しか渡さない**ので、`GARDEN_S6_*` のつまみは
このスクリプトからは効かない（最初、効かないことに気づかず 4 走行を無駄にした。`ok 43` が出て
`s6probe frame_time set to …` の行が無いのでようやく分かった）。スクラッチパッドに
`rungarden.sh`（`docker run` を手で書き、つまみを `-e` で渡すだけのもの）を置いて回した。
S6 も同じ壁に当たっていたはずで、計画書の気づいた点に既に「`GARDEN_RELOAD_AT` はコンテナから
渡せない」とある。**同じ話である。**

### 0.2 ブラウザで新しい揺れを 1 つ見た（S7 の仕事ではない）

lock を上げただけの状態でブラウザの箱庭を 3 走行したところ、1 走行目だけが
`FAIL 2 s paused: every creature is where it was` と `FAIL 2 s paused: nobody got hungrier` の
**2 行**を落とした（2・3 走行目は `ok 43`）。S6 の 16 走行（古い rubevy）ではこの 2 行は 1 度も
落ちていない。

同じ走行で `nothing ran while it was paused`（VM が 1 命令も走っていない）と
`2 s paused: the day did not turn` は **ok** である。つまり **VM は止まっていたのに、
コンポーネントへの書き込みだけが止まりの中で着地した**。これは R4 の
「`frame_time` が尽きたフレームで答え切れなかった質問は次のフレームに回す」がちょうど作る形で、
Ruby の書き込みは `Request` として答えの側を通るから、`P` の前のフレームで積まれた書き込みが
`P` の後のフレームで適用されうる。ブラウザは 1 フレーム 250 ms で `frame_time` 8 ms を毎フレーム
使い切るので、PC より桁違いに起こりやすい。

**確かめてはいない**（3 走行のうち 1 走行で、原因の読みは R4 の差分を読んだだけ）。S7 の範囲外なので
直していない。末尾の「気づいた点」に挙げる。

---

## 1. 原因 A — 種の「世代」（案 A1）

### 1.1 何を足したか

`Brains`（種ごとに「エディタが持っている本文」を持つ Resource）に、**その種が今着ているプログラム**を
足した:

```rust
struct Wearing {
    handle: Handle<MrbAsset>,
    prelude_lines: u32,
    in_memory: bool,
    /// hand-overs so far, starting at 1
    generation: u32,
}
```

`Mind`（個体）には `generation: u32` を 1 つ。`give_mind` は生まれた瞬間の
`brains.generation(species)` を刻む。Apply / Revert / Save / ファイルの再読み込みは
`brains.hand_over(...)` で世代を 1 つ進め、`restart_species` は見えた個体にその世代を刻む。

そして毎フレーム走る `catch_up_minds`:

```rust
for (entity, mut mind) in minds.iter_mut() {
    let Some(wearing) = brains.wearing(mind.species) else { continue };
    if mind.generation == wearing.generation { continue }
    wear_mind(&mut commands, entity, &mut mind, wearing);
}
```

**新しい数は 1 つも置いていない。** 世代は「何回目の差し替えか」を数えるだけの整数で、閾値ではない。

**今の形と併用にした**（置き換えではなく）。理由は 2 つ。`restart_species` をそのまま残せば、
差し替えが「Apply を押したフレーム」に起きるという今の振る舞いが 1 ミリも変わらない
（置き換えると全員が 1 フレーム遅れる）。もう 1 つは、エディタが出す
「12 Beetles restarted on it」の数が `restart_species` の戻り値だということ。二重に差し替える
心配は無い: `restart_species` が刻んだ個体は世代が一致するので `catch_up_minds` は素通りする。

代償は S6 が書いたとおり「同じフレームに生まれた個体は 1 フレーム遅れて差し替わる」こと。
今は**永久に漏れる**のだから、交換条件として釣り合っている。

### 1.2 世界の規則には同じ穴が無い

`world.rb` は 1 本しかないので `Query` を回る形でもない、で済ませずに読んだ。
規則を着ているエンティティ（`WorldScript`）を作るのは `give_the_world_its_rules` だけで、
これは `Startup` にしか登録されていない。だから「編集が適用されるフレームに `Script::<World>` が
`Commands` の列の中にいる」という状態が作れない。`every` が作るタイマーのタスクも `Query` では
拾われない——`wear_the_rules` の doc が書いているとおり、古いタイマーは次に目を覚ましたときに
`run_world` が書き換えた `$world_being` を見て**自分で止まる**（`ruby/world_prelude.rb`）。
Ruby の側で閉じているので、世代の話が入り込む余地が無い。`Brains::hand_over` の doc に 1 段落で書いた。

### 1.3 sabibots は形を残し、理由をコメントに

Battle の `Apply to all` も `Query` を回る同じ形である。同じ直し方にしなかった理由を
`do_editor_actions` の doc に段落で書いた。要は **Battle には数える対象が無い**:
箱庭の世代は「種が着ているプログラム」に付く番号で、Battle は brain が robot のフィールドだから
「ファイルごとのプログラム」という Resource がそもそも無い。作れば穴を塞げるが、その穴は開かない——
robot を作るのは `start_match` だけで、それを呼ぶのは `restart_match` だけで、`restart_match` は
**同じフレームで全 robot を despawn し**、エディタも消す。だから「この `Query` に見えない `Robot`」は
「これから `KeptBrains` から brain をもらって生まれる robot」であって、
「間違った本文を走らせ続ける robot」ではない。

### 1.4 常設の判定にした

S6 の `GARDEN_S6_FORCE_BIRTH` を、窓のチェックの 1 段として常設した。

* `window::birth_in_the_apply_frame`: `WindowTest` が Apply を押したフレームに、
  `Births` へ甲虫を 1 件積む。**`Commands` で自分で spawn してはいけない**——
  S6 が §2 で踏んだ穴で、`.before(children_arrive)` を付けた時点で Bevy が同期点を差し込み、
  `Mind` が同じフレームで見えるようになって、再現しようとしていた競合そのものが消える。
  Resource への書き込みは遅延しないので、`Births` に積んで**本物の `children_arrive` に作らせる**。
* 遺伝子は `Genome::of(Species::Beetle)`（種そのものの 3 つ）。この甲虫は「どのフレームに来るか」
  だけが仕事なので、数を 1 つも発明しない。
* 判定は step 8 の 1 行: `a beetle born in the very frame of Apply is restarted too`。
  Apply の前に居た甲虫の entity を控えておき、それ以降に生まれたものが全部 `in_memory` かを見る。
* **窓のチェックのときしか登録しない**（`GARDEN_SELFTEST=1` + 窓）。ふつうに遊んでいる庭に
  甲虫が 1 匹増えることは無い。

**この判定が本当に噛むことを確かめた。** `catch_up_minds` の登録だけを一時的に外して 2 走行:

```
== run 1
selftest: FAIL Apply restarts every beetle on the edited text
selftest: FAIL a beetle born in the very frame of Apply is restarted too
== run 2
selftest: FAIL Apply restarts every beetle on the edited text
selftest: FAIL a beetle born in the very frame of Apply is restarted too
```

2 走行 / 2 走行とも両方落ちる。戻すと両方 ok になり、ログに

```
garden::window: Beetle 871v0 was born in the frame its species was handed a new program — handing it over now
```

が 1 行だけ出る（差し替えが 1 回だけ起きた、ということ）。

---

## 2. 原因 B と 4 件目 — 条件で待ち、上限はフレームで（案 B1 + B2）

### 2.1 形

`WindowTest` に「その段が時計のほかに何を待っているか」を足した:

```rust
enum Turn {
    NotWaiting,             // `at` は世界の時間で、言葉どおりの意味
    RestartedBeetles,       // 全部の甲虫のタスクが 1 命令でも走った
    AMeterMoved,            // 誰かのメーターが動いた（書けるのは world.rb の each_frame だけ）
    EguiHasThePointer(bool),// egui がポインタを取った／離した（§2.4 で足した）
}
```

判定の頭で、時計を過ぎたあとに:

```rust
if test.turn != Turn::NotWaiting {
    let came_round = ...;
    test.waited += 1;
    if !came_round && test.waited < SCHEDULER_FRAMES { return }
    info!("selftest: waited {} frame(s) for the thing the next check is about, and it {}", ...);
    test.turn = Turn::NotWaiting;
    test.waited = 0;
}
```

上限に達したらそこで判定する。落ちれば FAIL で、しかも本当のことを言っている——
「VM は順番を配る機会をこれだけ与えられて、走れるタスクに配らなかった」。

Apply / Revert の 2 か所は `test.at = now`（壁時計の待ちは 0）+ `Turn::RestartedBeetles` に、
`the meters move again` の段は **0.5 秒はそのまま**（下の分類参照）+ `Turn::AMeterMoved` にした。

待つ条件が判定の述語そのものである、という点は意図したものである。判定されているのは
「**VM の何フレーム以内に起きたか**」で、それを言っているのが `SCHEDULER_FRAMES` と、
上限に達したときの FAIL である。

### 2.2 上限の 7 フレーム — 式と、式に入れた実測値の出どころ

```
SCHEDULER_FRAMES = 2 + ceil(budget / 1 フレームが買う命令数)
                 = 2 + ceil(200_000 / 45_600)
                 = 2 + 5 = 7
```

* **2 フレームは構造で、縮められない。** 差し替えは `Commands` で新しい `Script` を付けるので、
  コンポーネントが付くのは頼んだフレームの終わり（1）。rubevy の `start_scripts` がそこから
  タスクを作り、`RubevySet::Tick` が走らせる（2）。S6 §4.4 がまさにこれを測っている
  （息継ぎを 1 フレームに縮めると `ScriptTask` がまだ無い、2 走行 / 2 走行）。
  `NEWBORN_DEAF_FRAMES = 2` は同じ勘定を生まれたてについて書いたものである。
* **残り 5 は「1 フレーム分の budget」。** creature の VM の 1 フレームは
  `ScriptWorld::budget`（rubevy の既定 200,000 命令）か `ScriptWorld::frame_time`
  （rubevy の既定 8 ms の壁時計）のどちらかが尽きたら終わる。**8 ms が何命令買うかは S6 が測っている**:
  `frame_time` を 300 µs に絞った走行で、いちばん少ないフレームが 1,708 命令
  （`s6/ft300.log` の f59 — 甲虫 2 体の最初の一走り 854 × 2）。**5.7 命令/µs** なので
  8 ms は約 45,600 命令で、`ceil(200_000 / 45_600) = 5`。
  ここを過ぎたら、VM は 1 フレーム分の順番をまるまる配りきってなお走れるタスクに配っていない——
  それは「機械が混んでいる」ではなく「スケジューラが配るのをやめた」（`restart_species` の doc が
  書いている sabiruby 0.5.1 のバグ。この判定はそれを見張るためにある）である。

**なぜ秒ではだめで、フレームならいいのか**が数字で言える: 同じ 7 フレームが、1 フレーム 133 ms の
静かな PC では 0.9 秒、280 ms の混んだ PC では 2.0 秒になる。秒はそれができない。
0.6 秒が片方で 5 フレーム、もう片方で 3 フレームだったのが揺れの正体だった。

world の VM 側（`Turn::AMeterMoved`）の勘定は小さく出る: スクリプトは作り直されず再開されるだけなので
構造の 2 フレームが無く、budget は 45,000（`install_world_answers` の 30 行のコメントが測って決めた数）
なので 1 フレーム分は 1 フレーム。**7 は 2 つのうち大きい方**で、定数が 1 つで済むならその方がよい。

### 2.3 実測 — 直す前と直した後で、同じつまみを回す

S6 のつまみ（creature / world の `frame_time` を絞る）を**一時的に**当てて、
lock を上げただけの版（`a693187`）と S7 の版で、同じ値を回した。**この仕込みは常設しない**——
出どころの無い数を selftest に持ち込むことになるからで、確かめたのはこの記録の中だけである。

| VM | `frame_time` | 直す前 | 直した後 |
|---|---:|---|---|
| creature | 300 µs | ok（S6） | ok |
| creature | **250 µs** | **FAIL** `every restarted beetle's new task has run` | **ok** |
| creature | **200 µs** | **FAIL** | **ok**（6 フレーム待って回ってきた） |
| creature | 150 µs | FAIL | FAIL（7 フレームで回らず） |
| creature | 100 µs | FAIL | FAIL |
| world | 40 µs | ok | ok（3 フレーム） |
| world | 35 µs | ok | ok（5 フレーム） |
| world | **33 µs** | **FAIL** `the meters move again` | **ok**（7 フレーム、ぎりぎり） |
| world | 30 µs | FAIL | FAIL |
| world | 20 µs | FAIL | FAIL |

境目が creature で 300〜250 µs のあいだから 200〜150 µs のあいだへ、world で 35〜33 µs のあいだから
33〜30 µs のあいだへ動いた。**「直す前は落ちて直した後は通る」値がどちらにもある**（creature 200・250、
world 33）。

**100 µs や 20 µs では直した後も落ちる**ことは隠さずに書く。これは直っていないのではなく、
つまみが「混んだ機械」を模していないからである: 100 µs は VM に本来の 1/80 の時間しか与えない設定で、
10 体の最初の一走り 8,540 命令を 7 フレーム（5.7 命令/µs で 3,990 命令）では買えない。
これを通すには上限を 15 フレームほどにする必要があり、**15 には出どころが作れない**。
そして上限に達したときの FAIL は正しい——VM は本当に配っていない。
混んだ機械の実物での確認は §3 の 8 本同時の方である。

（この表の「直した後」は §2.4 の直しが入る前の S7 の版で取った。§2.4 が触ったのは
ポインタとホイールの 2 段だけで、この表の 2 行——`every restarted beetle's new task has run` と
`the meters move again`——はそこを通らない。）

### 2.4 途中で自分で 1 つ壊した — 順序の辺 1 本の値段

8 本同時の実験を始めてすぐ、**S7 の版だけが 16 走行中 8 走行で
`the wheel over the editor scrolls the editor and not the garden` を落とした**
（lock を上げただけの版は 16 走行中 0）。S6 は 136 走行でこの行を 1 度も落としていない。
**自分で入れた回帰である。**

何が起きているか。この判定（step 13〜15）は、ポインタをエディタの真ん中に置き、0.2 秒待ち、
ホイールを 1 ノッチ回し、0.2 秒待って「egui がポインタを持っていて、カメラは動いていない」を見る。
偽造した `MouseWheel` を読むのは `orbit_camera` で、**`window_selftest` と `orbit_camera` のあいだに
順序の辺は 1 本も無い**。同じフレームで読まれれば、egui はまだポインタを知らない
（bevy_egui は `CursorMoved` を `PreUpdate` で読み、`EguiWantsInput` を書くのは egui のパス）ので
カメラは動き、次のフレームには egui が持っているので判定は
「持っているのに動いた」= FAIL になる。**どちらのフレームで読まれるかは Bevy のスケジューラ次第**で、
1 フレーム 280 ms の機械では 0.2 秒がちょうど 1 フレームなので、ここがそのまま出る。

最初に疑ったのは `catch_up_minds` をエディタの chain の末尾に足したことだった。
`Commands` を持つシステムを別のシステムの後ろに並べると Bevy が `ApplyDeferred` を差し込み、
`Update` の切れ目が動いて、どのシステムがどの区画に入るかが変わる。chain から出して
（順序の辺を持たない `add_systems(Update, catch_up_minds)` にして）測り直したら **8 走行中 3 走行**に
減ったが **0 にはならなかった**ので、これだけが原因ではない（`birth_in_the_apply_frame` が
`window_selftest` に辺を 1 本足していることと、甲虫が 1 匹増えてフレームが少し長くなることも効きうる）。
`catch_up_minds` は chain から出したままにし、**理由をシステムの登録の所にコメントで残した**——
順序の辺 1 本の値段が測れた場所だからである。

そのうえで**判定の側を直した**。0.2 秒待つのをやめ、**「egui がポインタを持った」を待つ**
（`Turn::EguiHasThePointer(true)`）。持ったあとにホイールを回すなら、`orbit_camera` が同じフレームで
読もうが次のフレームで読もうが答えは同じになる。裏の判定（パネルを閉じて同じ所で回すと
ズームする）も対称に「egui がポインタを**離した**」を待つ形にした。
これは §2.1 の直し方をそのまま 2 か所に当てたもので、上限も同じ 7 フレームである
（egui が要るのは 1 フレームなので、7 は大いに余裕がある）。

**本体の指示は「0.2 秒の待ちはこの段階では触らない」だった。** ここだけ触ったのは、触らないと
S7 が新しい揺れを持ち込むからで、触り方は S7 がやっていることそのもの（秒をやめて、待つものを言う）である。
報告に明記する。

### 2.5 ほかの秒の待ちの分類（触っていない。S5b の (d) へ）

`window.rs` の `test.at = now + …` は 17 か所。S7 が触ったのは 2 か所（0.6 → 条件）と
1 か所への条件の追加だけで、残りは 1 つも動かしていない。分類:

| 秒 | 箇所 | 何を待っているか | 分類 |
|---:|---|---|---|
| 0.2 | 6 か所（`F2` 開く / 閉じる / また開く、`F3`、ホイールを 2 回回す） | 偽造したキーが読まれ、それを読むシステムが動くこと。数フレーム | **入力のフレーム**。秒である必要は無いが、VM は関係ない |
| （秒をやめた） | 2 か所（ポインタを置く、パネルを閉じる） | **egui がポインタを取る / 離すこと**。§2.4 | **egui のフレーム**。S7 が条件に置き換えた |
| 2.0 | 1 か所（`P` で止めたあとの窓） | 止まっていなければ creature が 4 単位歩き、メーターが 1 割落ち、日が 1/30 回るだけの時間 | **世界の時間**（秒が正しい） |
| 0.5 | 1 か所（`P` で再開したあと） | 歩いたこと・日が回ったことが見えるだけの時間。**メーターの分だけ** S7 が条件を足した | **世界の時間**（秒が正しい）+ VM の順番 |
| 9.0 | 1 か所（Apply の 9 秒後） | 差し替えのあとも VM 全体が回り続けていること | **世界の時間**（長さが意味を持つ） |
| **0.6** | **2 か所**（`Ctrl+Enter` で world.rb を適用、その Revert） | **world のスクリプトが作り直されて `garden.rules(day_length:)` を言い直すこと** | **VM の順番待ち。S7 が直した 2 か所と同じ形で、まだ秒のまま** |

最後の行が残件である。`Ctrl+Enter: the garden is running the edited rules` と
`Revert puts the file's rules back` は、S7 が直した 2 つとまったく同じ形（スクリプトを差し替えて、
新しいタスクが走るのを待つ）をしていて、まだ落ちたことが無いだけである。本体の指示が
「ほかの秒の待ちはこの段階では触らない」だったので触っていないが、S5b で同じ形にするべきだと思う。
`Turn` に `DayLength(f32)` を足すだけで済む。

---

## 3. 確認

### 3.1 組み立てとテスト

`cargo build --workspace --all-targets` 警告 0、`cargo test --workspace` **34 passed**（着手前と同じ）。
新しい単体テストは足していない——足したものはどれも、単体テストより「窓を開けて Apply を押す」方が
安い（世代は `Commands` の遅延そのものが対象で、フレームの上限は VM のスケジューラが対象）。

### 3.2 PC、1 本ずつ

| 走行 | 回数 | FAIL |
|---|---:|---|
| 箱庭 ヘッドレス（`--headless 90`） | 1 | 0（13 行） |
| Battle ヘッドレス（`--headless 25`） | 1 | 0（4 行） |
| 箱庭 窓 | 3 | 0（3 走行とも 44 行） |
| Battle 窓 | 2 | 0（2 走行とも 32 行） |

S6 は静かな機械で 56 走行 FAIL 0 だったので、**1 本ずつ回数を重ねても「直った」とは言えない**。
これは「壊していない」を見るためのもので、直ったことの根拠は §3.3 の 8 本同時の方である。
（§2.4 の直しが入る前の版では箱庭の窓を 10 走行して 10 走行とも 44 行・FAIL 0 だった。
その版を捨てたのは §2.4 の理由で、判定の中身は同じである。）

行の集合は `tools/fixedlines.sh` で比べて、**箱庭の窓だけが 1 行増え、ほかは同じ**:

```
19a20
> selftest: ok   a beetle born in the very frame of Apply is restarted too
```

Battle のヘッドレスだけ 1 行が入れ替わったが、これは
`verification/selftest-lines.md` が「判定が走行ごとに動く唯一の行」と名指ししている行である
（`the handler tasks of every robot that went down ended` が `ok (N/N)` と
`--   (none was down long enough to check)` のあいだで動く。25 秒の試合で撃墜が
間に合ったかどうか）。S7 はこの道を 1 行も触っていない。

### 3.3 PC、8 本同時（負荷の実験。時間を区切った）

S6 が FAIL を出した条件（8 本同時）で、**直した版（`shared-crate` の作業ツリー）と
lock を上げただけの版（`a693187`）を 8 本ずつ交互に**回した。交互にしたのは
`implementer.md` の作法（この機械には「遅い状態」がある）で、同じ docker の target volume の中に
`garden` と `garden-before` の 2 本を置き、`md5sum` で別物だと確かめてから回している。

22:44〜23:15 の **31 分**、8 本ずつ 21 束。`vmstat` の idle は 21 回の標本のうち 19 回が **0%**、
2 回が 1%（S6 の 8 本同時は 1〜3%）。1 走行は 86 秒（S6 は 85〜91 秒）。同じ混み方である。

| 判定 | 直す前（80 走行） | 直した後（88 走行） |
|---|---:|---:|
| **`Apply restarts every beetle on the edited text`**（原因 A） | **2** | **0** |
| **`Revert puts every beetle back on the file`**（原因 A） | **2** | **0** |
| `every restarted beetle's new task has run`（原因 B） | 0 | 0 |
| `the meters move again`（原因 B、world） | 0 | 0 |
| （狙いの 4 行の計） | **4 / 80 = 5.0%** | **0 / 88 = 0%** |
| `and with the panel closed the same wheel in the same place zooms` | 4 | 4 |
| `the wheel over the editor scrolls the editor and not the garden` | 0 | 1 |

**狙いの 4 行は 4 件から 0 件になった。** S6 は 136 走行で 4 件（うち狙いの 3 行が 2 件）だったので、
ここで直す前の版が 80 走行で 4 件出したのは、S6 より当たりが濃い条件だったということである
（S6 は計器を入れた版で、ログを書くぶん 1 走行が長く、当たりの窓が相対的に狭かった可能性がある）。

**ホイールの 2 行は前後で同じ**（4 対 4、0 対 1）。§2.4 で直したのは
`the wheel over the editor …` の側で、そちらは chain に入れていたときの 8/16 から 1/88 に戻った。
残る `and with the panel closed …` は**直す前の版でも同じ 4 件出ている**ので S7 の回帰ではなく、
S6 の 4 件にも計画書の 3 行にも入っていない**5 つ目の揺れ**である。落ち方は 2 通りで、どちらも
egui の「ポインタを持っている」が揺れている:

```
waited 7 frame(s) … did not — judging it as it stands
FAIL and with the panel closed the same wheel in the same place zooms (egui holds the pointer: true; camera 42.00 -> 42.00)
```

（パネルを閉じても 7 フレームのあいだ egui がポインタを離さなかった）と

```
waited 1 frame(s) … happened
FAIL … (egui holds the pointer: true; camera 42.00 -> 38.18)
```

（離したのでホイールを回し、カメラは**ズームした**のに、0.2 秒後の判定の時点でまた持っていた）。
**S7 の範囲外**なので直していない。「気づいた点」に挙げる。

### 3.4 ブラウザ

`web/build.sh all` → `web/serve.sh`（PORT=8099）→ Playwright（`--use-angle=swiftshader
--enable-unsafe-swiftshader`、`page.goto` は `waitUntil:'commit'`）で 110 秒ずつ。
S6 は箱庭を 16 走行して 1 走行 FAIL（原因 A の実物）だったので、同じくらい回した。

| ページ | 走行 | pageerror | requestfailed | FAIL |
|---|---:|---:|---:|---|
| `garden/?selftest` | **14** | 0 | 0 | **0**（14 走行とも ok 44） |
| `sabibots/?selftest` | 2 | 0 | 0 | 0（2 走行とも ok 68 = 固定 33 行 + 当たりの行） |

**S6 でブラウザに出た原因 A（14 走行中 1 走行）は出なかった。** 出ようが無い——
`catch_up_minds` が拾うからで、しかも**常設の判定が毎走行その競合を作っている**（`a beetle born in
the very frame of Apply is restarted too` が 14 走行とも ok）。ブラウザは 1 フレーム 250 ms で
S6 が「混んだ PC と同じ姿」と書いた環境なので、原因 B の側も 14 走行で 1 度も落ちていない。

行の集合は `tools/fixedlines.sh` に通して `verification/selftest-lines.md` の
（S7 で 1 行足した）一覧と **`diff` が空**: 箱庭のページ 45 行、Battle のページ 33 行。

§0.2 の `2 s paused: …` は**この 16 走行では 1 度も出ていない**（lock を上げただけの 3 走行のうち
1 走行で出たもの）。出なかったことは「無い」ことの証拠にはならないので、気づいた点に残す。

---

## 4. 直さなかったもの・捨てた案

* **案 A2（`children_arrive` を `do_editor_actions` の前に置く）**: 1 行で済むが、
  `do_editor_actions` は窓のときしか登録されないので、順序の辺を窓の側に書くことになる
  （「ヘッドレスと窓でシステムの順序が違う」が 1 つ増える）。それに直るのは**この 1 フレームだけ**で、
  「差し替えのあとに来たものが古いまま」という形そのものは残る。採らなかった。
* **案 A3（判定が生まれたてを数えない）**: 本体が最初から却下している。バグが残る。
* **案 B3（0.6 → 1.2 秒）**: その数の出どころが作れない。`book/CLAUDE.md` に正面からぶつかる。
* **上限を「VM が使った命令数」で言う案**（「1 フレーム分の budget を配ってなお届かなければ諦める」）:
  負荷に強く、機械の混み具合に依らないという点では上限をフレームで言うより良い。捨てた理由は
  **VM が本当に止まったときに永久に待つ**こと——命令が 1 つも進まないので上限そのものが来ない。
  フレームなら必ず来る。結局この考え方は式の分子（`budget`）としてフレーム数の導出に残っている。
* **`the meters move again` の条件を `WorldMeter::passes` で言う案**: 最初こう書いて、
  world の `frame_time` を 20 µs にした走行で「2 フレームで回ってきた」と言いながら判定は FAIL した。
  `passes` は名前に反して「**スクリプトが 1 命令でも走ったフレーム**」を数えており、
  `each_frame` が `Hunger` を書く行まで到達したことは言っていない（`world_clock_end` の
  `pass = ran.saturating_sub(last)` が `> 0` なら push）。判定と同じ述語
  （誰かのメーターが動いたか）に直した。**`WorldMeter::passes` の doc コメントは
  「one entry per frame in which the script ran at all」と正しいことを書いており、
  誤解したのは読んだ側である**が、`passes` という名前は `each_frame` の pass と紛らわしい。

---

## 気づいた点

1. **（バグ候補）ブラウザで `2 s paused: …` の 2 行が落ちた。** lock を上げただけの状態、3 走行中 1 走行
   （§0.2）。同じ走行で `nothing ran while it was paused` は ok なので、**VM は止まっていたのに
   コンポーネントへの書き込みだけが止まりの中で着地している**。rubevy の R4 が作る形
   （`frame_time` が尽きたフレームで答え切れなかった質問を次のフレームに回す。書き込みも
   `Request` として答えの側を通る）と読めるが、**確かめていない**。ブラウザは 1 フレーム 250 ms で
   `frame_time` 8 ms を毎フレーム使い切るので PC より桁違いに起こりやすい。
   再現手順: `web/build.sh garden` → `?selftest` を何度か。
   → rubevy（R4 の副作用）／箱庭の判定（`P` が「VM を止める」だけで「書き込みを止める」ではないこと）。
2. **`docker/run.sh` は `<GAME>_SELFTEST` 以外の環境変数を渡さない。** 計測のつまみも
   `GARDEN_RELOAD_AT` も、コンテナの中の走行には届かない（`docker/run.sh` の `PASS=(-e …)` は 1 つだけ）。
   `-e` を並べるだけの手書きの `docker run` を毎回スクラッチパッドに書くことになる。
   → 道具。`PASS` に「`<GAME>_` で始まる環境変数を全部渡す」を足せば済む。
3. **`window.rs` の 0.6 秒の待ちが 2 か所残っている**（world.rb の `Ctrl+Enter` と Revert。§2.4）。
   S7 が直した 2 つと同じ形で、まだ落ちたことが無いだけである。`Turn` に `DayLength(f32)` を
   足せば同じ直し方が当たる。 → 判定の作り／S5b。
4. **`WorldMeter::passes` の名前が `each_frame` の pass と紛らわしい**（§4 の最後）。
   数えているのは「スクリプトが 1 命令でも走ったフレーム」で、`each_frame` が 1 周したことではない。
   HUD の「N passes of `each_frame`」もこの数を出している。 → 箱庭（名前と表示）。
5. **共有した docker の target は、worktree を切り替えても作り直されない。**
   `docker/build.sh` はどの worktree も `/app` に mount するので、cargo から見ると同じパスであり、
   別のブランチの同じファイルを「変わっていない」と読む。前後を比べる走行で 1 度これに刺さり、
   `Finished in 1.44s` と「直したはずの判定が無い」ログで気づいた。`touch garden/src/*.rs` で直る。
   `implementer.md` が警告しているとおりの形が、docker の volume でも起きる。
   → 作法／本（測り方）の素材。
6. **（5 つ目の揺れ）`and with the panel closed the same wheel in the same place zooms` が
   8 本同時で 88 走行中 4 件落ちる。直す前の版でも 80 走行中 4 件**なので S7 の回帰ではない（§3.3）。
   原因は egui の「ポインタを持っている」が揺れること: パネルを閉じても離さないまま 7 フレーム経つか、
   離してホイールを回してカメラがズームしたあとに**また持つ**。S6 の 4 件にも計画書の 3 行にも
   入っていない。直すなら判定の `held` を「ホイールを回した瞬間の値」にすれば 2 つ目の形は消える。
   → ゲーム固有（箱庭の判定）／S5b の (d) と同じ棚。
7. **`Mind` の 3 つの数（`last_instructions` / `spent` / `frames`）と `heat` / `own_line` / `at` を
   0 に戻すのは「新しいスクリプトを着せる」という 1 つの動作の一部**で、S7 でそれを
   `wear_mind` という関数に出した。出すまで、差し替えは `restart_species` の中にしか無く、
   同じことを別の場所からやりたくなったときに 10 行を写すしかなかった。
   sabibots の `restart` は 1 行（`replace_script`）で、そこに同じものが無いのは
   Battle が `Robot` に同じ数を持っていないからである。 → 本（ゲームの状態とスクリプトの寿命）の素材。
