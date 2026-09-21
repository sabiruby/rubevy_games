# Factory — the words the control stage is written in.
#
# This file is put in front of `ruby/control.rb`, so that file is all `on(:delivered) { … }` and
# `goal deliver: { … }` with nothing to set up (`factory/src/control.rs`, which is also where the
# block of Ruby the *game* writes goes — the item names, the building names, and how far down the
# program your first line is).
#
# **What the control stage is.** `data.rb` says what the world is made of and an inserter's script
# says what one arm does; this says **what the factory is for**. It is one script for the whole
# game — not one per machine — and it hears about the factory at the granularity of a *machine*:
#
#   on(:built)     { |what, n, x, y| }   something was put on the map
#   on(:removed)   { |what, n, x, y| }   something was taken off it
#   on(:crafted)   { |item, n, x, y| }   machines finished crafts and are holding what they made
#   on(:delivered) { |item, n, x, y| }   things went into chests
#   on(:jammed)    { |what, n, x, y| }   a machine is holding what it made and cannot start again
#
# **Every one of them is the same four**: *what* (a Symbol — an item's name for the two that are
# about items, a building's name for the other three), *how many* of it this frame, and *where* —
# the tile of one of them, in the column-and-row the game logs.
#
# **One message a frame per kind of thing, and that is the design.** A subscription holds sixty
# four messages before the oldest is dropped (rubevy's `ScriptWorld::queue_limit`), and a script
# is woken once a frame, so what may be published between two looks is sixty four. A message for
# every gear that went into a chest would reach that in a factory of a few hundred arms; a message
# for *each kind of thing that happened* cannot, because how many kinds there are is what
# `ruby/data.rb` declares — three items and two machines, in the file this game ships. So the game
# adds up a frame's happenings by kind before it publishes them, `n` is how many there were, and
# the tile is the last of them. Nothing here is ever told about one item on one belt: an item on a
# belt is not a machine, and there are thousands of them (`docs/plans/factory-plan.md` §3.4).
#
# **The goal is here and not in `data.rb`**, because a goal is what you are playing *for* and the
# data stage is what the world is made *of*. `goal deliver: { gear: 60 }` counts what arrives in
# chests from the moment this script starts and wins when it has them all.
#
# **A control.rb that will not run leaves a factory with no goal, and not a game that stopped.**
# Whatever is wrong is said with the line of *this* file it happened on, the way a broken
# inserter's script is, and the belts go on carrying.

class Control
  # === what the game publishes ================================================

  # **How many `on` blocks one control.rb may have**: one per event the game publishes, which is
  # what `crate::control::EVENTS` says and not a budget anybody chose. The dispatch below has to
  # name its methods in the source — `instance_exec`, `send` and `Method#call` all run a block in
  # a nested run loop of the VM, and a task cannot be parked across one — so the number of slots
  # is the number of lines written out in `run_handler` (rubevy_games `docs/worklog/…-reflex.md`,
  # and the garden's prelude, which is where this shape comes from).
  ON_SLOTS = 5

  # Which of the five carry an **item**'s name; the rest carry a **building**'s.
  ABOUT_ITEMS = [:crafted, :delivered]

  def self.handlers
    @handlers ||= []
  end

  def self.on(event, &block)
    raise "on needs a block" if block.nil?
    slot = handlers.size
    raise "a control.rb may have #{ON_SLOTS} handlers at most" if slot >= ON_SLOTS
    define_method("__handler_#{slot}", &block)
    handlers << [event.to_sym, slot]
    block
  end

  # `goal deliver: { gear: 60 }` — how many of what has to reach a chest.
  def self.goal(deliver)
    deliver.each do |item, upto|
      wanted[item.to_sym] = upto.to_i
    end
    wanted
  end

  def self.wanted
    @wanted ||= {}
  end

  def initialize
    @got = {}
    @dropped = 0
    @dropped_by = {}
    @won = false
  end

  # A payload the game published, handed to the block that asked for it. The game answers numbers
  # — an in-tick answer and a published message are both flat values — and the name is looked up
  # here, out of the lists the game wrote from `data.rb`.
  def run_handler(slot, event, payload)
    what = name_of(event, payload[0])
    n = payload[1].to_i
    x = payload[2].to_i
    y = payload[3].to_i
    note_seen(event, n)
    case slot
    when 0 then __handler_0(what, n, x, y)
    when 1 then __handler_1(what, n, x, y)
    when 2 then __handler_2(what, n, x, y)
    when 3 then __handler_3(what, n, x, y)
    when 4 then __handler_4(what, n, x, y)
    end
  end

  # An item's or a building's name, as the number the game published.
  def name_of(event, number)
    list = ABOUT_ITEMS.include?(event) ? items : buildings
    list[number.to_i]
  end

  # === what the game wrote from data.rb =======================================

  # Every item `data.rb` declares, as Symbols, in the order it declares them.
  def items
    self.class.declared_items
  end

  # Everything that can stand on a tile: the four the world has built in, and then the machines
  # `data.rb` declares, in the order it declares them.
  def buildings
    self.class.declared_buildings
  end

  # === the goal ===============================================================

  # **The main task**: it counts what has been delivered and wins when the goal is met.
  #
  # It reads a subscription of its own rather than the `on(:delivered)` block's, so that a
  # control.rb that never writes one still has a goal, and one that writes it for something else
  # does not have to remember to count.
  def watch_the_goal
    wanted = self.class.wanted
    wanted.each_key { |item| @got[item] = 0 }
    say_where_it_is
    queue = Rubevy.subscribe(:delivered)
    loop do
      payload = queue.pop
      note_dropped(-1, queue.dropped)
      item = items[payload[0].to_i]
      next if item.nil? || !wanted.key?(item)
      @got[item] += payload[1].to_i
      # where the last thing the goal counted went, so that `win!` can point the camera at it
      @where = [payload[2].to_i, payload[3].to_i]
      say_where_it_is
      done = true
      wanted.each { |name, upto| done = false if @got[name].to_i < upto }
      win! if done
    end
  end

  # **Won.** The game reads `@won` off this object once a frame — the way the garden reads a
  # creature's `@asleep` — so this is the whole of what the control stage does *to* the game:
  # everything else here is listening. A control.rb may call it itself for a goal of its own.
  def win!
    return if @won
    @won = true
    say_where_it_is
    log "the goal is reached"
    # **Ruby moving the camera.** `look_at` is rubevy's optional `Rubevy::Camera` layer and
    # nothing the game answers; `@where` is the tile the last delivery the goal counted went
    # into, so winning leaves you looking at the chest you won in. A run with no window has no
    # camera and this does nothing.
    look_at(@where[0], @where[1]) if @where
  end

  def won?
    @won
  end

  # How many of `item` have reached a chest since this script started.
  def delivered(item)
    @got[item.to_sym].to_i
  end

  # **The line the game puts on the screen**, in this script's own words — `@saying`, read with
  # one `ivar_get`. A control.rb with no goal in it leaves it alone and the game shows nothing.
  def say_where_it_is
    wanted = self.class.wanted
    return if wanted.empty?
    parts = []
    wanted.each { |name, upto| parts << "#{name} #{@got[name].to_i} / #{upto}" }
    @saying = (@won ? "won — " : "goal: ") + parts.join(", ")
  end

  # === what the game reads off this object ====================================

  # **How much of what the game published this script has heard**, one counter per event, under
  # the name the game looks for (`@seen_delivered`). It is counted here rather than asked for,
  # which is the garden's way with `mutation_rate`: a class-level fact the host reads with two
  # `ivar_get`s and no new question for a script to answer.
  def note_seen(event, n)
    key = "@seen_#{event}"
    instance_variable_set(key, instance_variable_get(key).to_i + n)
  end

  # **What was published to this script and never reached it**, over every subscription it has:
  # `Rubevy::Subscription#dropped` is the one number that says a queue was full when the game
  # published to it, and a number that is not zero says the events are too fine-grained for what
  # the factory does (`docs/factory.md`). The game shows it beside its own count of the same
  # thing (`ScriptWorld::dropped`), which is the VM's total rather than this script's.
  def note_dropped(slot, n)
    return if @dropped_by[slot] == n
    @dropped_by[slot] = n
    total = 0
    @dropped_by.each { |_, lost| total += lost }
    @dropped = total
  end

  def log(text)
    Rubevy.log "control.rb: #{text}"
  end

  # === looking at the factory ==================================================

  # **Where a tile is in the world**, as `[x, y]`. The map is centred on the origin and a tile is
  # `tile_px` across, both of which the game writes in front of this file out of `data.rb`.
  def world_of(column, row)
    across, up = self.class.map_size
    px = self.class.tile_px.to_f
    [column * px + px / 2 - across * px / 2, row * px + px / 2 - up * px / 2]
  end

  # **Point the camera at a tile.** `Rubevy::Camera` is rubevy's optional layer (the game loads it
  # with `ScriptWorld::load_and_run`), so this is ordinary Ruby over `e[:Transform] =` and not
  # something the game answers — and a run with no window has no camera at all, where it is nil
  # and this does nothing.
  #
  # It is here rather than in `control.rb` so that a rewritten control stage still has the word.
  def look_at(column, row)
    camera = Rubevy::Camera.find
    return nil if camera.nil?
    at = world_of(column, row)
    camera.move_to(at[0], at[1])
  end

end

# **A class of its own for each file.**
#
# The VM is one VM and `class Control` is one class in it, so a control.rb applied over another
# would otherwise inherit the first one's handlers and its goal — and `define_method` would have
# quietly replaced the bodies under them. A subclass made when the program is loaded is the
# garden's answer (`creature "Beetle" do … end` is `Class.new(Creature)`), with the wrapper word
# left out: there is exactly one control stage, so `on` and `goal` are written at the top of the
# file rather than inside a block.
$control_class = Class.new(Control)

def on(event, &block)
  $control_class.on(event, &block)
end

# `goal deliver: { gear: 60 }`. It takes the Hash rather than a keyword so that a data file and a
# control file are read by the same kind of Ruby.
def goal(spec)
  $control_class.goal(spec[:deliver] || {})
end

# Called by the game after control.rb has been read.
def run_control
  tasks = []
  # **your file, run here** rather than where it stands. The game puts it inside a method of one
  # line (`crate::control::in_a_method`) so that `goal` and `on` are called from inside this
  # method — which is what lets the ending rubevy sends say the line of *your* file anything in it
  # went wrong on, the way a broken inserter's script says it. At the top level they would run
  # while the program was being loaded, before `run_control` was reached at all.
  the_control_file
  being = $control_class.new
  # where the game finds it: the counters above, `@won`, and the line for the screen are read off
  # this object with two `ivar_get`s (`crate::control::what_it_says`)
  Task.current.instance_variable_set(:@being, being)
  start_control_handlers(being, tasks)
  # and this task becomes the one that watches the goal, for ever
  being.watch_the_goal
ensure
  # the handlers are tasks of their own: nothing else stops them when this one ends
  tasks.each { |t| t.terminate }
end

# One task per `on`, started before the goal is watched.
#
# The subscription is taken here, in the script's own task, because `Rubevy.subscribe` belongs to
# the entity whose task asks; the queue it answers with is an ordinary object and is handed to the
# task that reads it. They run at this task's own priority — which the game sets ahead of the
# inserters' — because a factory with three thousand arms in it must not be able to starve the one
# script that says whether the game has been won.
def start_control_handlers(being, tasks)
  here = Task.current
  $control_class.handlers.each do |event, slot|
    queue = Rubevy.subscribe(event)
    task = Task.new(name: "control-#{event}", priority: here.priority) do
      begin
        loop do
          payload = queue.pop # parked here, costing nothing, until it happens
          being.note_dropped(slot, queue.dropped)
          begin
            being.run_handler(slot, event, payload)
          rescue => e
            Rubevy.log "control.rb: #{event}: #{e.class}: #{e.message}"
          end
        end
      rescue Rubevy::Unsubscribed
        # the script was replaced or taken away; rubevy closed the queue, so the `pop` raised
        # rather than parking for ever. This is the ordinary end of a handler task.
      end
    end
    tasks << task
  end
  tasks
end
