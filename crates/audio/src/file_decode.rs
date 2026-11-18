//! Desktop decoding for audio attachments, independent of the output device.

use std::io::Cursor;

use symphonia::core::{
    codecs::{
        CodecParameters,
        audio::{AudioDecoder, AudioDecoderOptions, well_known::CODEC_ID_OPUS},
    },
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

use crate::player::PlayerError;

// The player holds decoded audio so seeking and speed changes need no disk I/O.
pub(super) const SAMPLE_LIMIT: usize = 24 * 1024 * 1024;

pub(super) struct DecodedAudio {
    pub samples: Vec<f32>,
    pub rate: u32,
    pub channels: usize,
}

pub(super) fn decode(data: Vec<u8>) -> Result<DecodedAudio, PlayerError> {
    let source = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(decode_error)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| decode_error("This file has no audio track."))?;
    let track_id = track.id;
    let Some(CodecParameters::Audio(params)) = track.codec_params.as_ref() else {
        return Err(decode_error("This audio codec is unavailable."));
    };
    let mut opus_frame = vec![0.0; 5760 * 2];
    let mut decoder = if params.codec == CODEC_ID_OPUS {
        let (mode, channels) = match params.channels.as_ref().map(|channels| channels.count()) {
            Some(1) => (opus::Channels::Mono, 1),
            Some(2) => (opus::Channels::Stereo, 2),
            _ => {
                return Err(decode_error(
                    "Only mono and stereo Opus tracks are supported.",
                ));
            }
        };
        Decoder::Opus(
            opus::Decoder::new(48_000, mode).map_err(decode_error)?,
            channels,
        )
    } else {
        Decoder::Registered(
            symphonia::default::get_codecs()
                .make_audio_decoder(params, &AudioDecoderOptions::default())
                .map_err(decode_error)?,
        )
    };

    let mut decoded: Option<DecodedAudio> = None;
    let mut frame = Vec::<f32>::new();
    while let Some(packet) = format.next_packet().map_err(decode_error)? {
        if packet.track_id != track_id {
            continue;
        }
        let (samples, rate, channels) = match &mut decoder {
            Decoder::Registered(decoder) => {
                let audio = match decoder.decode(&packet) {
                    Ok(audio) => audio,
                    Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                    Err(error) => return Err(decode_error(error)),
                };
                audio.copy_to_vec_interleaved(&mut frame);
                (
                    frame.as_slice(),
                    audio.spec().rate(),
                    audio.spec().channels().count(),
                )
            }
            Decoder::Opus(decoder, channels) => {
                let frames = decoder
                    .decode_float(&packet.data, &mut opus_frame, false)
                    .map_err(decode_error)?;
                (&opus_frame[..frames * *channels], 48_000, *channels)
            }
        };
        if channels == 0 || rate == 0 {
            return Err(decode_error("Invalid audio track."));
        }
        let output_channels = if channels > 2 { 1 } else { channels };
        let output = decoded.get_or_insert_with(|| DecodedAudio {
            samples: Vec::new(),
            rate,
            channels: output_channels,
        });
        if output.rate != rate || output.channels != output_channels {
            return Err(decode_error("The audio format changes mid-track."));
        }
        if output.samples.len() + samples.len() / channels * output_channels > SAMPLE_LIMIT {
            return Err(PlayerError::AudioTooLong);
        }
        if channels > 2 {
            output.samples.extend(
                samples
                    .chunks_exact(channels)
                    .map(|frame| frame.iter().sum::<f32>() / channels as f32),
            );
        } else {
            output.samples.extend_from_slice(samples);
        }
    }
    decoded
        .filter(|audio| !audio.samples.is_empty())
        .ok_or(PlayerError::EmptyAudio)
}

enum Decoder {
    Registered(Box<dyn AudioDecoder>),
    Opus(opus::Decoder, usize),
}

fn decode_error(error: impl std::fmt::Display) -> PlayerError {
    PlayerError::DecodeError(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn stereo_wav_is_probed_from_bytes_and_preserves_both_channels() {
        let mut data = b"RIFF".to_vec();
        data.extend(44_u32.to_le_bytes());
        data.extend(b"WAVEfmt ");
        data.extend(16_u32.to_le_bytes());
        data.extend(1_u16.to_le_bytes());
        data.extend(2_u16.to_le_bytes());
        data.extend(44_100_u32.to_le_bytes());
        data.extend(176_400_u32.to_le_bytes());
        data.extend(4_u16.to_le_bytes());
        data.extend(16_u16.to_le_bytes());
        data.extend(b"data");
        data.extend(8_u32.to_le_bytes());
        for sample in [16_384_i16, -16_384, 8192, -8192] {
            data.extend(sample.to_le_bytes());
        }

        let audio = decode(data).expect("stereo PCM should decode");
        assert_eq!(audio.rate, 44_100);
        assert_eq!(audio.channels, 2);
        assert_eq!(audio.samples, [0.5, -0.5, 0.25, -0.25]);
    }
}
