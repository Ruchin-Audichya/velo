//! `velo-cli mcp --serve`: Velo as an MCP server, so Claude (or any MCP
//! client) can read and send WhatsApp messages through the daemon the way the
//! Chrome extension lets it use a browser.
//!
//! Every tool is one of this binary's own `--json` commands, run as a child
//! process. Nothing about the protocol, the account or the daemon is written a
//! second time here: what a tool can do is exactly what the CLI can, with the
//! same refusals, and a fix to a command is a fix to its tool.
//!
//! Speaks MCP over stdio: newline-delimited JSON-RPC 2.0. stdout carries only
//! protocol frames; anything worth saying otherwise goes to stderr.
//!
//! Register it with Claude Code:
//!
//! ```text
//! claude mcp add velo -- "%LOCALAPPDATA%\Programs\Velo\velo-cli.exe" mcp --serve
//! ```

use std::io::{BufRead as _, Write as _};
use std::process::{Command, ExitCode};

use serde_json::{Value, json};

const PROTOCOL: &str = "2024-11-05";

struct Tool {
    name: &'static str,
    description: &'static str,
    schema: fn() -> Value,
    /// The CLI arguments this call runs, after `--json`.
    args: fn(&Value) -> Result<Vec<String>, String>,
}

fn text(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("`{key}` is required"))
}

fn optional(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn limit(args: &Value, default: u64) -> String {
    args.get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(default)
        .clamp(1, 500)
        .to_string()
}

fn chat_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "properties": { "chat": { "type": "string", "description": description } },
        "required": ["chat"],
    })
}

const CHAT: &str = "Chat JID, e.g. 5511999999999@s.whatsapp.net or 1203...@g.us (from list_chats)";

fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "status",
            description: "Velo's connection state and which WhatsApp account it is signed in as.",
            schema: || json!({ "type": "object", "properties": {} }),
            args: |_| Ok(vec!["status".into()]),
        },
        Tool {
            name: "list_chats",
            description: "The chat list, newest activity first: name, JID, unread count and last message. \
                          `query` filters by name or JID.",
            schema: || {
                json!({
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "description": "default 30" },
                        "query": { "type": "string" },
                        "archived": { "type": "boolean", "description": "include archived chats" },
                    },
                })
            },
            args: |a| {
                let mut out = vec![
                    "chats".into(),
                    "list".into(),
                    "--limit".into(),
                    limit(a, 30),
                ];
                if let Some(query) = optional(a, "query") {
                    out.extend(["--query".into(), query]);
                }
                if a.get("archived").and_then(Value::as_bool) == Some(true) {
                    out.push("--archived".into());
                }
                Ok(out)
            },
        },
        Tool {
            name: "read_messages",
            description: "Recent messages in one chat, with sender, time and text. Pass `before` \
                          (a previous page's next_cursor) to read further back.",
            schema: || {
                json!({
                    "type": "object",
                    "properties": {
                        "chat": { "type": "string", "description": CHAT },
                        "limit": { "type": "integer", "description": "default 30" },
                        "before": { "type": "string" },
                    },
                    "required": ["chat"],
                })
            },
            args: |a| {
                let mut out = vec![
                    "messages".into(),
                    "list".into(),
                    text(a, "chat")?,
                    "--limit".into(),
                    limit(a, 30),
                ];
                if let Some(before) = optional(a, "before") {
                    out.extend(["--before".into(), before]);
                }
                Ok(out)
            },
        },
        Tool {
            name: "search_messages",
            description: "Full-text search over stored messages, optionally in one chat.",
            schema: || {
                json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "chat": { "type": "string", "description": CHAT },
                        "limit": { "type": "integer", "description": "default 20" },
                    },
                    "required": ["query"],
                })
            },
            args: |a| {
                let mut out = vec![
                    "messages".into(),
                    "search".into(),
                    text(a, "query")?,
                    "--limit".into(),
                    limit(a, 20),
                ];
                if let Some(chat) = optional(a, "chat") {
                    out.extend(["--chat".into(), chat]);
                }
                Ok(out)
            },
        },
        Tool {
            name: "search_contacts",
            description: "Saved contacts whose name or number matches `query`.",
            schema: || {
                json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "limit": { "type": "integer", "description": "default 20" },
                    },
                })
            },
            args: |a| {
                let mut out = vec!["contacts".into(), "search".into()];
                if let Some(query) = optional(a, "query") {
                    out.push(query);
                }
                out.extend(["--limit".into(), limit(a, 20)]);
                Ok(out)
            },
        },
        Tool {
            name: "send_message",
            description: "Send a WhatsApp text message as the signed-in account. This really sends \
                          it: confirm the recipient and the exact text with the person first. \
                          `to` is a JID or a phone number with country code.",
            schema: || {
                json!({
                    "type": "object",
                    "properties": {
                        "to": { "type": "string" },
                        "text": { "type": "string" },
                        "reply_to": { "type": "string", "description": "message ID to quote" },
                    },
                    "required": ["to", "text"],
                })
            },
            args: |a| {
                let mut out = vec![
                    "send".into(),
                    "text".into(),
                    "--to".into(),
                    text(a, "to")?,
                    text(a, "text")?,
                ];
                if let Some(reply) = optional(a, "reply_to") {
                    out.extend(["--reply-to".into(), reply]);
                }
                Ok(out)
            },
        },
        Tool {
            name: "mark_read",
            description: "Mark a chat as read, which sends read receipts as opening it would.",
            schema: || chat_schema(CHAT),
            args: |a| Ok(vec!["chats".into(), "mark-read".into(), text(a, "chat")?]),
        },
    ]
}

/// Run one tool: this binary again, with `--json`, and its envelope back.
fn call(tool: &Tool, arguments: &Value) -> (String, bool) {
    let args = match (tool.args)(arguments) {
        Ok(args) => args,
        Err(problem) => return (problem, true),
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => return (format!("velo-cli could not find itself: {error}"), true),
    };
    let mut command = Command::new(exe);
    command.arg("--json").args(&args);
    hide_console(&mut command);
    match command.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if output.status.success() {
                (stdout, false)
            } else if stderr.is_empty() {
                (stdout, true)
            } else {
                (stderr, true)
            }
        }
        Err(error) => (format!("velo-cli could not run: {error}"), true),
    }
}

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_: &mut Command) {}

fn answer(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The answer to one request, or `None` for a notification.
fn handle(request: &Value, tools: &[Tool]) -> Option<Value> {
    let id = request.get("id")?.clone();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    Some(match method {
        "initialize" => answer(
            &id,
            json!({
                "protocolVersion": PROTOCOL,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "velo", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Velo is the person's WhatsApp on Windows. Read freely; before \
                                 send_message, confirm the recipient and exact text with them.",
            }),
        ),
        "ping" => answer(&id, json!({})),
        "tools/list" => answer(
            &id,
            json!({
                "tools": tools.iter().map(|tool| json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": (tool.schema)(),
                })).collect::<Vec<_>>(),
            }),
        ),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(Value::Null);
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            match tools.iter().find(|tool| tool.name == name) {
                Some(tool) => {
                    let (text, is_error) = call(tool, &arguments);
                    answer(
                        &id,
                        json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": is_error,
                        }),
                    )
                }
                None => failure(&id, -32602, &format!("no tool called {name}")),
            }
        }
        _ => failure(&id, -32601, &format!("method not found: {method}")),
    })
}

/// Serve MCP on stdin/stdout until the client closes stdin.
pub fn serve() -> ExitCode {
    let tools = tools();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle(&request, &tools),
            Err(error) => Some(failure(
                &Value::Null,
                -32700,
                &format!("parse error: {error}"),
            )),
        };
        if let Some(reply) = reply
            && (writeln!(stdout, "{reply}").is_err() || stdout.flush().is_err())
        {
            break;
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_introduces_itself_and_lists_its_tools() {
        let tools = tools();
        let init = handle(
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
            &tools,
        )
        .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "velo");
        let list = handle(
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            &tools,
        )
        .unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"send_message") && names.contains(&"read_messages"));
        // A notification has no id and gets no answer.
        assert!(
            handle(
                &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                &tools
            )
            .is_none()
        );
    }

    #[test]
    fn tools_become_the_cli_commands_they_stand_for() {
        let tools = tools();
        let find = |name: &str| tools.iter().find(|tool| tool.name == name).unwrap();
        assert_eq!(
            (find("send_message").args)(&json!({"to":"5511999990001","text":"hi"})).unwrap(),
            ["send", "text", "--to", "5511999990001", "hi"]
        );
        assert_eq!(
            (find("read_messages").args)(&json!({"chat":"1@g.us","limit":5})).unwrap(),
            ["messages", "list", "1@g.us", "--limit", "5"]
        );
        assert!((find("send_message").args)(&json!({"to":"x"})).is_err());
    }
}
