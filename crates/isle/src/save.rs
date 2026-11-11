//! Saving: the whole game as one JSON file, written after every round.

use std::path::{Path, PathBuf};

use isle_core::Game;

/// Where a bare launch keeps its character.
pub fn default_path() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or("no data directory for saves")?;
    Ok(base.join("isle").join("save.json"))
}

/// The game saved at `path`, if there is one.
pub fn load(path: &Path) -> Result<Option<Game>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|e| format!("{} is unreadable: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Writes `game` to `path` whole, so a crash mid-write never leaves half a save.
pub fn write(path: &Path, game: &Game) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string(game).map_err(|e| format!("cannot encode the save: {e}"))?;
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, text).map_err(|e| format!("cannot write {}: {e}", partial.display()))?;
    std::fs::rename(&partial, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
}

/// Renames an existing save so a new character can start without losing it.
pub fn set_aside(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let stem = path.file_stem().map_or("save".into(), |s| s.to_string_lossy().into_owned());
    let aside = path.with_file_name(format!("{stem}-{stamp}.json"));
    std::fs::rename(path, &aside).map_err(|e| format!("cannot set aside {}: {e}", path.display()))
}
