//! Bounded, frame-at-a-time GIF conversion using the existing H.264 encoder.

use anyhow::{Context, Result, ensure};
use image::{AnimationDecoder, ImageDecoder};
use openh264::formats::{RgbSliceU8, YUVBuffer};
use std::io::Cursor;

pub fn to_mp4(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))?;
    let (source_width, source_height) = decoder.dimensions();
    ensure!(
        source_width > 0
            && source_height > 0
            && u64::from(source_width) * u64::from(source_height) <= 16_000_000,
        "This GIF is too large to convert. Send it as a document to preserve the original."
    );
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    decoder.set_limits(limits)?;
    let scale = (640.0 / source_width.max(source_height) as f64).min(1.0);
    let width = ((source_width as f64 * scale) as u32).max(2) & !1;
    let height = ((source_height as f64 * scale) as u32).max(2) & !1;
    let mut encoder = crate::encoder::H264Encoder::new(crate::VideoQuality {
        width,
        height,
        fps: 30,
        bitrate_kbps: 1200,
    })?;
    let config = mp4::Mp4Config {
        major_brand: "isom".parse()?,
        minor_version: 512,
        compatible_brands: vec!["isom".parse()?, "avc1".parse()?],
        timescale: 1000,
    };
    let mut writer = mp4::Mp4Writer::write_start(Cursor::new(Vec::new()), &config)?;
    let mut timestamp = 0u64;
    let mut frame_count = 0usize;
    for frame in decoder.into_frames() {
        let frame = frame?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let duration = (numerator / denominator.max(1)).max(20);
        ensure!(
            timestamp + u64::from(duration) <= 30_000 && frame_count < 900,
            "GIFs up to 30 seconds and 900 frames are supported. Send this animation as a document."
        );
        let picture = image::DynamicImage::ImageRgba8(frame.into_buffer())
            .resize_exact(width, height, image::imageops::FilterType::Triangle)
            .to_rgb8();
        let pixels = YUVBuffer::from_rgb8_source(RgbSliceU8::new(
            picture.as_raw(),
            (width as usize, height as usize),
        ));
        let encoded = encoder
            .encode(&pixels, openh264::Timestamp::from_millis(timestamp))?
            .context("The GIF encoder did not produce a frame")?;
        let nals: Vec<_> = openh264::nal_units(&encoded.data)
            .filter_map(|nal| nal.strip_prefix(&[0, 0, 1]))
            .collect();
        if frame_count == 0 {
            let sps = nals
                .iter()
                .find(|nal| nal.first().is_some_and(|byte| byte & 31 == 7))
                .context("GIF encoder omitted its sequence parameters")?;
            let pps = nals
                .iter()
                .find(|nal| nal.first().is_some_and(|byte| byte & 31 == 8))
                .context("GIF encoder omitted its picture parameters")?;
            writer.add_track(&mp4::TrackConfig::from(mp4::AvcConfig {
                width: width as u16,
                height: height as u16,
                seq_param_set: sps.to_vec(),
                pic_param_set: pps.to_vec(),
            }))?;
        }
        let mut sample = Vec::new();
        for nal in nals {
            sample.extend_from_slice(&(nal.len() as u32).to_be_bytes());
            sample.extend_from_slice(nal);
        }
        writer.write_sample(
            1,
            &mp4::Mp4Sample {
                start_time: timestamp,
                duration,
                rendering_offset: 0,
                is_sync: encoded.keyframe,
                bytes: sample.into(),
            },
        )?;
        timestamp += u64::from(duration);
        frame_count += 1;
    }
    ensure!(frame_count > 0, "The GIF contains no frames");
    writer.write_end()?;
    Ok(writer.into_writer().into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converted_gif_preserves_frame_count_and_timing() {
        let mut gif = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif);
            for (color, duration) in [([255, 0, 0, 255], 100), ([0, 255, 0, 255], 200)] {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        image::RgbaImage::from_pixel(32, 32, image::Rgba(color)),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(duration, 1),
                    ))
                    .unwrap();
            }
        }
        let mp4 = to_mp4(&gif).unwrap();
        let reader = mp4::Mp4Reader::read_header(Cursor::new(&mp4), mp4.len() as u64).unwrap();
        assert_eq!(reader.sample_count(1).unwrap(), 2);
        assert_eq!(reader.duration().as_millis(), 300);
    }
}
