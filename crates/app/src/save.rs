//! SQLite I/O around pure, versioned core snapshots. One file owns one character and world.

use std::path::{Path, PathBuf};

use blah_core::{Game, GameSnapshot};
use rusqlite::{Connection, OpenFlags, params};

const SCHEMA_VERSION: i64 = 1;

/// A save session with optimistic concurrency protection against another running app.
pub struct Save {
    connection: Connection,
    revision: i64,
    /// The bytes last committed, so an action that changes nothing writes nothing.
    committed: Vec<u8>,
    /// Whether this session created the file rather than resuming it.
    pub created: bool,
    pub path: PathBuf,
}

impl Save {
    /// Exclusively create a new character file. Never replace an existing file.
    pub fn create(path: &Path, game: &Game) -> Result<Self, String> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("Cannot create save {}: {e}", path.display()))?;
        let mut connection = Connection::open(path).map_err(|e| e.to_string())?;
        connection.pragma_update(None, "synchronous", "FULL").map_err(|e| e.to_string())?;
        let payload = serde_json::to_vec(&game.snapshot()).map_err(|e| e.to_string())?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction
            .execute_batch(
                "CREATE TABLE state (
                slot INTEGER PRIMARY KEY CHECK (slot = 1),
                character_id TEXT NOT NULL,
                revision INTEGER NOT NULL,
                snapshot BLOB NOT NULL
            );
            PRAGMA user_version = 1;",
            )
            .map_err(|e| e.to_string())?;
        transaction
            .execute("INSERT INTO state VALUES (1, lower(hex(randomblob(16))), 0, ?1)", [&payload])
            .map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(Self { connection, revision: 0, committed: payload, created: true, path: path.to_owned() })
    }

    /// Open only an existing save. Validate before performing any write.
    pub fn load(path: &Path) -> Result<(Self, Game), String> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|e| format!("Cannot open save {}: {e}", path.display()))?;
        let (revision, payload, snapshot) = read(&connection, path)?;
        let game = Game::from_snapshot(snapshot).map_err(str::to_owned)?;
        connection.pragma_update(None, "synchronous", "FULL").map_err(|e| e.to_string())?;
        Ok((Self { connection, revision, committed: payload, created: false, path: path.to_owned() }, game))
    }

    /// The saved snapshot without restoring it or opening the file for writing, for listing characters.
    pub fn peek(path: &Path) -> Result<GameSnapshot, String> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Cannot open save {}: {e}", path.display()))?;
        read(&connection, path).map(|(_, _, snapshot)| snapshot)
    }

    /// Replace the whole state atomically, only if this session still owns the latest revision.
    pub fn write(&mut self, game: &Game) -> Result<(), String> {
        let payload = serde_json::to_vec(&game.snapshot()).map_err(|e| e.to_string())?;
        if payload == self.committed {
            return Ok(());
        }
        let next = self.revision.checked_add(1).ok_or("Save revision exhausted.")?;
        let transaction = self.connection.transaction().map_err(|e| e.to_string())?;
        let changed = transaction
            .execute(
                "UPDATE state SET revision = ?1, snapshot = ?2 WHERE slot = 1 AND revision = ?3",
                params![next, payload, self.revision],
            )
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("Another session changed this character. Quit and reopen the save before playing.".into());
        }
        transaction.commit().map_err(|e| e.to_string())?;
        self.revision = next;
        self.committed = payload;
        Ok(())
    }

    /// Committed revisions so far: one per action that changed the game.
    #[cfg(test)]
    pub const fn revision(&self) -> i64 {
        self.revision
    }
}

fn read(connection: &Connection, path: &Path) -> Result<(i64, Vec<u8>, GameSnapshot), String> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("Cannot read save schema: {e}"))?;
    if version != SCHEMA_VERSION {
        return Err(format!(
            "Unsupported save schema {version} (expected {SCHEMA_VERSION}); {} was not changed.",
            path.display()
        ));
    }
    let (revision, payload): (i64, Vec<u8>) = connection
        .query_row("SELECT revision, snapshot FROM state WHERE slot = 1", [], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("Cannot read saved character: {e}"))?;
    let snapshot: GameSnapshot = serde_json::from_slice(&payload).map_err(|e| format!("Invalid save snapshot: {e}"))?;
    Ok((revision, payload, snapshot))
}

/// Apply one completed action, committing its costs, learning, and effects together.
/// A failed save rolls the in-memory action back too, so play cannot outrun durable progress.
pub fn commit_action(game: &mut Game, save: &mut Option<Save>, action: impl FnOnce(&mut Game)) -> Result<(), String> {
    // The snapshot, not a full clone: rollback is rare, and restoring regrows the caches a clone would copy.
    let before = save.as_ref().map(|_| game.snapshot());
    action(game);
    if let Some(save) = save
        && let Err(error) = save.write(game)
    {
        let before = before.expect("persistent actions keep their previous state");
        *game = Game::from_snapshot(before).expect("a state the game just held restores");
        return Err(format!("Autosave failed; action undone: {error}"));
    }
    Ok(())
}

/// Default location on every platform, XDG style: `$XDG_DATA_HOME/blah/saves`, else
/// `~/.local/share/blah/saves`, independent of the shell's working directory.
pub fn directory() -> Result<PathBuf, String> {
    xdg_directory(std::env::var_os("XDG_DATA_HOME").map(PathBuf::from), dirs::home_dir())
}

fn xdg_directory(data_home: Option<PathBuf>, home: Option<PathBuf>) -> Result<PathBuf, String> {
    // The XDG spec says a relative XDG_DATA_HOME is invalid and must be ignored.
    data_home
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(".local").join("share")))
        .map(|p| p.join("blah").join("saves"))
        .ok_or_else(|| "Cannot locate the home directory; use --save PATH.".into())
}

/// The automatically named character files in `directory`, oldest first.
pub fn characters(directory: &Path) -> Result<Vec<(u64, PathBuf)>, String> {
    let mut found = Vec::new();
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(found),
        Err(e) => return Err(format!("Cannot read save directory {}: {e}", directory.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if let Some(number) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.strip_prefix("character-"))
            .and_then(|s| s.strip_suffix(".sqlite3"))
            .and_then(|s| s.parse::<u64>().ok())
        {
            found.push((number, entry.path()));
        }
    }
    found.sort();
    Ok(found)
}

/// One line per character for `--list`: number, seed, turn, deaths, and path, or why it can't be read.
pub fn list(directory: &Path) -> Result<Vec<String>, String> {
    Ok(characters(directory)?
        .into_iter()
        .map(|(number, path)| match Save::peek(&path) {
            Ok(s) => {
                format!("{number:>4}  seed {}  turn {}  deaths {}  {}", s.seed(), s.turn(), s.deaths(), path.display())
            }
            Err(error) => format!("{number:>4}  unreadable: {error}"),
        })
        .collect())
}

/// Resume the latest automatically named character, or create a separate one for `--new`.
pub fn automatic(directory: &Path, fresh: bool, seed: u64) -> Result<(Save, Game), String> {
    std::fs::create_dir_all(directory).map_err(|e| format!("Cannot create save directory: {e}"))?;
    let latest = characters(directory)?.pop();
    if let Some((_, path)) = &latest
        && !fresh
    {
        return Save::load(path);
    }
    let last = latest.map_or(0, |(number, _)| number);
    let game = Game::new(seed);
    let next = last.checked_add(1).ok_or("Character filenames exhausted.")?;
    let save = Save::create(&directory.join(format!("character-{next:06}.sqlite3")), &game)?;
    Ok((save, game))
}

#[cfg(test)]
mod tests {
    use super::*;
    use blah_core::{Dir, Maneuver};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "blah-save-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn file(&self) -> PathBuf {
            self.0.join("test.sqlite3")
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn continuous_play_equals_real_sqlite_resume_without_repeated_learning() {
        let scratch = Scratch::new();
        for demo in [false, true] {
            let path = scratch.0.join(format!("{demo}.sqlite3"));
            let mut continuous =
                if demo { Game::composition_trial(42, Maneuver::backstab_burst()).unwrap() } else { Game::new(7) };
            let mut resumed = continuous.clone();
            let mut save = Some(Save::create(&path, &resumed).unwrap());
            for i in 0..120 {
                let action = |g: &mut Game| match i % 11 {
                    0 => _ = g.toggle_sneak(),
                    1 => _ = g.use_technique(Dir::East),
                    2 => _ = g.recover_focus(),
                    3 => _ = g.use_move(blah_core::LearnedMove::Withdraw, Dir::West),
                    _ => _ = g.step(Dir::ALL[(i / 9) % 4]),
                };
                action(&mut continuous);
                commit_action(&mut resumed, &mut save, action).unwrap();
                drop(save.take());
                let (opened, loaded) = Save::load(&path).unwrap();
                save = Some(opened);
                resumed = loaded;
                assert_eq!(continuous, resumed, "action {i}");
                let learned = resumed.wanderer().weapons;
                let (_, again) = Save::load(&path).unwrap();
                assert_eq!(again, resumed);
                assert_eq!(again.wanderer().weapons, learned);
            }
        }
    }

    #[test]
    fn failed_transaction_and_stale_session_preserve_last_commit_and_undo_action() {
        let scratch = Scratch::new();
        let path = scratch.file();
        let initial = Game::new(42);
        let mut save = Some(Save::create(&path, &initial).unwrap());
        let (mut stale, _) = Save::load(&path).unwrap();
        let mut game = initial.clone();
        commit_action(&mut game, &mut save, |g| _ = g.wait()).unwrap();
        let mut other = initial.clone();
        other.toggle_sneak();
        assert!(stale.write(&other).unwrap_err().contains("Another session"));
        assert_eq!(Save::load(&path).unwrap().1, game);
        save.as_ref()
            .unwrap()
            .connection
            .execute_batch(
                "CREATE TRIGGER fail_save BEFORE UPDATE ON state BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
            )
            .unwrap();
        let before = game.clone();
        assert!(commit_action(&mut game, &mut save, |g| _ = g.wait()).is_err());
        assert_eq!(game, before);
        assert_eq!(Save::load(&path).unwrap().1, before);
    }

    #[test]
    fn commitments_survive_sqlite_resume_and_failed_commit_rolls_back_capacity_and_log() {
        let scratch = Scratch::new();
        let path = scratch.file();
        let mut state = serde_json::to_value(Game::new(42).snapshot()).unwrap();
        state["wanderer"]["focus_skill"]["practice"] = 4_000.into();
        state["wanderer"]["stealth"]["practice"] = 4_000.into();
        let mut game = Game::from_snapshot(serde_json::from_value(state).unwrap()).unwrap();
        let mut save = Some(Save::create(&path, &game).unwrap());
        commit_action(&mut game, &mut save, |g| {
            g.commit_adaptation(blah_core::Adaptation::DeepReserves).unwrap();
        })
        .unwrap();
        assert_eq!(game.turn(), 0);
        assert_eq!(Save::load(&path).unwrap().1, game);
        let before = game.clone();
        save.as_ref()
            .unwrap()
            .connection
            .execute_batch(
                "CREATE TRIGGER fail_save BEFORE UPDATE ON state BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
            )
            .unwrap();
        assert!(
            commit_action(&mut game, &mut save, |g| {
                g.commit_adaptation(blah_core::Adaptation::VeiledMovement).unwrap();
            })
            .is_err()
        );
        assert_eq!(game, before);
        assert_eq!(game.wanderer().adaptations.free(), 1);
        assert_eq!(Save::load(&path).unwrap().1, before);
    }

    #[test]
    fn legacy_sqlite_payload_migrates_without_writing_until_next_action() {
        let scratch = Scratch::new();
        let path = scratch.file();
        let game = Game::new(42);
        let save = Save::create(&path, &game).unwrap();
        let mut legacy = serde_json::to_value(game.snapshot()).unwrap();
        legacy["snapshot_version"] = 1.into();
        legacy["wanderer"].as_object_mut().unwrap().remove("adaptations");
        let payload = serde_json::to_vec(&legacy).unwrap();
        save.connection.execute("UPDATE state SET snapshot = ?1", [&payload]).unwrap();
        drop(save);
        let (save, mut resumed) = Save::load(&path).unwrap();
        assert_eq!(resumed, game);
        let stored: Vec<u8> = save.connection.query_row("SELECT snapshot FROM state", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, payload, "opening does not rewrite the old save");
        let mut save = Some(save);
        commit_action(&mut resumed, &mut save, |g| _ = g.wait()).unwrap();
        assert_eq!(Save::load(&path).unwrap().1, resumed);
        let stored: Vec<u8> =
            save.as_ref().unwrap().connection.query_row("SELECT snapshot FROM state", [], |r| r.get(0)).unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&stored).unwrap()["snapshot_version"], 3);
    }

    #[test]
    fn separate_characters_resume_latest_and_never_overwrite_existing_files() {
        let scratch = Scratch::new();
        let (mut a, mut first) = automatic(&scratch.0, false, 7).unwrap();
        first.wait();
        a.write(&first).unwrap();
        let (b, second) = automatic(&scratch.0, true, 42).unwrap();
        assert_ne!(a.path, b.path);
        assert!(Save::create(&a.path, &second).is_err());
        assert_eq!(Save::load(&a.path).unwrap().1, first);
        assert_eq!(automatic(&scratch.0, false, 999).unwrap().1, second);
    }

    #[test]
    fn a_turn_rewrites_a_small_snapshot_and_the_file_does_not_grow() {
        let scratch = Scratch::new();
        let path = scratch.file();
        let mut game = Game::new(42);
        let mut save = Some(Save::create(&path, &game).unwrap());
        let mut largest = 0;
        let start = std::time::Instant::now();
        for i in 0..400 {
            commit_action(&mut game, &mut save, |g| _ = g.step(Dir::ALL[(i / 9) % 4])).unwrap();
            largest = largest.max(serde_json::to_vec(&game.snapshot()).unwrap().len());
        }
        let per_turn = start.elapsed() / 400;
        let file = std::fs::metadata(&path).unwrap().len();
        eprintln!("largest snapshot {largest} bytes, file {file} bytes, {per_turn:?} per saved turn");
        assert!(largest < 64 * 1024, "snapshot is {largest} bytes");
        assert!(file < 256 * 1024, "save file is {file} bytes");
    }

    #[test]
    fn default_directory_follows_xdg_on_every_platform() {
        let home = Some(PathBuf::from("/home/u"));
        assert_eq!(xdg_directory(None, home.clone()).unwrap(), PathBuf::from("/home/u/.local/share/blah/saves"));
        assert_eq!(xdg_directory(Some("/data".into()), home.clone()).unwrap(), PathBuf::from("/data/blah/saves"));
        assert_eq!(
            xdg_directory(Some("relative".into()), home).unwrap(),
            PathBuf::from("/home/u/.local/share/blah/saves")
        );
        assert!(xdg_directory(None, None).is_err());
    }

    #[test]
    fn actions_that_change_nothing_write_nothing() {
        let scratch = Scratch::new();
        let mut game = Game::new(42);
        let mut save = Some(Save::create(&scratch.file(), &game).unwrap());
        commit_action(&mut game, &mut save, |_| {}).unwrap();
        assert_eq!(save.as_ref().unwrap().revision(), 0);
        // A refused action passes no turn, but its explanation is in the log, which is saved.
        commit_action(&mut game, &mut save, |g| _ = g.recover_focus()).unwrap();
        assert_eq!(save.as_ref().unwrap().revision(), 1);
        commit_action(&mut game, &mut save, |g| _ = g.wait()).unwrap();
        assert_eq!(save.as_ref().unwrap().revision(), 2);
        assert_eq!(Save::load(&scratch.file()).unwrap().1, game);
    }

    #[test]
    fn missing_corrupt_and_unsupported_saves_are_not_overwritten() {
        let scratch = Scratch::new();
        let path = scratch.file();
        assert!(Save::load(&path).is_err());
        assert!(!path.exists());
        std::fs::write(&path, b"not SQLite").unwrap();
        assert!(Save::load(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"not SQLite");
        let path = scratch.0.join("future.sqlite3");
        let save = Save::create(&path, &Game::new(42)).unwrap();
        save.connection.pragma_update(None, "user_version", 99).unwrap();
        drop(save);
        let bytes = std::fs::read(&path).unwrap();
        assert!(Save::load(&path).err().unwrap().contains("Unsupported save schema"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
