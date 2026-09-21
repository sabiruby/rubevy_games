# The inserter every inserter starts with.
#
# **This is the file the game is about.** Everything else in the factory is Rust — the belts, the
# miners, the furnaces — and nothing goes into or out of a machine except through one of these. So
# a line that ends at a furnace ends there until you put an inserter beside it, and what the
# inserter does is this file.
#
# Click one in the game to edit it. `Apply` runs what you have typed in that one arm; `Apply to
# every inserter on this script` runs it in all of them. The words are in `ruby/prelude.rb`.
#
# Ideas, in the order they get interesting:
#
#   * `wants?` below is asked about everything the arm can see. Make it `item == :iron_ore` and
#     this arm feeds a furnace and lets the plates go past.
#   * an arm can watch two things: `behind` is what is waiting, `holding` is what it is carrying.
#     A `move` that answers false is an arm holding something with nowhere to put it.
#   * nothing says the loop has to look every time. An arm that knows its furnace takes two
#     seconds over a plate could `sleep 2.0` and skip the looking.

inserter "Smart" do
  def run
    loop do
      # **read, decide, move** — in that order, because the first two are answered out of the
      # world as it stands and the arm moves it a moment later
      item = behind
      if item && wants?(item) && front_takes?(item)
        move
      else
        idle
      end
    end
  end

  # What this arm is willing to pick up. Everything, to begin with.
  def wants?(item)
    true
  end
end
