# 2026-09-18 エディタの構文色付け（段階 H2）

`docs/plans/editor-highlight-plan.md` の段階 H2。H0（sabiruby `7be7b86`、
`sabiruby_compiler::highlight`）と H1（playground `d72e000`、`sabi_highlight` と
`sabi.js` の `highlight()`）が済んでいるので、ここでやるのは**受け取って塗る側**だけ——
ゲームの `platform::highlight`、`rubevy-arena` のエディタ、ページの橋、そして色。

作業場所は worktree `rubevy_games-wt-highlight`（ブランチ `editor-highlight`、main `0e73635`）。
`main` には触っていない。push もしていない。**`unsafe` は 1 行も書いていない**
（`grep -rn "unsafe" garden/src/ sabibots/src/ crates/` は `#[wasm_bindgen]` の宣言も含めて 0 行。
wasm の橋は wasm-bindgen が生成するグルーの向こうにあり、こちらに `unsafe` ブロックは要らない）。
**閾値は 1 つも動かしていない。**

コミットは 3 本 + この文書:

| コミット | 何を |
|---|---|
| `663f3f2` | SabiRuby を `7be7b86` に（VM 0.5.2、`highlight` の入った compiler） |
| `ad80e2f` | エディタ・橋・色・窓のチェック 2 行・`PLAYGROUND_REF` |
| （この下） | docs と絵 |

---

## 0. 最初に VM を上げて、壊れていないことを確かめた

H2 には `sabiruby_compiler::highlight` が要る。`Cargo.lock` の SabiRuby は `bd6829b3`
（0.5.1）で止まっていて、そこには無い。

```
$ cargo update -p sabiruby -p sabiruby-macros --precise 7be7b86ffdfc220531c79ec8ec4b056835c1ca67
    Updating sabiruby v0.5.1 (…#bd6829b3) -> #7be7b86f
    Updating sabiruby-compiler v0.2.2 (…#bd6829b3) -> #7be7b86f
    Updating sabiruby-macros v0.1.0 (…#bd6829b3) -> #7be7b86f
    Updating sabiruby-serde v0.1.0 (…#bd6829b3) -> #7be7b86f
```

lock の中に SabiRuby が 2 つ居ないことを機械で確かめた。

```
$ grep -o 'sabiruby#[0-9a-f]*' Cargo.lock | sort -u
sabiruby#7be7b86ffdfc220531c79ec8ec4b056835c1ca67
```

1 行。`rubevy`（`e7b3eb5`、git 依存）の `dependencies` に並ぶ `sabiruby` はこの 1 つを指すので、
ゲームの VM と rubevy の VM は同じビルドのままである。ここが 2 つに割れると
`Value` が互いに通じなくなるので、毎回見る値打ちがある。

VM が 0.5.0 → 0.5.2 に動く（117 コミット）ので、**コードを 1 行も変えていない状態で**
先にチェックを回した。

```
$ GARDEN_SELFTEST=1 ./target/release/garden --headless 90     # ×3
run 1: ok=13 FAIL=0 na=0
run 2: ok=13 FAIL=0 na=0
run 3: ok=13 FAIL=0 na=0
$ SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 30
ok=4 FAIL=0
```

13 と 4 は `2026-09-18-garden-leftovers.md` §7 が記録している数と同じ。ここで差が出ていたら
H2 の話ではなく VM の話になっていたので、先に切り分けておいた。これが 1 本目のコミット。

着手前の clippy も取ってある（`cargo clippy -p garden -p sabibots -p rubevy-arena`）:
**arena 2、sabibots 12、garden 13**。leftovers の記録と同数。

---

## 1. 解析関数をどう渡すか

計画書 §3.3 は「`Editor` は `rubevy-arena` にあり `platform` はゲーム側なので、解析関数は
ゲームがプラグインに渡す（`EditorPlugin::with_highlighter(fn(&str) -> Vec<u8>)` か
`Resource` の `Highlighter(Box<dyn Fn>)`。既存の `compile` の渡し方に揃える）」と書いている。

**「既存の `compile` の渡し方」は無かった。** `compile` は `rubevy-arena` を通らない。
エディタは `EditorAction` を立てるだけで、それを受けて `platform::compile` を呼ぶのは
ゲーム側（`do_editor_actions`）である。arena に渡されている関数ポインタで既にあるのは
1 組だけで、それは `Settings::load(path, header, platform::read, platform::write)`——
**素の `fn` ポインタ**だった（`crates/rubevy-arena/src/settings.rs`）。

なので `Box<dyn Fn>` ではなく `fn` を選んだ。理由は 2 つ。既にある 1 例に揃うこと。そして
捕まえるものが無いこと——`platform::highlight` は自由関数で、PC ではリンク済みの compiler を、
ブラウザではページのグローバルを呼ぶだけで、状態を持たない。`Copy` なので `Resource` に
入れて `Res<Highlight>` から `copied()` で取り出せ、`ResMut<Editor>` との借用が衝突しない。

```rust
pub type Highlighter = fn(&str) -> Vec<u8>;
EditorPlugin::with_highlighter(platform::highlight)
```

素の `EditorPlugin` も残してある（`Default`）。色を渡さないゲームは H2 以前のエディタを
そのまま得る。今は両ゲームとも渡しているので、これは「arena が Ruby を読まない」を
型で言うためだけの口である。

## 2. 「変わったときだけ」をどう測るか

計画書の既定 5。`layouter` は毎フレーム呼ばれる。`Editor` に `kinds: Vec<u8>` と
`highlighted_for: Option<u64>`（本文のハッシュ）を持たせ、`draw_editor` の頭で
1 回だけ比べる。

ハッシュにしたのは、本文が変わる道が多いから——打鍵、`show`、`applied`、`reset_to`、
ゲームが直接 `editor.text` に書く場合。フラグにすると、そのどれか 1 つが立て忘れた瞬間に
色が古いまま止まる。ハッシュなら `text` を見ている限り取りこぼしが無い。

代金は測った。`DefaultHasher`（SipHash-1-3）で、この機械、`rustc -O`、best of 200:

| ファイル | バイト | ハッシュ |
|---|---:|---:|
| `garden/ruby/world.rb` | 16249 | **2.3 µs** |
| `garden/ruby/prelude.rb` | 21433 | 3.1 µs |
| `garden/ruby/creatures/beetle.rb` | 5870 | 0.8 µs |
| `sabibots/ruby/robots/scout.rb` | 2708 | 0.4 µs |

エディタが持つ最大のバッファ（`world.rb`）で 2.3 µs。60 fps の 1 フレーム 16.7 ms の
**0.014%**。省く相手（H0 の実測で `world.rb` の Prism が 147 µs）の 1.6% で済む。
この 2 つの数を `editor.rs` のフィールドのコメントに置いた。

（「長さ + 変更フラグ」も考えたが捨てた。打鍵は 1 文字置換で長さが変わらないことが多く、
長さだけでは足りず、結局フラグが要る。上に書いた取りこぼしの問題に戻る。）

## 3. 色 — 何を見て決めたか

計画書の既定 6 が 2 つを決めている。**分類 0 は今の本文色 `210,214,222` のまま**、
**コメントは gutter と同じ弱い色**。gutter は `120,128,140` なので、これで 3 は決まる。

残り 7 つを選ぶために、まず**背景を 2 つ実測した**。

* **パネルの地**: egui の dark テーマで `TextEdit` が敷くのは
  `Visuals::dark().extreme_bg_color`＝`Color32::from_gray(10)`
  （`egui-0.36.2/src/style.rs:1513`、`TextEdit` は `text_edit_bg_color` が `None` なら
  これを使う: 同 `1153`）。つまり **(10,10,10)**。
* **熱の帯**: `from_rgba_unmultiplied(150,110,20, α)`、α は `share * 170` で最大 170。
  上の地に重ねると `(150·170 + 10·85)/255 = 103`、同様に g=77、b=17 で **(103,77,17)**。

この 2 つに対する WCAG コントラストと、9 色どうしの CIE76 ΔE を計算した
（スクラッチパッドの使い捨て `colors.py`）。

| # | 分類 | RGB | 地 (10,10,10) | 帯 (103,77,17) |
|---|---|---|---:|---:|
| 0 | 既定 | 210,214,222 | 13.59 | 5.45 |
| 1 | キーワード | 185,145,235 | 7.84 | 3.14 |
| 2 | 文字列 | 150,200,130 | 10.26 | 4.11 |
| 3 | コメント | 120,128,140 | 4.96 | 1.99 |
| 4 | 数値 | 120,200,205 | 10.32 | 4.13 |
| 5 | シンボル | 235,150,200 | 9.17 | 3.68 |
| 6 | 定数 | 230,205,140 | 12.70 | 5.09 |
| 7 | 変数 | 240,130,120 | 7.70 | 3.09 |
| 8 | メソッド名 | 140,180,230 | 9.23 | 3.70 |

ΔE の**最小は 25.9**（0 既定 と 4 数値）。以下 28.3（0 と 8）、28.7（1 と 5）、
28.9（3 と 8）と続く。ΔE 2.3 が「やっと見分けられる」の目安なので、25 は
「見間違えようがない」側にある。

判断に使った決まりは 3 つ。

**(a) 暖色は熱のもの。** 帯はオレンジで、パネルの `* edited` も `240,190,90` の琥珀色。
「この行に時間が行っている」と「この語は数値だ」が同じ色相だと読み手が混ざる。
だから 9 色にオレンジ・琥珀は 1 つも無い。定数の `230,205,140`（砂色）が最も暖かいが、
これは黄寄りで、帯 `(103,77,17)` に対して 5.09——表の中で 0 の次に明るい。

**(b) 分類 8 はいちばん静かに。** §6 の申し送りのとおり、Ruby では演算子も `[]` も
メソッド呼び出しで、`x =~ /re/` の `=~` も `me[:Hunger]` の角括弧も 8 になる
（sabiruby の worklog「演算子は 8 になる」）。箱庭の脚本は行頭がほぼ全部メソッド呼び出し
なので、ここに強い色を置くと全行が塗り潰される。既定色から ΔE 28.3 の柔らかい青
`140,180,230` にした——「呼ばれる名前だ」とは言うが、叫ばない。

**(c) コメントは弱くてよい、そして帯には（ほぼ）乗らない。** 3 のコントラストは帯の上で
1.99 と表の中でただ 1 つ 3.0 を切る。これを許したのは、**熱はその行で実行された命令から
来る**ので、コメントだけの行に帯は立たないから。乗りうるのは `@n = 1 # ha` のような
行末コメントで、そこは元から弱いことが望ましい。地の上では 4.96 で、WCAG AA の 4.5 は
超えている。

残りは慣習どおりに割り当てた（キーワード＝紫、文字列＝緑、数値＝シアン、シンボル＝桃、
定数＝砂、変数＝珊瑚）。One Dark の割り当てを下敷きにして、(a) に引っかかる 2 つ——
あちらの数値（オレンジ）と定数（金）——だけをシアンと砂に振り直した形。

### 捨てた案: 熱い行の前景を琥珀のまま残す

今の `listing()` は `share > 0.6` の行を `255,236,150` で描いていた。トークンごとの色を
入れると前景はそちらに要るので、この分岐の行き場が無くなる。

3 通り考えた。(1) 熱い行だけ色付けを止めて琥珀一色にする。(2) 分類色を琥珀へ寄せて混ぜる。
(3) 前景は分類色だけにし、熱は帯（背景）だけで言う。

**(3) を選んだ。** (1) は「いちばん読みたい行だけ色が消える」で本末転倒。(2) は混ぜ方の
係数が根拠の無い数になる。そして計画書の決めごと 2 は「**熱の帯（背景）は残す**。色は前景」
——背景だけを名指しして残すと言っているのだから、前景が Ruby に明け渡されるのは筋である。
帯の α は `share` に比例して 0〜170 まで連続に動くので、熱の強弱はそこで言えている。

**閾値は動かしていない。** 帯が立つかどうかを決める `share < 0.1` はそのまま。消えたのは
`> 0.6` の分岐で、これは「2 つの前景色のどちらを選ぶか」だけを決めていたもので、
選ぶ相手が無くなったから消えた——閾値を動かしたのではない。

実物で確かめたのが `docs/vm-inspector.png` の 31 行目。`sleep 0.05` が帯の中にあり、
`sleep` が 8 の青、`0.05` が 4 のシアンで、どちらも帯の上で読める。

## 4. UTF-8 — バイトの表と `&str` の間

計画書 §5 の 4。分類表はバイト単位、`job.append` は `&str` を取る。文字の途中で切ると
`&line[a..b]` が panic し、しかもそれが起きるのは**エディタが開いている毎フレーム**である。

`listing()` の走査を `char_indices()` にした。run の開始も終了も `char_indices` が返した
オフセットか行末なので、**構造上、文字の途中では切れない**。分類はその文字の先頭バイトの
ものを採る。

構造上そうだ、で終わらせずに、**表が嘘をついたらどうなるか**をテストにした
（`crates/rubevy-arena/src/editor.rs` の `mod tests`）。

```rust
let text = "# 甲虫は歩く — 3 bytes a character\n";
let mut kinds: Vec<u8> = vec![3; text.len()];
for (i, k) in kinds.iter_mut().enumerate() { *k = (i % 9) as u8; }
```

3 バイト文字の**真ん中で必ず分類が変わる**表を食わせ、(1) panic しないこと、(2) 描かれた
run をつなぐと元の文字列に戻ること、(3) どの run も元の文字列の部分文字列であること
（＝文字として壊れていない）を見る。Prism の表がこうなることは無い（H0 が
`is_char_boundary` をテストしている）が、パネルはそれを確かめる場所ではない。

テストは 5 本足した。上の 1 本のほか、分類ごとに run が分かれること、表が短い／空のときは
既定色 1 本になること、表が**行頭からではなく本文の先頭から**引かれること（ここを間違えると
2 行目が 1 行目の色を着る）、そして §2 の「変わったときだけ」が本当に 1 回しか呼ばないこと。

```
$ cargo test -p rubevy-arena
test result: ok. 12 passed; 0 failed
```

実物での確認は窓のセルフテストが兼ねている。`F3` で開く `world.rb` には em ダッシュ
（`—`、3 バイト）が 22 行に入っていて、11 番のチェックから 13 番まで、その本文が毎フレーム
`listing()` を通る。そこを通り抜けて 43 行の ok が出ているので、実データでも切れていない。

（`world.rb` に日本語は無かった——計画書は「日本語コメントのある `world.rb`」と書いているが、
実際に入っている非 ASCII は 3 種で、`—`（3 バイト）、`…`（3 バイト）、`§`（2 バイト）。
2 バイトと 3 バイトの両方が居るので「複数バイトの文字を切らない」の実データとしては同じ働きを
するが、文面と実物が違う点として記録しておく。日本語そのものはテスト（`甲虫`）とブラウザの
橋（`# 甲虫` を通した、§6.3）で見ている。）

### 分かったこと: egui は同じ書式の run をつなぐ

テストを書いていて分かった。`LayoutJob::append` は「直前の section と書式が同じで
leading_space が 0 なら、その section を伸ばすだけ」という最適化を持っている
（`epaint-0.36.2/src/text/text_layout_types.rs:205`）。

おかげで、色の付かない本文（表が空、あるいは全部 0）は**行数に関係なく section 1 つ**に
なる。H2 以前は 1 行 1 section だったので、橋の無い古いページではむしろ section が減る。
最初に書いたテストは 2 行なら 2 section だろうと決めつけて落ちた。

## 5. 橋の名前 — 計画書と実物が食い違った 1 点

計画書の既定 4 と依頼は `window.gardenHighlight` / `window.battleHighlight` と書いている。
ところが Battle 側の既存の橋は `window.sabibotsCompile` で、`sabibots/src/platform.rs` の
コメントも「ページがゲーム自身の名前で定義する関数」と言っている。`battleHighlight` を
`sabibotsCompile` の隣に並べると、1 ページの中で同じゲームが 2 つの名前を名乗ることになる。

**`window.sabibotsHighlight` にした。** 計画書が言っている「`gardenCompile` の隣、同じ try の
中」という規則——ページはゲーム自身の名前で定義する——に従うと、こちらになる。ページと
ゲームは同じコミットで一緒に変わるので、どちらの名前でも壊れはしない。ここは本体の判断で
1 行ずつ直せる。

橋が無いときの扱いは計画書どおり「全部 0」。加えて**長さが合わない表も 0 に落とす**ことにした
（`Ok(kinds) if kinds.length() as usize == src.len()`）。長さの違う表は「この本文についての表」
ではないので、色として信じる理由が無い。どちらも panic させないのが肝で、
`2026-09-18-web-black-screen.md` が、ページで 1 回 panic すると canvas ごと死ぬことを
記録している。ページ側の `try { … } catch` も同じ理由で置いた。

## 6. 確認

### 6.1 ネイティブ

```
$ cargo build --release -p garden -p sabibots      # 警告 0、エラー 0
$ cargo clippy -p garden -p sabibots -p rubevy-arena
warning: `rubevy-arena` (lib) generated 2 warnings
warning: `sabibots` (bin "sabibots") generated 12 warnings
warning: `garden` (bin "garden") generated 13 warnings
```

**着手前と同数**（2 / 12 / 13）。

```
$ GARDEN_SELFTEST=1 ./target/release/garden --headless 90    # ×3
run 1..3: ok=13 FAIL=0
$ SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 30
ok=61 FAIL=0
```

（sabibots の 61 は 1 発ごとの ok 行を含む数。まとめの 4 行は全部 ok。）

### 6.2 窓（WSLg + lavapipe、`docker/`）

volume は leftovers のものを使い回した（`rubevy-games-target-leftovers`。あちらは main に
入っていて、同じ main から切った枝なので温まっている）。`docker/run.sh` は
`GARDEN_SELFTEST` を渡さないので、leftovers の記録と同じく `docker run` を直接書いた。

箱庭: **43 行すべて ok、FAIL 0**。42 行が前回までのもので、増えた 1 行が H2。

```
selftest: ok   the editor shows the file of the creature that was clicked
selftest: ok   `def` in the listing is painted in the keyword colour (kind Some(1))
```

Battle: 編集チェック一式が ok で、同じ 1 行が増えた。

```
selftest: ok   `def` in the listing is painted in the keyword colour (kind Some(1))
selftest: ok   typing marks the text edited
```

この 1 行は**画素を見ていない**。`Editor.kinds` の該当バイトを見るだけでも計画書は良しと
しているが、それだと「表に 1 が入っている」しか言えない。`drawn_kind()` を
`rubevy-arena` に足して、パネル自身の `listing()` を通し、できた `LayoutJob` の section の
`format.color` を色表で引き直している。つまり「**キーワードの色で塗られている**」まで言える。
`byte_range` は `Range<ByteIndex>`（epaint 0.36 が導入した強い型）なので、`usize` を
`egui::text::ByteIndex` に包む 1 行が要った。

### 6.3 ブラウザ（`web/build.sh all` + Playwright）

```
$ PATH=~/.local/binaryen-version_132/bin:$PATH \
  SABIRUBY_PLAYGROUND=../sabiruby-playground web/build.sh all
sabibots/game_bg.wasm: 34937939 bytes (10677505 gzipped)
garden/game_bg.wasm:   35647958 bytes (10885349 gzipped)
*/sabiruby.wasm:        2445509 bytes (840268 gzipped)
```

`web/dist` を `python3 -m http.server` で配り、playwright-core（playground の
`node_modules` のもの、Chromium 1243、`--use-angle=swiftshader`）で開いた。
`page.goto` は `waitUntil: 'commit'`（動いているページは `'load'` を撃たない）。

**箱庭 `garden/?selftest`、170 秒:**

```
== state {"status":{"hidden":true,…},"canvas":{"w":1280,"h":800,"cw":1280,"ch":800},
          "bridges":{"compile":true,"highlight":true},
          "sample":{"len":19,"bytes":"1110803333333338040"}}
== selftest ok=43 FAIL=0 n/a=0
== problems: none
```

* **pageerror 0、console.error 0、requestfailed 0、400 以上の応答 0。**
* **`selftest: ok` が 43 行、FAIL 0、n/a 0。**窓と同じ数、同じ新 1 行。
* canvas が `1280×800`（`300×150` のままなら 1 フレームも完走していない、が
  `2026-09-18-web-black-screen.md` の測り方）。
* `window.gardenHighlight` がページの文脈から関数として見え、`"def a\n# 甲虫\np 1\n"` に
  **`1110803333333338040`** を返した。19 バイト＝ソースのバイト数。`# 甲虫` は 9 バイトで
  9 つとも 3、`p` が 8、`1` が 4。H1 が Node と console で取った `3333333338040` と
  1 バイトも同じで、**Rust の `highlight()` → wasm → `sabi.js` → wasm-bindgen → ゲーム**の
  4 つ境界を越えて表がずれていないことになる。

**Battle `sabibots/?selftest`、40 秒:** `problems: none`、`bridges.highlight: true`、
同じ `sample`。`selftest` の行は 0 行で、これは**仕様どおり**——Battle の編集チェックは
`std::env::var("SABIBOTS_SELFTEST")` で入るので、環境変数の無いページでは動かない
（箱庭だけが `platform::selftest_asked()` でクエリ文字列を読む）。ページで確かめられるのは
「例外が出ないこと」と「橋が居ること」までで、そこは両方満たしている。

スクリーンショットは両方とも目で見た。エディタの中でコメントが灰、文字列が緑、
キーワードが紫、数値がシアン、`@swerve` が珊瑚、`turn:` が桃で出ている。

### 6.4 大きさ

色付けが wasm をいくら太らせたかは、**同じ木を 2 回組んで**測った。`git checkout 663f3f2 --
crates garden/src sabibots/src` で色の入る前のコードに戻して `web/build.sh all`、戻して
もう一度。VM もツールチェーンも動かないので、差はこの変更そのものである。

| | 色なし | 色あり | 差 |
|---|---:|---:|---:|
| `sabibots/pkg/game_bg.wasm` | 34,934,405 | **34,937,939** | **+3,534**（+0.010%） |
| その `gzip -9` | 10,675,858 | **10,677,505** | +1,647 |
| `garden/pkg/game_bg.wasm` | 35,642,569 | **35,647,958** | **+5,389**（+0.015%） |
| その `gzip -9` | 10,881,862 | **10,885,349** | +3,487 |

数キロバイト。ブラウザ版のゲームモジュールには**字句解析器が 1 バイトも入らない**からで、
Prism は既に読み込まれている compiler モジュールの側にある。入ったのは 9 色の表と run の
ループと `wasm_bindgen` の import 1 つだけ。2 回目のビルドが 1 回目とバイト単位で同じ値に
なったので、測り直しとしても成立している。

`compiler/sabiruby.wasm` は `3f47c9d` の 2,435,191 から `d72e000` の **2,445,509** へ
（gzip 835,234 → 840,268）。うち +1,736 / +493 が `sabi_highlight` そのもので、これは H1 が
同じ木で export ありなしを組んで測った値。残りは playground が組み直された SabiRuby の差。

### 6.5 `PLAYGROUND_REF`

`3f47c9d` → **`d72e000`**（フル SHA）。これを忘れると公開版だけ色が付かない（§5 の 7）。
`pages.yml` は playground を `Cargo.lock` の SabiRuby に対して組むので、**2 つのピンは
一緒に動く**必要がある——`highlight()` は `7be7b86` で入ったので、lock が `bd6829b3` の
ままだと CI の playground ビルドがコンパイルエラーで落ちる。この関係を `docs/web.md` の
CI の節に 1 段落で書いた。

---

## 7. 絵

窓（WSLg + lavapipe）で撮り直して、既存の 2 枚を置き換えた。

* `docs/garden.png` ← `garden --shot docs/garden.png 22`。`docs/garden.md` が 2 か所から
  参照している、エディタが大きく写る 1 枚。`rabbit.rb` の `on(:night)` / `on(:bumped)` が
  9 色のうち 7 色を含んでいる。
* `docs/vm-inspector.png` ← `sabibots --shot docs/vm-inspector.png 14 --vm`。
  `docs/sabiruby-battle.md` の VM パネルの節。31 行目に熱の帯があり、その中の
  `sleep 0.05` が読める——§3 の「帯と文字は喧嘩しない」の実物。

どちらも既存の撮り方（docs に書かれているコマンド）をそのまま使い、秒数もフラグも
変えていない。

---

## 8. 未実施・気づいたこと

* **ベンチは取っていない。** 性能に触れる変更は「毎フレームのハッシュ 2.3 µs」だけで、
  それは §2 で単体で測った。ゲーム全体のフレーム時間に対しては 0.014% で、
  `tools/bench.sh` に相当するものがこのリポジトリには無い。
* **Battle のページには `?selftest` が無い**（§6.3）。箱庭だけが `platform::selftest_asked()`
  を持つ。H2 の範囲外なので足していないが、Battle の編集チェックをブラウザで回す道は
  今も無い。
* **`sabibotsHighlight` か `battleHighlight` か**（§5）は本体の判断が要る 1 行。
* **`kinds` は `pub`** にした。窓のチェックが `drawn_kind()` 経由で読むだけなら `pub` で
  なくてよいが、計画書が「`Editor.highlight` の該当バイトが 1 であることを見る」という
  もう一方の道も許しているので、そちらも閉ざさないようにした。
* **playground の CodeMirror は触っていない**（H1 の範囲外の続きとして、こちらでも何も
  していない）。
