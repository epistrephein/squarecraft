//! A zlib stream writer specialised for run-length data.
//!
//! The input is described as `(byte, count)` runs rather than as a byte
//! buffer, plus repetitions of the data just written. Every run is encoded as
//! a literal followed by back-references at distance 1, which is the optimal
//! LZ77 parse for data made of long runs, and every repetition as
//! back-references one period away. The whole stream goes into a single
//! deflate block with Huffman codes built from the exact symbol frequencies.
//! Both the encoding and the Adler-32 checksum are computed per run and per
//! repetition, so the cost is proportional to the number of runs and emitted
//! symbols, never to the uncompressed size.

const MAX_MATCH: u64 = 258;
const MIN_MATCH: u64 = 3;
const MAX_DISTANCE: u64 = 32768;
const END_OF_BLOCK: usize = 256;
const LITLEN_CODES: usize = 286;
const DIST_CODES: usize = 30;
const CODELEN_CODES: usize = 19;
const CODELEN_ORDER: [usize; CODELEN_CODES] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// The uncompressed data, as seen by a [`zlib`] source.
pub trait Stream {
    /// Appends `count` copies of `byte`.
    fn run(&mut self, byte: u8, count: u64);

    /// Appends `times` more copies of `period`, which must be exactly the
    /// runs that were appended last.
    fn repeat(&mut self, period: &[(u8, u64)], times: u64);
}

/// Compresses the data produced by `source` into a zlib stream.
///
/// `source` is called twice (once to gather statistics, once to emit) and
/// must produce the same data both times.
pub fn zlib<F>(source: F) -> Vec<u8>
where
    F: Fn(&mut dyn Stream),
{
    let mut stats = Stats { litlen: [0; LITLEN_CODES], dist: [0; DIST_CODES], adler: Adler32::new() };
    let mut coder = RunCoder::new(&mut stats);
    source(&mut coder);
    coder.flush();

    let Stats { mut litlen, dist, adler } = stats;
    litlen[END_OF_BLOCK] += 1;

    let mut writer = BlockWriter::new(&litlen, &dist);
    let mut coder = RunCoder::new(&mut writer);
    source(&mut coder);
    coder.flush();

    // CMF/FLG: 32K window, deflate, maximum compression.
    let mut out = vec![0x78, 0xda];
    out.extend(writer.finish());
    out.extend(adler.value().to_be_bytes());
    out
}

/// Receives the LZ77 symbols of the stream, and the raw data for checksums.
trait Symbols {
    fn literal(&mut self, byte: u8);
    fn matches(&mut self, len: u64, distance: u64, times: u64);
    fn raw_run(&mut self, _byte: u8, _count: u64) {}
    fn raw_repeat(&mut self, _period: &[(u8, u64)], _times: u64) {}
}

/// Turns the stream into literals and matches, merging adjacent runs of the
/// same byte.
struct RunCoder<'a, S: Symbols> {
    symbols: &'a mut S,
    previous: Option<u8>,
    pending: Option<(u8, u64)>,
}

impl<S: Symbols> Stream for RunCoder<'_, S> {
    fn run(&mut self, byte: u8, count: u64) {
        if count == 0 {
            return;
        }
        match &mut self.pending {
            Some((pending, total)) if *pending == byte => *total += count,
            _ => {
                self.flush();
                self.pending = Some((byte, count));
            }
        }
    }

    fn repeat(&mut self, period: &[(u8, u64)], times: u64) {
        let len: u64 = period.iter().map(|&(_, count)| count).sum();
        if len > MAX_DISTANCE || len * times < MIN_MATCH {
            (0..times).for_each(|_| period.iter().for_each(|&(byte, count)| self.run(byte, count)));
            return;
        }

        self.flush();
        self.symbols.raw_repeat(period, times);
        self.emit_matches(len * times, len);
        self.previous = period.iter().rev().find(|&&(_, count)| count > 0).map(|&(byte, _)| byte);
    }
}

impl<'a, S: Symbols> RunCoder<'a, S> {
    fn new(symbols: &'a mut S) -> Self {
        RunCoder { symbols, previous: None, pending: None }
    }

    fn flush(&mut self) {
        let Some((byte, mut count)) = self.pending.take() else { return };
        self.symbols.raw_run(byte, count);

        if self.previous != Some(byte) {
            self.symbols.literal(byte);
            self.previous = Some(byte);
            count -= 1;
        }
        if count < MIN_MATCH {
            (0..count).for_each(|_| self.symbols.literal(byte));
        } else {
            self.emit_matches(count, 1);
        }
    }

    /// Covers `count` (at least 3) bytes with matches at `distance`.
    fn emit_matches(&mut self, count: u64, distance: u64) {
        let (full, rest) = (count / MAX_MATCH, count % MAX_MATCH);
        match rest {
            // Too short for a match: borrow from the last full one instead.
            1 | 2 => {
                self.symbols.matches(MAX_MATCH, distance, full - 1);
                self.symbols.matches(MAX_MATCH + rest - MIN_MATCH, distance, 1);
                self.symbols.matches(MIN_MATCH, distance, 1);
            }
            _ => {
                self.symbols.matches(MAX_MATCH, distance, full);
                if rest > 0 {
                    self.symbols.matches(rest, distance, 1);
                }
            }
        }
    }
}

/// First pass: symbol frequencies and checksum.
struct Stats {
    litlen: [u64; LITLEN_CODES],
    dist: [u64; DIST_CODES],
    adler: Adler32,
}

impl Symbols for Stats {
    fn literal(&mut self, byte: u8) {
        self.litlen[byte as usize] += 1;
    }

    fn matches(&mut self, len: u64, distance: u64, times: u64) {
        self.litlen[length_code(len).0] += times;
        self.dist[distance_code(distance).0] += times;
    }

    fn raw_run(&mut self, byte: u8, count: u64) {
        self.adler.update_run(byte, count);
    }

    fn raw_repeat(&mut self, period: &[(u8, u64)], times: u64) {
        self.adler.update_repeat(period, times);
    }
}

/// Second pass: a single, final, dynamic Huffman block.
struct BlockWriter {
    bits: BitWriter,
    litlen: Vec<(u16, u8)>,
    dist: Vec<(u16, u8)>,
}

impl BlockWriter {
    fn new(litlen_freqs: &[u64; LITLEN_CODES], dist_freqs: &[u64; DIST_CODES]) -> Self {
        let litlen_lengths = code_lengths(litlen_freqs, 15);
        let dist_lengths = code_lengths(dist_freqs, 15);

        let mut bits = BitWriter::default();
        bits.write(1, 1); // BFINAL
        bits.write(2, 2); // BTYPE: dynamic Huffman codes
        write_code_lengths(&mut bits, &litlen_lengths, &dist_lengths);

        BlockWriter { bits, litlen: canonical_codes(&litlen_lengths), dist: canonical_codes(&dist_lengths) }
    }

    fn finish(mut self) -> Vec<u8> {
        let (code, len) = self.litlen[END_OF_BLOCK];
        self.bits.write(u64::from(code), len);
        self.bits.finish()
    }
}

impl Symbols for BlockWriter {
    fn literal(&mut self, byte: u8) {
        let (code, len) = self.litlen[byte as usize];
        self.bits.write(u64::from(code), len);
    }

    fn matches(&mut self, len: u64, distance: u64, times: u64) {
        let (symbol, extra_len, extra) = length_code(len);
        let (code, code_len) = self.litlen[symbol];
        let length_bits = (u64::from(code) | (extra << code_len), code_len + extra_len);

        let (symbol, extra_len, extra) = distance_code(distance);
        let (code, code_len) = self.dist[symbol];
        let distance_bits = (u64::from(code) | (extra << code_len), code_len + extra_len);

        for _ in 0..times {
            self.bits.write(length_bits.0, length_bits.1);
            self.bits.write(distance_bits.0, distance_bits.1);
        }
    }
}

/// Returns the length symbol, its number of extra bits and their value.
fn length_code(len: u64) -> (usize, u8, u64) {
    const BASES: [u64; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227,
        258,
    ];
    const EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];

    debug_assert!((MIN_MATCH..=MAX_MATCH).contains(&len));
    let index = BASES.partition_point(|&base| base <= len) - 1;
    (257 + index, EXTRA[index], len - BASES[index])
}

/// Returns the distance symbol, its number of extra bits and their value.
fn distance_code(distance: u64) -> (usize, u8, u64) {
    const BASES: [u64; DIST_CODES] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097,
        6145, 8193, 12289, 16385, 24577,
    ];

    debug_assert!((1..=MAX_DISTANCE).contains(&distance));
    let index = BASES.partition_point(|&base| base <= distance) - 1;
    (index, (index.saturating_sub(2) / 2) as u8, distance - BASES[index])
}

/// Writes HLIT, HDIST, HCLEN and the run-length coded code lengths.
fn write_code_lengths(bits: &mut BitWriter, litlen: &[u8], dist: &[u8]) {
    let hlit = litlen.iter().rposition(|&l| l > 0).map_or(0, |i| i + 1).max(257);
    let hdist = dist.iter().rposition(|&l| l > 0).map_or(0, |i| i + 1).max(1);
    let lengths: Vec<u8> = litlen[..hlit].iter().chain(&dist[..hdist]).copied().collect();

    // (symbol, extra bits count, extra bits value)
    let mut symbols: Vec<(usize, u8, u64)> = Vec::new();
    let mut i = 0;
    while i < lengths.len() {
        let len = lengths[i];
        let run = lengths[i..].iter().take_while(|&&l| l == len).count();
        let mut left = run;
        if len == 0 {
            while left >= 11 {
                let n = left.min(138);
                symbols.push((18, 7, (n - 11) as u64));
                left -= n;
            }
            if left >= 3 {
                symbols.push((17, 3, (left - 3) as u64));
                left = 0;
            }
        } else {
            symbols.push((len as usize, 0, 0));
            left -= 1;
            while left >= 3 {
                let n = left.min(6);
                symbols.push((16, 2, (n - 3) as u64));
                left -= n;
            }
        }
        symbols.extend((0..left).map(|_| (len as usize, 0, 0)));
        i += run;
    }

    let mut freqs = [0u64; CODELEN_CODES];
    for &(symbol, _, _) in &symbols {
        freqs[symbol] += 1;
    }
    let codelen_lengths = code_lengths(&freqs, 7);
    let codelen_codes = canonical_codes(&codelen_lengths);
    let hclen = CODELEN_ORDER.iter().rposition(|&s| codelen_lengths[s] > 0).map_or(0, |i| i + 1).max(4);

    bits.write((hlit - 257) as u64, 5);
    bits.write((hdist - 1) as u64, 5);
    bits.write((hclen - 4) as u64, 4);
    for &symbol in &CODELEN_ORDER[..hclen] {
        bits.write(u64::from(codelen_lengths[symbol]), 3);
    }
    for (symbol, extra_len, extra) in symbols {
        let (code, len) = codelen_codes[symbol];
        bits.write(u64::from(code), len);
        bits.write(extra, extra_len);
    }
}

/// Computes Huffman code lengths no longer than `limit` bits. At least two
/// symbols always get a code, so the resulting code is complete.
fn code_lengths(freqs: &[u64], limit: u8) -> Vec<u8> {
    let mut freqs = freqs.to_vec();
    for fallback in [0, 1] {
        if freqs.iter().filter(|&&f| f > 0).count() < 2 && freqs[fallback] == 0 {
            freqs[fallback] = 1;
        }
    }

    loop {
        let lengths = huffman_lengths(&freqs);
        if lengths.iter().all(|&l| l <= limit) {
            return lengths;
        }
        // Flatten the distribution until the tree fits.
        for f in freqs.iter_mut().filter(|f| **f > 0) {
            *f = f.div_ceil(2);
        }
    }
}

fn huffman_lengths(freqs: &[u64]) -> Vec<u8> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    // Nodes 0..n are leaves; internal nodes are appended with their parent
    // links so depths can be read back from the root.
    let mut parent: Vec<usize> = vec![usize::MAX; freqs.len()];
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> =
        freqs.iter().enumerate().filter(|&(_, &f)| f > 0).map(|(i, &f)| Reverse((f, i))).collect();

    while heap.len() > 1 {
        let Reverse((f1, a)) = heap.pop().unwrap();
        let Reverse((f2, b)) = heap.pop().unwrap();
        let node = parent.len();
        parent.push(usize::MAX);
        parent[a] = node;
        parent[b] = node;
        heap.push(Reverse((f1 + f2, node)));
    }

    // Parents always come after their children, so walk backwards.
    let mut depth = vec![0u8; parent.len()];
    for node in (0..parent.len()).rev() {
        if parent[node] != usize::MAX {
            depth[node] = depth[parent[node]] + 1;
        }
    }

    depth.truncate(freqs.len());
    depth
}

/// Canonical Huffman codes, bit-reversed for LSB-first output.
fn canonical_codes(lengths: &[u8]) -> Vec<(u16, u8)> {
    let mut count = [0u16; 16];
    for &len in lengths.iter().filter(|&&l| l > 0) {
        count[len as usize] += 1;
    }
    let mut next = [0u16; 16];
    for bits in 1..16 {
        next[bits] = (next[bits - 1] + count[bits - 1]) << 1;
    }

    lengths
        .iter()
        .map(|&len| {
            if len == 0 {
                return (0, 0);
            }
            let code = next[len as usize];
            next[len as usize] += 1;
            (code.reverse_bits() >> (16 - len), len)
        })
        .collect()
}

#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    filled: u8,
}

impl BitWriter {
    /// Appends the `count` low bits of `bits` (at most 32), LSB first.
    fn write(&mut self, bits: u64, count: u8) {
        debug_assert!(count <= 32 && bits >> count == 0);
        self.acc |= bits << self.filled;
        self.filled += count;
        if self.filled >= 32 {
            self.out.extend_from_slice(&(self.acc as u32).to_le_bytes());
            self.acc >>= 32;
            self.filled -= 32;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        let bytes = self.filled.div_ceil(8) as usize;
        self.out.extend_from_slice(&self.acc.to_le_bytes()[..bytes]);
        self.out
    }
}

struct Adler32 {
    a: u64,
    b: u64,
}

impl Adler32 {
    const MOD: u64 = 65521;

    fn new() -> Self {
        Adler32 { a: 1, b: 0 }
    }

    /// Feeds `count` copies of `byte` in constant time:
    /// `a += n·v` and `b += n·a + v·n(n+1)/2`.
    fn update_run(&mut self, byte: u8, count: u64) {
        let m = Self::MOD;
        let v = u64::from(byte);
        let n = count % m;

        self.b = (self.b + n * self.a + v * triangle(count)) % m;
        self.a = (self.a + n * v) % m;
    }

    /// Feeds `times` copies of `period`: with `m` its length and `(s, t)` its
    /// own sums from a zero state, `a += k·s` and
    /// `b += k·m·a + m·s·k(k-1)/2 + k·t`.
    fn update_repeat(&mut self, period: &[(u8, u64)], times: u64) {
        let m = Self::MOD;
        let mut block = Adler32 { a: 0, b: 0 };
        period.iter().for_each(|&(byte, count)| block.update_run(byte, count));
        let len = period.iter().map(|&(_, count)| count).sum::<u64>() % m;
        let k = times % m;

        self.b =
            (self.b + k * len % m * self.a + len * block.a % m * triangle(times.saturating_sub(1)) + k * block.b) % m;
        self.a = (self.a + k * block.a) % m;
    }

    fn value(&self) -> u32 {
        ((self.b << 16) | self.a) as u32
    }
}

/// `n(n+1)/2` modulo the Adler-32 modulus.
fn triangle(n: u64) -> u64 {
    let m = Adler32::MOD;
    // Halve whichever of n and n+1 is even before reducing.
    if n.is_multiple_of(2) { (n / 2 % m) * ((n + 1) % m) % m } else { (n % m) * (n.div_ceil(2) % m) % m }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block of runs, written once and then repeated `times` more.
    type Segment = (Vec<(u8, u64)>, u64);

    fn compress(segments: &[Segment]) -> Vec<u8> {
        zlib(|stream| {
            for (runs, times) in segments {
                runs.iter().for_each(|&(byte, count)| stream.run(byte, count));
                stream.repeat(runs, *times);
            }
        })
    }

    fn expand(segments: &[Segment]) -> Vec<u8> {
        let mut out = Vec::new();
        for (runs, times) in segments {
            for _ in 0..=*times {
                out.extend(runs.iter().flat_map(|&(byte, count)| std::iter::repeat_n(byte, count as usize)));
            }
        }
        out
    }

    fn reference_adler(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in data {
            a = (a + u32::from(byte)) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn inflate(data: &[u8]) -> Vec<u8> {
        miniz_oxide::inflate::decompress_to_vec_zlib(data).unwrap()
    }

    #[test]
    fn adler32_of_runs_and_repeats_matches_bytewise_computation() {
        let cases: [Vec<Segment>; 4] = [
            vec![(vec![(0, 1)], 0)],
            vec![(vec![(255, 100_000), (1, 3), (255, 65_521)], 0)],
            vec![(vec![(7, 5_552), (200, 5_553)], 0), (vec![(3, 17), (250, 2)], 70_000)],
            vec![(vec![(255, 65_521)], 3), (vec![(9, 1)], 131_042)],
        ];
        for segments in cases {
            let mut adler = Adler32::new();
            for (runs, times) in &segments {
                runs.iter().for_each(|&(byte, count)| adler.update_run(byte, count));
                adler.update_repeat(runs, *times);
            }
            assert_eq!(adler.value(), reference_adler(&expand(&segments)));
        }
    }

    #[test]
    fn length_codes_follow_rfc_1951() {
        assert_eq!(length_code(3), (257, 0, 0));
        assert_eq!(length_code(10), (264, 0, 0));
        assert_eq!(length_code(12), (265, 1, 1));
        assert_eq!(length_code(130), (280, 4, 15));
        assert_eq!(length_code(257), (284, 5, 30));
        assert_eq!(length_code(258), (285, 0, 0));
    }

    #[test]
    fn distance_codes_follow_rfc_1951() {
        assert_eq!(distance_code(1), (0, 0, 0));
        assert_eq!(distance_code(4), (3, 0, 0));
        assert_eq!(distance_code(6), (4, 1, 1));
        assert_eq!(distance_code(2012), (21, 9, 475));
        assert_eq!(distance_code(32768), (29, 13, 8191));
    }

    #[test]
    fn code_lengths_respect_the_limit() {
        let fibonacci: Vec<u64> = (0..30)
            .scan((1u64, 1u64), |s, _| {
                *s = (s.1, s.0 + s.1);
                Some(s.0)
            })
            .collect();
        let lengths = code_lengths(&fibonacci, 7);
        assert!(lengths.iter().all(|&l| (1..=7).contains(&l)));
        let kraft: f64 = lengths.iter().map(|&l| 0.5f64.powi(l as i32)).sum();
        assert!(kraft <= 1.0);
    }

    #[test]
    fn single_symbol_still_gets_a_complete_code() {
        let mut freqs = [0u64; 19];
        freqs[5] = 10;
        let lengths = code_lengths(&freqs, 7);
        assert_eq!(lengths.iter().filter(|&&l| l == 1).count(), 2);
    }

    #[test]
    fn round_trips_through_a_reference_inflater() {
        // Deterministic pseudo-random data covering literals, short and long
        // matches, every length remainder, and near and far repetitions.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        for _ in 0..50 {
            let segments: Vec<Segment> = (0..next(20) + 1)
                .map(|_| {
                    let runs = (0..next(30) + 1)
                        .map(|_| (next(6) as u8 * 51, [1, 2, 3, 258, 300, 5000][next(6) as usize] + next(3)))
                        .collect();
                    (runs, [0, 0, 1, 2, 40][next(5) as usize])
                })
                .collect();
            assert_eq!(inflate(&compress(&segments)), expand(&segments));
        }
    }

    #[test]
    fn round_trips_tiny_inputs() {
        for segments in
            [vec![(vec![(9, 1)], 0)], vec![(vec![(9, 1)], 1)], vec![(vec![(9, 1)], 2)], vec![(vec![(1, 1), (2, 1)], 1)]]
        {
            assert_eq!(inflate(&compress(&segments)), expand(&segments));
        }
    }

    #[test]
    fn writes_a_zlib_header_and_trailer() {
        let segments = vec![(vec![(1u8, 1000u64), (2, 1), (1, 259)], 3)];
        let out = compress(&segments);
        assert_eq!(&out[..2], &[0x78, 0xda]);
        assert_eq!(u16::from_be_bytes([out[0], out[1]]) % 31, 0);
        assert_eq!(out[out.len() - 4..], reference_adler(&expand(&segments)).to_be_bytes());
    }
}
