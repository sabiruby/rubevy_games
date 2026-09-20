//! **The command line, as both games read it.**
//!
//! Neither game has a parser and neither needs one: what they take is a handful of flags, some
//! with a word or a number after them, and both had written the same four `args.iter().position`
//! chains. They are here so the third game inherits them rather than writing them a third time —
//! with the numbers left outside. `--shot` waits six seconds in the garden and three in the
//! Battle, and a default that lives here would have had to choose one of them; every default is
//! the caller's argument, so moving these lines changed nothing a run does.
//!
//! A page has no command line. `std::env::args()` answers nothing there, every flag is absent,
//! and the browser build takes the branch a bare `cargo run` takes.

/// The words this run was started with, `argv[0]` and all.
pub struct Args {
    words: Vec<String>,
}

impl Args {
    /// What the run was started with.
    pub fn from_env() -> Args {
        Args { words: std::env::args().collect() }
    }

    /// A line made up, for a test.
    pub fn of<I: IntoIterator<Item = S>, S: Into<String>>(words: I) -> Args {
        Args { words: words.into_iter().map(Into::into).collect() }
    }

    /// The whole line, for a game that wants to say something about it.
    pub fn words(&self) -> &[String] {
        &self.words
    }

    /// Whether a flag with nothing after it is there: `--vm`, `--guide`.
    pub fn has(&self, flag: &str) -> bool {
        self.words.iter().any(|a| a == flag)
    }

    /// The word after a flag: `--lang ja`, `--save g.json`. `None` if the flag is not there or is
    /// the last word on the line.
    pub fn value(&self, flag: &str) -> Option<String> {
        self.words.iter().position(|a| a == flag).and_then(|i| self.words.get(i + 1).cloned())
    }

    /// The number after a flag: `--eye 18`. `None` if it is not there or is not a number.
    pub fn number(&self, flag: &str) -> Option<f32> {
        self.value(flag).and_then(|s| s.parse::<f32>().ok())
    }

    /// **`--headless [SECONDS]`**: no window, the game reported on stdout when the time is up.
    /// `Some` whenever the flag is there — a `--headless` with no number is still a headless run,
    /// and `default_seconds` is how long it lasts.
    pub fn headless(&self, default_seconds: f32) -> Option<f32> {
        self.has("--headless").then(|| self.number("--headless").unwrap_or(default_seconds))
    }

    /// **`--shot [FILE] [SECONDS]`**: a window, one picture of it, and out. Both are the
    /// caller's defaults, because the two games wait different lengths for theirs.
    pub fn shot(&self, default_file: &str, default_seconds: f32) -> Option<(String, f32)> {
        let i = self.words.iter().position(|a| a == "--shot")?;
        Some((
            self.words.get(i + 1).cloned().unwrap_or_else(|| default_file.to_string()),
            self.words.get(i + 2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(default_seconds),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flag_a_word_and_a_number() {
        let args = Args::of(["garden", "--vm", "--lang", "ja", "--eye", "18", "--save"]);
        assert!(args.has("--vm"));
        assert!(!args.has("--guide"));
        assert_eq!(args.value("--lang").as_deref(), Some("ja"));
        assert_eq!(args.number("--eye"), Some(18.0));
        assert_eq!(args.value("--save"), None, "a flag at the end of the line has no word");
        assert_eq!(args.number("--lang"), None, "a word that is not a number is not one");
    }

    /// The three shapes `--headless` comes in, which is what the games' own lines did before this
    /// module: the flag alone, the flag with a number, and the flag with something else after it.
    #[test]
    fn headless_with_and_without_its_number() {
        assert_eq!(Args::of(["garden"]).headless(10.0), None);
        assert_eq!(Args::of(["garden", "--headless"]).headless(10.0), Some(10.0));
        assert_eq!(Args::of(["garden", "--headless", "90"]).headless(10.0), Some(90.0));
        assert_eq!(
            Args::of(["garden", "--headless", "--save", "g.json"]).headless(10.0),
            Some(10.0),
            "a flag after it is not its number"
        );
    }

    /// **The two games' `--shot` defaults differ** (six seconds and three), so the default is the
    /// caller's and this module holds no number of its own.
    #[test]
    fn a_shot_takes_the_callers_defaults() {
        assert_eq!(Args::of(["garden"]).shot("shot.png", 6.0), None);
        assert_eq!(
            Args::of(["garden", "--shot"]).shot("shot.png", 6.0),
            Some(("shot.png".to_string(), 6.0))
        );
        assert_eq!(
            Args::of(["sabibots", "--shot"]).shot("shot.png", 3.0),
            Some(("shot.png".to_string(), 3.0))
        );
        assert_eq!(
            Args::of(["garden", "--shot", "n.png", "12", "--at", "midnight"]).shot("shot.png", 6.0),
            Some(("n.png".to_string(), 12.0))
        );
        assert_eq!(
            Args::of(["garden", "--shot", "g.png", "--guide"]).shot("shot.png", 6.0),
            Some(("g.png".to_string(), 6.0)),
            "a flag where the seconds would be is not the seconds"
        );
    }
}
