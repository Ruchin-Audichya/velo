"""Velo's built-in sticker packs: original text stickers, 512x512 WebP.

Every phrase here is written for Velo; no images, characters or real people
are used. Text is set in Windows' Segoe UI Black / Impact and the emoji in
Segoe UI Emoji, rendered into pixels (the fonts themselves are not shipped).
"""
import math
import os
import random
import sys

from PIL import Image, ImageDraw, ImageFilter, ImageFont

OUT = sys.argv[1]
FONTS = r"C:\Windows\Fonts"
BLACK = os.path.join(FONTS, "seguibl.ttf")
IMPACT = os.path.join(FONTS, "impact.ttf")
EMOJI = os.path.join(FONTS, "seguiemj.ttf")

PACKS = {
    "hinglish": [
        ("Bhai kya scene hai?", "🤔"), ("Chill maar", "😎"), ("Arre yaar!", "🤦"),
        ("Kya baat hai!", "👏"), ("Haan bhai haan", "🙄"), ("Bas kar pagle", "😂"),
        ("Rulayega kya?", "🥲"), ("Scene on hai", "🔥"), ("Theek hai boss", "👍"),
        ("Ho jayega", "💪"), ("Jugaad lagao", "🛠️"), ("Kal pakka", "🤞"),
        ("Main toh gaya", "💀"), ("Chai pe chalein?", "☕"), ("Exam kal hai", "😭"),
        ("Mast hai!", "🤩"), ("Pagal hai kya?", "😳"), ("Abhi aaya", "🏃"),
        ("Sab moh maya hai", "🧘"), ("Party kab hai?", "🎉"),
    ],
    "english": [
        ("LOL", "😂"), ("Same", "🙋"), ("On my way", "🚗"), ("Done!", "✅"),
        ("Thank you!", "🙏"), ("Good morning", "☀️"), ("Good night", "🌙"),
        ("Congrats!", "🎉"), ("BRB", "⏳"), ("No cap", "🧢"), ("Big mood", "😌"),
        ("Facts", "💯"), ("I'm dead", "💀"), ("Let's gooo", "🚀"), ("Sorry", "🥺"),
        ("Miss you", "🤍"), ("Happy birthday", "🎂"), ("You got this", "💪"),
        ("Call me", "📞"), ("Love it", "😍"),
    ],
    "memes": [
        ("Plot twist", "😱"), ("Task failed successfully", "🫠"),
        ("Monday again", "😩"), ("Loading brain...", "🧠"),
        ("404: motivation not found", "🤖"), ("Error: sleep needed", "😴"),
        ("This is fine", "🔥"), ("Me pretending to work", "💻"),
        ("Not my problem", "🤷"), ("Instant regret", "😬"),
        ("It's a feature, not a bug", "🐛"), ("Brain.exe has stopped", "🖥️"),
        ("Main character energy", "✨"), ("Who asked?", "🧐"),
        ("Respectfully, no", "🙅"), ("Bro thinks he's him", "😏"),
        ("Touch grass", "🌱"), ("Certified classic", "🏆"),
        ("Galti se mistake", "🙈"), ("Ye toh hona hi tha", "🤷"),
    ],
}

PALETTE = ["#ff4d6d", "#ffb703", "#21c063", "#4cc9f0", "#9b5de5", "#f15bb5",
           "#fb8500", "#00bbf9", "#80ed99", "#ffd166"]


def wrap(draw, text, font, width):
    words, lines, line = text.split(), [], ""
    for word in words:
        trial = f"{line} {word}".strip()
        if draw.textlength(trial, font=font) <= width or not line:
            line = trial
        else:
            lines.append(line)
            line = word
    lines.append(line)
    return lines


def sticker(text, emoji, color, rng, path):
    size = 512
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    face = IMPACT if rng.random() < 0.35 else BLACK
    for points in range(110, 30, -4):
        font = ImageFont.truetype(face, points)
        lines = wrap(draw, text, font, 440)
        line_h = font.getbbox("Ag")[3] - font.getbbox("Ag")[1]
        block = len(lines) * (line_h + 12)
        if block <= 300 and all(draw.textlength(l, font=font) <= 450 for l in lines):
            break
    emoji_font = ImageFont.truetype(EMOJI, 109)
    top = 60 + (300 - block) // 2
    # The emoji, above the text, in colour.
    draw.text((size // 2, top - 10), emoji, font=emoji_font, anchor="ms",
              embedded_color=True)
    y = top + 30
    text_layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    tdraw = ImageDraw.Draw(text_layer)
    for line in lines:
        tdraw.text((size // 2, y), line, font=font, anchor="ma", fill=color,
                   stroke_width=14, stroke_fill="white")
        y += line_h + 12
    # A soft shadow under the die-cut outline, as printed stickers have.
    alpha = text_layer.split()[3].filter(ImageFilter.GaussianBlur(6))
    shadow = Image.new("RGBA", (size, size), (0, 0, 0, 90))
    shadow.putalpha(alpha.point(lambda a: a * 90 // 255))
    img = Image.alpha_composite(Image.alpha_composite(Image.new("RGBA", (size, size)), shadow.transform(
        (size, size), Image.AFFINE, (1, 0, -4, 0, 1, -6))), img)
    img = Image.alpha_composite(img, text_layer)
    angle = rng.uniform(-6, 6)
    img = img.rotate(angle, resample=Image.BICUBIC)
    img.save(path, "WEBP", quality=82, method=6)


def main():
    rng = random.Random(1210)
    os.makedirs(OUT, exist_ok=True)
    for pack, items in PACKS.items():
        for index, (text, emoji) in enumerate(items):
            color = PALETTE[(index * 3 + len(pack)) % len(PALETTE)]
            sticker(text, emoji, color, rng, os.path.join(OUT, f"{pack}-{index:02}.webp"))
    total = sum(os.path.getsize(os.path.join(OUT, f)) for f in os.listdir(OUT))
    print(f"{len(os.listdir(OUT))} stickers, {total // 1024} KB")


main()
