//! A local log of real play time, for pacing: one JSON line per played command, beside the save.
//!
//! Only time between inputs counts, and any one gap counts at most [`IDLE`], so stepping away from
//! the window does not inflate it. Nothing here touches the rules or the save.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use isle_core::{Game, LevelId};
use serde_json::json;

/// The most one quiet stretch between inputs counts as play.
pub const IDLE: Duration = Duration::from_secs(60);

/// The log kept beside `save`: `save.json` keeps `save.playlog.jsonl`.
pub fn path_for(save: &Path) -> PathBuf {
    save.with_extension("playlog.jsonl")
}

fn depth(at: LevelId) -> u32 {
    match at {
        LevelId::Surface => 0,
        LevelId::Deep(_, depth) => depth,
    }
}

fn unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn seconds(d: Duration) -> f64 {
    (d.as_secs_f64() * 100.0).round() / 100.0
}

/// Appends play-time entries for one window session.
pub struct PlayLog {
    file: File,
    last_input: Instant,
    active: Duration,
    idle: Duration,
    keys: u32,
    round: u64,
    deaths: u32,
    depth: u32,
    line: String,
}

impl PlayLog {
    /// Opens the log for appending and marks the start of a session.
    pub fn open(path: &Path, game: &Game, now: Instant) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let p = game.player();
        writeln!(
            file,
            "{}",
            json!({"session": unix(), "seed": game.seed(), "round": game.round_number(), "name": p.name})
        )?;
        Ok(PlayLog {
            file,
            last_input: now,
            active: Duration::ZERO,
            idle: Duration::ZERO,
            keys: 0,
            round: game.round_number(),
            deaths: p.deaths,
            depth: depth(p.at),
            line: String::new(),
        })
    }

    fn elapse(&mut self, now: Instant) {
        let gap = now.saturating_duration_since(self.last_input);
        self.active += gap.min(IDLE);
        self.idle += gap.saturating_sub(IDLE);
        self.last_input = now;
    }

    /// A keystroke: the time since the last one was spent playing, up to [`IDLE`].
    pub fn key(&mut self, now: Instant) {
        self.elapse(now);
        self.keys += 1;
    }

    /// The command line that is about to play.
    pub fn command(&mut self, line: &str) {
        line.clone_into(&mut self.line);
    }

    /// Writes an entry if rounds passed since the last one; time spent on instant commands such as
    /// `skills` carries into the next entry.
    pub fn record(&mut self, game: &Game, now: Instant) -> io::Result<()> {
        if game.round_number() == self.round {
            return Ok(());
        }
        self.elapse(now);
        let p = game.player();
        writeln!(
            self.file,
            "{}",
            json!({
                "t": unix(), "from": self.round, "to": game.round_number(),
                "active_s": seconds(self.active), "idle_s": seconds(self.idle), "keys": self.keys,
                "line": self.line, "depth_before": self.depth, "depth": depth(p.at),
                "died": p.deaths > self.deaths, "deaths": p.deaths, "level": p.level, "xp": p.xp,
                "hp": p.hp, "max_hp": p.max_hp, "gold": p.gold,
            })
        )?;
        self.file.flush()?;
        self.round = game.round_number();
        self.deaths = p.deaths;
        self.depth = depth(p.at);
        self.active = Duration::ZERO;
        self.idle = Duration::ZERO;
        self.keys = 0;
        self.line.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use isle_core::Command;

    fn lines(path: &Path) -> Vec<serde_json::Value> {
        std::fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    #[test]
    fn entries_count_active_time_cap_idle_gaps_and_skip_rounds_that_did_not_pass() {
        let dir = std::env::temp_dir().join(format!("isle-playlog-{}", std::process::id()));
        let path = path_for(&dir.join("save.json"));
        assert_eq!(path.file_name().unwrap(), "save.playlog.jsonl");
        let _ = std::fs::remove_file(&path);
        let mut game = Game::new(7);
        game.answer("Wren");
        let start = Instant::now();
        let mut log = PlayLog::open(&path, &game, start).unwrap();
        log.key(start + Duration::from_secs(3));
        log.command("skills");
        log.record(&game, start + Duration::from_secs(4)).unwrap();
        log.key(start + Duration::from_secs(5));
        log.command("rest");
        game.round(Some(Command::Rest));
        log.record(&game, start + Duration::from_secs(6)).unwrap();
        log.key(start + Duration::from_secs(306));
        log.command("wait; wait");
        game.round(Some(Command::Wait));
        game.round(Some(Command::Wait));
        log.record(&game, start + Duration::from_secs(307)).unwrap();
        drop(log);
        // A reopened log keeps earlier entries and marks a new session.
        let mut log = PlayLog::open(&path, &game, start).unwrap();
        log.record(&game, start).unwrap();
        drop(log);

        let entries = lines(&path);
        assert_eq!(entries.len(), 4, "{entries:?}");
        assert_eq!(entries[0]["seed"], 7);
        assert_eq!(entries[0]["name"], "Wren");
        let first = &entries[1];
        assert_eq!((first["from"].as_u64(), first["to"].as_u64()), (Some(0), Some(1)));
        assert_eq!(first["active_s"], 6.0, "instant commands carry their time into the next round");
        assert_eq!((first["keys"].as_u64(), first["line"].as_str()), (Some(2), Some("rest")));
        let second = &entries[2];
        assert_eq!((second["from"].as_u64(), second["to"].as_u64()), (Some(1), Some(3)));
        assert_eq!(second["active_s"], 61.0, "a five-minute gap counts one idle minute");
        assert_eq!(second["idle_s"], 240.0);
        assert_eq!((second["depth"].as_u64(), second["died"].as_bool()), (Some(0), Some(false)));
        assert!(entries[3]["session"].is_u64());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
