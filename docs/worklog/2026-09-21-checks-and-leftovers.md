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
