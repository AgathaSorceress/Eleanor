use lofty::{ItemKey, Tag};
use miette::miette;

use symphonia::core::{
    audio::SampleBuffer,
    codecs::{Decoder, DecoderOptions},
    formats::{Packet, Track},
};

use super::error::EleanorError;

#[derive(Debug, Clone)]
pub struct ReplayGainResult {
    pub track_gain: f32,
    pub track_peak: f32,
    pub album_gain: Option<f32>,
    pub album_peak: Option<f32>,
}

impl TryFrom<Option<&Tag>> for ReplayGainResult {
    type Error = EleanorError;
    fn try_from(tags: Option<&Tag>) -> Result<Self, Self::Error> {
        let rg_track_gain = tags
            .and_then(|t| t.get_string(&ItemKey::ReplayGainTrackGain))
            .and_then(|v| parse_gain(v).ok());
        let rg_track_peak = tags
            .and_then(|t| t.get_string(&ItemKey::ReplayGainTrackPeak))
            .and_then(|v| parse_gain(v).ok());
        let rg_album_gain = tags
            .and_then(|t| t.get_string(&ItemKey::ReplayGainAlbumGain))
            .and_then(|v| parse_gain(v).ok());
        let rg_album_peak = tags
            .and_then(|t| t.get_string(&ItemKey::ReplayGainAlbumPeak))
            .and_then(|v| parse_gain(v).ok());

        if let (Some(track_gain), Some(track_peak)) = (rg_track_gain, rg_track_peak) {
            Ok(Self {
                track_gain,
                track_peak,
                album_gain: rg_album_gain,
                album_peak: rg_album_peak,
            })
        } else {
            Err(
                miette!("REPLAYGAIN_TRACK_GAIN and/or REPLAYGAIN_TRACK_PEAK tags were missing")
                    .into(),
            )
        }
    }
}

pub struct ReplayGain {
    data: Vec<f32>,
    rg: replaygain::ReplayGain,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_buf: Option<SampleBuffer<f32>>,
}

impl ReplayGain {
    pub fn init(sample_rate: usize, track: &Track) -> Result<Self, EleanorError> {
        let rg = replaygain::ReplayGain::new(sample_rate)
            .ok_or(miette!("Unsupported sample rate: {}", sample_rate))?;

        let params = &track.codec_params;
        let track_id = track.id;

        let decoder = symphonia::default::get_codecs().make(params, &DecoderOptions::default())?;

        Ok(Self {
            rg,
            data: Vec::new(),
            decoder,
            track_id,
            sample_buf: None,
        })
    }

    pub fn handle_packet(&mut self, packet: &Packet) -> Result<(), EleanorError> {
        if packet.track_id() == self.track_id {
            match self.decoder.decode(packet) {
                Ok(buffer) => {
                    if self.sample_buf.is_none() {
                        let spec = *buffer.spec();
                        let duration = buffer.capacity() as u64;

                        self.sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                    }

                    if let Some(target) = self.sample_buf.as_mut() {
                        target.copy_interleaved_ref(buffer);
                    }
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => (),
                Err(e) => return Err(e.into()),
            }
            if let Some(buf) = self.sample_buf.as_mut() {
                self.data.extend(buf.samples());
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<ReplayGainResult, EleanorError> {
        if self.data.is_empty() {
            Err(miette!("No samples were decoded from input audio"))?;
        };

        self.rg.process_samples(&self.data);

        let (track_gain, track_peak) = self.rg.finish();

        Ok(ReplayGainResult {
            track_gain,
            track_peak,
            album_gain: None,
            album_peak: None,
        })
    }
}

pub(crate) fn parse_gain(gain: &str) -> Result<f32, EleanorError> {
    gain.chars()
        .filter(|c| c.is_numeric() || matches!(*c, '-' | '+' | '.'))
        .collect::<String>()
        .parse::<f32>()
        .map_err(EleanorError::from)
}

#[test]
fn rg_parse_expected() {
    for (input, expected) in [("-8.97 dB", -8.97), ("12.75 dB", 12.75), ("0.00 dB", 0.0)] {
        assert_eq!(parse_gain(input).unwrap(), expected)
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
        assert_eq!(parse_gain(input).unwrap(), expected)
    }
}

#[test]
fn rg_parse_invalid() {
    for input in ["", "akjdfnhlkjfh", "🥺", "   DB "] {
        assert!(parse_gain(input).is_err())
    }
}
