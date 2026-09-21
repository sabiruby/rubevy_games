//! **The words of Factory's in-game guide — the one file to edit.**
//!
//! Every string the `H` panel shows is here, English and Japanese side by side, and nothing else
//! is: the window, the keys that open it and the Japanese font are `games-shell`'s
//! ([`games_shell::guide`]). The garden's words are in `garden/src/guide_text.rs` and Battle's in
//! `sabibots/src/guide_text.rs`; the frame is shared and the words are not.
//!
//! **After editing the Japanese, re-cut the font**: `tools/subset-font.sh`. The font in the
//! binary carries only the characters these strings use, so a word with a character that was not
//! here before would be drawn as a blank box until the subset is made again.

use games_shell::Guide;

/// What the `H` panel says about the factory: what the three Ruby files are for, what an inserter
/// is, and — the part a first player actually needs — **how to build the first line**.
pub fn guide() -> Guide {
    Guide::new(
        "Factory — what you are looking at",
        "Factory — いま画面で起きていること",
    )
    .note(
        "Ore comes out of the ground, goes along belts, is smelted and assembled, and ends up in \
         a chest. Everything on the map is Rust: belts carry, miners dig, furnaces smelt. What is \
         Ruby is what the factory is made of, what it is for, and when each arm moves.",
        "鉱石を掘り、ベルトで運び、かまどと組立機で加工して、箱に納めるゲームです。\
         地図の上で動くものはすべて Rust です。ベルトが運び、採掘機が掘り、かまどが焼きます。\
         Ruby が受け持つのは、世界が何でできているか、何を目指すか、そして腕がいつ動くかです。",
    )
    .note(
        "Three Ruby files, and the editor shows all three. data.rb says what exists — the items, \
         the recipes, the machines, and the size of the map. control.rb says what the game is for \
         — it hears what the factory did and decides when you have won. inserter.rb is the one \
         every arm runs, and you can rewrite one arm or all of them.",
        "Ruby のファイルは 3 つで、エディタはその 3 つを切り替えられます。data.rb は\
         「何があるか」——アイテム・レシピ・機械・地図の大きさ。control.rb は「何を目指すか」——\
         工場の出来事を受け取り、勝ちを決めます。inserter.rb は腕が走らせるスクリプトで、\
         1 台だけ書き換えることも、全台に適用することもできます。",
    )
    .note(
        "An inserter is the only building here with a mind. It takes one thing from the tile \
         behind it and puts it in the tile in front, and when it does that is your Ruby: behind \
         tells you what is waiting, front_takes? whether it would be accepted, move swings the \
         arm and waits until it arrives. Nothing goes into or out of a machine any other way, so \
         a furnace with no arm beside it never gets fed.",
        "インサータ(腕)は、この世界で唯一「考える」建物です。後ろのタイルから 1 つ取り、\
         前のタイルに置きます。その「いつ」があなたの Ruby です。behind は後ろにあるもの、\
         front_takes? は前が受け取るかどうか、move は腕を振って着くまで待ちます。\
         機械への出入りは腕だけなので、腕を置かないかまどには何も入りません。",
    )
    .note(
        "Your first line: press 2 and click on ore for a miner, 1 and click a few tiles for a \
         belt running away from it (R turns what you are holding), then 3 for a chest at the end. \
         That line runs with no Ruby at all. To smelt, put a furnace (5) beside the belt and an \
         inserter (4) in the gap between them — and then click that inserter to write what it \
         does.",
        "最初の線の引き方。2 を押して鉱石の上をクリックすると採掘機、1 を押してそこから伸びる\
         タイルを何枚かクリックするとベルト(R で向きが変わります)、最後に 3 で箱。\
         ここまでは Ruby なしで動きます。焼くには、ベルトの隣に 5 でかまどを置き、\
         その間の 1 タイルに 4 でインサータを置いて、そのインサータをクリックして中身を書きます。",
    )
    .note(
        "Nothing is written to disk until you press Ctrl+S in the editor. Apply runs a text \
         without touching the file; F5 writes the whole factory down and F9 reads it back, and a \
         save taken against a different data.rb is refused rather than read into the wrong world.",
        "エディタで Ctrl+S を押すまで、ディスクのファイルは書き換わりません。Apply は\
         ファイルに触らずにそのテキストを走らせます。F5 で工場全体を保存し、F9 で読み戻します。\
         別の data.rb で保存したデータは、間違った世界に読み込まずに断ります。",
    )
    .key("H  ?", "this panel", "この説明")
    .key("1 – 3", "belt, miner, chest", "ベルト・採掘機・箱")
    .key("4", "inserter", "インサータ(腕)")
    .key("5 …", "the machines data.rb declares", "data.rb が宣言した機械")
    .key("0", "nothing in hand — click to take away", "手ぶら(クリックで撤去)")
    .key("R", "turn what you are holding", "手に持っているものの向きを変える")
    .key("click", "build, or open an inserter", "建てる/インサータを開く")
    .key("right-drag", "walk the map", "地図を動かす")
    .key("wheel", "zoom", "拡大縮小")
    .key("F1", "the editor", "エディタ")
    .key("F2", "the VM panel", "VM パネル")
    .key("F5  F9", "save, load", "セーブ・ロード")
    .key("Ctrl+Enter", "apply the text", "テキストを適用")
    .key("Ctrl+S", "write the file", "ファイルに書き出す")
    .key("P", "pause the whole factory", "工場ごと一時停止")
}
