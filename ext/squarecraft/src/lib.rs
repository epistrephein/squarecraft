//! Native PNG renderer for Squarecraft.

mod canvas;
mod deflate;
mod png;

use magnus::{Error, RString, Ruby, function, prelude::*};

use crate::canvas::Rect;

/// `Squarecraft::Native.render_png(width, height, palette, rects)`
///
/// `palette` is an Array of `0xRRGGBB` Integers and `rects` an Array of
/// `[x, y, width, height, palette_index]` Arrays, painted in order.
/// Returns the PNG file as a binary String.
fn render_png(
    ruby: &Ruby,
    width: u32,
    height: u32,
    palette: Vec<u32>,
    rects: Vec<(i64, i64, i64, i64, u8)>,
) -> Result<RString, Error> {
    let rects: Vec<Rect> =
        rects.into_iter().map(|(x, y, width, height, color)| Rect { x, y, width, height, color }).collect();

    png::render(width, height, &palette, &rects)
        .map(|png| ruby.str_from_slice(&png))
        .map_err(|message| Error::new(ruby.exception_arg_error(), message))
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let native = ruby.define_module("Squarecraft")?.define_module("Native")?;
    native.const_set("MAX_COLORS", png::MAX_COLORS)?;
    native.define_module_function("render_png", function!(render_png, 4))?;

    Ok(())
}
