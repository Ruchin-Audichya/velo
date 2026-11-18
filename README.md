<p align="center">
  <img src="assets/velo-logo-256.png" width="112" alt="Velo logo: a white speech bubble with a V, on a green tile">
</p>

<h1 align="center">Velo</h1>

<p align="center">
  <b>WhatsApp, at home on Windows.</b><br>
  A fast, native desktop client that doesn't secretly want to be a browser tab.
</p>

<p align="center">
  <a href="https://github.com/Ruchin-Audichya/velo/releases/latest"><b>⬇ Download for Windows</b></a>
  &nbsp;·&nbsp; Windows 10 &amp; 11, 64-bit &nbsp;·&nbsp; free
</p>

---

## The problem

Let's be honest about WhatsApp on a PC.

**WhatsApp Web** lives in a browser tab. That tab is somewhere between tab #14 ("Q3 report FINAL v7") and tab #31 (a recipe you will never cook). You close it by accident during a Ctrl+W spree, and your mum's "call me" message waits patiently in the void.

**WhatsApp Desktop** solved that by… also being a web page, just wearing an app costume. It takes its sweet time to open. It eats memory like it's training for something. Sometimes it decides you have been logged out, as a treat.

Meanwhile your PC has a perfectly good processor that would love to draw some chat bubbles, quickly, if anybody asked.

## Why Velo exists

Someone asked.

Velo is a **real Windows app**: written in Rust, drawn on your GPU, no embedded browser anywhere. It links to your phone the same way WhatsApp Web does, by scanning a QR code, and then it just… works. Quickly. Quietly. In about **90 MB for the window plus ~36 MB for the background service** on the machine it was built on. (Your mileage may vary. Your RAM will thank you either way.)

It also keeps running in the tray when you close the window, so messages and calls still reach you. Like a messenger is supposed to.

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
- **Search across every chat at once**, and a per-chat **files panel** you can actually search and sort
- Native **notifications with soft chimes**, a tray icon, and **start at sign-in**
- Keyboard-first: Ctrl+K search, Alt+↑↓ to move between chats, Alt+Shift+↓ for the next unread, Ctrl+N for a new chat
- **Themes, accent colours and chat wallpapers**: Classic dark & light, plus Velo violet, Ultramarine and Coral accents
- Snappy, native animations: things fade and settle, nothing bounces at you

## Install

1. Download the **`Velo-Setup-…-x64.exe`** installer from the [latest release](https://github.com/Ruchin-Audichya/velo/releases/latest).
2. Run it and press **Install**. It installs just for you, with no administrator prompt.
3. Open Velo, then on your phone go to **WhatsApp → Settings → Linked devices → Link a device** and scan the QR code.

That's it. Your chats show up and you're in.

Prefer no installer? Grab the **portable zip**, unzip it anywhere, and run `oxidezap.exe`.

**To remove Velo:** Windows Settings → Apps → Installed apps → Velo → Uninstall. You choose whether to keep your chats on this PC for next time.

> **A note on Windows warnings.** Velo isn't code-signed yet (signing certificates cost money, and this is a free app). Until it is, Windows SmartScreen may show a notice the first time you run the installer, and PCs with **Smart App Control** switched on won't run it at all. Signing is on the roadmap. Every release lists SHA-256 checksums so you can check the file you downloaded is the one published here.

## Is it safe?

- **Your messages stay end-to-end encrypted.** Velo is a *linked device*, exactly like WhatsApp Web. Your phone stays the boss.
- **Your data stays on your PC**, in `%LOCALAPPDATA%\oxidezap`. Nothing is sent anywhere except to WhatsApp itself.
- **The honest part:** Velo is *unofficial*. WhatsApp's terms don't love third-party clients, so there's a small but real chance WhatsApp restricts an account that uses one. Plenty of people use unofficial clients without trouble, but you should know it's a possibility. Use it at your own risk.

## What it's built on

Velo stands on the shoulders of some excellent open-source work:

- **[oxidezap/client](https://github.com/oxidezap/client)**, the Rust WhatsApp desktop client Velo is built from, by João Lucas de Oliveira Lopes and contributors
- **[whatsapp-rust](https://github.com/oxidezap/whatsapp-rust)**, the library that actually speaks WhatsApp
- **[GPUI](https://github.com/zed-industries/zed)**, the GPU-accelerated UI framework behind the Zed editor, and **[gpui-kit](https://github.com/longbridge/gpui-kit)** by Longbridge for the components

Velo adds the redesign, the logo, the animations, the chat actions, the installer and a lot of polish. All of it is released under the MIT License, the same as the projects above. Full credits and the licences of all 700-odd libraries inside are in [`THIRD-PARTY-NOTICES.txt`](THIRD-PARTY-NOTICES.txt).

## FAQ

**Is this made by WhatsApp or Meta?**
No. Velo is an independent project and isn't affiliated with, endorsed by or sponsored by WhatsApp LLC or Meta Platforms, Inc. "WhatsApp" is their trademark; we only use it to say what Velo connects to.

**Does my phone need to stay online?**
No. Like WhatsApp Web, Velo keeps working when your phone is off, for up to about two weeks.

**Mac or Linux?**
Velo is Windows-first. The code underneath runs elsewhere too, so maybe one day.

**Why "Velo"?**
It's short for *velocity*. Also all the good names were taken by companies with lawyers.

**I found a bug.**
Of course you did. Please [open an issue](https://github.com/Ruchin-Audichya/velo/issues) and say what you did, what you expected and what happened. Screenshots help, as long as there are no private chats in them.

---

<p align="center">
  Made with Rust, caffeine and mild frustration by <a href="https://github.com/Ruchin-Audichya">Ruchin Audichya</a>.<br>
  MIT License · Not affiliated with WhatsApp or Meta.
</p>
