# Velo vs WhatsApp: feature parity and known bugs

A working list, checked against the code and the running app on 19 Nov 2026.
✅ has it · ⚠️ partly · ❌ missing. **Priority** marks what to build next.

## Messages

| Feature | Velo | Notes |
| --- | --- | --- |
| Text, emoji, *bold* / _italic_ / ~strike~ / `mono` | ✅ | |
| Reply, react, edit, copy | ✅ | |
| Delete for me (any message) / for everyone (yours, 2 days) | ✅ | |
| Forward | ⚠️ | One chat at a time; WhatsApp lets you pick up to five |
| **Star messages + Starred list** | ❌ | **P1.** The store already syncs stars; the window never sees them |
| Pin a message in a chat | ❌ | P2 |
| Message info (delivered / read by) | ❌ | P3 |
| Select several messages (delete / forward together) | ❌ | P2 |
| Reply privately (from a group) | ❌ | P2 |
| **@mention picker in groups** | ❌ | **P1.** Mentions render; typing `@` offers nobody |
| **Link previews** | ❌ | **P1** |
| Disappearing messages, view once | ❌ | P3 |
| Polls | ⚠️ | Vote ✅; creating one ❌ |
| Location, contact cards | ⚠️ | Shown as text, no map or card |

## Media

| Feature | Velo | Notes |
| --- | --- | --- |
| Send photos, videos, documents, stickers, GIFs | ✅ | |
| Voice notes: record, play, 1× / 1.5× / 2× | ✅ | |
| Preview before downloading | ✅ | Velo extra |
| Drag and drop, paste images | ✅ | |
| Media viewer | ✅ | |
| Per-chat files panel, global Files screen | ✅ | Restyled today |

## Chats

| Feature | Velo | Notes |
| --- | --- | --- |
| Pin, mute, archive, mark unread, clear, delete | ✅ | Row and header menus |
| New chat by number (Ctrl+N) | ✅ | |
| Search all chats, search in a chat | ✅ | |
| Filters: All, Unread, Groups, Archived | ✅ | Favourites ❌ |
| Drafts | ✅ | |
| Jump to latest message | ✅ | Added today |
| **"Unread messages" divider in a chat** | ❌ | **P1** |
| Chat lock | ❌ | P3 |
| Export chat | ❌ | P3 |

## Groups and contacts

| Feature | Velo | Notes |
| --- | --- | --- |
| Group / contact info, members | ✅ | |
| **Block / unblock a contact** | ❌ | **P1.** The library supports it |
| Report | ❌ | P3 |
| Create a group, add / remove people, admins | ❌ | P2 |
| Leave group, invite link | ❌ | P2 |

## Calls and Status

| Feature | Velo | Notes |
| --- | --- | --- |
| Voice and video calls | ⚠️ | Work; still beta |
| Call history with call-back | ✅ | Redesigned today |
| Silence unknown callers | ❌ | P2 |
| View Status, with photos | ✅ | Photos fixed today |
| Post a Status, reply to one | ❌ | P2 |

## Settings and desktop

| Feature | Velo | Notes |
| --- | --- | --- |
| Themes, accent colours, wallpapers, text size | ✅ | |
| Notifications, chimes, tray, start at sign-in | ✅ | |
| Privacy: last seen, read receipts, profile photo | ❌ | P2 |
| Keyboard shortcuts | ✅ | |
| Installer, uninstaller, winget | ✅ | winget pending review |
| Code signing | ❌ | Waiting on a certificate |

## Bugs

Fixed on 19 Nov 2026:

1. The rail kept Status highlighted while Calls or Files was open.
2. Clicking the destination already underneath Calls or Files did nothing.
3. Clicking Files while Calls was open left Calls on screen.
4. The Calls screen read like a debug log: "Synced" on every row, no photos,
   no call-back, a redundant back button.
5. The Files screen: grey cards, one generic file icon, a back button.
6. Status rings never showed anyone's profile photo.
7. No way back to the newest message after scrolling up.
8. A pinned taskbar icon still opened the pre-release development copy.

Still open:

- A chat deleted or cleared in Velo may come back after a restart until
  WhatsApp syncs the change back; the background service does not record it
  itself. Needs a test on a throwaway chat.
- Group members who have no chat with you never get a photo: WhatsApp gives
  pictures by chat, and there is none to ask through.
- Business messages' buttons are text only.
