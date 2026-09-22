//! **The words of Factory's in-game guide and its palette — the one file to edit.**
//!
//! Every string the `H` panel shows is here, English and Japanese side by side: the window, the
//! keys that open it and the Japanese font are `games-shell`'s ([`games_shell::guide`]). The
//! garden's words are in `garden/src/guide_text.rs` and Battle's in `sabibots/src/guide_text.rs`;
//! the frame is shared and the words are not.
//!
//! **F7's palette says its words from here too** ([`Say`]), and that is not tidiness: the font in
//! the binary is cut to the characters `tools/subset-font.sh` finds, and the four files it reads
//! are the three games' `guide_text.rs` and the shared `guide.rs`. A Japanese word written
//! anywhere else in this crate is drawn as a blank box. So this file is "Factory's Japanese",
//! not only "Factory's guide".
//!
//! **After editing the Japanese, re-cut the font**: `tools/subset-font.sh`. The font in the
//! binary carries only the characters these strings use, so a word with a character that was not
//! here before would be drawn as a blank box until the subset is made again.

use games_shell::guide::GuideLang;
use games_shell::Guide;

use crate::data::Data;
use crate::grid::{Dir, What};

/// **One thing to say, in both languages** — the shape [`games_shell::GuideNote`] has, for the
/// words that are not in the guide at all (F7's palette, the line under it).
#[derive(Debug, Clone, Copy)]
pub struct Say {
    pub en: &'static str,
    pub ja: &'static str,
}

impl Say {
    pub const fn new(en: &'static str, ja: &'static str) -> Say {
        Say { en, ja }
    }

    pub fn of(&self, lang: GuideLang) -> &'static str {
        match lang {
            GuideLang::En => self.en,
            GuideLang::Ja => self.ja,
        }
    }
}

/// The palette's own words (F7): the window, the row of zoom buttons, and the line that says
/// which way what is in hand is facing.
pub const PALETTE_TITLE: Say = Say::new("Build", "設置");
pub const PALETTE_HINT: Say = Say::new(
    "Click one, or press its key. Then click the map.",
    "クリックするか、キーを押して手に持ちます。そのあと地図をクリック。",
);
pub const FACING: Say = Say::new("facing", "向き");
pub const TURN_IT: Say = Say::new("R turns it", "R で向きを変える");
pub const ZOOM: Say = Say::new("Zoom", "拡大縮小");
pub const WHOLE_MAP: Say = Say::new("Whole map", "地図全体");
pub const BACK_HOME: Say = Say::new("Home", "はじめの見え方");
pub const NEXT_STEP: Say = Say::new("Next", "次の 1 手");

/// **What to build next**, one step at a time (F7): the sentences, with the key to press and the
/// name of the thing left as `{key}` and `{name}` so that a `data.rb` which declares a different
/// machine says its own name.
pub const STEP_MINER: Say = Say::new(
    "{key}: put a {name} on a patch of ore",
    "{key}: まず鉱石の上に{name}を置く",
);
pub const STEP_BELT: Say = Say::new(
    "{key}: a {name} leading away from it — R turns it",
    "{key}: そこから伸びる{name}(R で向き)",
);
pub const STEP_CHEST: Say = Say::new(
    "{key}: a {name} at the end of the line",
    "{key}: 線の終わりに{name}",
);
pub const STEP_MACHINE: Say = Say::new(
    "{key}: a {name} beside the belt",
    "{key}: ベルトの隣に{name}",
);
pub const STEP_INSERTER: Say = Say::new(
    "{key}: an {name} between the belt and the machine — click it to write what it does",
    "{key}: ベルトと機械の間に{name}。クリックで中身を書く",
);

/// **The four fittings, in Japanese.** A machine is called whatever `data.rb` called it — that is
/// the only name a player has for one ([`crate::build::word_for`]) — but the belt, the miner, the
/// chest and the arm are this game's own and the guide has been calling them these all along.
const FITTINGS: [(&str, Say); 5] = [
    ("belt", Say::new("belt", "ベルト")),
    ("miner", Say::new("miner", "採掘機")),
    ("chest", Say::new("chest", "箱")),
    ("inserter", Say::new("inserter", "インサータ")),
    ("the wrecking ball", Say::new("the wrecking ball", "撤去")),
];

/// What to call what is in hand, in the language the guide is showing.
pub fn word_in(lang: GuideLang, what: Option<What>, data: &Data) -> String {
    let english = crate::build::word_for(what, data);
    match lang {
        GuideLang::En => english,
        GuideLang::Ja => FITTINGS
            .iter()
            .find(|(en, _)| *en == english)
            .map(|(_, say)| say.ja.to_string())
            .unwrap_or(english),
    }
}

/// **Which way, as a word and as an arrow.** The arrows are here rather than in the palette for
/// the reason the Japanese is: they are characters, and the font is cut to this file.
pub fn way_round(lang: GuideLang, dir: Dir) -> String {
    let arrow = match dir {
        Dir::East => "→",
        Dir::North => "↑",
        Dir::West => "←",
        Dir::South => "↓",
    };
    let word = match (lang, dir) {
        (GuideLang::En, _) => dir.word(),
        (GuideLang::Ja, Dir::East) => "東",
        (GuideLang::Ja, Dir::North) => "北",
        (GuideLang::Ja, Dir::West) => "西",
        (GuideLang::Ja, Dir::South) => "南",
    };
    format!("{arrow} {word}")
}

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
        "Your first line: the Build panel lists everything you can put down, with the key beside \
         it. Click one — or press the key — and the tile under the mouse shows what you are about \
         to build, red where it cannot go. The panel's bottom line says the one thing to do next, \
         all the way from the first miner to the goal.",
        "最初の線の引き方。「設置」の窓に、置けるものが全部キーつきで並んでいます。\
         クリックするかキーを押すと手に持ち、マウスのあるタイルにこれから建つものが出ます\
         (置けない所は赤)。窓の下の 1 行が「次にやること」を 1 つだけ言います。\
         最初の採掘機から目標まで、それに従えば線が引けます。",
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
    .key("Build", "the same list, with pictures", "「設置」の窓。同じ一覧を絵で")
    .key("right-drag", "walk the map", "地図を動かす")
    .key("wheel", "zoom where the mouse is", "マウスの位置を中心に拡大縮小")
    .key("+  -", "zoom in, zoom out", "拡大・縮小")
    .key("Home", "back to the first view", "はじめの見え方に戻る")
    .key("F1", "the editor", "エディタ")
    .key("F2", "the VM panel", "VM パネル")
    .key("F5  F9", "save, load", "セーブ・ロード")
    .key("Ctrl+Enter", "apply the text", "テキストを適用")
    .key("Ctrl+S", "write the file", "ファイルに書き出す")
    .key("P", "pause the whole factory", "工場ごと一時停止")
}
