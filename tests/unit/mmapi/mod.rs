use super::*;

mod audio_preparation;
mod decoding;
mod lifecycle_cleanup;
mod limits;
mod native_api;
mod playback;

#[derive(Default)]
pub(super) struct CaptureSink(pub(super) Vec<i16>);

impl AudioSink for CaptureSink {
    fn write(&mut self, samples: &[i16]) -> Result<(), EmuError> {
        self.0.extend_from_slice(samples);
        Ok(())
    }
}

fn decode_tone_sequence(bytes: &[u8], limits: Limits) -> Result<Clip, EmuError> {
    super::decode_tone_sequence(bytes, limits, &|| false)
}

fn decode_wav(bytes: &[u8], limits: Limits) -> Result<Clip, EmuError> {
    super::decode_wav(bytes, limits, &|| false)
}

fn resample_mono(samples: Vec<i16>, rate: u32, limits: Limits) -> Result<Clip, EmuError> {
    super::resample_mono(samples, rate, limits, &|| false)
}

fn wav(samples: &[i16]) -> Vec<u8> {
    let data_bytes = u32::try_from(samples.len() * 2).unwrap();
    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    output.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    output.extend_from_slice(&OUTPUT_SAMPLE_RATE.to_le_bytes());
    output.extend_from_slice(&(OUTPUT_SAMPLE_RATE * 2).to_le_bytes());
    output.extend_from_slice(&2_u16.to_le_bytes());
    output.extend_from_slice(&16_u16.to_le_bytes());
    output.extend_from_slice(b"data");
    output.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        output.extend_from_slice(&sample.to_le_bytes());
    }
    output
}

fn ima_adpcm_wav_constant() -> Vec<u8> {
    let block_align = 8_u16;
    let samples_per_block = 9_u16;
    let sample_rate = 8_000_u32;
    let byte_rate = sample_rate * u32::from(block_align) / u32::from(samples_per_block);
    let data = [0xe8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]; // predictor=1000
    let riff_size = 4_u32 + (8 + 20) + (8 + u32::try_from(data.len()).unwrap());
    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&riff_size.to_le_bytes());
    output.extend_from_slice(b"WAVEfmt ");
    output.extend_from_slice(&20_u32.to_le_bytes());
    output.extend_from_slice(&0x11_u16.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&sample_rate.to_le_bytes());
    output.extend_from_slice(&byte_rate.to_le_bytes());
    output.extend_from_slice(&block_align.to_le_bytes());
    output.extend_from_slice(&4_u16.to_le_bytes());
    output.extend_from_slice(&2_u16.to_le_bytes());
    output.extend_from_slice(&samples_per_block.to_le_bytes());
    output.extend_from_slice(b"data");
    output.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&data);
    output
}
