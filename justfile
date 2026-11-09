# Tasks run cargo through mise, so the pinned toolchain is used even outside an activated shell.

cargo := "mise exec -- cargo"

# Launch the game: just run [seed]
run *args:
    {{cargo}} run --quiet -p blah -- {{args}}

# Run every test in the workspace.
test:
    {{cargo}} test --workspace

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
