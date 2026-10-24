#!/bin/bash
# Usage: strip.sh NATION  -> target/raw_NATION_N.png (full demo shots) and target/strip_NATION.png
# (the two horizontal walkers' rows for steps 1..10).
export CARGO_PROFILE_DEV_DEBUG=0
export FN_DEVSHELL=1
cd /home/user/four-nations/.claude/worktrees/agent-a5d4d49e735654c22
for n in 1 2 3 4 5 6 7 8 9 10; do
  FN_WALK_DEMO=$n cargo xtask screenshot 7 "$1" target/raw_$1_$n.png 1280 720 >/dev/null 2>&1
  convert target/raw_$1_$n.png -crop 300x200+90+160 +repage -fill white -pointsize 18 -gravity NorthWest -annotate +4+2 "step $n" target/crop_$1_$n.png
done
montage target/crop_$1_1.png target/crop_$1_2.png target/crop_$1_3.png target/crop_$1_4.png target/crop_$1_5.png target/crop_$1_6.png target/crop_$1_7.png target/crop_$1_8.png target/crop_$1_9.png target/crop_$1_10.png -tile 5x2 -geometry +3+3 -background '#222' target/strip_$1.png
