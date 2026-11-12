# Tasks run cargo through mise, so the pinned toolchain is used even outside an activated shell.

cargo := "mise exec -- cargo"

# Launch Isle, the Kesmai-style island game: just isle [--new] [--round SECONDS]
isle *args:
    {{cargo}} run --quiet -p isle -- {{args}}

# Summarize real Isle play time per round from the log beside the save: just playtime [LOG]
playtime *args:
    python3 crates/isle/scripts/playtime.py {{args}}

# Run every test in the workspace.
test:
    {{cargo}} test --workspace
    {{cargo}} test -p blah-core --example skill_pacing
    {{cargo}} test -p blah-core --example journey_playtest

# Check formatting and lints; warnings fail.
lint:
    {{cargo}} fmt --all --check
    {{cargo}} clippy --workspace --all-targets -- -D warnings

# Print a terrain map: just map [seed] [x] [y] [width] [height] [step] [--plain]
map *args:
    {{cargo}} run --quiet --release -p blah-core --example ascii_map -- {{args}}

# Write a contact sheet of every clade across biomes, and time region growth: just bestiary out.png [seed]
bestiary *args:
    {{cargo}} run --quiet --release -p blah-core --example bestiary_sheet -- {{args}}

# Ask Jev typed questions about the loadout rules (costs a fraction of a cent): just jev-review [--dry-run]
jev-review *args:
    python3 scripts/jev_loadout_review.py {{args}}

# Print the actual dagger and staff composition trials without opening a window.
composition-demo:
    {{cargo}} run --quiet -p blah-core --example composition_demo

# Measure skill pacing and learned moves: just skill-pacing [seeds] [actions] [baseline|cautious|compare|move-compare|move-effect [horizon]]
skill-pacing *args:
    {{cargo}} run --quiet --release -p blah-core --example skill_pacing -- {{args}}

# Follow whole fresh-character journeys: [seeds] [actions] [first-seed] [trails|blind]
journey-playtest *args:
    {{cargo}} run --quiet --release -p blah-core --example journey_playtest -- {{args}}
