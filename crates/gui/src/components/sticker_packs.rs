//! Velo's own sticker packs, built into the app: Hinglish, English and memes.
//!
//! Original text stickers made for Velo (`scripts`-style generator in the
//! release notes), 512×512 WebP as WhatsApp wants them, so they send as real
//! stickers and look the same on every phone. `index.txt` beside them names
//! each file's pack and phrase, which is what the panel's search matches.

use std::sync::Arc;

use rust_embed::RustEmbed;

use super::input_area_view::PickerMedia;

#[derive(RustEmbed)]
#[folder = "assets/stickers"]
#[include = "*.webp"]
#[include = "index.txt"]
struct Packs;

/// The packs, in the order the panel offers them.
pub const PACKS: [(&str, &str); 3] = [
    ("hinglish", "Hinglish"),
    ("english", "English"),
    ("memes", "Memes"),
];

/// One sticker's bytes, to send.
#[must_use]
pub fn bytes(name: &str) -> Option<Vec<u8>> {
    Packs::get(&format!("{name}.webp")).map(|file| file.data.into_owned())
}

/// Every built-in sticker as a panel tile, decoded once per process.
#[must_use]
pub fn tiles() -> Vec<PickerMedia> {
    thread_local! {
        static TILES: Vec<PickerMedia> = load();
    }
    TILES.with(Clone::clone)
}

fn load() -> Vec<PickerMedia> {
    let Some(index) = Packs::get("index.txt") else {
        return Vec::new();
    };
    String::from_utf8_lossy(&index.data)
        .lines()
        .filter_map(|line| {
            let (name, phrase) = line.split_once('\t')?;
            let data = Packs::get(&format!("{name}.webp"))?.data.into_owned();
            Some(PickerMedia {
                chat_jid: String::new(),
                message_id: name.to_string(),
                sticker: true,
                label: phrase.to_string(),
                image: Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Webp, data)),
                builtin: Some(name.to_string()),
            })
        })
        .collect()
}

/// Which pack a built-in sticker belongs to, from its name.
#[must_use]
pub fn pack_of(name: &str) -> &str {
    name.split_once('-').map_or(name, |(pack, _)| pack)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_sticker_is_built_in_and_small_enough_to_send() {
        let tiles = tiles();
        assert!(tiles.len() >= 50, "{} stickers", tiles.len());
        for tile in &tiles {
            let name = tile.builtin.as_deref().unwrap();
            let data = bytes(name).expect("listed sticker is embedded");
            // WhatsApp refuses a static sticker over 100 KB.
            assert!(data.len() < 100 * 1024, "{name} is {} bytes", data.len());
            assert!(PACKS.iter().any(|(pack, _)| *pack == pack_of(name)));
        }
    }
}
