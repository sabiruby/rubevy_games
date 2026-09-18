# 2026-09-18 箱庭の残件 5 件 — ホイール、読み込んだ空、行番号、世界の VM、8 番の揺れ

著者から渡された 5 件を、この順に 1 件 1 コミットで片づけた記録。
作業場所は worktree `rubevy_games-wt-leftovers`（ブランチ `leftovers`、main `0d569d7` から）。
`main` には触っていない。push もしていない。rubevy は `e7b3eb5` のまま（別の担当が
`rubevy-wt-guard` で作業中なので、こちらからは 1 行も触っていない）。

コミットは 5 本 + この文書:

| コミット | 何を |
|---|---|
| `8e985ed` | 1. パネルの上で回したホイールはパネルのもの |
| `51b3bbb` | 2. セーブから読み込んだ生き物にも今の空を伝える |
| `7ac89a7` | 3. コンパイルエラーの行番号から prelude 分を引く |
| `75ce171` | 4. VM パネルが 2 本目の VM も映す（`F3` の間は `F2` が世界） |
| `09f177f` | 5. 誰もつがいにならなかった走行は 8 番を測っていない |

**閾値は 1 つも動かしていない。** `world.rb` は 1 文字も触っていない。
`unsafe` は 0 のまま（`grep -rn unsafe garden/src/ sabibots/src/ crates/` が 0 行）。
新しく置いた数は 1 つもない——`rubevy_arena::editor::{MARGIN, WIDTH, HEIGHT}` は
`draw_editor` に既にあったリテラルに名前を付けただけで、値は変えていない。

---

## 0. 着手前の状態

```
$ cargo build --release -p garden -p sabibots
    Finished `release` profile [optimized] target(s) in 3m 36s
$ cargo clippy -p garden -p sabibots -p rubevy-arena
warning: `rubevy-arena` (lib) generated 2 warnings
warning: `sabibots` (bin "sabibots") generated 12 warnings
warning: `garden` (bin "garden") generated 13 warnings
```

内訳（`too_many_arguments` 12、`collapsible_if` 7、`wrong_self_convention` 2、
`type_complexity` 2、`manual_range_contains` 2、`manual_checked_ops` 1、
`explicit_counter_loop` 1）を基準にして、5 本のあいだ何度も測り直した（§7.1）。

---

## 1. ホイール — パネルの上で回したらパネルのもの（`8e985ed`）

### 1.1 穴は 1 つのループにあった

`orbit_camera`（`garden/src/main.rs`）は G4 の時点で egui に聞いていた:

```rust
let (mine, mine_keys) = match pointer {
    Some(p) => (!p.wants_pointer_input(), !p.wants_keyboard_input()),
    None => (true, true),
};
…
let turning = mine && buttons.pressed(MouseButton::Left) && !shift;
```

`mine` は**ドラッグにしか使われていない**。`for w in wheel.read()` のループはこの手前でも後ろでも
`mine` を見ておらず、ポインタがどこにあろうとホイールを読んでいた。egui も同じ
`MouseWheel` を自分の `ScrollArea` のために読むので、エディタの上で回すと**両方が同時に起きる**。
著者が見たのはこれ。

### 1.2 聞き方を広げた理由と、その代償

聞く内容を `wants_pointer_input() || is_pointer_over_area()` に広げた。
egui 自身の `wants_pointer_input` は

```
is_using_pointer() || (is_pointer_over_area() && ボタンが 1 つも押されていない)
```

なので、**ボタンを押したままパネルの上にポインタがある状態は egui の「欲しい」に入らない**。
そこで回したホイールはカメラに戻ってきてしまう。「エリアの上か」はエリアの上かであって、
ボタンの状態とは関係が無い、というのがこの行の言い分。

ただし広げるとドラッグが割を食う: 草の上で始めた回転がエディタの上を横切った瞬間に止まる。
今までそうならなかったのは、上の `any_down` のおかげで**たまたま**だった。
そこで、**ドラッグはボタンが下りた瞬間に決める**ことにした（`grabbed: Local<bool>`）:

```rust
let dragging = [MouseButton::Left, MouseButton::Right];
if !buttons.any_pressed(dragging) {
    *grabbed = false;
} else if buttons.any_just_pressed(dragging) {
    *grabbed = !egui_pointer;
}
let mine = *grabbed;
```

庭で始めたドラッグはポインタがどこへ行こうとカメラのもの、パネルで始めたドラッグは
どこまで引きずってもカメラのものにならない。今までの偶然を意図に書き直した形。

ホイールのループは `mine` ではなく `egui_pointer` で弾く。`wheel.read()` 自体は**必ず回す**
（`continue` で捨てる）——読まずに戻ると、パネルの上で回したぶんがリーダに残って、
ポインタが外れたフレームにまとめてカメラへ届く。

### 1.3 Battle 側

`grep -rn "MouseWheel\|MouseMotion\|MouseButton" sabibots/src/ crates/rubevy-arena/src/` が
**0 行**。Battle はマウスを 1 つも読んでいない（カメラは固定で、アリーナの方が窓に合わせて
縮む `ArenaPlugin`）ので、同じ形の穴は無い。両ゲームのカメラが `rubevy-arena` にあるのでは、
という前提は実物では成り立たなかった——箱庭の 3D カメラは箱庭のもの。

### 1.4 窓のセルフテスト 2 行

「エディタの上でホイール → カメラの距離が変わらない」を書くには、**ポインタを好きな所へ置く**
手立てが要る。egui に「ポインタがここにあることにしてくれ」と言う口は無く、本物のカーソルを
ワープさせるのはデスクトップ次第なので、**winit が書くはずの `WindowEvent::CursorMoved` を
自分で書いた**。bevy_egui はこのメッセージを `PreUpdate` で読み、egui にポインタの位置を
伝えるのはこれだけ（`bevy_egui::input::write_pointer_moved_and_button_messages_system`）。
既存のチェックが `keys.press(KeyCode::F2)` で鍵を偽造しているのと同じ種類の偽造で、
他に道は無い。

`window_selftest` は既に 14 個のシステムパラメータを持っていて Bevy の上限は 16 なので、
必要な 5 つ（窓、`Orbit`、`MouseWheel` の writer、`WindowEvent` の writer、`EguiWantsInput`）を
`FakePointer` という `SystemParam` にまとめた（`VmReport` と同じ手）。

置き場所は**いちばん最後**。ポインタを動かすので、上の 40 行はポインタが player の置いた所に
あるままの方がよい。段取りは、エディタの既定の矩形の真ん中へポインタを置く →
1 ノッチ回す → 距離が変わっていないこと → **エディタを閉じて**同じ所で同じノッチを回す →
距離が変わること。2 行目は対照実験で、これが無いと「カメラが動かなかった」は何も言っていない
（パネルがそこに無くても通ってしまう）。行には egui がポインタを持っていたかも印字する。

矩形は `rubevy_arena::editor` の `MARGIN` / `WIDTH` / `HEIGHT` に名前を付けて共有した。
`draw_editor` が `.default_width(520.0).default_pos([right - 8.0 - 520.0, 8.0])` と
書いていたリテラルそのもので、**値は変えていない**。チェックが矩形を推測すると
推測をテストすることになるので、同じ数を両方から読む。

### 1.5 実測（窓あり、WSLg + lavapipe、`docker/`）

```
selftest: ok   the wheel over the editor scrolls the editor and not the garden
               (egui holds the pointer: true; camera 42.00 -> 42.00)
selftest: ok   and with the panel closed the same wheel in the same place zooms
               (egui holds the pointer: false; camera 42.00 -> 38.18)
```

42.00 / 1.10 = 38.18 で、1 ノッチちょうど。ブラウザでも同じ 2 行・同じ数字が出る（§7.4）。

---

## 2. `--load` / F9 で読み込まれた生き物も空を聞く（`51b3bbb`）

### 2.1 同じ穴の反対側

前日の記録（`docs/worklog/2026-09-18-selftest-fixes.md` §5）が「範囲外だが事実として」と
書いていたもの。`load_world` は体を作って `give_mind` を呼ぶ——`children_arrive` と同じ道で、
そこに付く script は同じだけ耳が無い。`"night"` と `"day"` は空が返る瞬間に 1 通ずつ出るだけ
なので、**夜のセーブを開くと朝まで全員起きている**。

直しは `Newborns` に積むだけ。`give_mind` 側ではなく読み込みの道だけ、という指示のとおり。
F9 で庭ごと入れ替えるときは、待っていた分を `clear()` する（その entity はもう居ない。
`tell_newborns_the_sky` は `Mind` が無い相手を落とすので放っておいても消えるが、
捨てたものを数えないほうが読める）。

### 2.2 「夜なら積む」か「常に積む」か

実物を見て**常に積む**にした。理由は 2 つ:

* 「`sky.night` のときだけ」は、`day_night` が持っている「空が言えることの一覧」の**2 つ目の写し**
  になる。手で同期を保つものが 1 つ増える。
* 節約できるのは昼のセーブでの 1 匹 1 通だけで、その 1 通は**既に起きている生き物への
  `"day"`**——`on(:day)` が `@asleep = false` を書くだけで、元から `nil` なので何も起きない。
  「重複を許すほうが旗より安い」は `tell_newborns_the_sky` が空の反転フレームについて
  既にしている取引と同じ。

昼のセーブを読み込んで全員が歩き続けることは実測で確かめた（§2.3 の最後）。

### 2.3 実測

35.0 秒（夜、phase 0.66）のセーブを作り、読み込んで、**使い捨ての probe**で全個体の速さを
0.5 秒ごとに印字した。

```
$ ./target/release/garden --headless 35 --save night.json
night at 25.2 s / saved 11 creatures, 46 plants at 35.0 s
```

**直す前**（`newborns.waiting.push` をコメントアウトしたバイナリ）:

```
fixprobe speed at 0.51 (world 35.52): 116v0:2.00 117v0:2.00 … 125v0:3.00 126v0:1.92
fixprobe speed at 3.59 (world 38.60): 116v0:2.00 117v0:2.00 … 125v0:2.00 126v0:2.00
```

11 匹全部が `CRUISE`（2.00）のまま。3.6 秒たっても誰も寝ない。

**直した後**:

```
fixprobe speed at 0.51 (world 35.51): 9 匹が 0.00、123v0:2.19 124v0:2.00
fixprobe speed at 1.03 (world 36.03): 11 匹すべて 0.00
（以降ずっと 0.00）
```

0.5 秒の時点で動いている 2 匹は、言葉が届いたときに**逃走ハンドラが舵を握っていた**個体
（2.19 は `DASH`）。ハンドラが `drop_wheel` すれば止まる。**1 秒後には全員 0.00**で、
これが依頼の「1 秒後に全員が寝ていること」。

位置からも同じことが言える。1.01 秒ぶん走らせて前後のセーブを genome で突き合わせた:

| | 動いた距離（11 匹、昇順） |
|---|---|
| 直す前 | 1.03 1.13 1.15 1.19 1.46 1.50 1.67 1.77 1.79 1.88 2.79 |
| 直した後 | 0.00 ×9、1.01、1.24 |

**F9 の道**も同じ（`GARDEN_SELFTEST=1 GARDEN_RELOAD_AT=5`、昼の庭に夜のセーブを入れる）:
読み込みの 0.5 秒後に 9 匹、1.0 秒後に 11 匹全部が 0.00。

**昼のセーブ**（15.0 秒、`night=false`）を読み込むと 10 匹全部が 2.00 のまま歩き続ける——
`"day"` の publish が何もしない、という §2.2 の読みどおり。

probe は全部外した（`grep -c fixprobe garden/src/main.rs` が 0）。
セーブの往復（`--headless 30 --save a` → `--load a --headless 0 --save b` → `diff`）は
`IDENTICAL` のまま。

---

## 3. コンパイルエラーの行番号（`7ac89a7`）

### 3.1 ずれの正体

`compile_source` も `compile_world_source` も、prelude と本文を**1 本のプログラム**にして
コンパイラに渡す。だからコンパイラの言う行番号はそのプログラムの行であって、
パネルに映っているファイルの行ではない。実測:

```
$ (beetle.rb の 118 行目を `if hunger < < hungry_below` に壊して)
ERROR garden: beetle.rb: beetle.rb:600:19: syntax error, unexpected '<'; …
```

118 行目が 600 行目として出る（prelude が 480 行 + 見出し 2 行 = 482）。
VM パネルは `prelude_lines` でずっと引いていた（`VmInspector::fill`）。引いていなかったのは
**エディタの status とログ**で、打ち間違えた人が見るのはその 2 つ。

### 3.2 なぜ文字列を直すのか

ブラウザには聞く相手がいない。`window.gardenCompile(source)` はソースしか取らず
（G5 の 3 つ目の発見）、投げてくるのはページ側のコンパイラの文言そのまま。
ネイティブ側の `sabiruby_compiler::Diagnostic` も `FILE:LINE:COL: message` を 1 行 1 件で書く
（`mrbc` と同じ形）ので、**両方に共通するのは文字列の形だけ**。
だから探すものは「行の中で最初に現れる `:<数字>:<数字>:`」ひとつで、それが場所を表す唯一の候補。

prelude の長さ以下の行は**prelude 側のエラー**で、負の数にするわけにも
「author のファイルの N 行目」と言うわけにもいかないので、ファイル名ごと差し替える:

```
beetle.rb: prelude.rb:47:3: syntax error
```

`compile` / `compile_source` / `compile_world` / `compile_world_source` は
`Option` をやめて `Result<_, String>` を返すようにした（メッセージは補正済み）。
`compile_world_source` は prelude の行数も返すようになった——生き物側は最初から返していて、
「それはエディタの帯と HUD の列のためで、世界には要らない」と W3 が書いていた読みが、
**コンパイラのエラーという 3 人目の読者**で崩れたところ。

### 3.3 実測

本物のファイルを壊して、走行が印字したものをそのまま:

```
beetle.rb   118 行目を壊す  ->  beetle.rb: beetle.rb:118:19: syntax error, unexpected '<'; …
world.rb    128 行目を壊す  ->  world.rb: world.rb:128:14: syntax error, unexpected '<'; …
prelude.rb   43 行目を壊す  ->  beetle.rb: prelude.rb:45:5: syntax error, unexpected 'end'; …
```

3 つとも一致（prelude の 45 は、コンパイラが 43 行目の `def … (` を 45 行目の `end` で
気づいたという意味で、これはコンパイラの言い分そのまま。ファイル名が `beetle.rb` ではなく
`prelude.rb` になっているのが直した点）。

**単体テスト 1 本**（`a_compiler_line_is_reported_in_the_authors_own_file`）に、上の本物の
メッセージ、境界（prelude の最終行と、その次の行 = author の 1 行目）、世界側の prelude、
1 行 2 件、そして場所を持たない 2 種類のメッセージ（`compile error` と
`platform::read` の「そんなファイルは無い」——パスにコロンが入っていて行番号が無い）を入れた。
`cargo test -p garden --release` は 6 passed / 0 failed。

**エディタの status 自体は画面で読んでいない。** status に入る文字列は `error!` がログに出す
`why` の 1 行目そのもの（`first_trouble`）なので、上の 3 行がそのまま出る——という筋は
通っているが、「壊したファイルで Apply を押した画面」は撮っていない。未実施として §8 に。

### 3.4 分かったが直していないこと

* コンパイラがファイルの**末尾より後ろ**に置くエラー（閉じていない `do` など、
  end-of-input で報告されるもの）は、ゲームが末尾に足す `run_creature` / `run_world` の行に
  落ちる。`world.rb` を `if if` で壊したら `world.rb:313`（本文は 312 行）と出た。
  プログラムは本当にそこまで続いているので、嘘ではない。丸めていない。
* ブラウザでは**内側のファイル名**がページ側コンパイラの既定（`playground.rb`）になる。
  外側（ゲームが書く `beetle.rb: ` / `world.rb: `）は正しい。行番号の補正は効く。

---

## 4. 世界の VM パネル（`75ce171`）

### 4.1 総称にしたのは構造体ではなく**メソッド**

`docs/worklog/2026-09-17-garden-world.md` §24.1 が「2 本目を映すにはパネル側に VM を渡す口が
要る」と書いていたもの。原因は `VmInspector::fill(&mut self, world: &ScriptWorld, …)` が
`ScriptWorld` を既定の名札で名指ししていること。

依頼は「`VmInspector` を名札 `M` で総称にして（`RubevySet` と同じ既定型引数 `M = ()`）」だった。
**実物を見て、名札をメソッドに付けた。** 理由を書いておく:

`VmInspector<M = ()>` を Resource にすると、VM ごとに `VmInspector<M>` が 1 つ、
`VmInspectorPlugin<M>` が 1 つ、そして `egui::Window::new("VM")` が 2 つになる——
egui は窓のタイトルを id にするので、同じ id の窓が 2 つ立つ。
そのうえパネルは**構造上 1 度に 1 つしか映さない**: `F2` が開くのは「その」パネルで、
何を映しているかはゲームが毎フレーム答える質問でしかない。
`fill` が読むもの（snapshot、タスクのフレーム、ヒープ）はどれも名札と関係が無い。
だから名札はメソッドに付くのが正しく、そうすると Battle の呼び出しは
**1 文字も変わらない**（依頼の「Battle は無変更で通ること」も満たす）。

計画と違う形にしたので、報告に上げて著者の判断を仰ぐ。戻すのは難しくない。

### 4.2 箱庭側

`Watched::world`——`F3` がエディタのために既に立てている旗——で、どちらの VM から
`fill` するかを決める。2 つのパネルが同じファイルの上に揃い、`Tab` か creature のクリックで
両方が戻る。

パネルが creature について言うことは、全部そのまま rules についても言える。
どれも creature についての話ではなかったから:

* **なぜ待っているか**は `rubevy_arena::inspect::why` がフレームだけから出す。
  `each_frame` の 1 パスは `Rubevy.ask("frame").pop` で終わる。
* **どこで待っているか**は `world.rb` の最も内側のフレームから世界の prelude 分を引いたもの。
  そのための `WorldPrelude`（生き物が `Mind` に持っているものの世界側）。
  Resource にしたのは rules が 1 組だからで、rules を着せるたびに書く——
  `world_prelude.rb` を保存して行数が変わったら行番号も一緒に動く。
* **2 つの数**は `WorldMeter` の最後のパスと中央のパス。

`VmInspector::prelude_file` も足した。prelude 内のフレームは `prelude.rb:315` と印字されていて、
世界の prelude は `world_prelude.rb` なので嘘になる。`None` が今までの名前なので
Battle と箱庭の creature は言うことが変わらない。

### 4.3 実測

窓（WSLg + lavapipe）で `--shot` を撮った。セルフテストが `F3` を押している 18.2 秒の 1 枚:

```
the rules  world.rb                      the world is running — P to stop it
waiting for the game to answer `run_world`
on world.rb:313
world.rb:313        (block or top level)
12087 insn/frame · 4 tasks in the VM · VM 0.40 / 8.0 ms
```

headless でも同じパネルを creature の後に印字するようにした（`stop_when_over`）。
そちらの詳細行:

```
vm:   #1 world_prelude.rb:315   run_world   pc 17    n=nil
```

`world_prelude.rb:315` は `n = Rubevy.ask("frame").pop` の行そのもの。

**言葉について 2 つ。** 依頼は「`each_frame` は `Rubevy.ask("frame")` で park =
`Waiting::Ask("frame")`」と書いていたが、実際に出るのは `run_world` の方。
`Rubevy.ask` は `Rubevy::Proxy` ではなく**モジュール関数**なので、`pop` を呼んだフレームは
`run_world` で、`why()` の表の 4 行目（「`pop` を呼んだそれ以外の何か」＝呼んだメソッド名で
名乗る）に落ちる。表が既に文書化しているフォールスルーで、指す行（`world_prelude.rb:315`）は
正しい。`Waiting::Ask("frame")` にするには `Rubevy.ask` の引数を読む必要があり、
それは `rubevy-arena` を Battle ごと変える話なので手を付けていない。

もう 1 つ、`world.rb:313` は本文（312 行）の 1 行後ろ——ゲームが足す `run_world` の行で、
**パスとパスのあいだで park しているタスクが持つ唯一の「自分のファイルの」フレーム**。
パスの最中なら `world.rb` の実在の行になる。

そして末尾の `VM 0.40 / 8.0 ms` は**生き物側**の tick のまま。`VmClock` は 1 本目の VM を
測る道具で（`rubevy-arena`、W1 が名前を付けた `VmClockSet` の話）、世界の tick の ms は
HUD の `the rules …· 1.05 ms` の側にある。パネルの最終行だけは 2 本目に付いてこない。

---

## 5. 8 番の揺れ（`09f177f`）

### 5.1 まず 40 回回した — 落ちなかった

`GARDEN_SELFTEST=1 --headless 90` を 8 本ずつ 5 巡、計 40 回。

* **8 番の FAIL は 0 回。** 走行ごとの pairings は最小 2、中央 7、最大 17。
* 6 番が 1 回落ちた（`19/20`。調査 §5 の 1/98 の flake で、今回の 5 件とは別件）。
* 5 番の `n/a` が 2 回（案 D が狙ったとおりの回）。

ただし「最初の子が生まれた時刻」の分布が尾を引いていた:

```
n=40  min 1.91  median 2.15  max 61.39
（2 秒台が 32 回、9.88 / 13.12 / 14.72 / 15.20 / 19.30 / 56.69 / 56.92 / 61.39 が各 1 回）
```

**8 回に 1 回は仕込みの隅が子を作れていない**（そのあと野良のつがいを待っている）。
調査 §5.4 の「90 秒でつがい 0 組」はこの尾の先端。

### 5.2 隅を細かく印字した — 確率の源が見えた

使い捨ての `Lover` コンポーネントと probe を入れて、2 匹の meter・位置・
（自分たち以外の）最も近い生き物・2 匹の距離を 0.5 秒ごとに印字し、20 秒 ×64 回。

**64 回のうち 62 回で隅はつがいを作り、2 回は作らなかった。** 落ちた 2 回の形は同じ:

```
run41  2.03 s  123v0 h=76.0 (-11.77, 9.01) | 122v0 h=74.7 (-15.89, 8.75)  apart=4.13
       2.54 s  123v0 h=100.0                | 122v0 h=73.9                 apart=4.50
       （以降 離れる一方。mate の行は 20 秒で 0 件）
run59  2.04 s  121v0 h=99.3 (-12.46, 7.87) | 120v0 h=83.7 (-14.63, 9.63)  apart=2.79
       2.55 s  両方 100.0                                                  apart=3.04
```

**原因は確率ではなく幾何。** `world.rb` の数を並べると:

```
reach       = 1.1  # + 草の大きさの半分。plant_max = 1.4 なので最大 1.8
mate_reach  = 2.0
mate_hunger = 75.0
hungry_below = 55.0  （beetle.rb）
```

生き物は草に**乗る**のではなく `reach` 離れたところから食べる。そして script は
meter が `hungry_below` を超えた瞬間に「草へ歩く」のをやめて `wander` に移る。
だから 4.5 単位ずつ両側から来た 2 匹は、**それぞれ自分の側に 1.8 残して止まり、
最大 3.6 離れたまま食べる**。`mate_reach` は 2.0。

つまり隅は「2 匹が触れるほど近くに立つ」ようには**できていない**。
実際に近づくのは、満腹になったあとの `wander` がたまたま 2 匹を寄せたときで、
それが 64 回中 62 回。ソースのコメント（"are told about each other because they are full and
touching"）はこの点で事実と違っていたので、`docs/garden.md` の側に書き直した。

### 5.3 直し方: 仕込みではなく判定

**仕込み側は採らなかった。** 確実にするには `reach` と `mate_reach` が決める場所に 2 匹を
置くことになるが、その 2 つの数は `ruby/world.rb` にあり、**player が編集してよいファイル**
（`F3` で開く）で、Rust からは読めない。Rust 側でその写しから幾何を組むと、
ファイルが書き換わった瞬間に間違いになる。今回の制約（`world.rb` は触らない、
根拠のない数を置かない）とも正面からぶつかる。

**判定側を直した。** `Genome#mix` は creature の `on(:mate)` からしか呼ばれないので、
`"mate"` が 1 度も出なかった走行は**この判定の質問を 1 度もしていない**。
5 番が「ウサギに邪魔された走行」に対して持っている 3 つ目の言い方をそのまま使う:

```rust
None if test.courtings == 0 => unmeasured(…),   // 測っていない
None => ok(false, …),                            // つがいはできたのに子が来ない = FAIL
```

**つがいができて子が来なかった走行は今までどおり FAIL。** それは `on(:mate)` から
`garden.spawn` までの道の話で、まさにこの判定が見るべきもの。
n/a に倒すのは「鳴っていないベルを鳴ったと言わない」だけで、道が壊れたら黙る、ではない。

閾値は動かしていない。新しい数も置いていない。

### 5.4 両方の枝を実際に見た

素の 90 秒走行では 40 回に 1 回も出ないので、**走行を短く切って**両方を出した。

```
$ GARDEN_SELFTEST=1 ./target/release/garden --headless 1.2
selftest: n/a  a child was born whose genome is its parents' mixed and mutated
         (not measured: the rules paired nobody in the whole run, so nothing asked `Genome#mix` anything)
```

FAIL の枝は「つがいができてから子が届くまでの 1〜2 フレーム」でしか出ないので、
1.81〜2.28 秒を 0.01 秒刻みで 48 回回して 1 回引き当てた:

```
selftest: FAIL a child was born whose genome is its parents' mixed and mutated (none was, from 1 pairings)
```

（48 回の内訳は n/a 26、ok 21、FAIL 1。どれも人工的に短い走行の話で、
90 秒走行の分布ではない。）

probe と `Lover` は全部外した。

---

## 6. 捨てた案・迷った点

**1 番、ドラッグのラッチを入れるかどうか。** `wants_pointer_input` だけで弾いていた頃の
「パネルを横切ってもドラッグが続く」は、egui の定義に含まれる `any_down` による**偶然**だった。
`is_pointer_over_area` を足すとその偶然が消えるので、ラッチ（`grabbed`）を 4 行入れて
同じ振る舞いを意図として書いた。入れないほうが行数は少ないが、著者が実際に使う操作が
黙って悪くなるのは直しとは言えない。

**2 番、「夜なら積む」を採らなかった。** §2.2。

**3 番、status を「1 行目だけ」にした。** コンパイラは複数の診断を 1 行ずつ返しうる。
egui の 1 行ラベルに全部入れると折り返して panel が伸びるので、status は 1 行目、
ログには全部。`first_trouble` の 3 行がその全部。

**3 番、`{name}: ` の二重表示を削らなかった。** ネイティブのメッセージは
`beetle.rb: beetle.rb:118:19: …` とファイル名が 2 回出る。外側は `platform::compile` が
付けるもので、**ブラウザではそこだけが creature のファイル名**（内側は `playground.rb`）。
消すとブラウザで誰のエラーか分からなくなるので残した。

**4 番、`VmInspector` を構造体ごと総称にする案。** §4.1。著者の指示と違う形にしたので
判断を仰ぐ。

**4 番、パネル最終行の `VM x / 8.0 ms` を世界側に差し替える案。** `VmClock` は 1 本目の VM を
測る道具（`rubevy-arena`、W1 §「2 本目の tick を窓の中に入れない」）で、世界の ms は
`WorldMeter` の側にある。差し替えるには `VmClock` も総称にする必要があり、そこまでは
依頼に無いのでやっていない。事実として記録する。

**5 番、隅を作り直す案。** §5.3。`world.rb` の 2 つの数に依存するので採らなかった。
もし著者が採るなら「2 匹を同じ側から来させる」のがいちばん小さい変更で、
必要なのは「`reach` で止まった 2 匹の角度差が `2·asin(mate_reach / 2·reach)` 未満」
という 1 つの不等式。数が `world.rb` にある以上、Rust から書くなら
ゲーム起動時に `garden.rules(…)` でその 2 つも渡してもらう形になる（`day_length` と同じ道）。
**これは設計の話なので手を付けていない。**

**6 番の揺れ（`19/20`）は今回の範囲外。** 40 回に 1 回出た。調査の 1/98 と矛盾しない。

---

## 7. 確認

### 7.1 ビルドと clippy

5 本それぞれの後で測った。**着手前と同じ 13 + 12 + 2。**

```
$ cargo build --release -p garden -p sabibots
    Finished `release` profile [optimized] target(s)
$ cargo clippy -p garden -p sabibots -p rubevy-arena
warning: `rubevy-arena` (lib) generated 2 warnings
warning: `sabibots` (bin "sabibots") generated 12 warnings
warning: `garden` (bin "garden") generated 13 warnings
```

途中 2 回増えた。1 回目は `FakePointer` の `.map(|(e, w)| (e, w))`（`map_identity` ×2）で、
`the_window()` を切り出して消した。2 回目は `show_vm` が引数 9 個になった
`too_many_arguments` で、同じファイルの 5 つの隣人と同じく `#[allow]` を付けた。

`cargo test -p garden --release`: **6 passed / 0 failed**（既存 5 + 3 番の 1 本）。

### 7.2 headless（garden 13 判定 ×5、各コミットの後）

5 本すべての後で 90 秒 ×5。**どの回も 13 判定すべて ok、FAIL 0、n/a 0**（65 行の `ok`）。

セーブの往復（`--headless 30 --save a` → `--load a --headless 0 --save b` → `diff`）は
**IDENTICAL**。

### 7.3 sabibots

各コミットの後 ×1（`--headless 30`）: 4 判定すべて ok。

```
selftest: ok   the handler tasks of every robot that went down ended (1/1)
selftest: ok   24 hits on a robot with a handler were checked
selftest: ok   a handler ran within 0.3 s of the hit (24/24)
selftest: ok   the heading changed within 0.3 s of the hit (24/24)
```

### 7.4 窓あり（WSLg + lavapipe、`docker/`）

`docs/wsl-gpu.md` の道。volume は他の担当と分けるために
`rubevy-games-target-leftovers` を新しく作った（registry の `rubevy-games-cargo` は共用）。

```
docker run … -e GARDEN_SELFTEST=1 … /target/release/garden
```

**42 行すべて ok、FAIL 0。** 40 行は W3 までのもの、増えた 2 行が §1.4 のホイール。
`--shot` で世界の VM パネルの絵も撮った（§4.3）。

### 7.5 ブラウザ（`web/build.sh garden` + Playwright）

```
$ SABIRUBY_PLAYGROUND=…/sabiruby-playground web/build.sh garden
garden/game_bg.wasm: 39679600 bytes (10179503 gzipped)     # wasm-opt はこの機械に無い
$ (cd web/dist && python3 -m http.server 8123)
$ node drive.js "http://localhost:8123/garden/?selftest" 170 shot.png
```

* **pageerror 0、console.error 0。**
* **`selftest: ok` が 42 行、`FAIL` 0、`n/a` 0。**
* ホイールの 2 行はブラウザでも同じ数字:
  `(egui holds the pointer: true; camera 42.00 -> 42.00)` /
  `(… false; camera 42.00 -> 38.18)`。
* canvas は `1280×800`（`300×150` のままなら 1 フレームも完走していない、が
  `docs/worklog/2026-09-18-web-black-screen.md` の測り方）。スクリーンショットには
  庭・HUD・エディタ・ガイド、そして VM パネルの
  `the rules world.rb — waiting for the game to answer 'run_world' — on playground.rb:313` が写っている。
  `playground.rb` はブラウザ側コンパイラにファイル名を渡せないため（§3.4）。
* `selftest: done — the garden keeps running (a page has nothing to exit to)` で終わる。

画素読み（`lit 0 / 16000`）は前回の記録どおり**直ったページでは使えない道具**なので、
canvas の寸法とスクリーンショットで判断した。

---

## 8. 未実施

* **エディタの status を画面で読んでいない**（§3.3）。壊したファイルで Apply を押した
  スクリーンショットは撮っていない。status に入る文字列はログに出る 1 行目と同一なので
  筋は通っているが、「画面で確かめた」とは書かない。
* **ベンチは取っていない。** 性能に触れる変更が無いため（ホイールは 1 フレーム 1 回の分岐、
  空の publish は読み込み時だけ、エラー文字列はエラーのときだけ、パネルは開いている
  フレームだけ）。並行して docker のビルドと他の担当の作業が同じ機械で動いていたので、
  取っても「静かな機械」の条件を満たさない。
* **8 番の隅そのものは直していない**（§5.3、§6）。原因と候補は上に書いた。著者判断待ち。
