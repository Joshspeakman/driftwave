//! Rate and channel conversion to the output device.
//!
//! A stateful cubic (Catmull-Rom) interpolator: it keeps its place across blocks, so a stream can
//! arrive in any block sizes. Plenty for radio streams; a windowed-sinc resampler (rubato) can
//! replace it behind the same interface if listening shows a need.

/// Converts interleaved blocks at `in_rate` with `in_channels` to `out_rate` with `out_channels`.
pub struct Resampler {
    in_rate: u32,
    in_channels: usize,
    out_rate: u32,
    out_channels: usize,
    /// Position of the next output sample, in input frames, relative to `history[1]`.
    pos: f64,
    /// The last 3 input frames of the previous block (per channel), for the interpolator's window.
    history: Vec<[f32; 3]>,
    primed: bool,
}

impl Resampler {
    pub fn new(in_rate: u32, in_channels: usize, out_rate: u32, out_channels: usize) -> Resampler {
        Resampler {
            in_rate,
            in_channels: in_channels.max(1),
            out_rate,
            out_channels: out_channels.max(1),
            pos: 0.0,
            history: vec![[0.0; 3]; out_channels.max(1)],
            primed: false,
        }
    }

    pub fn matches(&self, in_rate: u32, in_channels: usize) -> bool {
        self.in_rate == in_rate && self.in_channels == in_channels.max(1)
    }

    pub fn reset(&mut self) {
        self.pos = 0.0;
        self.primed = false;
        self.history.fill([0.0; 3]);
    }

    /// Channel mapping: mono goes to every output channel; extra input channels are dropped.
    fn mapped(&self, frame: &[f32], ch: usize) -> f32 {
        if self.in_channels == 1 { frame[0] } else { frame[ch.min(self.in_channels - 1)] }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let frames = input.len() / self.in_channels;
        if frames == 0 {
            return Vec::new();
        }
        let oc = self.out_channels;
        // Per-channel signal: 3 frames of history, then this block.
        let mut chans: Vec<Vec<f32>> = (0..oc)
            .map(|c| {
                let mut v = Vec::with_capacity(frames + 3);
                v.extend_from_slice(&self.history[c]);
                v.extend(input.chunks(self.in_channels).map(|f| self.mapped(f, c)));
                v
            })
            .collect();
        if !self.primed {
            // Start on the first real sample, with the history copying it.
            for ch in &mut chans {
                let first = ch[3];
                ch[0] = first;
                ch[1] = first;
                ch[2] = first;
            }
            self.pos = 0.0;
            self.primed = true;
        }
        let step = self.in_rate as f64 / self.out_rate as f64;
        let mut out = Vec::with_capacity(((frames as f64 / step) as usize + 2) * oc);
        // Sample positions are relative to chans[c][2] (the newest history frame) = index 2.
        // We need p0..p3 = idx-1..idx+2, so idx+2 must exist: idx + 2 < frames + 3.
        while (self.pos.floor() as usize) + 2 + 2 < frames + 3 {
            let idx = self.pos.floor() as usize + 2;
            let t = (self.pos - self.pos.floor()) as f32;
            for ch in &chans {
                let (p0, p1, p2, p3) = (ch[idx - 1], ch[idx], ch[idx + 1], ch[idx + 2]);
                let v = p1 + 0.5 * t * (p2 - p0 + t * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3 + t * (3.0 * (p1 - p2) + p3 - p0)));
                out.push(v);
            }
            self.pos += step;
        }
        // Keep the last 3 frames and move the position back by the frames consumed.
        let total = frames + 3;
        for (c, ch) in chans.iter().enumerate() {
            self.history[c] = [ch[total - 3], ch[total - 2], ch[total - 1]];
        }
        self.pos -= frames as f64;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f64, rate: u32, secs: f64) -> Vec<f32> {
        (0..(rate as f64 * secs) as usize).map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin() as f32).collect()
    }

    fn crossings_hz(s: &[f32], rate: u32) -> f64 {
        let c = s.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        c as f64 / 2.0 / (s.len() as f64 / rate as f64)
    }

    #[test]
    fn keeps_pitch_and_length_across_odd_blocks() {
        let input = tone(440.0, 44100, 2.0);
        let mut r = Resampler::new(44100, 1, 48000, 2);
        let mut out = Vec::new();
        for block in input.chunks(1152) {
            out.extend(r.process(block));
        }
        let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        let secs = left.len() as f64 / 48000.0;
        assert!((secs - 2.0).abs() < 0.01, "length {secs}");
        assert!((crossings_hz(&left, 48000) - 440.0).abs() < 2.0);
        assert_eq!(out.chunks(2).filter(|f| f[0] != f[1]).count(), 0, "mono goes to both sides");
    }
}
