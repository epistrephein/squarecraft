# frozen_string_literal: true

require "zlib"

module Squarecraft
  # Minimal indexed-color (palette) PNG writer.
  # Takes one scanline String per pixel row, where each byte is a palette
  # index, and returns a complete PNG file as a binary blob. Repeated
  # scanline objects are packed only once, so callers can share row Strings.
  class Png
    SIGNATURE = "\x89PNG\r\n\x1a\n".b
    FILTER_NONE = "\x00".b
    MAX_COLORS = 256

    def self.indexed(width:, height:, palette:, scanlines:)
      new(width, height, palette, scanlines).blob
    end

    def initialize(width, height, palette, scanlines)
      raise ArgumentError, "Palette can hold at most #{MAX_COLORS} colors" if palette.size > MAX_COLORS

      @width = width
      @height = height
      @palette = palette
      @scanlines = scanlines
    end

    def blob
      SIGNATURE +
        chunk("IHDR", [@width, @height, bit_depth, 3, 0, 0, 0].pack("NNC5")) +
        chunk("PLTE", @palette.map { |hex| [hex.delete_prefix("#")].pack("H*") }.join) +
        chunk("IDAT", Zlib::Deflate.deflate(raw_stream, Zlib::BEST_COMPRESSION)) +
        chunk("IEND", "")
    end

    private

    def chunk(type, data)
      [data.bytesize].pack("N") + type + data + [Zlib.crc32(type + data)].pack("N")
    end

    def bit_depth
      @palette.size <= 16 ? 4 : 8
    end

    def raw_stream
      packed = {}.compare_by_identity
      raw = String.new(capacity: @height * (@width + 1), encoding: Encoding::BINARY)

      @scanlines.each do |line|
        raw << FILTER_NONE << (packed[line] ||= pack_row(line))
      end

      raw
    end

    def pack_row(line)
      return line.b if bit_depth == 8

      indices = line.unpack("C*")
      indices << 0 if indices.size.odd?
      indices.each_slice(2).map { |hi, lo| (hi << 4) | lo }.pack("C*")
    end
  end
end
