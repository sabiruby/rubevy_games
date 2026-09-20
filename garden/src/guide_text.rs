//! **The words of the in-game guide (G6) — the one file to edit.**
//!
//! Every string the `H` panel shows for the garden is here, English and Japanese side by side,
//! and nothing else is: the window it is drawn in, the keys that open it and the Japanese font
//! are `games-shell`'s ([`games_shell::guide`]). Editing the wording means editing this file
//! and nothing else — that is what it is for.
//!
//! **After editing the Japanese, re-cut the font**: `tools/subset-font.sh`. The font in the
//! binary carries only the characters these strings use, so a word with a character that was not
//! here before would be drawn as a blank box until the subset is made again. The script reads
//! this file, `sabibots/src/guide_text.rs` and `crates/games-shell/src/guide.rs`, and writes
//! `crates/games-shell/assets/fonts/NotoSansJP-Guide.subset.ttf`. `docs/garden.md` says so too.
//!
//! The author played the browser build knowing what every key did, and still wrote down "there is
//! no explanation in the game" — which is the one complaint a reader of `docs/garden.md` can
//! never make on the author's behalf. So the four paragraphs are not a key list in prose: they
//! are what a person who has just opened the page is actually looking at (grass, hunger, a day
//! that turns over in a minute, creatures pairing off), and *then* the fact that the behaviours
//! are Ruby and that the editor rewrites them while the garden runs, which is the whole point of the
//! game and the one thing a player would never guess.

use games_shell::Guide;

/// What the `H` panel says about the garden.
pub fn guide() -> Guide {
    Guide::new(
        "Garden — how to play",
        "箱庭 — 遊び方の説明",
    )
    .note(
        "Grass grows by itself and the creatures eat it. A beetle or a rabbit that finds nothing \
         to eat starves. One that has eaten enough goes looking for a mate, and a pair passes its \
         genes — speed, sight, appetite — to its young.",
        "草は自動で育ち、生き物がそれを食べます。何も食べられなかった虫やウサギは餓死してしまいます。\
         十分に食べた個体はつがいになる相手を探します。つがいになると、速さ・視野・食欲の遺伝子が子に渡ります。",
    )
    .note(
        "The sun goes round once a minute. When it sets, the creatures stop and sleep. How a \
         creature's behaviour changes with its surroundings is written in its behaviour script.",
        "太陽は 1 分で 1 周します。日が沈むと、生き物は動きを止めて眠ります。\
         生き物の行動アルゴリズムのスクリプトで、環境の変化による挙動の変化が実装されています。",
    )
    .note(
        "Every creature's behaviour is a Ruby script. It runs on SabiRuby, a VM written in Rust, \
         and it can read the game's Bevy ECS components — me[:Hunger], me[:Velocity].",
        "生き物の行動アルゴリズムは Ruby のスクリプトです。Rust で書かれた VM(SabiRuby)の上で動いています。\
         bevyのECS のコンポーネントを読むことができます(me[:Hunger]、me[:Velocity])。",
    )
    // G6b. The one paragraph added after the author's second play: the panel described what a
    // creature does and never named the thing that makes it answer at once. `on` is the
    // construct a reader will meet first in `beetle.rb`, and `docs/garden.md` explains it at
    // length; this is the sentence that says it is there at all. G7 gave it the three points the
    // plan asks for; the author's rewrite (2026-09-17) kept two — a task of its own beside `run`,
    // and `sleep` in it not stopping `run` — and dropped the one about holding the controls.
    .note(
        "A creature handles events as well. on(:touched) { ... } is a block that waits for an \
         event — being touched by another creature, night falling, a meal starting — and it runs \
         as a task of its own, apart from the behaviour's run. Because it is a separate task, a \
         sleep inside on does not stop run.",
        "生き物はイベントの処理も実行しています。on(:touched) { ... } は、\
         他の生き物に触られた、夜になった、食事が始まった、というイベントを待ち受けるブロックで、\
         行動アルゴリズムの run とは別のタスクとして、実行されます。\
         別のタスクなので、onの中で sleep を実行しても run は止まりません。",
    )
    .note(
        "Click a creature and its program opens in the editor. Change the text and press \
         Ctrl+Enter: every creature of that species is handed the new behaviour while the game keeps \
         running. The file on disk is left alone until you press Ctrl+S.",
        "生き物をクリックすると、そのプログラムがエディタに表示されます。内容を書き換えて Ctrl+Enter を押すと、\
         ゲームを止めないまま、その種の全個体が新しい行動アルゴリズムに入れ替わります。\
         ディスク上のファイルは Ctrl+S を押すまで書き換わりません。",
    )
    // W3. The rules of the world are a Ruby file too, and `F3` opens it. The paragraph is short
    // on purpose: what it has to say is that `world.rb` exists and that editing it is the same
    // gesture as editing a creature's file, which the paragraph above has just described.
    .note(
        "The rules of the world are Ruby as well. Press F3 and world.rb opens in the editor — how \
         the grass grows, how fast hunger falls, when two creatures pair off. Rewrite it, press \
         Ctrl+Enter, and the garden goes on running under the new rules without stopping.",
        "世界の規則も Ruby です。F3 を押すと world.rb がエディタに開きます。\
         草の育ち方、腹の減る速さ、つがいになる条件などが書かれています。\
         書き換えて Ctrl+Enter を押すと、箱庭を止めないまま新しい規則で動き続けます。",
    )
    .key("H  ?", "this panel", "この説明")
    .key("click", "look at a creature", "生き物を選ぶ")
    .key("Tab", "the next creature", "次の生き物")
    .key("F1", "the editor", "エディタ")
    .key("F2", "look inside the VM", "VM の状態を見る")
    .key("F3", "the rules of the world (world.rb)", "世界の規則(world.rb)")
    .key("Ctrl+Enter", "apply the edited behaviour", "編集した行動アルゴリズムを適用")
    .key("Ctrl+S", "write the behaviour to its file", "行動アルゴリズムをファイルに保存")
    .key("P", "pause the world", "世界の一時停止")
    .key("F5  F9", "save the state / read it back", "状態を保存 / 読み込み")
    .key("drag", "turn the camera round the garden", "視点を回す")
    .key("right-drag  Shift+drag", "slide the view over the field", "視点をスライド")
    .key("W A S D  arrows", "slide the view", "視点をスライド")
    .key("wheel", "zoom in / out", "ズームイン / アウト")
    .key("Home", "put the camera back where it started", "視点を最初の位置に戻す")
}
