# Desktop baseline

The yardstick for performance work on the installed Windows build. Each row
of `desktop-baseline.csv` is one run of
`pwsh tools/measure/desktop-baseline.ps1 -Label <label>`; rerun it on the same
machine, with nothing else heavy running, and compare labels.

## What the columns mean

| Column | Measured |
| --- | --- |
| `window_ms` | Starting the window (service already running) until Windows reports its main window |
| `service_ms` | Starting the background service until its pipe accepts a connection |
| `connected_ms` | Starting the service until it reports the WhatsApp session connected (network-bound) |
| `window_mb`, `service_mb` | Private memory 20 s after the window opens, idle on the chat list |
| `*_exe_mb`, `installer_mb` | File sizes of the shipped executables and the last-built installer |

Frame time is not measured yet: there is no frame instrumentation to read it
from. Phase 2 adds it.

## Baseline, 19 Nov 2026 (Velo 1.0.0 + Phase 0)

Machine: Intel Core i5-11300H (8 threads), 16 GB RAM, Iris Xe + GeForce RTX
3050 Laptop, Windows 11 Home build 26300.

| | Run 1 | Run 2 | Run 3 |
| --- | --- | --- | --- |
| Window appears | 204 ms | 252 ms | 202 ms |
| Service ready | 177 ms | 73 ms | 73 ms |
| WhatsApp connected | 1.24 s | 1.42 s | 1.65 s |
| Window memory | 118 MB | 142 MB | 118 MB |
| Service memory | 16 MB | 18 MB | 16 MB |

Sizes: window 23.4 MB, service 27.2 MB, installer 34.5 MB.

The first service start is slower (177 ms against 73 ms): it was the first
launch after an install, with a cold file cache.
