# frozen_string_literal: true

require "zlib"

# Decodes the indexed PNGs written by Squarecraft::Png, verifying chunk CRCs
# and the zlib stream, into rows of palette indices.
module PngReader
  Image = Struct.new(:width, :height, :bit_depth, :palette, :rows, keyword_init: true) do
    # Pixel colors as "#rrggbb" Strings.
    def colors
      rows.map { |row| row.map { |index| palette[index] } }
    end
  end

  module_function

  def read(blob)
    raise ArgumentError, "Not a PNG" unless blob.byteslice(0, 8) == "\x89PNG\r\n\x1a\n".b

    chunks = chunks(blob)
    width, height, bit_depth, color_type = chunks.fetch("IHDR").first.unpack("NNCC")
    raise ArgumentError, "Not an indexed PNG" unless color_type == 3

    palette = chunks.fetch("PLTE").first.unpack1("H*").scan(/.{6}/).map { |hex| "##{hex}" }
    raw = Zlib::Inflate.inflate(chunks.fetch("IDAT").join)

    Image.new(width: width, height: height, bit_depth: bit_depth, palette: palette,
              rows: unfilter(raw, width, height, bit_depth))
  end

  def chunks(blob)
    chunks = Hash.new { |hash, type| hash[type] = [] }
    offset = 8

    while offset < blob.bytesize
      length, type = blob.byteslice(offset, 8).unpack("Na4")
      data = blob.byteslice(offset + 8, length)
      crc = blob.byteslice(offset + 8 + length, 4).unpack1("N")
      raise ArgumentError, "Bad CRC in #{type}" unless Zlib.crc32(type + data) == crc

      chunks[type] << data
      offset += length + 12
    end

    chunks
  end

  def unfilter(raw, width, height, bit_depth)
    stride = ((width * bit_depth) + 7) / 8
    raise ArgumentError, "Bad image data length" unless raw.bytesize == height * (stride + 1)

    previous = Array.new(stride, 0)
    Array.new(height) do |y|
      filter = raw.getbyte(y * (stride + 1))
      line = raw.byteslice((y * (stride + 1)) + 1, stride).bytes
      case filter
      when 0 then nil
      when 2 then line = line.zip(previous).map { |byte, above| (byte + above) & 0xff }
      else raise ArgumentError, "Unsupported filter #{filter}"
      end
      previous = line

      line.pack("C*").unpack1("B*").scan(/.{#{bit_depth}}/).first(width).map { |bits| bits.to_i(2) }
    end
  end
end
