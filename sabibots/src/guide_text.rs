//! **The words of the in-game guide (G6) — the one file to edit.**
//!
//! Every string the `H` panel shows for SabiRuby Battle is here, English and Japanese side by
//! side, and nothing else is: the window, the keys that open it and the Japanese font are
//! `rubevy-arena`'s ([`rubevy_arena::guide`]). The garden's words are in
//! `garden/src/guide_text.rs`; the frame is shared and the words are not, because what a battle
//! is and what a garden is have nothing in common.
//!
//! **After editing the Japanese, re-cut the font**: `tools/subset-font.sh`. The font in the
//! binary carries only the characters these strings use, so a word with a character that was not
//! here before would be drawn as a blank box until the subset is made again.

use rubevy_arena::Guide;

/// What the `H` panel says about the Battle: two teams, energy, the handler task, and an editor
/// whose unit is one robot rather than a whole species.
pub fn guide() -> Guide {
    Guide::new(
        "SabiRuby Battle — what you are looking at",
        "SabiRuby Battle — いま画面で起きていること",
    )
    .note(
        "Eight robots in two teams fight until one team is left standing. Each has health and \
         energy: moving and firing spend energy and it comes back slowly, so a robot that never \
         stops has nothing left to shoot with.",
        "2 チーム 8 体のロボットが、片方のチームだけが残るまで戦います。各機には体力とエネルギーがあり、\
         移動と射撃でエネルギーを使い、少しずつ回復します。動き続ける機は撃つ分が残りません。",
    )
    .note(
        "Every robot's behaviour is a Ruby script running in SabiRuby, a VM written in Rust. The \
         rules of the game are all on the Rust side: a behaviour asks questions and gives orders \
         through one string-shaped door and never touches the world itself.",
        "各機の行動アルゴリズムは Ruby のスクリプトで、Rust で書かれた VM(SabiRuby)の上で動いています。\
         ゲームの規則はすべて Rust 側にあり、行動アルゴリズムは文字列の窓口越しに質問と指示を出すだけで、\
         世界には直接触れません。",
    )
    .note(
        "A robot can run handlers beside its behaviour. on(:hit) { ... } is a block with a task \
         of its own, beside the behaviour's run: it waits on one thing happening to it — being \
         hit — and fires the moment that arrives, while run goes on thinking about something \
         else. A sleep inside the block does not stop run, and a handler holds the controls for \
         as long as it is acting. The panel says which of its tasks is running.",
        "ロボットは行動アルゴリズムとは別に、イベントの処理も走らせられます。on(:hit) { ... } は、\
         撃たれたなどの出来事を待ち受けるブロックで、行動アルゴリズムの run とは別のタスクとして、\
         出来事が届いた瞬間に走ります。中で sleep しても run は止まりません。\
         動いている間は操作を預かります。どのタスクが動いているかはパネルに出ます。",
    )
    .note(
        "Pick a robot and the editor opens on its behaviour. Change the text and press \
         Ctrl+Enter or F5: that robot is handed the new behaviour in the middle of the fight, and \
         the file on disk is left alone until you press Ctrl+S.",
        "ロボットを選ぶと、その行動アルゴリズムがエディタに出ます。書き換えて Ctrl+Enter か F5 を押すと、\
         戦いの途中でその機だけが新しい行動アルゴリズムに入れ替わります。\
         ディスクのファイルは Ctrl+S を押すまで書き換わりません。",
    )
    .key("H  ?", "this panel", "この説明")
    .key("1 – 8", "pick a robot", "ロボットを選ぶ")
    .key("click a name", "the same, from the scoreboard", "同じこと(スコアボードから)")
    .key("Tab", "the next robot", "次のロボット")
    .key("F1", "the editor", "エディタ")
    .key("F2", "look inside the VM — which line each behaviour is waiting on", "VM の中を見る(どの行で何を待っているか)")
    .key("Ctrl+Enter  F5", "apply the edited behaviour", "編集した行動アルゴリズムを適用")
    .key("Ctrl+S", "write the behaviour to its file", "行動アルゴリズムをファイルに保存")
    .key("P", "stop the match — the rules too; it goes on being drawn", "試合ごと一時停止(規則も止まる。描画は続く)")
    .key("R", "a new match, keeping the behaviours applied so far", "適用済みの行動アルゴリズムのまま試合をやり直す")
}

