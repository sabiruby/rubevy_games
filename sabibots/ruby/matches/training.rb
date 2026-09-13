# The match: who is on the field, and what ends it. Ruby decides all of it;
# the game only moves what it is told to move.
# noise: how far radars and guns stray. Add `seed: 7` to roll the same dice every time.
match "Training", noise: 0.3 do
  team :red,  robots: %w[scout hunter]
  team :blue, robots: %w[scout scout]

  # the walls close in, one crate at a time, so a stalemate cannot last
  rule(:sudden_death, after: 20, every: 2.0) { |m| m.shrink 1 }

  on_destroyed { |file, team| Rubevy.log("match: #{team}'s #{file} is down") }
end
