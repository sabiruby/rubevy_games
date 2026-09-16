//! **The words of the in-game guide (G6) — the one file to edit.**
//!
//! Every string the `H` panel shows for the garden is here, English and Japanese side by side,
//! and nothing else is: the window it is drawn in, the keys that open it and the Japanese font
//! are `rubevy-arena`'s ([`rubevy_arena::guide`]). Editing the wording means editing this file
//! and nothing else — that is what it is for.
//!
//! **After editing the Japanese, re-cut the font**: `tools/subset-font.sh`. The font in the
//! binary carries only the characters these strings use, so a word with a character that was not
//! here before would be drawn as a blank box until the subset is made again. The script reads
//! this file, `sabibots/src/guide_text.rs` and `crates/rubevy-arena/src/guide.rs`, and writes
//! `crates/rubevy-arena/assets/fonts/NotoSansJP-Guide.subset.ttf`. `docs/garden.md` says so too.
//!
//! The author played the browser build knowing what every key did, and still wrote down "there is
//! no explanation in the game" — which is the one complaint a reader of `docs/garden.md` can
//! never make on the author's behalf. So the four paragraphs are not a key list in prose: they
//! are what a person who has just opened the page is actually looking at (grass, hunger, a day
//! that turns over in a minute, creatures pairing off), and *then* the fact that the minds are
//! Ruby and that the editor rewrites them while the garden runs, which is the whole point of the
//! game and the one thing a player would never guess.

use rubevy_arena::Guide;

/// What the `H` panel says about the garden.
pub fn guide() -> Guide {
    Guide::new(
        "Garden — what you are looking at",
        "箱庭 — いま画面で起きていること",
    )
    .note(
        "Grass grows by itself and the creatures eat it. A beetle or a rabbit that finds nothing \
         starves; one that has eaten enough goes looking for a mate, and a pair passes a mixture \
         of both genomes — speed, sight, appetite — to its young.",
        "草はひとりでに育ち、生き物がそれを食べます。何も食べられなかった甲虫やウサギは餓死し、\
         十分に食べた個体は相手を探します。つがいになると、速さ・視野・食欲を混ぜた遺伝子が子に渡ります。",
    )
    .note(
        "The sun goes round once a minute. When it sets the creatures stop and sleep — each one \
         is a Ruby task waiting for the world to publish \"night\" — and the moon is left to show \
         their outlines.",
        "太陽は 1 分で 1 周します。日が沈むと生き物は動きを止めて眠ります。\
         これは Ruby のタスクが世界からの「夜」の知らせを待っているからで、あとは月明かりが輪郭を見せます。",
    )
    .note(
        "Every creature's mind is a Ruby script running in SabiRuby, a VM written in Rust. It \
         reads the world through the game's ECS components by name — me[:Hunger], me[:Velocity] \
         — and the game has no glue code per component at all.",
        "生き物の頭脳は Ruby のスクリプトで、Rust で書かれた VM(SabiRuby)の上で動いています。\
         世界は ECS のコンポーネントを名前で読みます(me[:Hunger]、me[:Velocity])。\
         コンポーネントごとの接着コードはゲーム側に 1 行もありません。",
    )
    // G6b. The one paragraph added after the author's second play: the panel described what a
    // creature does and never named the thing that makes it answer at once. `reflex` is the
    // construct a reader will meet first in `beetle.rb`, and `docs/garden.md` explains it at
    // length; this is the sentence that says it is there at all.
    .note(
        "Besides that loop, a creature runs reflexes. reflex(:touched) { ... } is a block that \
         waits for one thing to happen to it — a rabbit walking over it, night falling, a meal \
         starting — and runs in a task of its own the moment it does, while the loop goes on \
         thinking. Whichever task writes to the body last is what the body does, so a reflex \
         holds the controls for as long as it is acting.",
        "その繰り返しとは別に、生き物は「反射」も走らせています。reflex(:touched) { ... } は、\
         自分の身に起きる出来事 — ウサギに触られた、夜になった、食事が始まった — を待ち受けるブロックで、\
         出来事が届いた瞬間に専用のタスクとして走ります。その間も繰り返しは考え続けています。\
         体に最後に書き込んだタスクが勝つので、反射は動いている間だけ操作を預かります。",
    )
    .note(
        "Click a creature and the editor opens on its species' file. Change the text and press \
         Ctrl+Enter: every creature of that species is handed the new mind while the garden keeps \
         running, and the file on disk is left alone until you press Ctrl+S.",
        "生き物をクリックすると、その種のファイルがエディタに出ます。書き換えて Ctrl+Enter を押すと、\
         庭を止めないまま、その種の全個体が新しい頭脳に入れ替わります。\
         ディスクのファイルは Ctrl+S を押すまで書き換わりません。",
    )
    .key("H  ?", "this panel", "この説明")
    .key("click", "look at a creature", "生き物を選ぶ")
    .key("Tab", "the next creature", "次の生き物")
    .key("F1", "the editor", "エディタ")
    .key("F2", "the VM panel: frames, registers, heap", "VM パネル(フレーム・レジスタ・ヒープ)")
    .key("Ctrl+Enter", "apply the edited mind to the species", "編集した頭脳を種の全個体に適用")
    .key("Ctrl+S", "write the mind to its file", "頭脳をファイルに保存")
    .key("P", "pause the scripts; the world goes on being drawn", "スクリプトを一時停止(世界は描かれ続ける)")
    .key("F5  F9", "save the garden / read it back", "庭を保存 / 読み込み")
    .key("drag", "turn the camera round the garden", "視点を回す")
    .key("right-drag  Shift+drag", "slide the view over the field", "視点をスライド")
    .key("W A S D  arrows", "slide the view", "視点をスライド")
    .key("wheel", "closer / further away, a tenth at a notch", "ズーム(1 ノッチで 10%)")
    .key("Home", "put the camera back where it started", "視点を最初の位置に戻す")
}
