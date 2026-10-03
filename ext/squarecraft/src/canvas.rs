//! Rasterization of filled rectangles into horizontal bands.
//!
//! The generated pictures are made of a few rows of tiles, so the image is
//! stored as bands: stretches of consecutive pixel rows sharing the exact same
//! content. Each band keeps its row once, already packed to the PNG bit depth
//! and run-length encoded, so the cost of the rest of the pipeline depends on
//! the number of bands and color runs rather than on the number of pixels.

/// A filled, axis-aligned rectangle painted with a palette index.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub color: u8,
}

/// A stretch of identical pixel rows.
#[derive(Debug, PartialEq, Eq)]
pub struct Band {
    /// Number of pixel rows in the band.
    pub rows: u64,
    /// The packed scanline (without filter byte) as `(byte, count)` runs.
    pub runs: Vec<(u8, u64)>,
}

impl Band {
    /// Whether every byte of the scanline is zero (background only).
    pub fn is_blank(&self) -> bool {
        self.runs.iter().all(|&(byte, _)| byte == 0)
    }
}

/// Paints `rects` in order over a background of palette index 0, clipping
/// them to the canvas, and returns the bands covering the whole image from
/// top to bottom. Rows are packed `bit_depth` bits per pixel, most
/// significant bits first, as PNG expects.
pub fn rasterize(width: u32, height: u32, bit_depth: u8, rects: &[Rect]) -> Vec<Band> {
    // Visible rects clipped to `[x0, y0, x1, y1]` (exclusive ends), in painting order.
    let visible: Vec<([u64; 4], u8)> =
        rects.iter().filter_map(|r| clip(r, width, height).map(|c| (c, r.color))).collect();

    let mut edges: Vec<u64> = vec![0, u64::from(height)];
    edges.extend(visible.iter().flat_map(|&([_, y0, _, y1], _)| [y0, y1]));
    edges.sort_unstable();
    edges.dedup();

    // Rects sorted by top edge, picked up as the sweep moves down.
    let mut by_top: Vec<usize> = (0..visible.len()).collect();
    by_top.sort_by_key(|&i| visible[i].0[1]);
    let mut pending = by_top.into_iter().peekable();
    let mut active: Vec<usize> = Vec::new();

    let mut pixels = vec![0u8; width as usize];
    let mut packed = vec![0u8; packed_len(width, bit_depth)];
    let mut bands: Vec<Band> = Vec::new();

    for span in edges.windows(2) {
        let (top, bottom) = (span[0], span[1]);

        active.retain(|&i| visible[i].0[3] > top);
        while let Some(i) = pending.next_if(|&i| visible[i].0[1] == top) {
            active.push(i);
        }
        // Later rects are painted over earlier ones.
        active.sort_unstable();

        pixels.fill(0);
        for &i in &active {
            let ([x0, _, x1, _], color) = visible[i];
            pixels[x0 as usize..x1 as usize].fill(color);
        }
        pack(&pixels, bit_depth, &mut packed);
        let runs = runs(&packed);

        match bands.last_mut() {
            Some(last) if last.runs == runs => last.rows += bottom - top,
            _ => bands.push(Band { rows: bottom - top, runs }),
        }
    }

    bands
}

/// Number of bytes in a packed scanline.
pub fn packed_len(width: u32, bit_depth: u8) -> usize {
    (width as usize * bit_depth as usize).div_ceil(8)
}

/// Clips a rect to the canvas, returning `[x0, y0, x1, y1]` (exclusive ends)
/// or `None` when nothing of it is left.
fn clip(rect: &Rect, width: u32, height: u32) -> Option<[u64; 4]> {
    let span = |start: i64, len: i64, max: u32| {
        let end = start.saturating_add(len.max(0));
        let clamp = |v: i64| v.clamp(0, i64::from(max)) as u64;
        (clamp(start), clamp(end))
    };
    let (x0, x1) = span(rect.x, rect.width, width);
    let (y0, y1) = span(rect.y, rect.height, height);

    (x0 < x1 && y0 < y1).then_some([x0, y0, x1, y1])
}

fn pack(pixels: &[u8], bit_depth: u8, out: &mut [u8]) {
    if bit_depth == 8 {
        out.copy_from_slice(pixels);
        return;
    }

    let per_byte = (8 / bit_depth) as usize;
    for (byte, chunk) in out.iter_mut().zip(pixels.chunks(per_byte)) {
        *byte = chunk.iter().enumerate().fold(0, |acc, (i, &p)| acc | (p << (8 - bit_depth as usize * (i + 1))));
    }
}

fn runs(bytes: &[u8]) -> Vec<(u8, u64)> {
    let mut runs: Vec<(u8, u64)> = Vec::new();
    for &byte in bytes {
        match runs.last_mut() {
            Some((last, count)) if *last == byte => *count += 1,
            _ => runs.push((byte, 1)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i64, y: i64, width: i64, height: i64, color: u8) -> Rect {
        Rect { x, y, width, height, color }
    }

    #[test]
    fn blank_canvas_is_a_single_band() {
        let bands = rasterize(10, 7, 8, &[]);
        assert_eq!(bands, vec![Band { rows: 7, runs: vec![(0, 10)] }]);
        assert!(bands[0].is_blank());
    }

    #[test]
    fn rects_split_the_canvas_into_bands() {
        let bands = rasterize(6, 6, 8, &[rect(1, 1, 2, 2, 1), rect(3, 2, 2, 3, 2)]);
        let runs: Vec<(u64, Vec<(u8, u64)>)> = bands.into_iter().map(|b| (b.rows, b.runs)).collect();
        assert_eq!(
            runs,
            vec![
                (1, vec![(0, 6)]),
                (1, vec![(0, 1), (1, 2), (0, 3)]),
                (1, vec![(0, 1), (1, 2), (2, 2), (0, 1)]),
                (2, vec![(0, 3), (2, 2), (0, 1)]),
                (1, vec![(0, 6)]),
            ]
        );
    }

    #[test]
    fn later_rects_paint_over_earlier_ones() {
        let bands = rasterize(4, 1, 8, &[rect(0, 0, 3, 1, 1), rect(1, 0, 3, 1, 2)]);
        assert_eq!(bands[0].runs, vec![(1, 1), (2, 3)]);
    }

    #[test]
    fn rects_are_clipped_to_the_canvas() {
        let bands = rasterize(4, 2, 8, &[rect(-2, -5, 3, 6, 1), rect(3, 1, 100, 100, 2), rect(1, 0, -1, 2, 3)]);
        assert_eq!(bands[0].runs, vec![(1, 1), (0, 3)]);
        assert_eq!(bands[1].runs, vec![(0, 3), (2, 1)]);
    }

    #[test]
    fn identical_neighbouring_bands_are_merged() {
        let bands = rasterize(4, 4, 8, &[rect(0, 0, 2, 2, 1), rect(0, 2, 2, 2, 1)]);
        assert_eq!(bands, vec![Band { rows: 4, runs: vec![(1, 2), (0, 2)] }]);
    }

    #[test]
    fn rows_are_packed_to_the_bit_depth() {
        let bands = rasterize(5, 1, 4, &[rect(1, 0, 3, 1, 3)]);
        assert_eq!(bands[0].runs, vec![(0x03, 1), (0x33, 1), (0x00, 1)]);

        let bands = rasterize(5, 1, 1, &[rect(1, 0, 3, 1, 1)]);
        assert_eq!(bands[0].runs, vec![(0b0111_0000, 1)]);
    }
}
