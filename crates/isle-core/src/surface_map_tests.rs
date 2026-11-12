use crate::{Game, Pos, SurfaceMapCell};

fn named(seed: u64) -> Game {
    let mut game = Game::new(seed);
    game.answer("Wren");
    game
}

fn one_cell(cells: Vec<Option<SurfaceMapCell>>) -> Option<SurfaceMapCell> {
    assert_eq!(cells.len(), 1);
    cells.into_iter().next().flatten()
}

#[test]
fn ordinary_surface_maps_return_only_remembered_terrain() {
    let game = named(7);
    let known = game.surface_map_player_pos();
    let unknown: Pos = (known.0 + 50, known.1 + 50);

    assert!(game.remembers_surface(known));
    assert!(one_cell(game.surface_map_region(known, 1, 1)).is_some());
    assert!(!game.remembers_surface(unknown));
    assert!(one_cell(game.surface_map_region(unknown, 1, 1)).is_none());
}

#[test]
fn debug_reveal_includes_unknown_terrain_and_settlements_without_persisting_exploration() {
    let game = named(7);
    let home = game.level().town_at(game.player().pos).expect("player starts in a settlement");
    let unknown = home.origin;
    assert!(!game.remembers_surface(unknown));

    let revealed = one_cell(game.debug_surface_map_region(unknown, 1, 1)).expect("debug reveal includes terrain");
    assert_eq!(revealed.settlement, Some(home));
    assert!(!revealed.visible);
    assert!(!game.remembers_surface(unknown), "debug reveal is not saved as player exploration");
    assert!(one_cell(game.surface_map_region(unknown, 1, 1)).is_none(), "normal mode remains fogged");
}

#[test]
fn zoomed_maps_keep_seen_cells_between_samples_without_revealing_the_rest() {
    let game = named(7);
    let player = game.surface_map_player_pos();
    let origin = (player.0 - 7, player.1 - 7);
    assert!(!game.remembers_surface(origin));
    let cell = one_cell(game.surface_map_samples(origin, 1, 1, 8, false)).expect("seen terrain inside the block");
    assert!(game.remembers_surface(cell.pos));
    assert_eq!(one_cell(game.surface_map_region(cell.pos, 1, 1)), Some(cell));
    for step in [1, 2, 4, 8, 16] {
        let cells = game.surface_map_samples((-50, -50), 16, 16, step, false);
        assert_eq!(cells.len(), 256);
        for cell in cells.into_iter().flatten() {
            assert!(game.remembers_surface(cell.pos));
        }
    }
}

#[test]
fn debug_exploration_and_underground_maps_preserve_surface_memory_and_saved_state() {
    let mut game = named(7);
    let town = game.level().town_at(game.player().pos).unwrap();
    let neighbor = game.level().towns().find(|t| *t != town).expect("seed 7 has another settlement");
    let origin = (neighbor.origin.0 - 32, neighbor.origin.1 - 32);
    game.player_mut().pos = town.stairs();
    game.round(Some(crate::Command::Down));
    assert_eq!(game.surface_map_player_pos(), town.stairs());
    let before = serde_json::to_string(&game).unwrap();
    let cells = game.surface_map_samples(origin, 64, 64, 8, true);
    assert!(cells.iter().all(|c| c.is_some_and(|c| !c.visible)));
    assert!(cells.iter().flatten().any(|c| c.settlement == Some(neighbor)));
    assert_eq!(serde_json::to_string(&game).unwrap(), before, "even undiscovered sectors stay out of the save");
    assert!(game.surface_map_samples(origin, 1, 1, 8, false)[0].is_none());
    assert!(game.surface_map_region((0, 0), 0, 5).is_empty());
}
