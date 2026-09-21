# Factory — the DSL an inserter is written in.
#
# This file is put in front of an inserter's own script, so `inserter "Smart" do … end` is all one
# of those needs (`factory/src/inserters.rs`, which is also where the block of Ruby the *game*
# writes from `ruby/data.rb` goes — the item names and how long a swing takes).
#
# **An inserter is the one machine in this game with a mind.** Everything else is Rust: the belts
# carry, the miners dig, the furnaces smelt, and none of it asks anybody anything. An inserter
# takes one thing from the tile behind it and puts it in the tile in front, and *when* it does
# that is a line of Ruby. Nothing goes into or out of a machine any other way, so a factory with
# no inserters in it is a factory that does not run (`factory/src/machines.rs`).
#
# **The shape is the garden's**: `def run`, a loop, and methods on an object of your own. What is
# different is that there are no components here — an inserter has no `Transform` to read and
# nothing to write — so every word below is either a question the game answers or a thing it does.
#
#   behind          what is waiting on the tile behind, as a Symbol, or nil
#   holding         what is in the hand, as a Symbol, or nil
#   front_takes?(x) whether the tile in front would take that
#   move            swing the arm: pick up what is behind and put it in front. **Waits.**
#   idle            wait one swing's worth, and let everyone else have the frame
#   swing_seconds   how long a swing takes (`inserter :arm, seconds_per_item:` in data.rb)
#   items           every item name `data.rb` declares
#   log "…"         a line in the game's log, with this inserter's name in front
#
# **The three questions cost no frame.** `behind`, `holding` and `front_takes?` are answered
# inside the tick that is running this script (rubevy's `answer_in_tick`), so the value is there
# in the line that asked for it. `move` is the one that waits, and it waits for as long as the arm
# takes; `idle` waits because waiting is what it is for.
#
# **Read, decide, then move.** The three questions are about the world as it stands *now*, and the
# swing `move` starts happens in the same frame, before anything else has moved — so what `behind`
# said is what `move` picks up. Do not keep an answer across an `idle` or a `move`: the belts run
# in between.

# ---------------------------------------------------------------------------------------------
# What an inserter is.
# ---------------------------------------------------------------------------------------------
class Inserter
  # The entity this script is attached to. No round trip: rubevy hangs it on the task.
  def me
    @me ||= Rubevy.entity
  end

  def name
    self.class.inserter_name
  end

  # === the three questions ===================================================
  #
  # Each is one `Rubevy.ask` the game answers **inside this tick**, so none of them costs a frame.
  # The game answers with an item's number, because a closure that answers inside a tick cannot
  # build a value in the VM — it can only hand back a flat one (rubevy `docs/host-api.md`) — and
  # the name is looked up here, out of the list the game wrote from `data.rb`.

  # What is waiting on the tile behind this inserter — the item on the front of a belt, what a
  # chest holds, or what a machine has made and not got rid of. nil where there is nothing, or
  # where the tile behind is something an arm cannot reach into (a miner puts its own digs down).
  def behind
    item_name(Rubevy.ask("factory.behind").pop)
  end

  # What is in the hand. It is nil almost always: an inserter picks something up and puts it down
  # in the same swing. It is **not** nil after a `move` that answered false, which is an arm that
  # carried something across and found nowhere to put it.
  def holding
    item_name(Rubevy.ask("factory.holding").pop)
  end

  # Whether the tile in front would take that item right now: a gap on the belt, room in the
  # chest, a recipe in the machine that wants it.
  def front_takes?(item)
    return false if item.nil?
    Rubevy.ask("factory.front_takes", number_of(item)).pop
  end

  # === the two things that take time =========================================

  # **Swing the arm.** It picks up what is behind — or carries on with what is already in the hand
  # — and puts it down in front, and this line does not come back until it has arrived. The answer
  # is whether it was put down: false means the tile in front would not take it and the hand is
  # still full, which is what `holding` will say.
  #
  # A swing with nothing behind it and nothing in hand moves nothing and answers false, on the
  # next frame. So a `loop { move }` is a slow loop and not a runaway one — but it is also an arm
  # that swings at nothing, which is what `behind` is for.
  def move
    Rubevy.ask("factory.move").pop
  end

  # **Wait one swing's worth.** There is nothing to do — nothing behind, or nowhere to put it —
  # so look again when something could have changed.
  #
  # It is a swing's worth and not a number of its own: an arm that is already as busy as it can be
  # could not have acted sooner, so looking more often than it can swing is looking for nothing
  # (`docs/numbers.md` §9.3). A shorter wait is a thousand inserters spending the frame's
  # instructions on asking; a longer one is an inserter that lets its belt back up.
  def idle
    sleep swing_seconds
  end

  # === what the game wrote from data.rb ======================================

  # How long one swing takes, in seconds.
  def swing_seconds
    self.class.swing_seconds
  end

  # Every item `data.rb` declares, as Symbols, in the order it declares them. `wants?` is usually
  # written against one or two names rather than this, but a script that wants to say "anything
  # but the ore" has it.
  def items
    self.class.declared_items
  end

  # An item's number as the name it was declared under, and nil for nil — which is what every
  # question above answers when there is nothing there.
  def item_name(number)
    number && items[number.to_i]
  end

  # The other way, for `front_takes?`. A name nothing declares answers -1, which no item is, so
  # the game says no rather than this raising.
  def number_of(item)
    return item.to_i if item.is_a?(Integer)
    at = items.index(item.to_sym)
    at.nil? ? -1 : at
  end

  def log(text)
    Rubevy.log "#{name}: #{text}"
  end

  # **Where an exception happened, in the lines of the file you are editing.**
  #
  # The prelude and your script are compiled as one program, so the line the VM reports is that
  # program's; `prelude_lines` is how far down it your first line is, and the game writes that
  # number in with the item names. Anything past it is yours; anything before it is the DSL's.
  #
  # It is here rather than in the game because **a task that has ended has nothing left to ask**:
  # by the time the game hears that a script stopped, the frames are gone. The `rescue` at the
  # bottom of this file runs while the exception still has its backtrace, and leaves the answer on
  # the task for the game to pick up.
  def self.said_at(error)
    frame = error.backtrace && error.backtrace.first
    return nil if frame.nil?
    # a frame is `file:line` or `file:line:in method`, so the line is the first piece between
    # colons that is a whole number — taking the last piece finds "in run"
    piece = frame.to_s.split(":").find { |p| p.to_i.to_s == p }
    at = piece.to_i
    return nil if at <= 0
    at > prelude_lines ? "inserter.rb:#{at - prelude_lines}" : "prelude.rb:#{at}"
  end
end

# `inserter "Smart" do … end` — a subclass of Inserter with the block evaluated in it (so `def run`
# lands where it belongs), remembered as the one this file defines. The garden's `creature` word,
# with the noun changed.
def inserter(name, &block)
  klass = Class.new(Inserter)
  klass.define_singleton_method(:inserter_name) { name }
  klass.class_eval(&block)
  $inserter_class = klass
end

# Called by the game after the inserter's file has been read.
def run_inserter
  klass = $inserter_class
  raise "this file defines no inserter" if klass.nil?
  being = klass.new
  # **Not all at once.** A thousand inserters started in the same frame would otherwise all wake
  # on the same frame for ever after — every one of them sleeping exactly one swing between looks
  # — and rubevy measured what that costs: three thousand scripts on one `sleep` put the 95th
  # frame in a hundred at 19.5 ms (its `docs/worklog/2026-09-20-factory-survey.md`). So each one
  # waits a part of a swing before its first look, and the phases stay spread from then on,
  # because nothing ever pulls them back together.
  #
  # The luck is the entity's own, the way a creature's is in the garden: `srand` with the entity's
  # bits means two inserters built in the same frame do not draw the same number, and a run is the
  # same run twice.
  srand(being.me.to_i)
  sleep being.swing_seconds * rand
  being.run
rescue => e
  # **the place, worked out here and left where the game will find it.** `@broke_at` on this task
  # is an ordinary instance variable; the game reads it with one `ivar_get` when it hears that
  # this script has ended, because by then the backtrace is gone (`Inserter.said_at`).
  at = klass && klass.said_at(e)
  Task.current.instance_variable_set(:@broke_at, at) if at
  Rubevy.log "#{klass ? klass.inserter_name : '?'} stopped at #{at || '?'}: #{e.class}: #{e.message}"
  raise
end
