//! A small readline-style command line. Plain arrows belong to the game, modified keys to the editor.

use gpui::Keystroke;
use isle_core::Dir;

const LIMIT: usize = 64;
const HISTORY: usize = 100;

#[derive(Debug, PartialEq, Eq)]
pub enum InputAction {
    Edit,
    Submit,
    Move(Dir),
    Command(&'static str),
    Ignore,
}

#[derive(Default)]
pub struct CommandLine {
    text: String,
    /// A byte offset, always on a character boundary.
    cursor: usize,
    killed: String,
    history: Vec<String>,
    recalled: Option<usize>,
    draft: (String, usize),
}

impl CommandLine {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn halves(&self) -> (&str, &str) {
        self.text.split_at(self.cursor)
    }

    /// Submit the line, or repeat the last submitted line when empty. Movement never enters history.
    pub fn submit(&mut self) -> Option<String> {
        let text = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.recalled = None;
        self.draft = (String::new(), 0);
        let line = text.trim();
        if line.is_empty() {
            return self.history.last().cloned();
        }
        let line = line.to_owned();
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
            if self.history.len() > HISTORY {
                self.history.remove(0);
            }
        }
        Some(line)
    }

    pub fn key(&mut self, stroke: &Keystroke, playing: bool) -> InputAction {
        let m = stroke.modifiers;
        let key = stroke.key.as_str();
        if m.platform {
            return InputAction::Ignore;
        }
        if m.control {
            if m.alt {
                return InputAction::Ignore;
            }
            match key {
                "a" => self.cursor = 0,
                "e" => self.cursor = self.text.len(),
                "b" => self.cursor = self.previous(),
                "f" => self.cursor = self.next(),
                "h" => self.erase(self.previous(), self.cursor),
                "d" => self.erase(self.cursor, self.next()),
                "w" => {
                    let mut start = self.cursor;
                    while start > 0 && self.text[..start].chars().next_back().unwrap().is_whitespace() {
                        start = self.previous_at(start);
                    }
                    while start > 0 && !self.text[..start].chars().next_back().unwrap().is_whitespace() {
                        start = self.previous_at(start);
                    }
                    self.kill(start, self.cursor);
                }
                "k" => self.kill(self.cursor, self.text.len()),
                "u" => self.kill(0, self.cursor),
                "y" => self.insert(&self.killed.clone()),
                "p" => self.recall(true),
                "n" => self.recall(false),
                _ => return InputAction::Ignore,
            }
        } else if m.alt && matches!(key, "left" | "b" | "right" | "f" | "backspace" | "d" | "delete") {
            match key {
                "left" | "b" => self.cursor = self.word_left(),
                "right" | "f" => self.cursor = self.word_right(),
                "backspace" => self.kill(self.word_left(), self.cursor),
                "d" | "delete" => self.kill(self.cursor, self.word_right()),
                _ => unreachable!(),
            }
        } else if !m.alt {
            if playing && self.is_empty() {
                let command = match stroke.key_char.as_deref().unwrap_or(key) {
                    "<" => Some("up"),
                    ">" => Some("down"),
                    "," => Some("get all"),
                    _ => None,
                };
                if let Some(command) = command {
                    return InputAction::Command(command);
                }
            }
            match key {
                "enter" => return InputAction::Submit,
                "up" | "down" | "left" | "right" if playing && !m.shift => {
                    let dir = match key {
                        "up" => Dir::N,
                        "down" => Dir::S,
                        "left" => Dir::W,
                        "right" => Dir::E,
                        _ => unreachable!(),
                    };
                    return InputAction::Move(dir);
                }
                "left" if !m.shift => self.cursor = self.previous(),
                "right" if !m.shift => self.cursor = self.next(),
                "up" if !m.shift => self.recall(true),
                "down" if !m.shift => self.recall(false),
                "home" => self.cursor = 0,
                "end" => self.cursor = self.text.len(),
                "backspace" => self.erase(self.previous(), self.cursor),
                "delete" => self.erase(self.cursor, self.next()),
                "escape" => {
                    self.text.clear();
                    self.cursor = 0;
                    self.recalled = None;
                }
                _ => {
                    if let Some(typed) = &stroke.key_char {
                        self.insert(typed);
                    } else {
                        return InputAction::Ignore;
                    }
                }
            }
        } else if let Some(typed) = &stroke.key_char {
            // Option can also produce printable characters on non-US keyboard layouts.
            self.insert(typed);
        } else {
            return InputAction::Ignore;
        }
        InputAction::Edit
    }

    fn insert(&mut self, typed: &str) {
        let room = LIMIT.saturating_sub(self.text.chars().count());
        let typed: String = typed.chars().filter(|c| !c.is_control()).take(room).collect();
        self.text.insert_str(self.cursor, &typed);
        self.cursor += typed.len();
    }

    fn previous_at(&self, cursor: usize) -> usize {
        self.text[..cursor].char_indices().next_back().map_or(0, |(i, _)| i)
    }

    fn previous(&self) -> usize {
        self.previous_at(self.cursor)
    }

    fn next(&self) -> usize {
        self.cursor + self.text[self.cursor..].chars().next().map_or(0, char::len_utf8)
    }

    fn word_left(&self) -> usize {
        let mut start = self.cursor;
        for (i, c) in self.text[..self.cursor].char_indices().rev() {
            if c.is_alphanumeric() || c == '_' {
                break;
            }
            start = i;
        }
        while start > 0 {
            let c = self.text[..start].chars().next_back().unwrap();
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            start = self.previous_at(start);
        }
        start
    }

    fn word_right(&self) -> usize {
        let mut end = self.cursor;
        let mut in_word = false;
        for c in self.text[self.cursor..].chars() {
            let word = c.is_alphanumeric() || c == '_';
            if in_word && !word {
                break;
            }
            in_word |= word;
            end += c.len_utf8();
        }
        end
    }

    fn erase(&mut self, start: usize, end: usize) {
        self.text.replace_range(start..end, "");
        self.cursor = start;
    }

    fn kill(&mut self, start: usize, end: usize) {
        if start != end {
            self.killed = self.text[start..end].to_owned();
            self.erase(start, end);
        }
    }

    fn recall(&mut self, previous: bool) {
        let index = match (self.recalled, previous) {
            (None, true) if !self.history.is_empty() => {
                self.draft = (self.text.clone(), self.cursor);
                self.history.len() - 1
            }
            (Some(i), true) => i.saturating_sub(1),
            (Some(i), false) if i + 1 < self.history.len() => i + 1,
            (Some(_), false) => {
                (self.text, self.cursor) = self.draft.clone();
                self.recalled = None;
                return;
            }
            _ => return,
        };
        self.recalled = Some(index);
        self.text = self.history[index].clone();
        self.cursor = self.text.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(line: &mut CommandLine, key: &str) -> InputAction {
        line.key(&Keystroke::parse(key).unwrap(), true)
    }

    fn type_text(line: &mut CommandLine, text: &str) {
        line.key(&Keystroke { key_char: Some(text.into()), ..Default::default() }, true);
    }

    #[test]
    fn punctuation_shortcuts_act_immediately_only_on_an_empty_play_line() {
        for (physical, typed, command) in [("shift-,", "<", "up"), ("shift-.", ">", "down"), (",", ",", "get all")] {
            let mut stroke = Keystroke::parse(physical).unwrap();
            stroke.key_char = Some(typed.into());
            let mut line = CommandLine::default();
            type_text(&mut line, "look");
            line.submit();
            assert_eq!(line.key(&stroke, true), InputAction::Command(command));
            assert!(line.is_empty());
            assert_eq!(line.submit().as_deref(), Some("look"), "shortcuts leave typed history alone");

            type_text(&mut line, "cast ");
            assert_eq!(line.key(&stroke, true), InputAction::Edit);
            assert_eq!(line.halves(), (format!("cast {typed}").as_str(), ""));
            key(&mut line, "ctrl-a");
            assert_eq!(line.key(&stroke, true), InputAction::Edit, "a cursor at zero is not an empty line");
            assert_eq!(line.halves(), (typed, format!("cast {typed}").as_str()));

            let mut creating = CommandLine::default();
            assert_eq!(creating.key(&stroke, false), InputAction::Edit);
            assert_eq!(creating.halves(), (typed, ""));

            let mut modified = CommandLine::default();
            stroke.modifiers.control = true;
            assert_eq!(modified.key(&stroke, true), InputAction::Ignore);
            assert!(modified.is_empty());
            stroke.modifiers.control = false;
            stroke.modifiers.platform = true;
            assert_eq!(modified.key(&stroke, true), InputAction::Ignore);
            stroke.modifiers.platform = false;
            stroke.modifiers.alt = true;
            assert_eq!(modified.key(&stroke, true), InputAction::Edit);
            assert_eq!(modified.halves(), (typed, ""), "Option text is not a game shortcut");
        }
    }

    #[test]
    fn requested_readline_keys_edit_at_the_cursor() {
        let mut line = CommandLine::default();
        type_text(&mut line, "buy long sword");
        key(&mut line, "alt-left");
        assert_eq!(line.halves(), ("buy long ", "sword"));
        key(&mut line, "ctrl-w");
        assert_eq!(line.halves(), ("buy ", "sword"));
        type_text(&mut line, "short ");
        key(&mut line, "ctrl-a");
        assert_eq!(line.halves(), ("", "buy short sword"));
        key(&mut line, "alt-right");
        assert_eq!(line.halves(), ("buy", " short sword"));
        key(&mut line, "ctrl-k");
        assert_eq!(line.halves(), ("buy", ""));
        key(&mut line, "ctrl-y");
        assert_eq!(line.halves(), ("buy short sword", ""));
        key(&mut line, "ctrl-a");
        type_text(&mut line, "list; ");
        key(&mut line, "ctrl-e");
        assert_eq!(line.halves(), ("list; buy short sword", ""));
    }

    #[test]
    fn character_motion_and_deletion_work_in_the_middle() {
        let mut line = CommandLine::default();
        type_text(&mut line, "loXok!");
        key(&mut line, "home");
        key(&mut line, "ctrl-f");
        key(&mut line, "ctrl-f");
        key(&mut line, "ctrl-d");
        assert_eq!(line.halves(), ("lo", "ok!"));
        key(&mut line, "end");
        key(&mut line, "backspace");
        key(&mut line, "ctrl-b");
        key(&mut line, "ctrl-h");
        assert_eq!(line.halves(), ("lo", "k"));
        type_text(&mut line, "o");
        key(&mut line, "delete");
        assert_eq!(line.submit().as_deref(), Some("loo"));
    }

    #[test]
    fn plain_arrows_preserve_text_cursor_and_history() {
        let mut line = CommandLine::default();
        type_text(&mut line, "look");
        assert_eq!(line.submit().as_deref(), Some("look"));
        type_text(&mut line, "buy potion");
        key(&mut line, "alt-left");
        for (stroke, dir) in [("up", Dir::N), ("down", Dir::S), ("left", Dir::W), ("right", Dir::E)] {
            assert_eq!(key(&mut line, stroke), InputAction::Move(dir));
            assert_eq!(line.halves(), ("buy ", "potion"));
        }
        key(&mut line, "ctrl-p");
        assert_eq!(line.halves(), ("look", ""));
        key(&mut line, "ctrl-n");
        assert_eq!(line.halves(), ("buy ", "potion"));
        key(&mut line, "escape");
        assert_eq!(line.submit().as_deref(), Some("look"), "arrow movement never replaces the repeated command");
    }

    #[test]
    fn history_preserves_a_draft_and_ignores_duplicate_submissions() {
        let mut line = CommandLine::default();
        for text in ["look", "inventory", "inventory"] {
            type_text(&mut line, text);
            line.submit();
        }
        type_text(&mut line, "cast goblin");
        key(&mut line, "ctrl-a");
        key(&mut line, "ctrl-p");
        assert_eq!(line.halves(), ("inventory", ""));
        key(&mut line, "ctrl-p");
        key(&mut line, "ctrl-p");
        assert_eq!(line.halves(), ("look", ""));
        key(&mut line, "ctrl-n");
        key(&mut line, "ctrl-n");
        key(&mut line, "ctrl-n");
        assert_eq!(line.halves(), ("", "cast goblin"));
        assert_eq!(line.history.len(), 2);
    }

    #[test]
    fn word_motion_skips_punctuation_but_ctrl_w_uses_whitespace() {
        let mut line = CommandLine::default();
        type_text(&mut line, "look;shoot goblin_2  ");
        key(&mut line, "alt-b");
        assert_eq!(line.halves(), ("look;shoot ", "goblin_2  "));
        key(&mut line, "alt-left");
        assert_eq!(line.halves(), ("look;", "shoot goblin_2  "));
        key(&mut line, "alt-left");
        assert_eq!(line.halves(), ("", "look;shoot goblin_2  "));
        key(&mut line, "alt-f");
        assert_eq!(line.halves(), ("look", ";shoot goblin_2  "));
        key(&mut line, "ctrl-e");
        key(&mut line, "ctrl-w");
        assert_eq!(line.halves(), ("look;shoot ", ""));
        key(&mut line, "ctrl-w");
        assert!(line.is_empty());
    }

    #[test]
    fn kill_and_yank_preserve_text_and_empty_kills_do_not_erase_the_buffer() {
        let mut line = CommandLine::default();
        type_text(&mut line, "chant shield");
        key(&mut line, "alt-backspace");
        assert_eq!(line.halves(), ("chant ", ""));
        key(&mut line, "ctrl-k");
        key(&mut line, "ctrl-y");
        assert_eq!(line.halves(), ("chant shield", ""));
        key(&mut line, "alt-left");
        key(&mut line, "ctrl-u");
        assert_eq!(line.halves(), ("", "shield"));
        key(&mut line, "ctrl-y");
        assert_eq!(line.halves(), ("chant ", "shield"));
        key(&mut line, "alt-d");
        key(&mut line, "ctrl-y");
        assert_eq!(line.halves(), ("chant shield", ""));
    }

    #[test]
    fn unicode_edits_stay_on_character_boundaries_and_limits_count_characters() {
        let mut line = CommandLine::default();
        type_text(&mut line, "éclair 狼!");
        key(&mut line, "alt-left");
        assert_eq!(line.halves(), ("éclair ", "狼!"));
        key(&mut line, "ctrl-d");
        type_text(&mut line, "猫");
        key(&mut line, "ctrl-b");
        key(&mut line, "backspace");
        assert_eq!(line.halves(), ("éclair", "猫!"));
        key(&mut line, "escape");
        type_text(&mut line, &"é".repeat(LIMIT + 5));
        assert_eq!(line.text.chars().count(), LIMIT);
        key(&mut line, "ctrl-a");
        key(&mut line, "ctrl-d");
        type_text(&mut line, "猫猫");
        assert_eq!(line.text.chars().count(), LIMIT);
        assert_eq!(line.cursor, "猫".len());
    }

    #[test]
    fn editing_at_empty_and_end_boundaries_is_safe() {
        let mut line = CommandLine::default();
        for stroke in [
            "ctrl-a",
            "ctrl-e",
            "ctrl-b",
            "ctrl-f",
            "ctrl-d",
            "backspace",
            "delete",
            "ctrl-w",
            "ctrl-k",
            "ctrl-u",
            "ctrl-y",
            "ctrl-p",
            "ctrl-n",
            "alt-left",
            "alt-right",
            "alt-backspace",
            "alt-d",
        ] {
            assert_eq!(key(&mut line, stroke), InputAction::Edit);
            assert_eq!(line.halves(), ("", ""));
        }
        assert_eq!(line.submit(), None);
        type_text(&mut line, "a");
        key(&mut line, "ctrl-f");
        key(&mut line, "ctrl-d");
        assert_eq!(line.halves(), ("a", ""));
    }

    #[test]
    fn modifiers_never_turn_editing_shortcuts_into_movement_or_control_text() {
        let mut line = CommandLine::default();
        for stroke in ["alt-left", "alt-right", "ctrl-a", "ctrl-e", "ctrl-w", "ctrl-k"] {
            assert_eq!(key(&mut line, stroke), InputAction::Edit);
        }
        for stroke in ["cmd-q", "ctrl-q", "ctrl-alt-left", "shift-up"] {
            assert_eq!(key(&mut line, stroke), InputAction::Ignore);
        }
        type_text(&mut line, "attack a; cast! \n\r\t");
        assert_eq!(line.halves(), ("attack a; cast! ", ""));
        assert_eq!(key(&mut line, "enter"), InputAction::Submit);
        assert_eq!(line.halves(), ("attack a; cast! ", ""));
    }

    #[test]
    fn arrows_edit_instead_of_moving_during_character_creation() {
        let mut line = CommandLine::default();
        type_text(&mut line, "Ash");
        assert_eq!(line.key(&Keystroke::parse("left").unwrap(), false), InputAction::Edit);
        assert_eq!(line.halves(), ("As", "h"));
        assert_eq!(line.key(&Keystroke::parse("right").unwrap(), false), InputAction::Edit);
        assert_eq!(line.halves(), ("Ash", ""));
    }

    #[test]
    fn history_is_bounded_without_losing_the_newest_command() {
        let mut line = CommandLine::default();
        for i in 0..HISTORY + 5 {
            type_text(&mut line, &format!("look {i}"));
            line.submit();
        }
        assert_eq!(line.history.len(), HISTORY);
        assert_eq!(line.history[0], "look 5");
        assert_eq!(line.submit(), Some(format!("look {}", HISTORY + 4)));
    }
}
