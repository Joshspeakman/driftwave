//! Shoutcast/Icecast in-band metadata, track titles and recordable formats. Port of `ShortwaveCore/ICY.cs`.

/// One piece of a parsed ICY stream: either audio bytes or a metadata block's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcyEvent {
    Audio(Vec<u8>),
    Metadata(String),
}

/// Splits a Shoutcast/Icecast byte stream (sent with `Icy-MetaData: 1`) into audio and metadata.
///
/// Every `meta_int` audio bytes the server inserts one length byte (n) followed by n*16 bytes of metadata text,
/// e.g. `StreamTitle='Artist - Title';StreamUrl='';` padded with NULs. Feed it chunks of any size.
#[derive(Debug, Clone)]
pub struct IcyParser {
    meta_int: usize,
    audio_left: usize,
    meta_left: usize,
    expecting_length: bool,
    meta_buffer: Vec<u8>,
}

impl IcyParser {
    /// `meta_int` must be positive (it comes from the `icy-metaint` header).
    pub fn new(meta_int: usize) -> IcyParser {
        assert!(meta_int > 0, "icy-metaint must be positive");
        IcyParser { meta_int, audio_left: meta_int, meta_left: 0, expecting_length: false, meta_buffer: Vec::new() }
    }

    pub fn meta_int(&self) -> usize {
        self.meta_int
    }

    pub fn feed(&mut self, data: &[u8]) -> Vec<IcyEvent> {
        let mut events = Vec::new();
        let mut audio = Vec::new();
        let mut i = 0;
        while i < data.len() {
            if self.expecting_length {
                self.meta_left = data[i] as usize * 16;
                i += 1;
                self.expecting_length = false;
                if self.meta_left == 0 {
                    self.audio_left = self.meta_int;
                }
            } else if self.meta_left > 0 {
                let n = self.meta_left.min(data.len() - i);
                self.meta_buffer.extend_from_slice(&data[i..i + n]);
                i += n;
                self.meta_left -= n;
                if self.meta_left == 0 {
                    if !audio.is_empty() {
                        events.push(IcyEvent::Audio(std::mem::take(&mut audio)));
                    }
                    events.push(IcyEvent::Metadata(decode_text(&self.meta_buffer)));
                    self.meta_buffer.clear();
                    self.audio_left = self.meta_int;
                }
            } else {
                let n = self.audio_left.min(data.len() - i);
                audio.extend_from_slice(&data[i..i + n]);
                i += n;
                self.audio_left -= n;
                if self.audio_left == 0 {
                    self.expecting_length = true;
                }
            }
        }
        if !audio.is_empty() {
            events.push(IcyEvent::Audio(audio));
        }
        events
    }
}

/// Metadata text is UTF-8, falling back to Latin-1. Trailing NULs are stripped.
pub fn decode_text(data: &[u8]) -> String {
    let end = data.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
    let bytes = &data[..end];
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Pulls the `StreamTitle` value out of an ICY metadata block. Titles may contain apostrophes, so the value ends
/// at the first `';` that is followed by another key or the end of the block.
pub fn stream_title(metadata: &str) -> Option<String> {
    const KEY: &str = "StreamTitle='";
    let start = metadata.find(KEY)?;
    let rest = &metadata[start + KEY.len()..];
    let mut end = rest.len();
    let mut search = 0;
    while search <= rest.len() {
        let Some(r) = rest[search..].find("';").map(|p| p + search) else { break };
        let after = &rest[r + 2..];
        if after.is_empty()
            || after.chars().next().is_some_and(char::is_alphabetic)
            || after.chars().all(|c| c == '\0' || c == ' ')
        {
            end = r;
            break;
        }
        search = r + 2;
    }
    if end == rest.len() && rest.ends_with('\'') {
        end = rest.len() - 1;
    }
    Some(rest[..end].trim().to_owned())
}

/// Splits "Artist - Title" into its parts; when there is no separator the whole string is the title.
pub fn split_artist_title(s: &str) -> (Option<String>, String) {
    if let Some(r) = s.find(" - ") {
        let artist = s[..r].trim_matches([' ', '\t']);
        let title = s[r + 3..].trim_matches([' ', '\t']);
        if !artist.is_empty() && !title.is_empty() {
            return (Some(artist.to_owned()), title.to_owned());
        }
    }
    (None, s.to_owned())
}

/// Raw stream formats the recorder can store without re-encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordableFormat {
    Mp3,
    Aac,
}

impl RecordableFormat {
    pub fn from_content_type(content_type: Option<&str>) -> Option<RecordableFormat> {
        let ct = content_type?.to_lowercase();
        match ct.split(';').next().unwrap_or("").trim() {
            "audio/mpeg" | "audio/mp3" | "audio/mpeg3" | "audio/x-mpeg" => Some(RecordableFormat::Mp3),
            "audio/aac" | "audio/aacp" | "audio/x-aac" | "audio/x-aacp" | "audio/aac-adts" => Some(RecordableFormat::Aac),
            _ => None,
        }
    }

    pub fn from_raw_value(raw: Option<&str>) -> Option<RecordableFormat> {
        match raw?.to_lowercase().as_str() {
            "mp3" => Some(RecordableFormat::Mp3),
            "aac" => Some(RecordableFormat::Aac),
            _ => None,
        }
    }

    /// Same as the Swift rawValue: "mp3" / "aac".
    pub fn raw_value(self) -> &'static str {
        match self {
            RecordableFormat::Mp3 => "mp3",
            RecordableFormat::Aac => "aac",
        }
    }

    pub fn file_extension(self) -> &'static str {
        self.raw_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str) -> Vec<u8> {
        let mut bytes = text.as_bytes().to_vec();
        while bytes.len() % 16 != 0 {
            bytes.push(0);
        }
        let mut out = vec![(bytes.len() / 16) as u8];
        out.extend(bytes);
        out
    }

    #[test]
    fn splits_audio_and_metadata_across_chunks() {
        let mut stream = vec![0xAA; 8];
        stream.extend(block("StreamTitle='A - B';"));
        stream.extend([0xBB; 8]);
        stream.push(0);
        stream.extend([0xCC; 5]);
        for chunk_size in [1, 3, 7, 100] {
            let mut parser = IcyParser::new(8);
            let (mut audio, mut titles) = (Vec::new(), Vec::new());
            for chunk in stream.chunks(chunk_size) {
                for e in parser.feed(chunk) {
                    match e {
                        IcyEvent::Audio(a) => audio.extend(a),
                        IcyEvent::Metadata(m) => titles.push(stream_title(&m).unwrap_or_else(|| "?".into())),
                    }
                }
            }
            let mut expected = vec![0xAA; 8];
            expected.extend([0xBB; 8]);
            expected.extend([0xCC; 5]);
            assert_eq!(audio, expected, "chunk size {chunk_size}");
            assert_eq!(titles, vec!["A - B"]);
        }
    }

    #[test]
    fn stream_title_parsing() {
        assert_eq!(stream_title("StreamTitle='Artist - Song';StreamUrl='';").as_deref(), Some("Artist - Song"));
        assert_eq!(
            stream_title("StreamTitle='Guns N' Roses - Don't Cry';StreamUrl='x';").as_deref(),
            Some("Guns N' Roses - Don't Cry")
        );
        assert_eq!(stream_title("StreamTitle='It's';").as_deref(), Some("It's"));
        assert_eq!(stream_title("StreamTitle='';").as_deref(), Some(""));
        assert_eq!(stream_title("StreamUrl='x';"), None);
    }

    #[test]
    fn latin1_fallback() {
        let data = [
            0x53, 0x74, 0x72, 0x65, 0x61, 0x6D, 0x54, 0x69, 0x74, 0x6C, 0x65, 0x3D, 0x27, 0x43, 0x61, 0x66, 0xE9, 0x27,
            0x3B, 0, 0,
        ];
        assert_eq!(stream_title(&decode_text(&data)).as_deref(), Some("Café"));
    }

    #[test]
    fn artist_title() {
        assert_eq!(
            split_artist_title("Daft Punk - One More Time"),
            (Some("Daft Punk".into()), "One More Time".into())
        );
        assert_eq!(split_artist_title("News").0, None);
    }

    #[test]
    fn formats() {
        assert_eq!(RecordableFormat::from_content_type(Some("audio/mpeg")), Some(RecordableFormat::Mp3));
        assert_eq!(RecordableFormat::from_content_type(Some("audio/aacp; charset=x")), Some(RecordableFormat::Aac));
        assert_eq!(RecordableFormat::from_content_type(Some("application/ogg")), None);
        assert_eq!(RecordableFormat::from_content_type(None), None);
    }
}
