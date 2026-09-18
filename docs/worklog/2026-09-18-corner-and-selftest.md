# 2026-09-18 8 番の隅を幾何から決める / Battle のブラウザ版に `?selftest`

著者から渡された 2 件を、1 件 1 コミットで片づけた記録。
作業場所は worktree `rubevy_games-wt-corner`（ブランチ `corner-and-selftest`、main `87ed16c` から）。
`main` には触っていない。push もしていない。

| コミット | 何を |
|---|---|
| `6da9fd4` | 1. 8 番の隅を `reach` と `mate_reach` から組み立てる |
| `f03d9dd` | 2. Battle のブラウザ版でも `?selftest` で編集チェックが走る |

**閾値は 1 つも動かしていない。** `world.rb` の規則は 1 行も変えていない（変えたのは
「game に渡る数が 2 つ増えた」ことを言うコメントだけ）。`unsafe` は 0 のまま。
新しく置いた数は 2 つ——`MATE_REACH`（2.0）と `MEADOW_CLEAR`（8.0）で、前者は
`world.rb` が黙っているときの既定（`CHILD_HUNGER` と同じ役）、後者は
`clear_of_fixtures` に G2 からあったリテラルに名前を付けただけで値は変えていない。

---

## 0. 着手前の状態

```
$ cargo build --release -p garden -p sabibots
    Finished `release` profile [optimized] target(s) in 2m 57s
$ cargo clippy -p garden -p sabibots -p rubevy-arena
warning: `rubevy-arena` (lib) generated 2 warnings
warning: `sabibots` (bin "sabibots") generated 12 warnings
warning: `garden` (bin "garden") generated 13 warnings
```

2 / 12 / 13 を基準にして、両方のコミットのあいだ測り直した。

---

## 1. 8 番の隅（`reach` と `mate_reach` から）

### 1.1 まず「前」を測った — 揺れは前日の記録より大きかった

前日の記録（`docs/worklog/2026-09-18-garden-leftovers.md` §5）は、20 秒走行 64 回のうち
**62 回で隅がつがいを作り、2 回は作らなかった**と書いていた。同じ条件
（`GARDEN_SELFTEST=1 --headless 20`、8 本並列、`TMPDIR` を分ける）で取り直すと、
「つがい 0 組」は **64 回中 0 回**だった——が、**最初の子が生まれた時刻**を並べると
隅の成功率はずっと低かった:

```
n=64  1.00 1.22 1.22 1.68 1.91×7 1.92×8 1.93×7 1.95 1.96 1.98 2.00 2.05 2.10 2.18 2.23 2.35
      2.67 2.79 2.94 3.42
      4.01 4.10 4.84 5.39 5.52 5.55 5.71 6.02 6.09 6.23 6.32 6.76 7.45 8.33
      12.34 12.48 12.85 13.27 13.55 14.17 16.17 16.76 18.66 19.31
```

隅は 2 秒で子を作るので、**2 秒台までの 35 回が隅の成功、残り 29 回は野良のつがい待ち**。
つまり隅は 64 回中 35 回しか働いていない。「つがい 0 組」が 0 回だったのは、
20 秒あれば野良がだいたい間に合うというだけで、隅の当たり外れとは別の話だった。
この 35/64 が、以下の「直した」の比較対象になる。

### 1.2 2 つの数を渡す道

`day_length` / `child_hunger` / `pop_max` と同じ道をそのまま使う。

* `garden/ruby/world_prelude.rb`: 既定として `def reach = nil` / `def mate_reach = nil` を
  `child_hunger` / `pop_max` の隣に置き、`run_world` が一度だけ投げる
  `being.garden.rules(...)` に `reach:` と `mate_reach:` を足した。
* `garden/src/main.rs`: `RuleBook` に `Option<f32>` を 2 つ。0 以下は
  「a reach of 0 is no distance at all」として断る（`day_length` が 0 を断るのと同じ扱い）。
* 受け皿は `Reaches { eat, mate, told }` という Resource。既定は
  `REACH`（1.1、既にある）と新しい `MATE_REACH`（2.0）。

`world.rb` 側は**規則を 1 行も変えていない**。`reach` と `mate_reach` は元から
`def reach = 1.1` / `def mate_reach = 2.0` としてそこにあり、増えたのは
「この 4 つは game にも渡る」と言うコメントだけ。

### 1.3 `spawn_world` では組めない — だから隅は 1〜2 フレーム遅れて建つ

最初に詰まったのはスケジュールだった。`spawn_world` は `Startup` のシステムで、
世界の script が task になるのは最初の `Update`、`run_world` が `garden.rules` を投げるのは
その task の 1 行目。**`Startup` の時点で規則はまだ一言も喋っていない。**

そこで隅だけを `spawn_world` から外し、`plant_the_meadow` という `Update` のシステムにした。
`spawn_world` は `Meadow` という空の Resource を置くだけで（`--load` の走行では置かない。
セーブから読む庭には仕込みの隅そのものが無い）、`plant_the_meadow` は

* `Reaches::told`（規則が喋った）か
* `WorldTrouble`（`world.rb` がコンパイルできず、**永遠に喋らない**と startup で分かっている）

のどちらかが立った最初のフレームに隅を建てて、`Meadow` を外す。
`RubevySet::<World>::answer()` の後ろに置いたので、規則が喋ったそのフレームに建つ。

両方の枝を実際に見た:

```
# 規則が喋る（通常）
selftest: two hungry beetles 1.34 apart, each 2.73 behind a blade of its own at (-14.0, 9.0)
          — from the rules' reach 1.10 and mate_reach 2.00

# world.rb を壊した走行
ERROR the world has no rules: world.rb: world.rb:318:10: syntax error, unexpected end-of-input…
selftest: two hungry beetles 1.34 apart, each 2.73 behind a blade of its own at (-14.0, 9.0)
          — from the rules' reach 1.10 and mate_reach 2.00 (the game's own: the rules never spoke)
```

### 1.4 選んだ配置と、その不等式

**「2 匹を草の同じ側から来させる」を採った。** ただし 1 つの草の塊ではなく、
**1 匹に 1 本ずつ草を置いて、その真後ろに 1 匹ずつ**立たせる形。理由は §1.5。

生き物は草に乗らず `reach + 草の大きさ/2` 離れたところから食べ、`hungry_below` を超えた
瞬間に草へ歩くのをやめる。だから**自分の草を通り越すことはなく**、止まる場所は
自分が歩いてきた線の上の `reach` から `reach + 草の半分` のどこか。2 本の草を
`apart` だけ横に並べ、2 匹をそれぞれ自分の草の真後ろに置くと、2 匹は**平行に歩き**、
止まったときの距離は

```
√( apart² + (どちらが何ぶん手前で止まったかの差)² )
```

で、2 つ目の項は最大でも草の半分（`PLANT_MAX/2`）。規則がつがいにするのはこれが
`mate_reach` 以下のときなので、**配置の全部がこの 1 本の不等式**:

```
apart² + (PLANT_MAX/2)² ≤ mate_reach²
```

もう一方の壁は 2 匹の体で、`apart > 2 × BEETLE_RADIUS`（下回ると最初のフレームで
`separate` に押し分けられる）。**`apart` はその窓の真ん中**を取った。
今の規則では 0.80 … 1.87 の窓で、**1.34**。

草の**後ろ**にどれだけ下がるか（`start`）も同じ形だが、壁が 3 枚ある。

| | 条件 | 理由 | 今の規則で |
|---|---|---|---|
| 下 | `start > reach + PLANT_MAX/2` | 下回ると歩かずにその場で食べる | 1.80 |
| 上 | `start ≤` 2 つの genome の小さいほうの `sight` | 超えると `garden.nearest` が草を見つけない | 7.00 |
| 上 | `2 × start + apart/2 ≤ MEADOW_CLEAR` | 超えると**庭の草のほうが自分の草より近い** | 3.67 |
| | | 窓の真ん中 | **2.73**（旧 4.5） |

3 枚目が本命だった（§1.5）。`MEADOW_CLEAR` は `clear_of_fixtures` が G2 から持っていた
8.0 で、**値は変えずに名前を付けただけ**。隅の生き物は隅の中心から
`√(start² + (apart/2)²)` の位置にいるので、庭が置ける最も近い草はそこから
`MEADOW_CLEAR − √(…)` 離れている。それが `start` より短ければ、
`garden.nearest(:Plant)` は自分の草ではなく庭の草を返す。

### 1.5 最初の実装は外した — 測って分かった 3 枚目の壁

最初は `start` を `(arm + sight)/2 = 4.4`（旧 4.5 とほぼ同じ）にして 64 回回した。
結果は **ok 61 / FAIL 2 / n/a 1**、最初の子は 2 秒台までが 39 回。
35 → 39 で、ほとんど良くなっていない。

**幾何を疑う前に、原因を仕込みの隅の外に置いてある可能性を潰した。**
使い捨ての `Lover` コンポーネントと probe を入れ（2 匹の meter・位置・速度・
**いちばん近い草とその大きさ**・自分たち以外の最も近い生き物・2 匹の距離を 0.25 秒ごと）、
`--headless 8` を 16 回。失敗した 7 回はどれも同じ形だった:

```
run12  0.26 s L0 h=44.6 at=(-9.93,9.67) v=(-2.0,0.0) plant=4.07@(-14.0,9.7) size=1.40
              L1 h=44.6 at=(-9.27,8.26) v=( 2.0,-0.4) plant=3.63@( -5.7,7.5) size=1.25
       1.26 s L0 h=43.2 …                             plant=2.06@(-14.0,9.7)
              L1 h=42.8 at=(-7.31,7.82) v=( 2.0,-0.4) plant=1.62@( -5.7,7.5) size=0.69
       （L1 は 20 秒のあいだ meter 40 前後のまま。apart は 5 を超えて戻らない）
```

**L1 は自分の草と反対方向へ歩いている。** `(-5.7, 7.5)` の草は隅の中心から 8.43 —
`clear_of_fixtures` の 8.0 の**すぐ外**——で、L1 の出発点からは **3.99**。
自分の草は 4.40。`garden.nearest` は近いほうを返すので、L1 は庭の草へ歩き、
そこは既に他の生き物が食べている小さい草だったので満腹にもならなかった。
別の走行では 2 匹とも同じ庭の草へ行っていた。

確かめとして、`clear_of_fixtures` の 8.0 を**使い捨てで** 14.0 に上げて 24 回回した:
2 秒台までが 19 回（4.4 のまま、clearance だけ変えた）。原因はこれで確定。

**直しは clearance ではなく `start`。** 8.0 は庭の見た目（40×30 の畑に 55 本）を決める数で、
上げると畑の 4 割が禿げる。隅のほうを庭の持ち物の内側に収める:
`2 × start + apart/2 ≤ MEADOW_CLEAR` を 3 枚目の壁にして、窓の真ん中で **2.73**。
このとき庭が置ける最も近い草は 5.19 離れていて、beetle の歩く距離（0.93）のほぼ 6 倍。

### 1.6 結果

`--headless 20` を 64 回（8 本並列、`TMPDIR` 別）:

| | 8 番の判定 | 最初の子が 2 秒台まで |
|---|---|---|
| 直す前 | ok 64 | 35 / 64 |
| `start` 4.4（外した案） | ok 61 / FAIL 2 / n/a 1 | 39 / 64 |
| **直した後** | **ok 64 / FAIL 0 / n/a 0** | **59 / 64**（1.08〜1.11 s） |

つがいが 0 組の走行（8 番が `n/a` になる走行）は **64 回中 0 回**。依頼の到達点はこれ。

`--headless 90` を 5 回（並列）は **13 判定すべて ok ×5**、FAIL も `n/a` も 0。

残り 5 回（4.45 / 5.76 / 8.07 / 9.27 / 11.03 s）は幾何ではなく beetle 自身の閾値で、
probe で形を見た: `hungry_below` を超えた瞬間に `wander` に移るので、**まだ草のすぐ手前に
いるうちに向きがランダムになる**。運悪く草から離れる向きを引くと、meter は 60 前後で
止まったまま（`hungry_below` を超えているので草を探しに行かない）数秒さまよい、
また空いてから戻る。閾値も `beetle.rb` も動かせないので、そこは直していない。

64 回のうち 1 回だけ 8 番が FAIL する走行も見た（`final64/run52`、2 pairings / 0 children）。
`on(:mate)` を持たないのは rabbit で、rabbit どうしのつがいも `courtings` に数えられる。
また beetle でも、相手が handler の起きる前に餓死すると `genome_of` が nil を返して
**何も log せずに** `next` する。どちらも今回の変更の前からある判定側の穴で、
`world.rb` にも隅にも関係が無いので手を付けていない（報告に上げる）。

probe と `Lover` は全部外した（`grep -c "fixprobe\|Lover(" garden/src/main.rs` が 0）。

### 1.7 数を写していないことの確かめ

`ruby/world.rb` の 2 行だけを書き換えて、**再ビルドせずに**走らせた:

```
def reach = 2.2 / def mate_reach = 3.0
→ selftest: two hungry beetles 1.86 apart, each 3.22 behind a blade of its own at (-14.0, 9.0)
            — from the rules' reach 2.20 and mate_reach 3.00
```

`apart` は 1.34 → 1.86、`start` は 2.73 → 3.22。どちらも §1.4 の式のとおり。

---

## 2. Battle のブラウザ版に `?selftest`

### 2.1 足りなかったのは 2 つの関数だった

箱庭は G5 で `platform::selftest_asked()` を持っている（PC は `GARDEN_SELFTEST`、
ブラウザは `window.location.search` に `selftest` が入っているか）。Battle は
`sabibots/src/main.rs` の中で `std::env::var("SABIBOTS_SELFTEST").is_ok()` を**2 か所で直に
呼んでいた**——`wasm32` では `std::env::var` はコンパイルは通るが必ず `Err` を返すので、
ブラウザ版はチェックを頼む口が無かった。

`sabibots/src/platform.rs` の両方の `mod imp` に、箱庭と同じ 2 つを足した:

* `selftest_asked()` — PC は環境変数、ブラウザは `?selftest`。
* `CHECKS_EXIT_WHEN_DONE` — PC `true` / ブラウザ `false`。

`main.rs` 側は `let checks_asked = platform::selftest_asked();` を 1 つ置いて 2 か所で使う。
（`selftest` はシステム関数の名前なので、ローカル変数名は `checks_asked`。
最初 `selftest` と書いて `no method named 'before' found for type 'bool'` で落ちた。）

### 2.2 `AppExit` を書いたままにしない

箱庭が G5 で踏んだ穴（`docs/web.md`「ページがマウスとキーボードに答えなくなる」）が
Battle にもそのまま残っていた。編集チェックの最後は `exit.write(AppExit::Success)` で、
ブラウザではこれは「走行の終わり」ではなく「この canvas が止まる」。winit の wasm の
イベントループが回らなくなり、最後のフレームが表示されたままアリーナに見える。

箱庭の `window_selftest` と同じ形にした:

```rust
if platform::CHECKS_EXIT_WHEN_DONE {
    exit.write(AppExit::Success);
} else {
    info!("selftest: done — the match keeps running (a page has nothing to exit to)");
}
```

なお `stop_when_over`（`--headless` の最後の `AppExit`）は触っていない。
`Headless` Resource はブラウザでは存在しないので、そのシステム自体が登録されない。

### 2.3 ページ側 — 鍵の一覧を箱庭に揃えた

`web/sabibots.html` が `preventDefault` していたのは `F5` / `Tab` / `Ctrl+S` / `Ctrl+Enter` だけ。
Battle が読む鍵を数えると `F1`（ガイド）と `F2`（VM パネル）があり、**チェックは `F2` を押す**。
ブラウザには `F1` について自分の考えがある。`web/garden.html` と同じ書き方に揃えた
（箱庭の `F9` は Battle が使わないので入れない）:

```js
const own = ["F1", "F2", "F5", "Tab"].includes(e.key);
if (own || (mod && (e.key === "s" || e.key === "Enter"))) e.preventDefault();
```

ページ側に `?selftest` のための仕掛けは**要らなかった**。箱庭の `garden.html` にも無く、
クエリ文字列は wasm 側が `window.location.search` で自分で読む。

### 2.4 実測

`web/build.sh sabibots` → `web/dist` を `python3 -m http.server 8099` で出し、
playwright-core（`~/.cache/ms-playwright` の Chromium、`--use-angle=swiftshader
--enable-unsafe-swiftshader`、`page.goto` は `waitUntil: 'commit'`）で 40 秒ずつ 4 回。

```
== status {"hidden":true,"text":"SabiRuby Battleloading the game…",
           "canvas":{"w":1280,"h":800,"cw":1280,"ch":800}}
== pageerror 0
== requestfailed 0
== console.error 0
== selftest lines 43
selftest: ok   `def` in the listing is painted in the keyword colour (kind Some(1))
selftest: ok   typing marks the text edited
selftest: ok   Apply gives robot 3 the edited behaviour
…
selftest: ok   nothing that was sleeping woke on the resume frame: the next one is due in 125 ticks, as it was two seconds ago
selftest: ok   P again gives the budget back
selftest: ok   the behaviours are running again
selftest: ok   and the match moves again: somebody has driven
selftest: ok   the match's clock runs again
selftest: done — the match keeps running (a page has nothing to exit to)
selftest: ok   3 blue/scout ran a handler within 0.3 s of the hit at 11.34 s
selftest: ok   3 blue/scout turned within 0.3 s of the hit at 11.34 s (0.71 rad)
…
```

**固定で 31 行**（編集チェックと VM パネルのチェック 30 + `done`）、そのあと
**当たり 1 発につき 2 行**。40 秒の走行で全体は 33〜43 行になり、当たりの数だけ動く。
4 回とも `pageerror` 0 / `requestfailed` 0 / `console.error` 0、canvas は 1280×800
（`300×150` のままなら 1 フレームも完走していない、というのが黒画面のときの読み方）。

FAIL は 4 回のうち 1 回だけ、`3 blue/scout ran a handler within 0.3 s of the hit at 3.78 s`
という**当たりの行**で出た。3.78 秒は編集チェックが Apply / ApplyAll / Restart を押している
最中で、`handler_selftest` の除外（0.3 秒以内に brain が差し替わった当たりは数えない）は
*その* robot の brain が替わった場合しか見ていない。swiftshader のページはフレームレートが
低いので 0.3 秒が数フレームしかない。**PC で窓を開けて `SABIBOTS_SELFTEST=1` を走らせても
同じ組み合わせになる**ので今回の変更で増えた穴ではないが、ブラウザで初めて見えた。
今回は手を付けていない（報告に上げる）。

`?selftest` の付かない `sabibots/` は selftest の行 **0 行**（チェックは頼まれなければ走らない）。

PC 側も測り直した:

```
$ SABIBOTS_SELFTEST=1 ./target/release/sabibots --headless 30
selftest: ok   the handler tasks of every robot that went down ended (1/1)
selftest: ok   26 hits on a robot with a handler were checked
selftest: ok   a handler ran within 0.3 s of the hit (26/26)
selftest: ok   the heading changed within 0.3 s of the hit (26/26)
（FAIL 0）
```

### 2.5 箱庭のブラウザ版も測り直した

同じ `web/build.sh` で箱庭も作り直して 60 秒:

```
== pageerror 0 / requestfailed 0 / console.error 0
== selftest: ok 43、FAIL 0、n/a 0
selftest: two hungry beetles 1.34 apart, each 2.73 behind a blade of its own at (-14.0, 9.0)
          — from the rules' reach 1.10 and mate_reach 2.00
```

§1 の隅はブラウザでも同じ数で建つ（`world.rb` は `build.rs` でモジュールに埋め込まれ、
`garden.rules` の道はプラットフォームに関係が無い）。43 行は `2026-09-18-editor-highlight.md`
が測った 43 行と同じ。


---

## 3. 2 件が入ったあとの確認

```
$ cargo build --release -p garden -p sabibots
    Finished `release` profile [optimized] target(s) in 10.48s
$ cargo clippy -p garden -p sabibots -p rubevy-arena
warning: `rubevy-arena` (lib) generated 2 warnings      （着手前と同じ）
warning: `garden` (bin "garden") generated 13 warnings  （着手前と同じ）
warning: `sabibots` (bin "sabibots") generated 12 warnings （着手前と同じ）
$ cargo test -p garden --release
test result: ok. 6 passed; 0 failed
$ grep -rn unsafe garden/src/ sabibots/src/ crates/ | wc -l
0
```

* 箱庭 `--headless 90` ×5（並列）: **13 判定すべて ok**、FAIL 0 / `n/a` 0。
* 箱庭 `--headless 20` ×64（8 本並列、`TMPDIR` 別）: 8 番 **ok 64 / FAIL 0 / n/a 0**。
* Battle `--headless 30` ×1: FAIL 0（当たり 26 発、`26/26` が 2 行）。
* ブラウザ（Chromium、swiftshader）: `garden/?selftest` は pageerror 0 / ok 43 / FAIL 0、
  `sabibots/?selftest` は pageerror 0 / 固定 31 行 + 当たり 2 行ずつ。
  どちらも `requestfailed` 0、`console.error` 0、canvas 1280×800。
