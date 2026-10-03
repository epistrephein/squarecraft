//! Indexed-color PNG encoding of a rasterized canvas.

use crate::canvas::{self, Band, Rect};
use crate::deflate;

const SIGNATURE: [u8; 8] = *b"\x89PNG\r\n\x1a\n";
const MAX_DIMENSION: u32 = i32::MAX as u32;
const MAX_CHUNK: usize = i32::MAX as usize;
const COLOR_TYPE_INDEXED: u8 = 3;
const FILTER_NONE: u8 = 0;
const FILTER_UP: u8 = 2;

pub const MAX_COLORS: usize = 256;

/// Paints `rects` over a background of palette index 0 and encodes the
/// result as an indexed PNG. `palette` holds `0xRRGGBB` colors; each rect
/// color is an index into it.
pub fn render(width: u32, height: u32, palette: &[u32], rects: &[Rect]) -> Result<Vec<u8>, String> {
    if !(1..=MAX_DIMENSION).contains(&width) || !(1..=MAX_DIMENSION).contains(&height) {
        return Err(format!("Width and height must be between 1 and {MAX_DIMENSION}"));
    }
    if palette.is_empty() || palette.len() > MAX_COLORS {
        return Err(format!("Palette must hold between 1 and {MAX_COLORS} colors"));
    }
    if let Some(color) = palette.iter().find(|&&c| c > 0xff_ffff) {
        return Err(format!("Palette color {color:#x} is not a 24-bit RGB value"));
    }
    if let Some(rect) = rects.iter().find(|r| r.color as usize >= palette.len()) {
        return Err(format!("Color index {} is outside the palette", rect.color));
    }

    let bit_depth = bit_depth(palette.len());
    let bands = canvas::rasterize(width, height, bit_depth, rects);
    let packed_len = canvas::packed_len(width, bit_depth) as u64;
    let image = [Repetition::UpFilter, Repetition::BackReference]
        .map(|repetition| deflate::zlib(|stream| scanlines(&bands, packed_len, repetition, stream)))
        .into_iter()
        .min_by_key(Vec::len)
        .unwrap();

    let mut header = Vec::with_capacity(13);
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    header.extend([bit_depth, COLOR_TYPE_INDEXED, 0, 0, 0]);

    let plte: Vec<u8> = palette.iter().flat_map(|c| c.to_be_bytes()[1..].to_vec()).collect();

    let mut png = SIGNATURE.to_vec();
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"PLTE", &plte);
    for data in image.chunks(MAX_CHUNK) {
        chunk(&mut png, b"IDAT", data);
    }
    chunk(&mut png, b"IEND", &[]);

    Ok(png)
}

/// Smallest PNG bit depth able to index the whole palette.
fn bit_depth(colors: usize) -> u8 {
    match colors {
        0..=2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    }
}

/// How the rows repeating the first row of a band are encoded.
#[derive(Clone, Copy)]
enum Repetition {
    /// Up-filtered, turning each of them into a filter byte and zeros. Best
    /// for wide rows: runs of zeros are the cheapest thing to encode.
    UpFilter,
    /// Unfiltered, as a back-reference one row away. Best for narrow rows,
    /// where a single match spans several of them.
    BackReference,
}

/// Writes the filtered scanlines. The first row of a band is stored as is;
/// the rows repeating it follow `repetition`. Background-only rows are stored
/// as is, so that whole margins collapse into a single run of zeros, filter
/// bytes included.
fn scanlines(bands: &[Band], packed_len: u64, repetition: Repetition, stream: &mut dyn deflate::Stream) {
    for band in bands {
        if band.is_blank() {
            stream.run(FILTER_NONE, band.rows * (packed_len + 1));
            continue;
        }

        let row: Vec<(u8, u64)> = std::iter::once((FILTER_NONE, 1)).chain(band.runs.iter().copied()).collect();
        row.iter().for_each(|&(byte, count)| stream.run(byte, count));

        match repetition {
            Repetition::UpFilter => (1..band.rows).for_each(|_| {
                stream.run(FILTER_UP, 1);
                stream.run(0, packed_len);
            }),
            Repetition::BackReference => stream.repeat(&row, band.rows - 1),
        }
    }
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend((data.len() as u32).to_be_bytes());
    png.extend(kind);
    png.extend(data);
    png.extend(crc32(kind.iter().chain(data)).to_be_bytes());
}

fn crc32<'a>(bytes: impl Iterator<Item = &'a u8>) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut n = 0;
        while n < 256 {
            let mut c = n as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 == 1 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
                k += 1;
            }
            table[n] = c;
            n += 1;
        }
        table
    };

    !bytes.fold(!0u32, |crc, &byte| TABLE[((crc ^ u32::from(byte)) & 0xff) as usize] ^ (crc >> 8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_png_reference_value() {
        assert_eq!(crc32(b"IEND".iter()), 0xae42_6082);
    }

    #[test]
    fn picks_the_smallest_bit_depth() {
        assert_eq!([2, 3, 4, 5, 16, 17, 256].map(bit_depth), [1, 2, 2, 4, 4, 8, 8]);
    }

    #[test]
    fn rejects_invalid_input() {
        let rect = Rect { x: 0, y: 0, width: 1, height: 1, color: 2 };
        assert!(render(0, 1, &[0], &[]).is_err());
        assert!(render(1, 1, &[], &[]).is_err());
        assert!(render(1, 1, &[0; 257], &[]).is_err());
        assert!(render(1, 1, &[0x100_0000], &[]).is_err());
        assert!(render(1, 1, &[0, 0], &[rect]).is_err());
    }

    #[test]
    fn writes_the_png_chunks() {
        let png = render(3, 2, &[0x2b3240, 0xdbcfb0], &[]).unwrap();
        assert_eq!(png[..8], SIGNATURE);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(png[16..29], [0, 0, 0, 3, 0, 0, 0, 2, 1, 3, 0, 0, 0]);
        assert_eq!(&png[37..41], b"PLTE");
        assert_eq!(png[41..47], [0x2b, 0x32, 0x40, 0xdb, 0xcf, 0xb0]);
        assert_eq!(png[png.len() - 12..], [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82]);
    }
}
