# frozen_string_literal: true

begin
  require_relative "squarecraft/squarecraft"
rescue LoadError
  raise LoadError, "Squarecraft native extension is missing: build it with `bundle exec rake compile`"
end

module Squarecraft
  # Indexed-color PNG renderer backed by the native extension.
  # Paints filled rectangles over the first palette color and returns the
  # complete PNG file as a binary blob.
  module Png
    MAX_COLORS = Native::MAX_COLORS

    # palette: Array of "#rrggbb" Strings, the first one being the background.
    # rects:   Array of [x, y, width, height, palette_index], painted in order.
    def self.render(width:, height:, palette:, rects:)
      Native.render_png(width, height, palette.map { |hex| hex.delete_prefix("#").to_i(16) }, rects)
    end
  end
end
