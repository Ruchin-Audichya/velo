"""Velo inspector: an MCP server for looking at and driving the Velo window.

Lets an agent (Claude Code, or any MCP client) debug the interface the way a
person would: take a screenshot, read the controls Windows can see, click,
type, scroll, and check memory and connection state.

Standard library only, so it runs with any Python 3.10+ and installs nothing.
Speaks MCP over stdio (newline-delimited JSON-RPC 2.0). Every action goes
through inspector.ps1 beside this file, which only ever touches Velo's own
window.

Register it with Claude Code, from the repository root:

    claude mcp add velo-inspector -- python tools/velo-inspector/velo_inspector.py

Windows only: the helper uses Win32 and UI Automation.
"""

import base64
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
HELPER = os.path.join(HERE, "inspector.ps1")
PROTOCOL = "2024-11-05"

TOOLS = [
    {
        "name": "screenshot",
        "description": "Capture the Velo window as a PNG. Coordinates in the image, divided by "
        "`scale`, are what `click` and `scroll` take.",
        "inputSchema": {
            "type": "object",
            "properties": {"scale": {"type": "number", "description": "0.2–1.0, default 0.6"}},
        },
    },
    {
        "name": "ui_tree",
        "description": "List the controls Windows UI Automation can see in Velo: type, name, "
        "automation id, and window-relative rectangle.",
        "inputSchema": {"type": "object", "properties": {"depth": {"type": "integer"}}},
    },
    {
        "name": "click",
        "description": "Click at window-relative physical pixels (screenshot pixels / scale). "
        "`right: true` right-clicks, which opens context menus.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x": {"type": "integer"},
                "y": {"type": "integer"},
                "right": {"type": "boolean"},
            },
            "required": ["x", "y"],
        },
    },
    {
        "name": "click_element",
        "description": "Invoke a control by its automation id or by its accessible name.",
        "inputSchema": {
            "type": "object",
            "properties": {"id": {"type": "string"}, "name": {"type": "string"}},
        },
    },
    {
        "name": "keys",
        "description": "Send keystrokes in .NET SendKeys syntax: ^ Ctrl, + Shift, % Alt, "
        "{ESC}, {DOWN}. Example: '^,' opens Settings.",
        "inputSchema": {
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
        },
    },
    {
        "name": "scroll",
        "description": "Scroll the wheel at window-relative pixels. Negative delta scrolls down.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x": {"type": "integer"},
                "y": {"type": "integer"},
                "delta": {"type": "integer", "description": "default -360"},
            },
            "required": ["x", "y"],
        },
    },
    {
        "name": "status",
        "description": "Window size, window and background-process memory, and session state.",
        "inputSchema": {"type": "object", "properties": {}},
    },
]


def run_helper(*args):
    shell = "pwsh" if _has("pwsh") else "powershell"
    proc = subprocess.run(
        [shell, "-NoProfile", "-NonInteractive", "-File", HELPER, *args],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if proc.returncode != 0:
        raise RuntimeError((proc.stderr or proc.stdout).strip() or "inspector failed")
    return proc.stdout.strip()


def _has(program):
    for folder in os.environ.get("PATH", "").split(os.pathsep):
        if os.path.exists(os.path.join(folder, program + ".exe")):
            return True
    return False


def call_tool(name, args):
    if name == "screenshot":
        scale = float(args.get("scale", 0.6))
        scale = min(max(scale, 0.2), 1.0)
        handle, path = tempfile.mkstemp(suffix=".png", prefix="velo-")
        os.close(handle)
        try:
            meta = json.loads(run_helper("-Action", "screenshot", "-Out", path, "-Scale", str(scale)))
            with open(path, "rb") as image:
                data = base64.b64encode(image.read()).decode("ascii")
        finally:
            os.remove(path)
        return [
            {"type": "image", "data": data, "mimeType": "image/png"},
            {"type": "text", "text": json.dumps(meta)},
        ]
    if name == "ui_tree":
        return [{"type": "text", "text": run_helper("-Action", "tree", "-Depth", str(int(args.get("depth", 12))))}]
    if name == "click":
        action = "rightclick" if args.get("right") else "click"
        return [{"type": "text", "text": run_helper("-Action", action, "-X", str(int(args["x"])), "-Y", str(int(args["y"])))}]
    if name == "click_element":
        if args.get("id"):
            out = run_helper("-Action", "element", "-Id", args["id"])
        elif args.get("name"):
            out = run_helper("-Action", "element", "-Name", args["name"])
        else:
            raise ValueError("Pass `id` or `name`.")
        return [{"type": "text", "text": out}]
    if name == "keys":
        return [{"type": "text", "text": run_helper("-Action", "keys", "-Text", args["text"])}]
    if name == "scroll":
        return [{"type": "text", "text": run_helper(
            "-Action", "scroll", "-X", str(int(args["x"])), "-Y", str(int(args["y"])),
            "-Delta", str(int(args.get("delta", -360))))}]
    if name == "status":
        return [{"type": "text", "text": run_helper("-Action", "status")}]
    raise ValueError(f"Unknown tool: {name}")


def respond(message_id, result=None, error=None):
    reply = {"jsonrpc": "2.0", "id": message_id}
    if error is not None:
        reply["error"] = error
    else:
        reply["result"] = result
    sys.stdout.write(json.dumps(reply) + "\n")
    sys.stdout.flush()


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        method = message.get("method")
        message_id = message.get("id")
        if message_id is None:
            continue  # a notification, e.g. notifications/initialized
        if method == "initialize":
            respond(message_id, {
                "protocolVersion": PROTOCOL,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "velo-inspector", "version": "0.1.0"},
            })
        elif method == "tools/list":
            respond(message_id, {"tools": TOOLS})
        elif method == "tools/call":
            params = message.get("params", {})
            try:
                content = call_tool(params.get("name"), params.get("arguments") or {})
                respond(message_id, {"content": content, "isError": False})
            except Exception as error:  # reported to the client, not raised
                respond(message_id, {"content": [{"type": "text", "text": str(error)}], "isError": True})
        elif method == "ping":
            respond(message_id, {})
        else:
            respond(message_id, error={"code": -32601, "message": f"Method not found: {method}"})


if __name__ == "__main__":
    main()
