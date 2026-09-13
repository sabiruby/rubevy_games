# The match: who is on the field, and what ends it. Ruby decides all of it;
# the game only moves what it is told to move.
match "Training" do
  team :red,  robots: %w[scout hunter]
  team :blue, robots: %w[scout scout]

  # the walls close in, so a stalemate cannot last
  rule(:sudden_death, after: 20) { |m| m.shrink 8.0 }
  rule(:closer, after: 35) { |m| m.shrink 6.0 }

  on_destroyed { |file, team| Rubevy.log("match: #{team}'s #{file} is down") }
end
