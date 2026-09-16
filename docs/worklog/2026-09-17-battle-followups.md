# 2026-09-17 rubevy が返してきた 4 つを SabiRuby Battle に入れる

著者判断 4 つ。どれも「rubevy か sabiruby の側で直った／分かった」ことを、ゲームの側で受け取る作業である。
ブランチ `battle-followups`、main `76b4d33` から。背景は `docs/plans/showpieces-plan.md` の D1・D3、
`docs/worklog/2026-09-16-reflex.md`、`docs/worklog/2026-09-16-showpieces-d2-d3.md`、
そして rubevy の `docs/worklog/2026-09-17-scheduling-and-vm-access.md`。

## 0. まず lock だけ（`9e8988b`）

`cargo update -p rubevy -p sabiruby -p sabiruby-compiler`。rubevy `9104f7c` → `fa37eaa`、
sabiruby `1c747bf` → `9c8ebf2`（0.5.0 のタグ）。ロックだけを 1 コミットにしたのは rubevy がそうしたのと
同じ理由で、このあとの 4 つの差分が「ゲームで何を変えたか」だけを見せてほしいから。
この時点で `cargo build --release -p sabibots` は通る（ゲーム側は 1 行も変えていない）。
`--headless 20` の selftest を 3 回回して、reflex の検査が 12/12、13/13、14/14 で通ることも見ておいた。
これが「前」の数字になる。

## 1. reflex の優先度をひっくり返す（`prelude.rb`、`scout.rb`）

### 何が間違っていたのか

計画書（D1）は「reflex の優先度はメインより高く（数が小さい）」と書いていた。実装してみると、
これは**後勝ちの `act` と噛み合わない**。優先度が高い＝そのフレームで**先に**走る＝リクエストが
先に積まれる＝あとから積まれた脳の `act` が上書きする。速いことが負ける理由になる、という形である。
D1 はそれを仕様として受け入れて、スカウトの側に回避策を置いた——reflex が 0.05 秒ごとに `act` を
言い直して舵を握り直し、脳は `@swerve` が立っている間は手を引く。worklog には
「優先度を**下げれば**後勝ちで勝てる、というひっくり返った解もある」と書いて残してあった。
今回はその解を採る、という判断である。

### 変えたのは 2 行

`prelude.rb` の `start_reflexes`:

```ruby
priority = here.priority - 20   # 前: 脳より高い
priority = here.priority + 20   # 後: 脳より低い（255 で頭打ち）
```

脳は `Script::with_priority(100)`（再読み込みは 128）なので、reflex は 120（148）になる。

### 回避策は本当に要らなくなったのか——測った

スカウトの reflex を「`act` 1 回 + `sleep 0.3`」に縮めて、selftest の
`the heading changed within 0.3 s of the hit`（被弾から 0.3 秒のうちに 0.2 rad 以上向きが変わること）を
`--headless 20` で回した。

| reflex の優先度 | reflex の書き方 | 5 回（3 回）の結果 |
|---|---|---|
| 低い（`+20`、今） | `act` 1 回 + `sleep 0.3` | **5/5 通過**（12/12、16/16、17/17、13/13、11/11） |
| 高い（`-20`、前） | `act` 1 回 + `sleep 0.3` | **3 回中 2 回 FAIL**（13/14、24/26、13/13） |
| 高い（`-20`、前） | `6.times { act; sleep 0.05 }` | 3/3 通過（12/12、13/13、14/14） |

対照（真ん中の行）を取ったのは、「通るようになった」のが優先度のおかげだと言うためである。
同じ 1 回の `act` で、優先度だけを戻すと落ちる。

**旋回の大きさも変わった。** 1 回の走りの `(… rad)` を並べると、低い優先度では最小 0.48・
最頻 0.74（旋回速度の頭打ち）で、15 回すべてが 0.48 以上。前の「握り直す」形では 0.25・0.30・0.31 が
混じっていた。握り直しは**舵を取り合っている**ので、脳が上書きした frame のぶんだけ旋回が痩せる。
優先度を直すと取り合いそのものが無くなる。

### `@swerve` は残す

優先度で直るのは「同じフレームの中でどちらが後か」だけである。被弾の瞬間、脳はたいてい質問を飛ばしていて
（`@swerve` を見て、レーダーを聞いた）、その答えが返ってから出す `act` は**別のフレーム**に来る。
これは順序では止まらないので、脳が `@swerve` を見て手を引く取り決めは要る。
`scout.rb` のコメントもそう書き直した。

## 2. `answer_requests` を `RubevySet::Answer` に入れる（`main.rs`）

### 鎖ごと動かすか、1 本だけ出すか

rubevy の worklog は「13 本の `.chain()` の中にいるので、鎖ごと `Answer` に入れるか、
`answer_requests` だけ鎖から出して `Answer` に入れるかはゲーム側の判断」と書いていた。
**鎖ごとにした。** 鎖の中の順序はこのゲームの規則そのものだからである——
`restart_match` → `answer_requests` → `move_robots` → … の並びは、
「答えを返してコントロールを設定してから、そのコントロールで戦車を動かす」という意味を持っている。
`answer_requests` だけを抜くと、この 1 本と `move_robots` の間の順序が指定なしに戻り、
`act` が効くのが 1 フレーム遅れるかどうかが executor のくじ引きになる。
往復を 1 フレームにする作業で、別のところに 1 フレームのくじを作るのは筋が悪い。

鎖の他の 12 本が `Answer` の中で回ることになるが、`ScriptWorld` を触るのは `answer_requests` と
`restart_match`／`reload_changed`（`Script`/`ScriptTask` の付け外し、どれも `Commands` 経由）だけで、
`Answer` の中に rubevy 自身の `answer_components` が同居しても順序の要求は無い。

### 測り直した

D3 のときの計測ロボットは**コミットしていない**（検討用なので）ので、worklog の全文から作り直した。
`sabibots/ruby/robots/study.rb` として置き、`matches/training.rb` を
「study 1 体 + 的の scout 1 体」に差し替えて `--headless 12`。測ったあと両方とも消してある。

変更前（rubevy `fa37eaa` に上げただけ、`in_set` なし）:

```
study: ask status x20 = 40 frames (2.0 each)
study: ask radar x20 = 40 frames (2.0 each)
study: act x20 = 40 frames (2.0 each)
study: A: the scout's decision (radar + incoming + act) x20 = 120 frames (6.0 each)
study: Proxy#method_missing -> ask x20 = 40 frames (2.0 each)
```

`.in_set(RubevySet::Answer)` のあと（2 回走らせて同じ）:

```
study: ask status x20 = 20 frames (1.0 each)
study: ask radar x20 = 20 frames (1.0 each)
study: act x20 = 20 frames (1.0 each)
study: A: the scout's decision (radar + incoming + act) x20 = 60 frames (3.0 each)
study: Proxy#method_missing -> ask x20 = 20 frames (1.0 each)
```

**ask の往復 2 → 1、スカウトの 1 判断 6 → 3。**

成分読みの側も測り直した。こちらは `--headless` が `MinimalPlugins` で型レジストリが空なので、
D3 のときと同じく `register_type::<Transform>()` を一時的に足して（測ったあと消した）:

```
study: components ["Transform"]
study: entity[:Transform] x20 = 20 frames (1.0 each)
study: entity.has?(:Transform) x20 = 20 frames (1.0 each)
study: Rubevy.find(:Transform) x5 = 5 frames (1.0 each)
study: find(:Transform) = 344 entities
study: B: find + 3 Transforms (one decision) x5 = 20 frames (4.0 each)
```

**成分読みは 1 フレームのまま。** rubevy が `answer_components` を鎖の先頭から末尾に動かしても
起きるフレームは同じ、と向こうの worklog が書いていたとおりである。
つまり今回の変更で消えたのは「ゲームが答える質問だけが 2 フレームだった」という**差**で、
`docs/sabiruby-battle.md` の *Why not `Rubevy.entity[:Transform]` and `Rubevy::Proxy`* の
「成分読みのほうが速い」という但し書きは要らなくなった（結論は変わらない——値段は質問の**数**である）。
`find(:Transform)` は 344 件で、D3 のときの 345 件とは 1 件違う。飛んでいる弾の数が違うだけである。

### 手触りは変わった。ロボットは調整していない

スカウトの 1 判断が 6 フレーム（100 ms）から 3 フレーム（50 ms）になり、`sleep 0.05` と合わせて
ループの周期が約 150 ms → 約 100 ms になる。**狙って撃つ回数が 1.5 倍になる**ということなので、
当たる数も増える。selftest が見ている被弾（reflex を持つロボットが受けた命中）を
20 秒の走り 5 回で数えると:

| | 5 回の被弾数 |
|---|---|
| 変更前（優先度の変更だけ入れた状態） | 12, 16, 17, 13, 11 |
| `RubevySet::Answer` のあと | 18, 17, 15, 21, 27 |

指示どおりロボットは**調整していない**。ここに書いておくのは、あとでロボットの強さを比べる人が
「このコミットの前後の数字は比べられない」と分かるようにするためである。
selftest は 5 回とも通る（reflex が走った・向きが変わった、どちらも全件）。
