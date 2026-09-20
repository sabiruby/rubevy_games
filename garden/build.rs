//! The browser build has no directory to read Ruby from, so every `.rb` under `ruby/` is put into
//! the binary: `OUT_DIR/ruby_files.rs` is a table of `("ruby/creatures/beetle.rb", include_str!(…))`,
//! which the browser half of `src/platform.rs` includes. The PC build reads the files themselves
//! (and reloads them when they change) and hands an empty table around in the table's place.
//!
//! The walk itself is `rubevy-build`'s: this file and sabibots' were the same 35 lines to the
//! byte, and rubevy's R6 made them one. The defaults are what those 35 lines wrote — `.rb` files,
//! a `pub static RUBY_FILES`, in `ruby_files.rs` — so the `include!` on the other side is
//! unchanged.

fn main() {
    rubevy_build::Embed::new("ruby").write();
}
