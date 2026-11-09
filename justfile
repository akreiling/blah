# Tasks run cargo through mise, so the pinned toolchain is used even outside an activated shell.

cargo := "mise exec -- cargo"

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
