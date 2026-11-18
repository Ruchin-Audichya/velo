"""Generate the notification chimes embedded in the Windows build.

Run from anywhere: `python packaging/windows/make-chimes.py`. Writes WAV files
beside this script. Every sound is synthesised here, so the build carries no
third-party audio.

- soft-chime.wav  : a direct message. Two bright rising notes (G5 -> C6).
- group-chime.wav : a group message. The same voice, lower and falling
                    (E5 -> C5), so the two can be told apart without looking.
"""

import math
import struct
import wave
from pathlib import Path

RATE = 48000
HERE = Path(__file__).resolve().parent


def render(notes, duration, path, peak_db=-16.0):
    frames = []
    for index in range(round(RATE * duration)):
        t = index / RATE
        sample = 0.0
        for start, frequency, level in notes:
            age = t - start
            if age < 0:
                continue
            attack = 1.0 - math.exp(-age / 0.009)
            decay = math.exp(-age / 0.12)
            fundamental = math.sin(2 * math.pi * frequency * age)
            overtone = 0.12 * math.exp(-age / 0.045) * math.sin(2 * math.pi * frequency * 2 * age)
            sample += level * attack * decay * (fundamental + overtone)
        tail = min(1.0, max(0.0, (duration - t) / 0.045))
        frames.append(sample * tail)
    peak = max(abs(s) for s in frames)
    gain = 10 ** (peak_db / 20) / peak
    with wave.open(str(path), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(b"".join(struct.pack("<h", round(s * gain * 32767)) for s in frames))
    print(f"{path.name}: {duration:.2f}s, {RATE} Hz, mono, peak {peak_db} dBFS")


render([(0.0, 783.9909, 0.72), (0.095, 1046.5023, 0.58)], 0.72, HERE / "soft-chime.wav")
render([(0.0, 659.2551, 0.70), (0.11, 523.2511, 0.62)], 0.78, HERE / "group-chime.wav", -17.0)
