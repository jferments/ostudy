//! Bounded local audio/video admission.
//!
//! Storage admission and runtime decode support are deliberately separate. This
//! module accepts a narrow set of containers/codecs after inspecting bounded
//! bytes; the platform WebView still owns actual playback capability.

use crate::error::CommandError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MediaKind {
    Audio,
    Video,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MediaInspection {
    pub kind: MediaKind,
    pub mime_type: &'static str,
    pub extension: &'static str,
}

pub(crate) fn inspect_media_bytes(bytes: &[u8]) -> Result<MediaInspection, CommandError> {
    if inspect_wav(bytes) {
        return Ok(audio("audio/wav", "wav"));
    }
    if inspect_ogg_audio(bytes) {
        return Ok(audio("audio/ogg", "ogg"));
    }
    if inspect_mp3(bytes) {
        return Ok(audio("audio/mpeg", "mp3"));
    }
    if inspect_adts_aac(bytes) {
        return Ok(audio("audio/aac", "aac"));
    }
    if let Some(kind) = inspect_iso_bmff(bytes) {
        return Ok(match kind {
            MediaKind::Audio => audio("audio/mp4", "m4a"),
            MediaKind::Video => MediaInspection {
                kind: MediaKind::Video,
                mime_type: "video/mp4",
                extension: "mp4",
            },
        });
    }
    Err(CommandError::new(
        "unsupported_media",
        "Use MP3, AAC, PCM WAV, Ogg Opus/Vorbis, M4A, or H.264/AAC MP4 media.",
    ))
}

fn audio(mime_type: &'static str, extension: &'static str) -> MediaInspection {
    MediaInspection {
        kind: MediaKind::Audio,
        mime_type,
        extension,
    }
}

fn inspect_wav(bytes: &[u8]) -> bool {
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return false;
    }
    let declared = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8;
    if declared > bytes.len() || declared < 12 {
        return false;
    }
    let mut offset = 12;
    let mut valid_format = false;
    let mut data_bytes = 0_usize;
    while offset + 8 <= declared {
        let chunk = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        let Some(end) = start.checked_add(size) else {
            return false;
        };
        if end > declared {
            return false;
        }
        if chunk == b"fmt " && size >= 16 {
            let format = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap());
            let channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap());
            let sample_rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
            let bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap());
            valid_format = matches!(format, 1 | 3)
                && (1..=8).contains(&channels)
                && (8_000..=384_000).contains(&sample_rate)
                && matches!(bits, 8 | 16 | 24 | 32 | 64);
        } else if chunk == b"data" {
            data_bytes = size;
        }
        offset = end.saturating_add(size & 1);
    }
    valid_format && data_bytes > 0
}

fn inspect_ogg_audio(bytes: &[u8]) -> bool {
    if bytes.len() < 28 || &bytes[..4] != b"OggS" || bytes[4] != 0 {
        return false;
    }
    let segments = bytes[26] as usize;
    let table_end = 27 + segments;
    if table_end > bytes.len() {
        return false;
    }
    let mut packet_len = 0_usize;
    let mut complete = false;
    for length in &bytes[27..table_end] {
        packet_len = packet_len.saturating_add(*length as usize);
        if *length < 255 {
            complete = true;
            break;
        }
    }
    if !complete || packet_len == 0 || table_end.saturating_add(packet_len) > bytes.len() {
        return false;
    }
    let packet = &bytes[table_end..table_end + packet_len];
    packet.starts_with(b"OpusHead") || packet.starts_with(b"\x01vorbis")
}

fn inspect_mp3(bytes: &[u8]) -> bool {
    let mut start = 0_usize;
    if bytes.starts_with(b"ID3") {
        if bytes.len() < 10 || bytes[6..10].iter().any(|value| value & 0x80 != 0) {
            return false;
        }
        let tag_size = ((bytes[6] as usize) << 21)
            | ((bytes[7] as usize) << 14)
            | ((bytes[8] as usize) << 7)
            | bytes[9] as usize;
        start = 10_usize.saturating_add(tag_size);
        if start >= bytes.len() {
            return false;
        }
    }
    let end = bytes.len().min(start.saturating_add(1024 * 1024));
    for offset in start..end.saturating_sub(4) {
        let Some(frame_len) = mp3_frame_length(&bytes[offset..]) else {
            continue;
        };
        let next = offset.saturating_add(frame_len);
        if next + 4 <= bytes.len() && mp3_frame_length(&bytes[next..]).is_some() {
            return true;
        }
    }
    false
}

fn mp3_frame_length(bytes: &[u8]) -> Option<usize> {
    let header = bytes.get(..4)?;
    if header[0] != 0xff || header[1] & 0xe0 != 0xe0 {
        return None;
    }
    let version_bits = (header[1] >> 3) & 0x03;
    let layer_bits = (header[1] >> 1) & 0x03;
    if version_bits == 0x01 || layer_bits != 0x01 {
        return None;
    }
    let bitrate_index = (header[2] >> 4) as usize;
    let sample_index = ((header[2] >> 2) & 0x03) as usize;
    if bitrate_index == 0 || bitrate_index == 15 || sample_index == 3 {
        return None;
    }
    const MPEG1_BITRATES: [usize; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    const MPEG2_BITRATES: [usize; 16] = [
        0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0,
    ];
    const SAMPLE_RATES: [usize; 3] = [44_100, 48_000, 32_000];
    let mpeg1 = version_bits == 0x03;
    let bitrate = if mpeg1 {
        MPEG1_BITRATES[bitrate_index]
    } else {
        MPEG2_BITRATES[bitrate_index]
    } * 1000;
    let divisor = match version_bits {
        0x03 => 1,
        0x02 => 2,
        0x00 => 4,
        _ => return None,
    };
    let sample_rate = SAMPLE_RATES[sample_index] / divisor;
    let padding = ((header[2] >> 1) & 1) as usize;
    let coefficient = if mpeg1 { 144 } else { 72 };
    let length = coefficient * bitrate / sample_rate + padding;
    (24..=bytes.len()).contains(&length).then_some(length)
}

fn inspect_adts_aac(bytes: &[u8]) -> bool {
    let Some(first) = adts_frame_length(bytes) else {
        return false;
    };
    first + 7 <= bytes.len() && adts_frame_length(&bytes[first..]).is_some()
}

fn adts_frame_length(bytes: &[u8]) -> Option<usize> {
    let h = bytes.get(..7)?;
    if h[0] != 0xff || h[1] & 0xf6 != 0xf0 || ((h[2] >> 2) & 0x0f) == 0x0f {
        return None;
    }
    let length = (((h[3] & 0x03) as usize) << 11) | ((h[4] as usize) << 3) | ((h[5] as usize) >> 5);
    (length >= 7 && length <= bytes.len()).then_some(length)
}

fn inspect_iso_bmff(bytes: &[u8]) -> Option<MediaKind> {
    let boxes = boxes(bytes)?;
    let ftyp = boxes.iter().find(|entry| entry.kind == *b"ftyp")?;
    if ftyp.payload.len() < 8
        || (!supported_mp4_brand(&ftyp.payload[..4])
            && !ftyp.payload[8..].chunks_exact(4).any(supported_mp4_brand))
    {
        return None;
    }
    if !boxes.iter().any(|entry| entry.kind == *b"moov")
        || !boxes.iter().any(|entry| entry.kind == *b"mdat")
    {
        return None;
    }
    let mut codecs = Vec::<[u8; 4]>::new();
    for entry in &boxes {
        if entry.kind == *b"moov" && !collect_sample_entries(entry.payload, 0, &mut codecs) {
            return None;
        }
    }
    let video = codecs
        .iter()
        .any(|codec| matches!(codec, b"avc1" | b"avc3"));
    let audio = codecs.iter().any(|codec| codec == b"mp4a");
    if video {
        Some(MediaKind::Video)
    } else if audio {
        Some(MediaKind::Audio)
    } else {
        None
    }
}

fn supported_mp4_brand(brand: &[u8]) -> bool {
    matches!(
        brand,
        b"isom" | b"iso2" | b"mp41" | b"mp42" | b"M4A " | b"M4V " | b"avc1"
    )
}

#[derive(Clone, Copy)]
struct BoxView<'a> {
    kind: [u8; 4],
    payload: &'a [u8],
}

fn boxes(mut bytes: &[u8]) -> Option<Vec<BoxView<'_>>> {
    let mut result = Vec::new();
    while !bytes.is_empty() {
        if bytes.len() < 8 {
            return None;
        }
        let size32 = u32::from_be_bytes(bytes[..4].try_into().ok()?) as u64;
        let kind: [u8; 4] = bytes[4..8].try_into().ok()?;
        let (header, size) = if size32 == 1 {
            if bytes.len() < 16 {
                return None;
            }
            (16_usize, u64::from_be_bytes(bytes[8..16].try_into().ok()?))
        } else if size32 == 0 {
            (8_usize, bytes.len() as u64)
        } else {
            (8_usize, size32)
        };
        let size = usize::try_from(size).ok()?;
        if size < header || size > bytes.len() {
            return None;
        }
        result.push(BoxView {
            kind,
            payload: &bytes[header..size],
        });
        bytes = &bytes[size..];
    }
    Some(result)
}

fn collect_sample_entries(bytes: &[u8], depth: usize, codecs: &mut Vec<[u8; 4]>) -> bool {
    if depth > 8 {
        return false;
    }
    let Some(entries) = boxes(bytes) else {
        return false;
    };
    for entry in entries {
        if entry.kind == *b"stsd" {
            if entry.payload.len() < 8 {
                return false;
            }
            let count = u32::from_be_bytes(entry.payload[4..8].try_into().unwrap()) as usize;
            let Some(samples) = boxes(&entry.payload[8..]) else {
                return false;
            };
            if samples.len() != count || codecs.len().saturating_add(count) > 64 {
                return false;
            }
            codecs.extend(samples.into_iter().map(|sample| sample.kind));
        } else if matches!(&entry.kind, b"trak" | b"mdia" | b"minf" | b"stbl")
            && !collect_sample_entries(entry.payload, depth + 1, codecs)
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_pcm_wav_and_rejects_compressed_or_truncated_wav() {
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&40_u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&44_100_u32.to_le_bytes());
        wav.extend_from_slice(&88_200_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&4_u32.to_le_bytes());
        wav.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(inspect_media_bytes(&wav).unwrap().mime_type, "audio/wav");
        wav[20] = 6;
        assert!(inspect_media_bytes(&wav).is_err());
        assert!(inspect_media_bytes(b"RIFF\0\0\0\0WAVE").is_err());
    }

    #[test]
    fn admits_two_valid_mp3_frames_not_an_id3_label_alone() {
        let mut mp3 = vec![0_u8; 417 * 2];
        for offset in [0, 417] {
            mp3[offset..offset + 4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x00]);
        }
        assert_eq!(inspect_media_bytes(&mp3).unwrap().mime_type, "audio/mpeg");
        assert!(inspect_media_bytes(b"ID3\x04\0\0\0\0\0\0not audio").is_err());
    }

    #[test]
    fn admits_ogg_opus_and_rejects_unknown_ogg_codec() {
        let packet = b"OpusHeadfixture";
        let mut ogg = vec![0_u8; 27];
        ogg[..4].copy_from_slice(b"OggS");
        ogg[26] = 1;
        ogg.push(packet.len() as u8);
        ogg.extend_from_slice(packet);
        assert_eq!(inspect_media_bytes(&ogg).unwrap().mime_type, "audio/ogg");
        let packet_start = ogg.len() - packet.len();
        ogg[packet_start..].copy_from_slice(b"BadCodecfixture");
        assert!(inspect_media_bytes(&ogg).is_err());
    }

    fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(&u32::try_from(payload.len() + 8).unwrap().to_be_bytes());
        result.extend_from_slice(kind);
        result.extend_from_slice(payload);
        result
    }

    fn sample_description(codec: &[u8; 4]) -> Vec<u8> {
        let sample = mp4_box(codec, &[]);
        let mut payload = vec![0_u8; 4];
        payload.extend_from_slice(&1_u32.to_be_bytes());
        payload.extend_from_slice(&sample);
        mp4_box(b"stsd", &payload)
    }

    fn mp4(codec: &[u8; 4]) -> Vec<u8> {
        let stbl = mp4_box(b"stbl", &sample_description(codec));
        let minf = mp4_box(b"minf", &stbl);
        let mdia = mp4_box(b"mdia", &minf);
        let trak = mp4_box(b"trak", &mdia);
        let moov = mp4_box(b"moov", &trak);
        let mut ftyp = b"isom".to_vec();
        ftyp.extend_from_slice(&0_u32.to_be_bytes());
        ftyp.extend_from_slice(b"isommp42");
        [mp4_box(b"ftyp", &ftyp), moov, mp4_box(b"mdat", b"payload")].concat()
    }

    #[test]
    fn inspects_mp4_sample_entries_instead_of_extension_or_brand_only() {
        let video = inspect_media_bytes(&mp4(b"avc1")).unwrap();
        assert_eq!(
            (video.kind, video.mime_type),
            (MediaKind::Video, "video/mp4")
        );
        let audio = inspect_media_bytes(&mp4(b"mp4a")).unwrap();
        assert_eq!(
            (audio.kind, audio.mime_type),
            (MediaKind::Audio, "audio/mp4")
        );
        assert!(inspect_media_bytes(&mp4(b"hev1")).is_err());
        assert!(inspect_media_bytes(b"\0\0\0\x18ftypisomfake fake fake").is_err());
    }
}
