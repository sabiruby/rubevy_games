# 見せ場（実装指示書）

作成 2026-09-16。SabiRuby Battle で「Ruby で書くと何が嬉しいか」が画面で分かるものを 2 つ。rubevy の ECS の橋（`Rubevy::Entity#[]`、`subscribe`/`publish`、
`answer_with`、`Rubevy::Proxy`）と Playground の VM インスペクタ（sabiruby `docs/design/inspect.md`、`docs/design/playground.md`）の上に載せる。
著者の判断: sabiruby の `from-mrubyedge-plan.md`・`leftovers-plan.md`・`perf3-plan.md` と並行でよい（別リポジトリ、ベンチは無い）。

## 状況

| 段階 | 内容 | 状態 |
|---|---|---|
| D1 | `reflex`: 被弾したら別タスクでブロックを動かす（mruby-task の複数タスクを 1 体の中で使う最初の機能） | 済み（`82efcc2`） |
| D2 | ゲームの窓に VM インスペクタ（選んだロボットのタスクのフレーム・レジスタ・ヒープ） | 未着手 |
| D3 | ロボットの DSL を `Rubevy::Entity#[]` と `Proxy` で書き直せるか（検討。今の `ask` の形との比較） | 未着手 |

## D1. `reflex`

**到達点**（ロボットのファイル）:

```ruby
robot "Scout" do
  reflex(:hit) do |by, damage|          # 被弾のたびに、メインのループとは別のタスクで走る
    act turn: 1.0, throttle: 1.0         # 逃げる
    sleep 0.3
  end
  def run; loop { …; sleep 0.05 }; end
end
```

**設計**:
* ゲーム側: `move_bullets` で命中したときに `ScriptWorld::publish(Some(target), "hit", Answer::List([owner_bits, damage]))`（rubevy の B の API）。
  撃った側にも `"hit_landed"` を出すかは D3 で。
* prelude 側: `reflex(name, &block)` はクラスに登録しておき、`run_robot` が `bot.run` の前に `Task.new(name: "#{name}-reflex") { q = Rubevy.subscribe(name); loop { args = q.pop; bot.instance_exec(*args, &block) } }`
  を起こす。**メインのループと reflex が同じロボットの `act` を同時に呼ぶ**ので、後勝ち（`act` は最後に設定した値が残る）と決め、`docs/sabiruby-battle.md` に書く。
  優先度はメインより高く（数が小さい）、`sleep` している間はメインが動く。
* 停止: ロボットが倒れたら `ScriptTask` の除去でメインのタスクは止まるが、`Task.new` で作った reflex のタスクは**別のタスク**なので、
  `run_robot` が終わるときに `terminate` する（`ensure`）。rubevy の `subscribe` はエンティティ宛の購読をスクリプトのタスクの終了で外すが、
  `Task.new` のタスクからの `subscribe` は断られる（B の設計）ので、購読はメインのタスクで取ってブロックに渡す形に。
* HUD: reflex が走った回数か、走っている間だけロボットの名前の横に印。
* 確認: `--headless` で命中のたびに reflex のログが出る、selftest に「被弾後 0.3 秒以内に向きが変わる」。

**実装で分かったこと**（`docs/worklog/2026-09-16-reflex.md`、成果は `docs/sabiruby-battle.md` の *Reflexes*）:

* 計画書が書いた「`Task.new` のタスクは `subscribe` できない」は、**`ask` にもそのまま当てはまる**。
  `Rubevy.ask` も `current_entity` を読むので、エンティティを持たないタスクからの `act` は
  `entity: None` のリクエストになり `nil` が返る。prelude がメインのタスクの `@rubevy_entity` を
  新しいタスクに写している。rubevy 側で `Task.new` が引き継ぐか `Rubevy.adopt(task)` を出すのが本筋。
* **ブロックを `instance_exec` で呼ぶと reflex は何も待てない。** `instance_exec` / `send` /
  `Method#call` は VM の入れ子の実行ループで、タスクはその境界をまたいで park できない
  （`blocking pop cannot be called from within a C function boundary`）。`reflex` は
  `define_method` でクラスの普通のメソッドにし、`run_reflex` がソースに書かれた名前で呼ぶ。
  そのぶん数は固定（`REFLEX_SLOTS = 4`）。
* **「後勝ち」と「優先度が高いほど先に走る」を組み合わせると、reflex は必ず負ける。**
  先に走る＝リクエストが先に積まれる＝あとから来たメインの `act` が上書きする。さらにメインは被弾の瞬間
  たいてい質問を飛ばしていて、その答えで出す `act` が数フレーム後に舵を打ち消す。後勝ちは仕様として残し、
  reflex は舵を握り直し（0.05 秒ごとに `act`）、脳は `@swerve` が立っている間は手を引く、という
  取り決めをロボットの側に置いた。優先度を**下げれば**後勝ちで勝てる、というひっくり返った解もある。
* **外から `terminate` されたタスクは `ensure` を通らない。** 計画書の「`run_robot` が終わるときに
  `ensure` で `terminate`」は、脳が自分で終わったときにしか効かない。ロボットが倒れる／ファイルを保存すると
  ゲームが `ScriptTask` を外し、VM はコンテキストを巻き戻さずに捨てる。reflex のタスクは
  誰も publish しないキューで `WAITING` のまま残る（`Task.list` で確認）。直す場所は rubevy の
  `unsubscribe` で、キューを close すれば `pop` が `Task::Error` を上げてタスクが自分で終わる。
* HUD の印は prelude から `Rubevy.ask("reflex", :begin)` / `:end` で、**答えを `pop` しない**。
  待たない質問は命令になり、印のために reflex が 1 フレーム損をしない。

## D2. ゲームの窓に VM インスペクタ

**到達点**: エディタの下（または別の egui ウィンドウ）に、選んだロボットのタスクの「いま立っているフレーム（ファイル:行）とローカル変数の値」「ヒープの生存数と GC の回数」を出す。
一時停止（`P`）でその瞬間のレジスタを見られる。

**設計**: sabiruby の `Vm::snapshot`（`inspect.rs`。Playground が JSON にして出しているもの）をタスク単位で取れるようにする（`task_snapshot(task)`。無ければ VM に足す —
sabiruby 側の小さな追加）。rubevy の `ScriptWorld::stats` の隣に `snapshot(script) -> Snapshot`。ゲームは egui で表にする。一時停止は `RubevyPlugin` の budget を 0 にする。
ローカル変数の名前は DBG/LVAR から（`sabiruby run --stats` と同じ経路）。値の表示は `inspect_str`。

**確認**: `--shot` で撮った画面にフレームと変数が出ている。ヘッドレスでは `snapshot` の中身をログに。

## D3. DSL の書き直しの検討

`ask("radar")`/`act` の今の形と、`Rubevy.entity[:Transform]` + `Rubevy::Proxy.new("robot")` で書いた場合を並べ、往復回数（フレーム）とコードの読みやすさで比べる。
結論だけ `docs/sabiruby-battle.md` に。ゲームの規則を Rust に閉じる方針（`rust-bridge.ja.md`）は変えない。
