# Contributing to Velo

Thanks for wanting to help. Velo is a native WhatsApp client for Windows, written in Rust and drawn on the GPU with GPUI. It exists to be fast and light, and contributions of every size are welcome: a typo fix, a bug report, a translation or a whole feature.

## Ways to help

- **Report a bug.** [Open an issue](https://github.com/Ruchin-Audichya/velo/issues/new/choose) with what you did, what you expected and what happened. A screenshot helps a lot.
- **Suggest a feature.** Use the feature request form. Say what problem it solves for you, not only what it should look like.
- **Pick up an issue.** Anything labelled [`good first issue`](https://github.com/Ruchin-Audichya/velo/labels/good%20first%20issue) is a gentle way in. Comment on it first so two people don't do the same work.
- **Ask or share ideas** in [Discussions](https://github.com/Ruchin-Audichya/velo/discussions).

For anything big (a new screen, a protocol change, a new dependency), open an issue to talk it through before writing code. That saves you from building something that can't be merged.

## Set up

You need:

- **Rust.** The version is pinned in `rust-toolchain.toml`, so `rustup` installs the right one on its own.
- **On Windows, the MSVC build tools** (Visual Studio Build Tools with "Desktop development with C++").
- `git`.

```bash
git clone https://github.com/Ruchin-Audichya/velo
cd velo
cargo build
./target/debug/velo
```

That builds the window (`velo`), the background service that holds the WhatsApp session (`velod`) and the command-line client (`velo-cli`). The window starts `velod` beside it automatically. Link it like WhatsApp Web: *Settings → Linked devices → Link a device* on your phone.

A tip if you already use Velo day to day: a development build shares the same sign-in and data folder (`%LOCALAPPDATA%\velo`) as your installed copy, so close the installed Velo first.

## Before you open a pull request

Run what CI runs (the exact flags live in `.github/workflows/ci.yml`):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Then:

1. **Work on a branch** of your fork, named for the change (`fix-badge-clipping`, `feature-message-search`).
2. **Keep it focused.** One fix or feature per pull request is far easier to review than five.
3. **Explain why**, not just what, in the description. For anything visible, add a before/after screenshot.
4. **Add a test** when you fix a bug, so it stays fixed.

## How the code is laid out

Start with [AGENTS.md](AGENTS.md). It is short and lists the decisions that aren't obvious from the code. Then read [docs/architecture.md](docs/architecture.md) and [docs/gotchas.md](docs/gotchas.md) for the area you're touching. The rules people trip over most:

- **One WhatsApp session, in the daemon.** The window (`crates/gui`) never talks to WhatsApp directly. It asks the daemon over the IPC protocol in `crates/ipc`.
- **Colours come from the theme** (`cx.theme()`), and **sizes come from the rem-based `Metrics`**, never hard-coded pixels.
- **A platform difference is a module** (`mod imp` for Windows and web), not `#[cfg]` sprinkled through the code.
- **No real personal data in tests:** no real phone numbers, names or captured messages.
- **Respect the budget.** Velo idles at around 136 MB. A change that adds memory, startup time or background CPU needs a good reason, and a measurement.

## Security

Please don't report a security problem in a public issue. Email the maintainer (address on the [GitHub profile](https://github.com/Ruchin-Audichya)) and you'll get a reply.

## Licence

By contributing you agree that your work is released under the MIT License, the same as the rest of Velo (see [LICENSE](LICENSE)).

Velo is unofficial and not affiliated with WhatsApp or Meta. Please don't add anything meant to get around WhatsApp's own limits or protections.
