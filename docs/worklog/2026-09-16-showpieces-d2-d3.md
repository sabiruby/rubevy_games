# 2026-09-16 見せ場の続き（rubevy に返したもの、窓の中の VM インスペクタ、DSL の比較）

`docs/plans/showpieces-plan.md` の D2 と D3、それに **D1 で「本当は rubevy がやるべき」と書いて
残しておいたもの**（`docs/worklog/2026-09-16-reflex.md` の 2 つ）を rubevy が引き受けたので、
ゲーム側から外す作業。ブランチ `showpieces-d2`（main `a5a3088` から）。

最初に `cargo update -p rubevy -p sabiruby -p sabiruby-compiler`（`755a8d1`）。
rubevy `67e2649` → `9104f7c`、sabiruby `9b11478` → `564434e`。ロックだけを先に 1 コミットにしたのは、
そのあとのコミットが「この版だから消せる」という話ばかりになるからで、差分の読み手が
どの版で何が変わったかを 1 か所で見られるようにするため。

## 0. rubevy が引き受けた 2 つを外す（`537177e`）

rubevy の `docs/worklog/2026-09-16-bridge-followups.md` が、D1 の worklog の 2 つに答えていた。

**エンティティの手写し。** `prelude.rb` の `start_reflexes` は、`Task.current` から
`@rubevy_entity` を読んで、`Task.new` で作った reflex のタスクに `instance_variable_set` で
写していた。そうしないと reflex の `act` が「誰でもない者の質問」になってゲームに握りつぶされる。
rubevy が `Task.new` を包んで親の `@rubevy_entity` を子に写すようになったので、この 2 行は消えた。
消しただけで reflex は今までどおり動く（`--headless` で `reflex ... turned within 0.3 s` が 21/21）。
**消したあとに何も足さなくてよい**のが今回の要点で、D1 のときは「rubevy 側で `Task.new` が
引き継ぐか `Rubevy.adopt(task)` を出すのが本筋」と書いた。rubevy は前者を選んでいて、
その理由（`adopt` だと書き忘れると静かに壊れる、という穴が名前を変えて残るだけ）は
向こうの worklog に書いてある。こちらから見ると、**消せる行が増えたのではなく、
書き忘れうる行が無くなった**という違いになる。

**倒れた機体の reflex タスク。** D1 は `Task.list` を出すプローブのタスクで測って、
`3 blue/scout` が `DORMANT` になったあとも `Scout-hit` が `WAITING` のまま残ることを確かめていた。
外から `terminate` されたタスクは巻き戻らないので `ensure` が走らず、そもそも `Task.new` の
タスクは rubevy に見えていないので `terminate` すらされない。rubevy が
`ScriptWorld::unsubscribe` でキューを `close` するようになり、閉じた購読の `pop` は
`Rubevy::Unsubscribed` を上げる。reflex のループをその `rescue` で囲んで終わりにした。

```ruby
begin
  loop do
    args = queue.pop
    …
  end
rescue Rubevy::Unsubscribed
  Rubevy.log "#{bot.name}: reflex #{event} off"
  Rubevy.ask("reflex", :off)             # 待たない質問は命令
end
```

`rescue` を書かなくても例外はタスクの結果になって終わるのだが、それだと**終わったことが
どこにも出ない**。D1 が「残っている」と測ったのと同じ強さで「消えた」と言えるようにしたいので、
ログに 1 行出し、ゲームにも伝えることにした。伝える手は既にある `Rubevy.ask("reflex", …)` で、
`:begin` / `:end` と同じく**答えを pop しない**。倒れた機体のエンティティはまだ居る
（灰色になって残る）ので、`answer_requests` は今までどおりこの質問を受け取れる。

ゲーム側は `Robot::reflex_off` を数え、`--headless` の最後の行に `1/1 reflex tasks ended` を足した。
selftest の項目も 1 つ増やした。「倒れてから 0.5 秒以上たった機体は、登録した reflex の数だけ
タスクが終わっていること」——0.5 秒の猶予があるのは、最後の 1 体が倒れた瞬間に
`winner` が出て集計が走るため。実際の出力:

```
3 blue/scout hp    0  …  8 reflexes  1/1 reflex tasks ended
selftest: ok   the reflex tasks of every robot that went down ended (1/1)
selftest: ok   a reflex ran within 0.3 s of the hit (21/21)
```

あとで D2 のインスペクタが入ってから、同じ走りで**別の角度から**も見えるようになった。
スナップショットの「生きているコンテキストの数」が、4 体 + reflex 3 本 + マッチ + ルート = 9 から、
1 体が倒れたあと **7 live of 9** に落ちる。タスクが 1 本残るたびにコンテキストが 1 本残るので、
`Task.list` を出さなくてもこの 1 行で漏れが見える。

**`REFLEX_SLOTS` は残す。** これはエンティティの話ではなく、`instance_exec` が VM の入れ子の
実行ループになってその中で `pop` が park できない、という別の理由でそうなっている。
rubevy の 3 つは何も変えていないので、`docs/sabiruby-battle.md` の *What it cost to get right* に
「この制限は入れ子の実行ループだけの話で、他の 2 つは rubevy が直した」と書き足した。

## 1. D2 — 窓の中の VM インスペクタ

### VM に足りないもの（先に報告すること）

計画書は「`Vm::snapshot` をタスク単位で取れるようにする（`task_snapshot(task)`。無ければ VM に足す）」
と書いていた。**無い。** sabiruby `564434e` の `src/inspect.rs` にあるのは
`Vm::snapshot(regs_frames) -> Snapshot` だけで、これは **VM 全体**——`contexts` に並んだ
すべてのコンテキスト——を返す。タスクの側には `src/vm.rs:882-911` の
`task_value` / `task_instructions` / `task_location` / `task_frames` / `task_finished` があるが、
**この 2 つをつなぐものが無い**。`ScriptTask` から `ObjId` は取れても、それが
`Snapshot::contexts` のどれなのかを言う入り口がない。

つなぐ鍵は VM の中にはある。`ObjKind::Task(t)` は `t.ctx` を持っていて（`ext_task.rs:227` などが
それを読んでいる）、`create_task`（`ext_task.rs:617`）が `vm.contexts.push` した位置がそれ。
タスクのコンテキストは `fib = None`、`proc_ = Some(block)` で作られるので、`ContextView` に出ている
`fiber: Option<ObjId>` からは辿れない（`fib` が付くのは `Fiber` だけ）。`ContextView` は
`proc_` を持っていない。

sabiruby は触らない指示なので、**欲しいものを書いて残す**。

```rust
// src/inspect.rs、`Vm::snapshot` の隣（今の 181 行のすぐ下）
pub struct TaskSnapshot {
    pub task: ObjId,
    pub context: ContextView,
    /// そのコンテキストのフレームから辿れる環境だけ
    pub envs: Vec<EnvView>,
    pub heap: HeapView,
    pub instructions: u64,          // vm.instructions（VM 全体）
    pub task_instructions: u64,     // このタスクのぶん
}

/// 1 つのタスクが立っているコンテキストだけを、[`Vm::snapshot`] と同じ形で。
/// タスクが終わっている（`ctx == usize::MAX`）ときは `None`。
pub fn task_snapshot(&self, task: ObjId, regs_frames: usize) -> Option<TaskSnapshot>
```

実装は `snapshot` のループの中身——1 コンテキストぶんを組み立てるところ——を
`fn context_view(&self, i: usize, regs_frames: usize, envs: &mut Vec<EnvView>) -> ContextView`
に切り出せば、`snapshot` はそれを回すだけになり、`task_snapshot` は `ObjKind::Task(t).ctx` の
1 本だけを呼ぶ。切り出し以外に新しい処理は要らない。

**もっと小さい版でも足りる。** ホストが欲しいのは結局「このタスクはどのコンテキストか」だけで、

```rust
// src/vm.rs、`task_frames`（905 行）の隣
/// A task's context, as [`Vm::snapshot`] indexes them. `None` where it has ended.
pub fn task_context(&self, task: ObjId) -> Option<usize>
```

があれば、ホストは `snapshot(n).contexts[i]` を自分で選べる。6 行で、`no_std` にも
`Send + Sync` にも触らない。**どちらか一方でよく、小さいほうで困らない**——ただし
`snapshot` は毎回 VM 全体を作るので、コンテキストが数十本になるゲームでは `task_snapshot` のほうが
無駄がない。今のこのゲーム（9 本）では測れる差ではない。

### 無いまま作った（そして動いた）

指示は「無ければ止めて報告し、あるもので出来るところまでやる」だった。あるもので
**全部**できてしまった。`Vm::render` が Task をこう描くからである（`inspect.rs` の `render_text`）:

```rust
ObjKind::Task(t) => format!("#<Task {} ctx={}>", o.0, t.ctx),
```

`ctx` は**文字列としてなら公開されている**。`crates/rubevy-arena/src/inspect.rs` の
`task_context` はこれを読んでいる。これは API ではなくデバッグ用の書式なので、
読めなければ静かに諦める（フレーム無し、パネルに理由を出す）ように書いた。上の
`Vm::task_context` が入ったら、この関数は中身を差し替えるだけで消える。

見つけた経緯も書いておく。最初は `ContextView` から辿れないか（`fiber`、`is_current`、`prev`）を
順に当たり、どれも駄目だと分かってから「`inspect_str` でタスクを見たら何が出るのか」を
`src/inspect.rs` の `render_text` で確かめた。**探していたものが、探していた場所の
`Debug` 表示にだけ載っていた**という形になる。

### ファイル名はもう 1 つの API から取る

`FrameView` は `irep` / `pc` / `line` / `mid` / `target_class` / `regs` を持っているが、
**ソースのファイル名を持っていない**。`Vm::task_frames(task)` は `(file, line)` を内側から順に
返すが、フレームの他の中身を持っていない。2 つを合わせるには順番が合っている必要がある。

合っている。どちらも `ir.lines.is_empty()` のフレームを飛ばし（`ext_task.rs:250` と
`inspect.rs` の `line_of`）、どちらも内側から回る。つまり **`FrameView::line.is_some()` の
フレームだけを順に `task_frames` の要素と対応させればよい**。パネルはそう組み立てている
（`VmInspector::fill`）。飛ばされるのは mrblib——`Kernel#loop` や `Task::Queue#pop` ——で、
画面には `(no debug info)` と出る。デバッグ情報が無いから飛ばされている、という事実がそのまま
見えるので、隠さずそう書いた。

### 何を出しているか

`crates/rubevy-arena/src/inspect.rs`（ゲーム固有のものは何も入っていないので arena 側）:

* フレーム（内側から）: `robots/scout.rb:36` / `prelude.rb:67` / `(no debug info)`、メソッド名、`pc`。
  行番号は**著者のファイルの番号に直してある**（プログラムは prelude + 著者のファイルを 1 本に
  コンパイルしたものなので、`line - prelude_lines`）。
* 選んだフレームのレジスタ: `R0 self`、`R1..nlocals` は DBG/LVAR から取った名前つき
  （`target` / `threat` / `heading`）、型と値。値は `Vm::render` が Rust 側で描くので、
  見ること自体がプログラムを動かさない。
* ヒープ: 生存数 / 総数 / 空き、前回の収集からの割り当て量としきい値、収集の回数、
  収集直後の生存数。
* コンテキストの数（生きている / 全部）。
* このタスクが走らせた命令の総数と、直前のフレームぶん。

**既定で「ロボット自身のフレーム」を選ぶ。** 最初はいちばん内側のフレームを選んでいたが、
スキャンを待っている脳は `Task::Queue#pop` の中に立っていて、そこのローカルは
`non_block=false` と `**={}` である。ロボットの作者が見たいのは 3 つ外の `scout.rb:36` で、
そこに `target=#<Contact …>` `threat=nil` `heading=-0.59` が入っている。`follow` を既定 on にし、
行をクリックすると off になる（チェックボックスで戻せる）。これは**画面を見てから直した**もので、
最初の `--shot` はまさに `pop` のローカルを大写しにしていた。

`--shot` はもう 2 回撮り直している。1 回目は窓が画面の下にはみ出した（フレームの一覧が
伸びるだけ伸びて egui の窓を押し下げる）。フレームとレジスタをそれぞれ高さ固定の
`ScrollArea` に入れ、窓は左下に置いた（左上はスコアボード、右はエディタ）。
撮れた画から `docs/vm-inspector.png`。

### 一時停止

計画書どおり `ScriptWorld::budget = 0`。`Vm::task_run_limits` は
`if budget.is_some_and(|b| spent >= b) { return Ok(spent) }` を**ループの頭で**見るので
（`vm.rs:773`）、0 なら 1 命令も走らない。止まるのは Ruby だけで、ゲームは描き続け、
戦車は脳が最後に設定した舵で走り続ける。**スケジューラの時計は止まらない**ので、
再開した瞬間に寝ていたタスクが全部起きる。ここは rubevy の `tick_scripts` が
`task_advance_ticks` を毎フレーム呼んでいるところで、直すならそちら。今回は触らず、
`docs/sabiruby-battle.md` に書いた。

窓の selftest に 3 段足して、`P` を**実際に押して**確かめた（`ButtonInput::press`）。

```
selftest: ok   P pauses: the scripts' budget is 0
selftest: ok   nothing ran while it was paused
selftest: ok   the VM panel has the watched robot's frames
selftest: ok   the panel has the heap counters
selftest: ok   P again gives the budget back
selftest: ok   the brains are running again
```

**1 回落ちた。** 2 回目の `P` が効かない。`ButtonInput::press` は**既に押されているキーには
`just_pressed` を立てない**——本物のキーボードなら離すイベントが来るが、ここでは誰も離さない。
`release` してから `press` する。押しっぱなしのキーというものを、入力を合成して初めて意識した。

### ヘッドレスでも同じものを

窓が無いところでは `--headless` の最後にスナップショットをログに出す（計画書の「確認」）。

```
vm: 1 red/scout — context 2  Suspended — 83656 insn — contexts 9 live of 9 — heap live 3892 of 5000 …
vm:   #0 scout.rb:59            run              pc 326   target=nil  threat=nil  heading=-0.71  throttle=1.0
vm:   #1 (no debug info)        loop             pc 31    block=#<Proc irep=633 env=#894>  e=nil
vm:   #2 scout.rb:29            run              pc 33    (no locals)
vm:   #3 prelude.rb:228         run_robot        pc 85    tasks=[#<Task 887 ctx=4>]  klass=…  bot=…
```

`tasks=[#<Task 887 ctx=4>]` が、上に書いた `ctx=` そのもの。
