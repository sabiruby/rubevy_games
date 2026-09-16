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

/// What the `H` panel says about the Battle: two teams, energy, the reflex task, and an editor
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
        "Every robot's brain is a Ruby script running in SabiRuby, a VM written in Rust. The \
         rules of the game are all on the Rust side: a brain asks questions and gives orders \
         through one string-shaped door and never touches the world itself.",
        "各機の頭脳は Ruby のスクリプトで、Rust で書かれた VM(SabiRuby)の上で動いています。\
         ゲームの規則はすべて Rust 側にあり、頭脳は文字列の窓口越しに質問と指示を出すだけで、\
         世界には直接触れません。",
    )
    .note(
        "A robot can run a second task as a reflex: it waits on something happening to it — being \
         hit — and answers in the same frame, while the main brain goes on thinking about \
         something else. The panel says which of its tasks is running.",
        "ロボットは「反射」としてもう 1 つのタスクを走らせられます。撃たれたなどの出来事を待ち、\
         本体が別のことを考えている間に、同じフレームで反応します。どのタスクが動いているかはパネルに出ます。",
    )
    .note(
        "Pick a robot and the editor opens on its brain. Change the text and press Ctrl+Enter or \
         F5: that robot is handed the new brain in the middle of the fight, and the file on disk \
         is left alone until you press Ctrl+S.",
        "ロボットを選ぶと、その頭脳がエディタに出ます。書き換えて Ctrl+Enter か F5 を押すと、\
         戦いの途中でその機だけが新しい頭脳に入れ替わります。\
         ディスクのファイルは Ctrl+S を押すまで書き換わりません。",
    )
    .key("H  ?", "this panel", "この説明")
    .key("1 – 8", "pick a robot", "ロボットを選ぶ")
    .key("click a name", "the same, from the scoreboard", "同じこと(スコアボードから)")
    .key("Tab", "the next robot", "次のロボット")
    .key("F1", "the editor", "エディタ")
    .key("F2", "the VM panel: frames, registers, heap", "VM パネル(フレーム・レジスタ・ヒープ)")
    .key("Ctrl+Enter  F5", "apply the edited brain to this robot", "編集した頭脳をこの機に適用")
    .key("Ctrl+S", "write the brain to its file", "頭脳をファイルに保存")
    .key("P", "pause the scripts; the arena goes on being drawn", "スクリプトを一時停止(闘技場は描かれ続ける)")
    .key("R", "a new match, keeping the brains applied so far", "適用済みの頭脳のまま試合をやり直す")
}

