# rubevy が crates.io に出るのに合わせて依存の書き方をそろえる（P2）

2026-09-18。rubevy 側の計画書 `rubevy/docs/plans/publish-plan.md` の段階 P2。ブランチ
`rubevy-published`、worktree `../rubevy_games-wt-publish`。触ったのは
**ルートの `Cargo.toml` とこの記録だけ**で、Rust のコードは 1 行も変えていない（したがって
`unsafe` も増えていない）。

## なぜ games を触るのか

rubevy は crates.io に出るために SabiRuby への git 依存を版に替えた（P1）。その瞬間、
games の `[workspace.dependencies]` がそのままだと **VM がバイナリに 2 つ入る**。
ルートの `Cargo.toml` のコメント自身が前からそう言っていた（「the same source as rubevy's,
so there is one VM in the binary and one set of types」）。理由は crate の同一性が
*名前と版ではなく source* で決まることで、crates.io の `sabiruby` と
`git+https://github.com/sabiruby/sabiruby` の `sabiruby` は cargo にとって別の crate になる。
`Vm` の型が 2 つになり、片方で作った `Value` はもう片方の `Value` ではない。

## 直接の依存を版にし、`[patch.crates-io]` で git に向け直す

計画書 §2.2 のとおり、`sabiruby`・`sabiruby-compiler`・`sabiruby-serde` を版で書き、
workspace のルートに `[patch.crates-io]` を置いて 3 つとも git に送った。`rubevy` は
**git のまま**（毎日動いているものに、まだ出してもいない 0.0.1 で games を縛らない）。

下限の版で 1 つだけ rubevy と違うところがある。rubevy は `sabiruby-compiler = "0.2.2"` で
足りるが、**games は 0.2.3 が要る**。エディタの色付けが `sabiruby_compiler::highlight` を
呼んでいて（`garden/src/platform.rs:81`、`sabibots/src/platform.rs:63`)、これを最初に
公開したのが 0.2.3 だからだ（sabiruby `docs/design/compiler.md` の "Publishing":
「0.5.2 (2026-09-18) published two: `sabiruby-compiler` 0.2.3 (`highlight()`)」）。
0.2.2 と書いてもここでは git の 0.2.3 に patch されて通ってしまうので、**書いた下限が
嘘にならない**ように 0.2.3 にし、その出どころをコメントに 1 行で残した。

`sabiruby-macros` を patch に入れるかは計画書が「`cargo tree` で決める」と言っていたので
見た。`cargo tree -i sabiruby-macros` の答えは

```
sabiruby-macros v0.1.0 (proc-macro) (https://github.com/sabiruby/sabiruby#7be7b86f)
└── sabiruby v0.5.2 (https://github.com/sabiruby/sabiruby#7be7b86f)
```

で、macros は git の `sabiruby` が自分の feature で引いている**だけ**であり、この workspace の
誰も直接は名指ししていない。patch 済みの crate の依存はもう git の側にいるので、
`[patch.crates-io]` に足す必要は無い（足せば「使われない patch」の警告になる）。**入れなかった。**

## 確認

P1 の rubevy はまだ push されていないので、隣の worktree を見るための
`.cargo/config.toml`（`[patch."https://github.com/sabiruby/rubevy"] rubevy = { path =
"../rubevy-wt-publish" }`）を置いて確かめた。これは **コミットしていない**。games の
`.gitignore` には `target/`、`shot.png`、`web/dist/`、`*.settings.txt` しか無く
`.cargo/config.toml` が入っていないので `git status` には `?? .cargo/` として出るが、
指示どおり `add` していない。

* `cargo tree -d` の重複の根は 17 種（`codespan-reporting`、`foldhash`、`font-types`、
  `guillotiere`、`harfrust`、`hashbrown`、`indexmap`、`itertools`、`linux-raw-sys`、
  `miniz_oxide`、`read-fonts`、`rustix`、`skrifa`、`syn`、`thiserror`、`thiserror-impl`）で、
  **`sabiruby` も `rubevy` も無い**（`cargo tree -d | grep -E '^(sabiruby|rubevy)'` が空）。
* `cargo tree -i sabiruby` は `sabiruby v0.5.2 (https://github.com/sabiruby/sabiruby#7be7b86f)`
  **1 つ**を根に、garden・sabibots・rubevy-arena・rubevy・`sabiruby-compiler`・
  `sabiruby-serde` の全部がそこへ集まる形を出した。**rubevy（版で crates.io を名指ししている
  側）も同じ 1 つに来ている** — patch が効いている証拠で、これが P2 の目的そのもの。
* `cargo check --workspace` は「使われない patch」の警告を 1 つも出さない。
* `pages.yml` 37 行目と同じ式
  `grep -m1 -o 'github.com/sabiruby/sabiruby#[0-9a-f]*' Cargo.lock | cut -d'#' -f2` は
  **`7be7b86ffdfc220531c79ec8ec4b056835c1ca67`** を返す。コミット済みの lock
  （`git show HEAD:Cargo.lock`）でも作業中の lock でも同じ。Pages の CI は 1 行も変えなくてよい。
* `cargo test --workspace`: **18 passed, 0 failed, 0 ignored**。
* ブラウザ（`web/build.sh garden` と `web/build.sh sabibots`、PATH に
  `~/.local/binaryen-version_132/bin`、Chromium は `~/.cache/ms-playwright` の
  playwright-core、`--use-angle=swiftshader --enable-unsafe-swiftshader`、`page.goto` は
  `waitUntil: 'commit'`、`web/dist` を `python3 -m http.server 8099` で配信):

  | ページ | 秒 | canvas | pageerror | requestfailed | console.error | selftest |
  |---|---|---|---|---|---|---|
  | `garden/?selftest` 1 回目 | 60 | 1280×800 | 0 | 0 | 0 | 59 行 — ok **43** / FAIL **0** / n/a 0 / `--` 2 |
  | `garden/?selftest` 2 回目 | 60 | 1280×800 | 0 | 0 | 0 | 57 行 — ok **43** / FAIL **0** / n/a 0 / `--` 1 |
  | `sabibots/?selftest` 1 回目 | 40 | 1280×800 | 0 | 0 | 0 | 38 行 — ok 36 / FAIL **0** / n/a 0 / `--` 1 |
  | `sabibots/?selftest` 2 回目 | 40 | 1280×800 | 0 | 0 | 0 | 40 行 — ok 38 / FAIL **0** / n/a 0 / `--` 1 |
  | `sabibots/?selftest` 3 回目 | 40 | 1280×800 | 0 | 0 | 0 | 39 行 — ok 38 / FAIL **0** / n/a 0 / `--` 0 |

  数を既知の基準と突き合わせる。箱庭は `2026-09-18-check-holes.md` が
  「`garden/?selftest` 60 秒 — **ok 43 / FAIL 0 / n/a 0**、pageerror 0 / requestfailed 0」と
  書いた基準にぴったり一致する。`--` の 2 行は同じ記録が塞いだ穴の**出るべき姿**
  （`on(:mate)` を持たない種のつがいは分母から引く）で、つがいが成立した回数で 1 行にも
  2 行にもなる。

  Battle は `2026-09-18-corner-and-selftest.md` §2.4 が「**固定で 31 行**、そのあと
  **当たり 1 発につき 2 行**」と書いている。1 回目を数え直すと当たりは 1.18 s・2.21 s・
  34.38 s の 3 発（2 行ずつ = 6 行）と、3.29 s の 1 発が `--` で除外（1 行）。
  38 − 6 − 1 = **31**。固定の 31 行が揃っている。2 回目・3 回目も同じ勘定で 31 に戻る。
  `--` の行（編集チェックが Apply to all を配っている最中の当たりは数えない）も
  同じ記録が入れた除外そのもので、1 回目の FAIL がこれに変わったのが `9e4de0f` の成果。

  canvas が `1280×800` であることも毎回見た。`300×150` のままなら 1 フレームも完走して
  いない、というのが `2026-09-18-web-black-screen.md` の読み方。

## `Cargo.lock` をコミットしない理由

manifest を書き換えたあとに lock に入った差は、**1 行の削除だけ**だった:

```
 [[package]]
 name = "rubevy"
 version = "0.0.1"
-source = "git+https://github.com/sabiruby/rubevy#e7b3eb52677ccf2dc818e26570f14fbd5f532e8b"
```

これは一時的な `.cargo/config.toml` が rubevy を path に向けているから（path 依存の lock は
`source` を持たない）で、**版を patch に替えたこととは関係が無い**。SabiRuby の 4 行は
git の `#7be7b86f` のまま 1 文字も動いていない。つまり `[patch.crates-io]` を入れても
lock の解決は変わらなかった — 前から git の同じ rev に来ていたので当然だが、これは patch が
ちゃんと版の要求（`sabiruby = "0.5.1"` など）を満たしていることの確かめでもある。

一時 patch を外したらどうなるかも見ておいた。`.cargo/` を退避して `git checkout -- Cargo.lock`
してから `cargo tree` を掛けると、**lock は 1 バイトも動かず**（`git diff Cargo.lock` が空）、
`cargo tree -i sabiruby` はやはり `sabiruby v0.5.2 (…#7be7b86f)` **1 つ**を出した。古い rubevy
（git の `e7b3eb52`、まだ sabiruby を git で引いている版）が混ざっていても VM は 1 つのままで、
それは古い rubevy の git の sabiruby と、このワークスペースが patch で送った先が**同じ source**
だから。つまりこのブランチは rubevy が push される前でも単体で正しく解決する。
`pages.yml` の grep もその状態で `7be7b86f…` を返す。

なので **lock はコミットしない**（`git checkout -- Cargo.lock` はしていない。作業ツリーに
差分として残してある）。本体が rubevy を push したあとに `cargo update -p rubevy` を掛けると、
その 1 行が本物の rev で埋まる。最初は `cargo update`（引数なし）を掛けてしまい、sabiruby の
rev が `7be7b86f` → `08d8e785` に進んだうえ syn・rustix・zlib-rs など 15 個が一緒に動いた。
この段階と関係の無い差分なので lock を `git checkout` で戻し、manifest の変更だけが lock に
届く形（`cargo metadata` を 1 回走らせるだけ）でやり直した。

## 残り

`cargo update -p rubevy`、main への merge・push、Pages の CI を見るところまでは本体（P3）。

## 追記（本体、P3）— rubevy を push したあとの lock

rubevy を main に merge して push（`2d9eb92`）、`cargo publish` で 0.0.1 を公開、タグ `v0.0.1`。
そのあとこの worktree の一時 `.cargo/config.toml` を消し、`git checkout -- Cargo.lock` →
`cargo update -p rubevy`。lock の差は rubevy の `source` が `e7b3eb52` → `2d9eb922` になった
1 行と、cargo が一緒に選び直した `windows-sys 0.61.2` → `0.60.2` の 5 行（Windows 専用の依存で、
PC 版（Linux）にもブラウザ版にも入らない）。

* `grep -m1 -o 'github.com/sabiruby/sabiruby#[0-9a-f]*' Cargo.lock` → `…#7be7b86f…`（`pages.yml` は無変更）
* `cargo tree -i sabiruby` → `sabiruby v0.5.2 (https://github.com/sabiruby/sabiruby#7be7b86f)` の 1 つ
* `cargo test --workspace` → 18 passed / 0 failed

wasm の確認（上の表）は `../rubevy-wt-publish` の path で取ったもので、`2d9eb92` との差は
rubevy の README の 1 行だけ（コードは同じ）なので取り直していない。公開版は Pages の CI の結果で見る。

## 追記（本体）— 公開版

Pages の CI（run 35342540658、`62a7cbf`）は build・deploy とも success（9 分 22 秒）。`pages.yml` が
lock から SabiRuby の rev を取る段も、Playground のコンパイラを作る段も無変更で通った。
公開版を同じ `check.js`（playwright-core、swiftshader）で 1 回ずつ、2 ページ同時に開いて確認:

| ページ | 秒 | canvas | pageerror | requestfailed | console.error | selftest |
|---|---|---|---|---|---|---|
| `sabiruby.github.io/rubevy_games/garden/?selftest` | 60 | 1280×800 | 0 | 0 | 0 | 57 行 — ok 43 / FAIL 0 |
| `sabiruby.github.io/rubevy_games/sabibots/?selftest` | 40 | 1280×800 | 0 | 0 | 0 | 31 行 — ok 30 / FAIL 0 |
