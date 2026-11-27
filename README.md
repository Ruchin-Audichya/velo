<p align="center">
  <img src="assets/velo-logo-256.png" width="112" alt="Velo logo: a white speech bubble with a V, on a green tile">
</p>

<h1 align="center">Velo</h1>

<p align="center">
  <b>WhatsApp, at home on Windows.</b><br>
  A fast, native desktop client that doesn't secretly want to be a browser tab.
</p>

<p align="center">
  <a href="https://github.com/Ruchin-Audichya/velo/releases/latest/download/Velo-Setup-1.3.1-x64.exe"><b>⬇ Download Velo 1.3 for Windows</b></a>
  &nbsp;·&nbsp; Windows 10 &amp; 11, 64-bit &nbsp;·&nbsp; free
  <br><b>New:</b> <a href="#linux">Linux</a> for Ubuntu, Debian and Fedora
  <br><sub><a href="https://github.com/Ruchin-Audichya/velo/releases/latest">All downloads, portable zip and checksums</a></sub>
</p>

---

## The problem

Let's be honest about WhatsApp on a PC.

**WhatsApp Web** lives in a browser tab. That tab is somewhere between tab #14 ("Q3 report FINAL v7") and tab #31 (a recipe you will never cook). You close it by accident during a Ctrl+W spree, and your mum's "call me" message waits patiently in the void.

**WhatsApp Desktop** solved that by… also being a web page, just wearing an app costume. It takes its sweet time to open. It eats memory like it's training for something. Sometimes it decides you have been logged out, as a treat.

Meanwhile your PC has a perfectly good processor that would love to draw some chat bubbles, quickly, if anybody asked.

## Why Velo exists

Someone asked.

Velo is a **real Windows app**: written in Rust, drawn on your GPU, no embedded browser anywhere. It links to your phone the same way WhatsApp Web does, by scanning a QR code, and then it just… works. Quickly. Quietly. In about **118 MB for the window plus ~18 MB for the background service** on the machine it was built on, against roughly 1 GB for the official app. (Your mileage may vary. Your RAM will thank you either way.)

It also keeps running in the tray when you close the window, so messages and calls still reach you. Like a messenger is supposed to.

## Velo vs the official app

| | Official WhatsApp app | **Velo** |
|-|-|-|
| What it is | WhatsApp Web inside a browser engine (WebView2) | A native app: Rust, drawn on your GPU |
| Memory, idle | ~1,053 MB across 8 processes | **~136 MB across 2 processes** |
| Opens in | not measured | **~0.2 s** |
| Download | Microsoft Store | **35 MB installer, no admin** |

**What you get on top:** an unread count on the taskbar, notifications that arrive with the window closed, a preview of photos and videos before you download them, search across every chat plus a search for any file by name, one-click HD camera presets, and 16 wallpapers, 8 themes and your own accent colour.

<sub>Memory is private bytes from Task Manager, measured on the same laptop with the same account and both apps idle on the chat list (Intel i5-11300H, Windows 11). Your numbers will vary.</sub>

## New in 1.3

The biggest update yet, much of it from users' feedback.

- **Message info:** right-click a message you sent to see when it was delivered and read, and in a group, who read it
- **60 stickers built in:** Hinglish, English and Memes packs, plus *Emoji · Stickers · GIFs* tabs with search
- **A new sidebar:** bigger icons, a marker on where you are, unread counts that never get cut off, and an expanded mode with labels (**Ctrl+B**)
- **Clean ringtones:** the incoming-call stutter is fixed, and there are two new original tones, *Velo Signature* and *Glass Chime*
- **Communities organised like WhatsApp:** each community's header, then its Announcements and groups
- **A redesigned Calls screen:** photos for everyone, no overlapping rows, and a side panel
- **Downloads work like WhatsApp:** files go to Velo's own cache, *Save as…* puts a copy where you choose, and old files clear themselves
- **Pictures everywhere:** Status shows each update's preview, and channel and community pictures load
- **Motion you control:** quick, light animations, with Off, Minimal, Balanced or Expressive in Settings
- **Read-only groups and channels** show WhatsApp's own line instead of a message box that can't send
- **Connect Claude to Velo** with `velo-cli mcp --serve` ([setup](docs/cli.md))
- **Fixes:** duplicate chats, missing group messages, jumps while scrolling history, notification clicks, the taskbar badge, and a background-service crash. Logs no longer include personal data.

## New in 1.2

- **Message anyone from a group:** click a name on a message, or a person in the group's member list, to open your private chat with them
- **Reactions that pop:** the reaction bar springs open, emoji pop in one after another, and they're bigger
- **16 wallpapers**, including Aurora, Sunset, a subtle Dots pattern, Lavender, Rose, Mint, Sand, Midnight and Graphite
- **Proper file tabs** in a chat's shared files: Docs, Photos, Videos and more, each with a count
- Settings shows what you've chosen at a glance

## New in 1.1

- **An unread count on the taskbar icon**, so a glance at the taskbar tells you if anything's new
- **The Velo logo on every notification**, instead of a blank square
- **A new ringtone**: a bright marimba (Classic) or a soft kalimba (Gentle), loud enough to hear across the room, with no clicks
- **HD video calls in one click**: pick *Data saver*, *Standard* or *HD* instead of scrolling a list of 40 camera modes. Webcams like the Logitech C920 and C922 get 720p.
- **Find any file you've ever been sent**: a fast search across file names in every chat, even ones you haven't opened
- **Start a chat from your contacts**, or add a new contact right from Velo
- A redesigned Calls and Files screen, photos on Status, and a "jump to latest" button in long chats
- Velo now uses its own name everywhere on your PC. Updating from 1.0 moves your chats across automatically, and you stay signed in.

## What it does

**The everyday stuff, done properly**
- Your chats, groups, communities, channels and status updates, synced from your phone
- Send text, photos, videos, documents, voice notes, stickers and GIFs
- Reply, react, edit, forward, and delete for you or for everyone
- Right-click a chat to **pin, mute, archive, mark unread, clear or delete** it
- **New chat by phone number** (Ctrl+N), no need to save the contact first
- Voice and video calls *(beta, be nice to it)*

**The stuff WhatsApp Web doesn't do**
- **Preview photos and videos before downloading them**, so you know if that 40 MB video is worth it
- **Search across every chat at once**, and **find any file by name** across all your chats in a blink
- **HD video calls** with one-click quality presets for your webcam
- Native **notifications with soft chimes and the Velo logo**, an **unread badge on the taskbar**, a tray icon, and **start at sign-in**
- Keyboard-first: Ctrl+K search, Alt+↑↓ to move between chats, Alt+Shift+↓ for the next unread, Ctrl+N for a new chat
- **Themes, accent colours and chat wallpapers**: Classic dark & light, plus Velo violet, Ultramarine and Coral accents
- Snappy, native animations: things fade and settle, nothing bounces at you

## Install

### Windows

1. Download the **`Velo-Setup-…-x64.exe`** installer from the [latest release](https://github.com/Ruchin-Audichya/velo/releases/latest).
2. Run it and press **Install**. It installs just for you, with no administrator prompt.
3. Open Velo, then on your phone go to **WhatsApp → Settings → Linked devices → Link a device** and scan the QR code.

That's it. Your chats show up and you're in.

Prefer no installer? Grab the **portable zip**, unzip it anywhere, and run `velo.exe`.

**Already on 1.0?** Just run the new installer. It updates in place, keeps your chats and your taskbar pin, and you stay signed in.

**To remove Velo:** Windows Settings → Apps → Installed apps → Velo → Uninstall. You choose whether to keep your chats on this PC for next time.

> **A note on Windows warnings.** Velo isn't code-signed yet (signing certificates cost money, and this is a free app). Until it is, Windows SmartScreen may show a notice the first time you run the installer, and PCs with **Smart App Control** switched on won't run it at all. Signing is on the roadmap. Every release lists SHA-256 checksums so you can check the file you downloaded is the one published here.

### Linux

64-bit Linux, tested on **Ubuntu 22.04 and 24.04, Debian 12 and Fedora**. Anything at least as new as Ubuntu 22.04 should work.

**Ubuntu, Debian, Linux Mint, Pop!_OS:**

```bash
wget https://github.com/Ruchin-Audichya/velo/releases/latest/download/velo_1.3.1_amd64.deb
sudo apt install ./velo_1.3.1_amd64.deb
```

**Fedora:**

```bash
sudo dnf install https://github.com/Ruchin-Audichya/velo/releases/latest/download/velo-1.3.1-1.x86_64.rpm
```

Then open **Velo** from your app menu (or run `velo`) and scan the QR code, the same as on Windows.

**Other distributions:** download `velo-1.3.1-linux-x86_64.tar.gz`, extract it somewhere it can stay, and run `./velo`. Run `./install-desktop.sh` once to add Velo to your app menu.

To remove it: `sudo apt remove velo` or `sudo dnf remove velo`. Your chats stay in `~/.local/share/velo` until you delete that folder.

Velo doesn't update itself on Linux yet. Install the new package over the old one when a release comes out.

## Is it safe?

- **Your messages stay end-to-end encrypted.** Velo is a *linked device*, exactly like WhatsApp Web. Your phone stays the boss.
- **Your data stays on your PC**, in `%LOCALAPPDATA%\velo`. Nothing is sent anywhere except to WhatsApp itself.
- **The honest part:** Velo is *unofficial*. WhatsApp's terms don't love third-party clients, so there's a small but real chance WhatsApp restricts an account that uses one. Plenty of people use unofficial clients without trouble, but you should know it's a possibility. Use it at your own risk.

## What it's built on

Velo stands on the shoulders of some excellent open-source work:

- **[oxidezap/client](https://github.com/oxidezap/client)**, the Rust WhatsApp desktop client Velo is built from, by João Lucas de Oliveira Lopes and contributors
- **[whatsapp-rust](https://github.com/oxidezap/whatsapp-rust)**, the library that actually speaks WhatsApp
- **[GPUI](https://github.com/zed-industries/zed)**, the GPU-accelerated UI framework behind the Zed editor, and **[gpui-kit](https://github.com/longbridge/gpui-kit)** by Longbridge for the components

Velo adds the redesign, the logo, the animations, the chat actions, the installer and a lot of polish. All of it is released under the MIT License, the same as the projects above. Full credits and the licences of all 700-odd libraries inside are in [`THIRD-PARTY-NOTICES.txt`](THIRD-PARTY-NOTICES.txt).

## Build it yourself

Velo is open source: everything that goes into the installer is in this repository.

You need Rust (the version is pinned in `rust-toolchain.toml`, so `rustup` picks it up) and, on Windows, the MSVC build tools.

```bash
git clone https://github.com/Ruchin-Audichya/velo
cd velo
cargo build --release
./target/release/velo
```

That builds the window (`velo`), the background service that holds your WhatsApp session (`velod`) and a command-line client (`velo-cli`). To build the Windows installer exactly as releases are made, run `packaging/windows/build-release.ps1`.

How the pieces fit together, and the reasoning behind the less obvious decisions, is in [docs/development.md](docs/development.md), [docs/architecture.md](docs/architecture.md) and [docs/gotchas.md](docs/gotchas.md).

## Contributing

Velo is built in the open, and it gets better every time someone brings an idea. You don't need to know Rust to help.

### Got an idea for a feature?

1. **Check it isn't already there.** Search the [issues](https://github.com/Ruchin-Audichya/velo/issues?q=label%3Aenhancement) and the [Ideas board](https://github.com/Ruchin-Audichya/velo/discussions/categories/ideas). If someone beat you to it, add a 👍 and your own reason for wanting it. That's how we tell what matters most.
2. **Share it.** Post in [Discussions → Ideas](https://github.com/Ruchin-Audichya/velo/discussions/new?category=ideas) to talk it through, or fill in the [feature request form](https://github.com/Ruchin-Audichya/velo/issues/new?template=feature_request.yml) if it's already clear. A good idea says:
   - **the problem:** what you were trying to do, and what got in the way
   - **what you'd like Velo to do** about it
   - **how other apps handle it**, if they do. Screenshots help, but blur any private chats.
3. **We shape it together.** An idea that fits Velo becomes an issue labelled `enhancement`. When it's ready for someone to pick up, it gets [`help wanted`](https://github.com/Ruchin-Audichya/velo/labels/help%20wanted).
4. **Want to build it yourself?** Comment on the issue so nobody else starts the same thing, then follow [Sending a change](#sending-a-change).

### Ideas we'd love help with

| Idea | Where to start |
|-|-|
| **Scheduled messages:** write now, send later | `crates/daemon`, which holds the session and could send at the right time |
| **A command palette** (Ctrl+Shift+P) for every action | `crates/gui/src/app/` |
| **Split view:** two chats side by side | `crates/gui/src/views/` |
| **Voice-note transcription**, on your PC | `crates/audio` |
| **More sticker packs** (original art only) | `crates/gui/assets/stickers/` and `scripts/make-stickers.py` |
| **Translations** into your language | [docs/localization.md](docs/localization.md) and `crates/gui/src/i18n/` |
| **Mac and Linux builds** | [docs/building.md](docs/building.md) |
| **Plugins:** auto-replies, reminders, bots | [examples/template](examples/template) and [docs/plugin-abi.md](docs/plugin-abi.md) |

Plugins are the easiest way to add a feature without touching Velo's core. They're small WebAssembly modules that run inside the background service, and each one asks for exactly the permissions it needs. Copy `examples/template`, then build it with `cargo xtask plugin build examples/<name>`.

### Sending a change

1. **Fork the repository** and create a branch for your change.
2. **Build and test** with `cargo build` and `cargo test --workspace --all-features` (see [Build it yourself](#build-it-yourself)).
3. **Open a pull request** that says what it changes and why, with a before-and-after screenshot for anything visible.

A pull request is easy to merge when it:

- does **one thing**, so it's quick to review
- passes `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` and the tests, the same checks CI runs
- follows the house rules in [AGENTS.md](AGENTS.md): colours come from the theme, sizes from the rem, never hard-coded
- uses **no real phone numbers, names or chats** in tests or screenshots

For anything big, open an issue first so we can agree on the approach before you spend a weekend on it.

### Other ways to help

- **Report bugs** with the [bug form](https://github.com/Ruchin-Audichya/velo/issues/new?template=bug_report.yml): what you did, what you expected, what happened
- **Try new releases** and say what feels off
- **Answer questions** in [Q&A](https://github.com/Ruchin-Audichya/velo/discussions/categories/q-a)
- **Star the repository** and tell a friend who's tired of WhatsApp Desktop

The full guide is in **[CONTRIBUTING.md](CONTRIBUTING.md)**.

## FAQ

**Is this made by WhatsApp or Meta?**
No. Velo is an independent project and isn't affiliated with, endorsed by or sponsored by WhatsApp LLC or Meta Platforms, Inc. "WhatsApp" is their trademark; we only use it to say what Velo connects to.

**Does my phone need to stay online?**
No. Like WhatsApp Web, Velo keeps working when your phone is off, for up to about two weeks.

**Mac or Linux?**
Linux, yes: see [Install → Linux](#linux). Mac isn't packaged yet. The code builds there, so if you'd like to help, see [Contributing](#contributing).

**Why "Velo"?**
It's short for *velocity*. Also all the good names were taken by companies with lawyers.

**I found a bug.**
Of course you did. Please [open an issue](https://github.com/Ruchin-Audichya/velo/issues) and say what you did, what you expected and what happened. Screenshots help, as long as there are no private chats in them.

---

<p align="center">
  Made with Rust, caffeine and mild frustration by <a href="https://github.com/Ruchin-Audichya">Ruchin Audichya</a>.<br>
  MIT License · Not affiliated with WhatsApp or Meta.
</p>
