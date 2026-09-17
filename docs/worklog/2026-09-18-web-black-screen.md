# 2026-09-18 公開中のブラウザ版が真っ黒 — 最初のフレームで死んでいた

## 症状

著者の報告（2026-09-18）。公開中の <https://sabiruby.github.io/rubevy_games/garden/> を
Chrome で開くと、ページ自身が出す「loading the game…」の文字までは出るのに、そこから先が
真っ黒のまま何も起きない。直前の公開は main `9c12a91`（箱庭の世界を Ruby に: 第 2 VM
`struct World`、`world.rb` / `world_prelude.rb`、`answer_in_tick`、`Rubevy.ask("frame")` の
往復）と `b1e5ec1`（docs のみ）。その間に rubevy の同期読み（`4f77881`〜`3124dfa`）も
入っている。ネイティブ（`cargo run -p garden`）は正常に動く。

## 再現

作業場所は worktree `rubevy_games-wt-web`（ブランチ `web-black-screen`、main `b1e5ec1`）。

```
web/build.sh garden                          # ../sabiruby-playground の compiler を使う。wasm-opt は無い
cd web/dist && python3 -m http.server 8099
node drive.js http://localhost:8099/garden/ 45 before.png
```

`drive.js` はスクラッチパッドに置いた 50 行ほどの playwright-core の走り書きで、
`console` / `pageerror` / `requestfailed` / 400 以上の応答 / `crash` を全部拾い、最後に
ページの状態を 2 つ測る。**`page.goto` は `waitUntil: 'commit'`**（`docs/web.md` のとおり、
`'load'` は動いているページでタイムアウトする）。`--use-angle=swiftshader
--enable-unsafe-swiftshader` でソフトウェア WebGL。

## console の実文

bevy の起動ログはすべて正常に出る。アダプタが選ばれ、egui が機能の不足を警告し、その直後:

```
[console.log] INFO bevy_render-0.19.1/src/renderer/mod.rs:288 AdapterInfo { name: "ANGLE (Google,
  Vulkan 1.3.0 (SwiftShader Device (Subzero) (0x0000C0DE)), SwiftShader driver)", ...
  backend: Gl, ... }
[console.log] WARN bevy_egui-0.42.0/src/render/mod.rs:263 Feature TEXTURE_BINDING_ARRAY is not
  supported on this device.
[console.error] panicked at /rustc/2d8144b7880597b6e6d3dfd63a9a9efae3f533d3/library/std/src/sys/
  time/unsupported.rs:13:9:
time not implemented on this platform

Stack:

Error
    at __wbg_new_227d7c05414eb861 (http://localhost:8099/garden/pkg/game.js:1016:25)
    at http://localhost:8099/garden/pkg/game_bg.wasm:wasm-function[11493]:0x10b23c8
    ...
[pageerror] RuntimeError: unreachable
    at http://localhost:8099/garden/pkg/game_bg.wasm:wasm-function[144417]:0x1f7e1a4
```

著者が Chrome で見たものと同じ 1 行である。

**黒画面を「黒い」と言えるようにする。** 「真っ黒に見えた」では直った証拠が作れないので、
2 つ測った。

```
== status {"hidden":true,"text":"Garden loading the game…",
           "canvas":{"w":300,"h":150,"cw":1280,"ch":800}}
== pixels {"lit":0,"of":16000,"meanRGBsum":0}
```

canvas の内部解像度が **`300×150`**（HTML の canvas の既定値）のまま、CSS 上は 1280×800。
Bevy の `fit_canvas_to_parent` は毎フレーム走る系なので、この数字は「**1 フレームも完走して
いない**」と読める。画素のほうは canvas を 160×100 に縮めて `getImageData` で読み、
R+G+B が 30 を超える画素を数えたもので、16000 中 0。スクリーンショットも一様な `#12150f` —
`web/garden.html` の `body` の背景そのもので、canvas は何も描いていない。

（この画素読みは**直った後には効かない**。`preserveDrawingBuffer` の無い WebGL の canvas を
rAF の外で `drawImage` すると空が返るので、直った側では 0 のままになる。直った証拠は
`page.screenshot` のほうで取った。この道具の限界はここに書いておく。）

## 原因

rubevy の `src/lib.rs`、`tick_scripts` の**答えループ**の頭の 1 行。

```rust
let started = std::time::Instant::now();
```

`wasm32-unknown-unknown` には std の時計の実装がなく、`Instant::now()` は
`sys/time/unsupported.rs` の `panic!` に落ちる。console のファイル名がそのまま答えである。
`git log -S "std::time::Instant::now" -- src/lib.rs` が返すコミットは 1 つだけで、
`d0e9b85`（sync reads、S1）。S1 の前は VM を 1 回走らせるだけでフレームの経過時間を数える
必要がなかった。ループにしたときに残り時間の引き算のための時計が要って、手近な std の
`Instant` が入った。ネイティブでは正しく動くので、rubevy のテストもゲームのネイティブの
チェックも全部通っていた。

**ゲーム側には原因はない。** 疑っていた候補は、`web/garden.html:46` の
`window.gardenCompile` が creature 用に書かれていて 2 本目の VM / 別ファイルに対応していない、
`build.rs` の埋め込みが `ruby/` 直下の `world.rb` / `world_prelude.rb` を拾っていない、
`localStorage` の古い `garden:ruby/...` と新しい prelude の食い違い、`Rubevy.ask("frame")` の
wasm 特有の問題、の 4 つだったが、いずれも読んだ時点で外れていた。`gardenCompile` は
ソース文字列を 1 つ取るだけでファイル名を見ないので 2 本目の VM でもそのまま使える
（`docs/web.md` の「What the page's compiler cannot pass on is the file's name」のとおり、
どちらも `playground.rb` という名前になるだけ）。`garden/build.rs` の `collect` は
`ruby/` 以下を再帰して `.rb` を全部拾うので、`world.rb` も `world_prelude.rb` も入っている。
残りの 2 つは console が先に答えを出したので、当たるまでもなかった。

**この panic はコンパイルでは捕まらない。** rubevy 単体の
`cargo build --target wasm32-unknown-unknown` は、バグのあるコードでも通る（rubevy 側の
worklog に実測を書いた）。CI に wasm ビルドを足しても、これは止められない。

## 直し

直したのは rubevy であって、このリポジトリではない。答えループの時計を、VM 自身に
`task_set_clock` で渡してある `clock_ns`（Bevy の `Instant`、ブラウザでは `web-time`）に
替えた。rubevy のブランチ `wasm-instant`、コミット `4e52f5d`。理由と捨てた案は
rubevy の `docs/worklog/2026-09-18-wasm-instant.md` にある。

**このリポジトリのコードは 1 行も変えていない。**

## 確かめたこと

rubevy の worktree を `.cargo/config.toml` の `[patch]` で一時的に差し込んで
`web/build.sh garden` を組み直し、同じ headless Chromium で開いた。

45 秒の普通の走行:

* **pageerror 0、panic 0、`requestfailed` 0、400 以上の応答 0。**
* canvas が `300×150` → **`1280×800`**。フレームが回っている。
* スクリーンショットに 12 匹・43 株の庭、HUD（`the rules 12270 / 45000 insn/frame`）、
  開いた how-to-play の案内、`world.rb` を出したエディタが写っている。夜 0.52 なので
  地面は暗いが、黒画面ではない。
* ログには `night at 25.4 s`、`[script] Beetle: child 1: speed 2.58, sight 6.9,
  appetite 1.14`、`[script] Beetle: {"meals":1}` — 第 2 VM の `world.rb` が動いている。

160 秒の `?selftest`:

* **`selftest: ok` が 40 行、`FAIL` が 0 行。** 内訳は数えて、VM パネルと `P` が 17、
  エディタと Apply / Revert が 12、W3 の `F3`（世界の規則）が 11。最後の 11 には
  *the day is what world.rb says it is*、*Ctrl+Enter: the garden is running the edited
  rules, without stopping*、*Revert puts the file's rules back* が入っている。
* `ok` ではない観測の行も出そろっている: *the grass grew at 0.46 s — `world.rb` is
  running*、*Beetle 433v0 knows it is the wet season at 0.46 s*、*first meal at 1.16 s*、
  *the hungry beetle reached its plant at 2.32 s*、`Rabbit 421v0 — 26 ask round trips,
  7 decisions, 1864 insn/decision`。
* `selftest: done — the garden keeps running (a page has nothing to exit to)` の後も
  世界は生きている（`night at 85.4 s`、`born at 82.3 s`）。
* version 99 の拒否も出る: `saved with version 99, this garden reads 1`、HUD にも琥珀色で。
* `Beetle walks, idles and eats from its model` — models は 404 していない。

ネイティブも壊していない（同じ patch を当てた release ビルド）:

| | ok | FAIL | panic |
|---|---|---|---|
| `GARDEN_SELFTEST=1 garden --headless 90`（1 回目） | 13 | 0 | 0 |
| 同（2 回目） | 13 | 0 | 0 |
| `SABIBOTS_SELFTEST=1 sabibots --headless 90` | 51 | 0 | 0 |

`.cargo/config.toml` は消し、`Cargo.lock` は `git checkout` で戻した。**このリポジトリには
コミットしない**（rubevy が main に入って push された後で、本体が `cargo update -p rubevy`
する）。

## 残っていること

* 公開サイトは rubevy を取り込んで Pages を出し直すまで黒いまま。
* 「ネイティブで通ってブラウザで死ぬ」をこれから止める仕掛けは入れていない。wasm ビルドを
  CI に足すだけでは捕まらないことは上で確かめた。案は rubevy 側の worklog に 2 つ書いた
  （src/ の grep か、wasm でヘッドレスに 1 フレーム回すか）。著者判断待ち。
