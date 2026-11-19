//! HLS radio: a master playlist (pick a variant), then the media playlist refreshed by media sequence
//! so each segment is fetched once, demuxed (MPEG-TS), decoded, and its timed-ID3 titles read.

use crate::decode::{self, BitrateMeter, FrameDecoder, FrameSplitter, PcmBlock};
use crate::engine::Event;
use crate::net;
use crate::resample::Resampler;
use crate::ts::{AudioKind, TsDemuxer, TsOut};
use driftwave_core::icy::RecordableFormat;
use driftwave_core::id3;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};
use url::Url;

/// Queues one decoded block (converting rate and channels) to the output.
pub type PushFn<'a> = Box<dyn FnMut(&mut Option<Resampler>, &PcmBlock) + 'a>;

/// What HLS needs from the engine.
pub struct Host<'a> {
    pub user_agent: &'a str,
    pub stop: &'a AtomicBool,
    pub tx: &'a Sender<Event>,
    pub push: PushFn<'a>,
    pub mark: Box<dyn Fn(&str) + 'a>,
    pub buffered: Box<dyn Fn() -> f64 + 'a>,
    /// The stream's bitrate in kbps (the variant's BANDWIDTH, else measured from the frames).
    pub set_kbps: Box<dyn Fn(u32) + 'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaPlaylist {
    pub target_duration: f64,
    pub media_sequence: u64,
    pub segments: Vec<(u64, Url, f64)>,
    pub ended: bool,
}

/// The variant to play from a master playlist: the highest bandwidth (radio variants are all small).
pub fn pick_variant(text: &str, base: &Url) -> Option<Url> {
    pick_variant_with_bandwidth(text, base).map(|v| v.0)
}

/// The chosen variant and its BANDWIDTH in bits per second (0 when not stated).
pub fn pick_variant_with_bandwidth(text: &str, base: &Url) -> Option<(Url, u64)> {
    let mut best: Option<(u64, Url)> = None;
    let mut bandwidth: Option<u64> = None;
    for line in text.lines().map(str::trim) {
        if let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            bandwidth = attrs
                .split(',')
                .find_map(|a| a.trim().strip_prefix("BANDWIDTH="))
                .and_then(|v| v.parse().ok())
                .or(Some(0));
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Some(bw) = bandwidth.take() {
                if let Ok(u) = base.join(line) {
                    if best.as_ref().is_none_or(|(b, _)| bw > *b) {
                        best = Some((bw, u));
                    }
                }
            }
        }
    }
    best.map(|(bw, u)| (u, bw))
}

pub fn parse_media(text: &str, base: &Url) -> MediaPlaylist {
    let mut p = MediaPlaylist { target_duration: 6.0, media_sequence: 0, segments: Vec::new(), ended: false };
    let mut duration = 0.0;
    let mut seq = None::<u64>;
    for line in text.lines().map(str::trim) {
        if let Some(v) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            p.target_duration = v.trim().parse().unwrap_or(6.0);
        } else if let Some(v) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            p.media_sequence = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("#EXTINF:") {
            duration = v.split(',').next().and_then(|d| d.trim().parse().ok()).unwrap_or(0.0);
        } else if line == "#EXT-X-ENDLIST" {
            p.ended = true;
        } else if !line.is_empty() && !line.starts_with('#') {
            let n = seq.map(|s| s + 1).unwrap_or(p.media_sequence);
            seq = Some(n);
            if let Ok(u) = base.join(line) {
                p.segments.push((n, u, duration));
            }
        }
    }
    p
}

fn send(host: &Host, e: Event) {
    let _ = host.tx.send(e);
}

/// Plays until stopped, the stream ends, or it fails.
pub fn run(mut host: Host, url: &Url) -> Result<(), String> {
    let (final_url, _, text) = net::fetch_text(url, host.user_agent, 1 << 20).map_err(|e| e.to_string())?;
    let mut stated_kbps = None;
    let media_url = if text.contains("#EXT-X-STREAM-INF") {
        let (u, bw) = pick_variant_with_bandwidth(&text, &final_url).ok_or_else(|| "The HLS playlist has no playable stream.".to_string())?;
        if bw >= 1000 {
            stated_kbps = Some((bw / 1000) as u32);
            (host.set_kbps)((bw / 1000) as u32);
        }
        u
    } else {
        final_url
    };
    let mut meter = BitrateMeter::default();
    let mut next_seq: Option<u64> = None;
    let mut demux = TsDemuxer::new();
    let mut decoder: Option<(AudioKind, Box<dyn FrameDecoder>, FrameSplitter)> = None;
    let mut resampler: Option<Resampler> = None;
    let mut announced = false;
    let mut failures = 0u32;
    let mut last_title: Option<String> = None;
    while !host.stop.load(Ordering::Relaxed) {
        let fetched = Instant::now();
        let playlist = match net::fetch_text(&media_url, host.user_agent, 1 << 20) {
            Ok((u, _, t)) => parse_media(&t, &u),
            Err(e) => {
                failures += 1;
                if failures > 5 {
                    return Err(e.to_string());
                }
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };
        // Live: start three segments from the end.
        let start = next_seq.unwrap_or_else(|| {
            let n = playlist.segments.len();
            playlist.segments.get(n.saturating_sub(3)).map(|s| s.0).unwrap_or(playlist.media_sequence)
        });
        let mut got_new = false;
        for (seq, seg_url, _) in playlist.segments.iter().filter(|s| s.0 >= start) {
            if host.stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let bytes = match net::fetch_bytes(seg_url, host.user_agent) {
                Ok(b) => b,
                Err(e) => {
                    failures += 1;
                    if failures > 5 {
                        return Err(e.to_string());
                    }
                    continue;
                }
            };
            failures = 0;
            got_new = true;
            next_seq = Some(seq + 1);
            let mut outs = demux.feed(&bytes);
            outs.extend(demux.finish());
            for o in outs {
                match o {
                    TsOut::Audio(es) => {
                        let Some(kind) = demux.audio_kind() else { continue };
                        if decoder.as_ref().map(|d| d.0) != Some(kind) {
                            let format = if kind == AudioKind::Adts { RecordableFormat::Aac } else { RecordableFormat::Mp3 };
                            decoder = Some((kind, decode::for_format(format), FrameSplitter::new(format)));
                        }
                        let (_, dec, split) = decoder.as_mut().unwrap();
                        if !announced {
                            send(&host, Event::Connected { titles: false, format: None, kind: format!("hls/{}", dec.name()) });
                            announced = true;
                        }
                        let format = if kind == AudioKind::Adts { RecordableFormat::Aac } else { RecordableFormat::Mp3 };
                        for frame in split.feed(&es) {
                            if stated_kbps.is_none() {
                                meter.add(format, &frame);
                                if let Some(k) = meter.kbps() {
                                    (host.set_kbps)(k);
                                }
                            }
                            if let Ok(blocks) = dec.decode(&frame) {
                                for b in blocks {
                                    (host.push)(&mut resampler, &b);
                                }
                            }
                        }
                    }
                    TsOut::Id3(tag) => {
                        let (title, artist) = id3::read_title_artist(&tag);
                        let full = match (artist, title) {
                            (Some(a), Some(t)) => Some(format!("{a} - {t}")),
                            (None, Some(t)) => Some(t),
                            _ => None,
                        };
                        if let Some(full) = full {
                            if last_title.as_deref() != Some(full.as_str()) {
                                (host.mark)(&full);
                                last_title = Some(full);
                            }
                        }
                    }
                }
            }
        }
        if playlist.ended && !got_new {
            return Ok(());
        }
        // Refresh at half the target duration, but don't read far ahead of playback.
        let wait = Duration::from_secs_f64((playlist.target_duration / 2.0).clamp(1.0, 10.0));
        while !host.stop.load(Ordering::Relaxed) && (fetched.elapsed() < wait || (host.buffered)() > playlist.target_duration * 2.0) {
            thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_master_and_media_playlists() {
        let base = Url::parse("https://example.com/live/index.m3u8?token=1").unwrap();
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=32000,CODECS=\"mp4a.40.29\"\nlow/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=64000\nhttps://cdn.example.com/hi/index.m3u8?sig=abc\n";
        assert_eq!(pick_variant(master, &base).unwrap().as_str(), "https://cdn.example.com/hi/index.m3u8?sig=abc");
        let media = "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:100\n#EXTINF:5.76,\nseg_100.ts\n#EXTINF:5.76,\nseg_101.ts\n";
        let p = parse_media(media, &base);
        assert_eq!(p.media_sequence, 100);
        assert_eq!(p.segments.len(), 2);
        assert_eq!(p.segments[1].0, 101);
        assert_eq!(p.segments[1].1.as_str(), "https://example.com/live/seg_101.ts"); // query dropped, RFC 3986
        assert!(!p.ended);
    }
}
