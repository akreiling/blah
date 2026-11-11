//! Opt-in, local debugging transport. One newline-delimited JSON request per connection.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::Duration;

use gpui::Keystroke;
use serde_json::{Value, json};

const LIMIT: u64 = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);
pub const POLL: Duration = Duration::from_millis(2);

/// How much of the map a response carries.
pub enum Map {
    Off,
    Rows,
    Cells,
}

/// What a response carries beyond its outcome. Nothing, unless asked for.
pub struct View {
    pub state: bool,
    pub map: Map,
    pub game: bool,
}

/// Keystrokes to deliver in order, then what to report.
pub struct Plan {
    pub keys: Vec<Keystroke>,
    /// Stop early when a keystroke hurts you or brings a new creature into sight.
    pub interrupt: bool,
    pub view: View,
}

pub struct Request {
    pub plan: Plan,
    pub reply: SyncSender<Value>,
}

/// Owns only the socket it created. Never unlinks an existing socket or file.
pub struct Server {
    pub requests: Receiver<Request>,
    path: PathBuf,
    identity: (u64, u64),
    stop: Arc<AtomicBool>,
}

impl Server {
    pub fn bind(path: PathBuf) -> Result<Self, String> {
        let listener = UnixListener::bind(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        let (send, requests) = mpsc::sync_channel(1);
        let server =
            Self { requests, path, identity: (metadata.dev(), metadata.ino()), stop: Arc::new(AtomicBool::new(false)) };
        std::fs::set_permissions(&server.path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let stop = server.stop.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = serve(stream, &send);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(POLL),
                    Err(_) => break,
                }
            }
        });
        Ok(server)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Not joined: the worker may be waiting on the reply to the very request that is quitting the app.
        self.stop.store(true, Ordering::Relaxed);
        if std::fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.identity) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn serve(mut stream: UnixStream, send: &SyncSender<Request>) -> std::io::Result<()> {
    // Accepted sockets inherit nonblocking mode on macOS, but not on every Unix.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let mut line = String::new();
    BufReader::new((&stream).take(LIMIT + 1)).read_line(&mut line)?;
    let plan = if line.len() as u64 > LIMIT || !line.ends_with('\n') {
        Err("request must end with a newline and fit in 64 KiB".into())
    } else {
        parse(&line)
    };
    let response = match plan {
        Ok(plan) => {
            let (reply, receive) = mpsc::sync_channel(1);
            if send.try_send(Request { plan, reply }).is_err() {
                json!({"error": "control queue is busy"})
            } else {
                receive.recv_timeout(TIMEOUT).unwrap_or_else(|_| json!({"error": "game did not respond"}))
            }
        }
        Err(error) => json!({"error": error}),
    };
    let mut encoded = serde_json::to_vec(&response)?;
    encoded.push(b'\n');
    stream.write_all(&encoded)
}

fn parse(line: &str) -> Result<Plan, String> {
    let value: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let keys = match value["op"].as_str() {
        Some("state") => Vec::new(),
        Some("key" | "text") => step(&value)?,
        Some("batch") => {
            let steps = value["steps"].as_array().ok_or("batch needs a steps array")?;
            let mut keys = Vec::new();
            for s in steps {
                keys.extend(step(s)?);
            }
            keys
        }
        _ => return Err("op must be state, key, text, or batch".into()),
    };
    let state = value["op"] == "state";
    let map = match value.get("map") {
        None => Map::Off,
        Some(map) => match map.as_str() {
            Some("none") => Map::Off,
            Some("rows") => Map::Rows,
            Some("cells") => Map::Cells,
            _ => return Err("map must be none, rows, or cells".into()),
        },
    };
    let view = View { state: flag(&value, "state", state)?, map, game: flag(&value, "game", false)? };
    Ok(Plan { keys, interrupt: flag(&value, "interrupt", true)?, view })
}

fn flag(value: &Value, name: &str, default: bool) -> Result<bool, String> {
    match value.get(name) {
        None => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("{name} must be true or false")),
    }
}

fn step(value: &Value) -> Result<Vec<Keystroke>, String> {
    match (value.get("key"), value.get("text")) {
        (Some(key), None) => Ok(vec![keystroke(key.as_str().ok_or("key needs a string")?)?]),
        (None, Some(text)) => typed(text.as_str().ok_or("text needs a string")?),
        _ => Err("each step needs exactly one of key or text".into()),
    }
}

fn keystroke(key: &str) -> Result<Keystroke, String> {
    let mut stroke = Keystroke::parse(key).map_err(|e| e.to_string())?;
    // GPUI's parser describes bindings, not typed text. Supply printable key characters too.
    if stroke.key_char.is_none() && !stroke.modifiers.control && !stroke.modifiers.platform {
        stroke.key_char = match stroke.key.as_str() {
            "space" => Some(" ".into()),
            key if key.chars().count() == 1 => {
                Some(if stroke.modifiers.shift { key.to_uppercase() } else { key.into() })
            }
            _ => None,
        };
    }
    Ok(stroke)
}

fn typed(text: &str) -> Result<Vec<Keystroke>, String> {
    if text.chars().any(|c| c.is_control() && c != '\n') {
        return Err("text accepts printable characters and newlines only".into());
    }
    Ok(text
        .chars()
        .map(|c| {
            if c == '\n' {
                Keystroke { key: "enter".into(), ..Default::default() }
            } else {
                Keystroke { key: c.to_string(), key_char: Some(c.to_string()), ..Default::default() }
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_preserves_punctuation_editing_and_newlines() {
        let keys = parse(r#"{"op":"text","text":"A>,\n"}"#).unwrap().keys;
        assert_eq!(keys.len(), 4);
        assert_eq!(keys[1].key_char.as_deref(), Some(">"));
        assert_eq!(keys[3].key, "enter");
        let keys = parse(r#"{"op":"key","key":"ctrl-a"}"#).unwrap().keys;
        assert!(keys[0].modifiers.control && keys[0].key_char.is_none());
        let keys = parse(r#"{"op":"key","key":"shift-,"}"#).unwrap().keys;
        assert!(keys[0].modifiers.shift);
        assert!(parse(r#"{"op":"text","text":"\t"}"#).is_err());
        assert!(parse(r#"{"op":"unknown"}"#).is_err());
    }

    #[test]
    fn batches_flatten_steps_in_order_and_validate_options() {
        let plan = parse(r#"{"op":"batch","steps":[{"key":"right"},{"text":"n\n"},{"key":"down"}],"interrupt":false,"map":"none","game":true}"#).unwrap();
        let keys: Vec<&str> = plan.keys.iter().map(|k| k.key.as_str()).collect();
        assert_eq!(keys, ["right", "n", "enter", "down"]);
        assert!(!plan.interrupt && plan.view.game && !plan.view.state && matches!(plan.view.map, Map::Off));
        let plan = parse(r#"{"op":"batch","steps":[{"key":"up"}],"state":true,"map":"rows"}"#).unwrap();
        assert!(plan.interrupt && plan.view.state && matches!(plan.view.map, Map::Rows));
        let plan = parse(r#"{"op":"state"}"#).unwrap();
        assert!(plan.keys.is_empty() && plan.view.state && !plan.view.game && matches!(plan.view.map, Map::Off));
        assert!(!parse(r#"{"op":"key","key":"up"}"#).unwrap().view.state);
        for bad in [
            r#"{"op":"batch"}"#,
            r#"{"op":"batch","steps":[{"key":"a","text":"b"}]}"#,
            r#"{"op":"batch","steps":[{}]}"#,
            r#"{"op":"state","map":"all"}"#,
            r#"{"op":"state","game":1}"#,
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn socket_serves_json_and_never_replaces_an_existing_path() {
        let dir = std::env::temp_dir().join(format!("isle-control-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("control.sock");
        let server = Server::bind(path.clone()).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(Server::bind(path.clone()).is_err());
        let client_path = path.clone();
        let client = thread::spawn(move || {
            let mut stream = UnixStream::connect(client_path).unwrap();
            stream.set_read_timeout(Some(TIMEOUT)).unwrap();
            stream.write_all(b"{\"op\":\"state\"}\n").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str::<Value>(&line).unwrap()
        });
        let request = server.requests.recv_timeout(TIMEOUT).unwrap();
        assert!(request.plan.keys.is_empty());
        let response = json!({"round": 7, "map": "x".repeat(512 * 1024)});
        request.reply.send(response.clone()).unwrap();
        assert_eq!(client.join().unwrap(), response);
        drop(server);
        assert!(!path.exists());
        std::fs::write(&path, "keep me").unwrap();
        assert!(Server::bind(path.clone()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
