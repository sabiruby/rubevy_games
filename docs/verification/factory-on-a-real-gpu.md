# Factory を実機の GPU で 1 度測る（著者へ）

Factory の**地図の既定の大きさ 32×32 だけが、誰も測っていない数**である
（`docs/plans/author-review.md` B、`docs/numbers.md` §9.6）。
スクリプトの側は測って決まった（腕 3,000 台でも予算に届かない）。工場の計算の側も測った。
**残っているのは描画だけ**で、この開発機の描画系は 2 つとも**ソフトウェア**（lavapipe と
SwiftShader）なので、ここからは導けない。**実機のブラウザで 1 度開けば数字が出る。**

## 5 行の手順

1. <https://sabiruby.github.io/rubevy_games/factory/?arms=1> を開き、ブラウザの console を開く（F12）。
2. **1 秒に 1 行** `stress: map …` が出る。**それを 3 行コピーする**（最初の 1 行は起動のフレームが
   混ざるので捨ててよい）。
3. もっと大きい地図を試す: `F1` → `data.rb` のボタン → `map :world, size: [32, 32]` を
   `[64, 64]` などに書き換える → **Apply**（組み直してよいか聞かれたらもう 1 度 Apply）→
   `Ctrl+S`（ブラウザの中に保存される）→ ページを `?arms=1` のまま**再読み込み**。
   上限は 2048、下限は 15（どちらも外れると `data.rb:行` で断られる）。
4. 工場を載せた状態も見るなら `?stress=4000`（ベルトの輪にアイテム 4,000 個。**地図の大きさは
   走行が自分で決める**）か `?arms=340`（既定の地図がちょうど持てる腕の数）。
5. コピーした行を貼る。元に戻すには `data.rb` を `[32, 32]` に戻して Apply + `Ctrl+S`、
   あるいはブラウザのサイトデータを消す。

## 出る行の読み方

```
stress: map 32x32 items=0 belts=3 arms=1 frame ms p50 16.73 p95 16.79 (about 60 fps) | step us p50 2 p95 2 | AMD Radeon ... (Gl, DiscreteGpu)
stress: vm insn p50 0 p95 0 of 39000 | tick us p50 3 p95 9 | answers p50 0 p95 0 | carried p50 0 p95 0 | dropped 0 | programs 2
```

| 見るところ | 何が分かるか |
|---|---|
| `map 32x32` | 測った地図の大きさ。`data.rb` が言ったものか、`?stress=N` が自分で決めたもの |
| `frame ms p50 / p95` | **1 フレームの長さ。60 fps なら 16.7 ms で頭打ち**（vsync）。これが 16.7 を超えたらその地図は重い |
| `step us` | 工場そのものの計算（ベルト・機械・腕）。ここは実測済みで、**2048×2048 でも 1 µs**（F4） |
| いちばん右 | **GPU のアダプタ名**。開発機は `llvmpipe` / `SwiftShader` としか出ない |
| `vm insn … of 39000` | スクリプトの命令数と予算。`carried` と `dropped` が 0 でなければ予算が噛んでいる |

**知りたいのは 1 つだけ: `frame ms` が 16.7 を離れるのはどの地図からか。**
そこから既定値を動かすかどうかが決まる（動かす先は `factory/ruby/data.rb` の
`map :world, size:` の 1 行で、`docs/numbers.md` §9.6 に出どころを書き足す）。

## なぜ URL のつまみがこの 2 つなのか

`?<name>=<value>` は `games_shell::checks::asked_number` の作法で、PC の環境変数
`FACTORY_<NAME>` と同じものである（S5b-5）。**地図の大きさを URL から渡す口は作っていない** —
地図は F3a から `data.rb` の宣言（`map :world, size:`）で、ページでもエディタで書き換えて
Apply できる。URL に 2 つ目の言い方を作ると、同じ数の綴りが 2 か所になる。

`?arms=1` を勧めるのは、**それが「`data.rb` の地図をそのまま使う」唯一の測り方**だからである。
`?stress=N` はベルトの輪を敷くために地図を自分で大きくし（√(N ÷ `items_per_tile`)）、
`?arms=N` は腕の置き場から大きくする（√(8N)）が、どちらも `data.rb` の地図より小さくはしない。
`arms=1` は下から当たらないので、残るのは `data.rb` の言う大きさだけになる。
