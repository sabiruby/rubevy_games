//! The browser build has no directory to read Ruby from, so every `.rb` under `ruby/` is put into
//! the binary: `OUT_DIR/ruby_files.rs` is a table of `("ruby/prelude.rb", include_str!(…))`, which
//! the browser half of `src/platform.rs` includes. The PC build reads the files themselves and
//! hands an empty table around in the table's place.
//!
//! This is the same one line the other two games have — the walk is `rubevy-build`'s since R6 —
//! and it is here from F0 rather than from the stage that first writes a `.rb`, so that the
//! browser build is never a build that quietly carries no scripts.

fn main() {
    rubevy_build::Embed::new("ruby").write();
}
