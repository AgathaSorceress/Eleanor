use miette::miette;

use symphonia::core::{audio::SampleBuffer, codecs::DecoderOptions, formats::FormatReader};

use super::error::EleanorError;

#[derive(Debug, Clone)]
pub struct ReplayGain {
    pub track_gain: f32,
    pub track_peak: f32,
    pub album_gain: Option<f32>,
    pub album_peak: Option<f32>,
}

impl ReplayGain {
    pub(crate) fn try_calculate(audio: &mut Box<dyn FormatReader>) -> Result<Self, EleanorError> {
        let track = audio
            .default_track()
            .ok_or_else(|| miette!("No default track was found"))?;

        let params = &track.codec_params;

        let (sample_rate, channels) = (params.sample_rate, params.channels);
        let Some(sample_rate) = sample_rate else {
            return Err(miette!("Sample rate must be known").into());
        };

        // Only stereo is supported.
        if !channels.is_some_and(|x| x.count() == 2) {
            return Err(miette!("Unsupported channel configuration: {:?}", channels).into());
        }

        // Only 44.1kHz and 48kHz are supported.
        let Some(mut rg) = replaygain::ReplayGain::new(sample_rate as usize) else {
            return Err(miette!("Unsupported sample rate: {:?}", sample_rate).into());
        };

        let mut decoder =
            symphonia::default::get_codecs().make(params, &DecoderOptions::default())?;

        let track_id = track.id;

        let mut samples: Vec<f32> = vec![];
        let mut sample_buf = None;
        loop {
            let packet = match audio.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(ref packet_error))
                    if packet_error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    // End of audio stream
                    break;
                }
                Err(e) => {
                    return Err(e.into());
                }
            };

            // Skip packets belonging to other audio tracks
            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(buffer) => {
                    if sample_buf.is_none() {
                        let spec = *buffer.spec();
                        let duration = buffer.capacity() as u64;

                        sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                    }
                    if let Some(target) = &mut sample_buf {
                        target.copy_interleaved_ref(buffer);
                    }
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => (),
                Err(_) => break,
            }

            if let Some(buf) = &mut sample_buf {
                samples.extend(buf.samples());
            }
        }

        if samples.is_empty() {
            return Err(miette!("No samples were decoded from input audio").into());
        };

        rg.process_samples(&samples);

        let (track_gain, track_peak) = rg.finish();

        Ok(Self {
            track_gain,
            track_peak,
            album_gain: None,
            album_peak: None,
        })
    }
}

pub(crate) fn format_gain(gain: &str) -> Result<f32, EleanorError> {
    gain.chars()
        .filter(|c| c.is_numeric() || matches!(*c, '-' | '+' | '.'))
        .collect::<String>()
        .parse::<f32>()
        .map_err(EleanorError::from)
}

#[test]
fn rg_parse_expected() {
    for (input, expected) in [("-8.97 dB", -8.97), ("12.75 dB", 12.75), ("0.00 dB", 0.0)] {
        assert_eq!(format_gain(input).unwrap(), expected)
    }
}

#[test]
fn rg_parse_malformed() {
    for (input, expected) in [
        ("-8.5712 dB", -8.5712),
        ("+2.3 dB", 2.3),
        ("  13.12 dB", 13.12),
        ("006.66  Db", 6.66),
        ("0.93dB", 0.93),
        ("24.12 deutscheBahn", 24.12),
    ] {
        assert_eq!(format_gain(input).unwrap(), expected)
    }
}

#[test]
fn rg_parse_invalid() {
    for input in ["", "akjdfnhlkjfh", "🥺", "   DB "] {
        assert!(format_gain(input).is_err())
    }
}
