# frozen_string_literal: true

RSpec.describe Squarecraft::Png do
  let(:palette) { ["#2b3240", "#dbcfb0", "#bfc8ad", "#90b494", "#718f94", "#545775"] }

  # Straightforward per-pixel painter the native renderer must agree with.
  def paint(width, height, rects)
    rows = Array.new(height) { Array.new(width, 0) }
    rects.each do |x, y, w, h, index|
      ([y, 0].max...[y + h, height].min).each do |py|
        ([x, 0].max...[x + w, width].min).each { |px| rows[py][px] = index }
      end
    end
    rows
  end

  def render(width, height, rects, palette: self.palette)
    described_class.render(width: width, height: height, palette: palette, rects: rects)
  end

  describe ".render" do
    it "returns a binary PNG blob" do
      picture = render(4, 3, [])
      expect(picture.encoding).to eq(Encoding::BINARY)
      expect(picture.byteslice(0, 8)).to eq("\x89PNG\r\n\x1a\n".b)
    end

    it "writes the dimensions and the palette" do
      image = PngReader.read(render(7, 5, []))
      expect([image.width, image.height]).to eq([7, 5])
      expect(image.palette).to eq(palette)
    end

    it "fills the canvas with the first palette color" do
      expect(PngReader.read(render(5, 4, [])).rows).to eq(Array.new(4) { Array.new(5, 0) })
    end

    it "paints rectangles in order, clipped to the canvas" do
      rects = [[1, 1, 3, 2, 1], [2, 2, 3, 3, 2], [-2, -2, 3, 3, 3], [5, 4, 10, 10, 4], [0, 0, 0, 5, 5]]
      expect(PngReader.read(render(6, 5, rects)).rows).to eq(paint(6, 5, rects))
    end

    it "matches a reference painter on random layouts" do
      random = Random.new(42)
      20.times do
        width = random.rand(1..120)
        height = random.rand(1..120)
        rects = Array.new(random.rand(0..30)) do
          [random.rand(-20..width), random.rand(-20..height), random.rand(-5..60), random.rand(-5..60), random.rand(palette.size)]
        end

        expect(PngReader.read(render(width, height, rects)).rows).to eq(paint(width, height, rects))
      end
    end

    it "encodes tall images with repeated rows" do
      rects = [[10, 10, 300, 900, 1], [400, 0, 1, 1000, 2], [0, 990, 1000, 10, 3]]
      expect(PngReader.read(render(1000, 1000, rects)).rows).to eq(paint(1000, 1000, rects))
    end

    {
      2   => 1,
      4   => 2,
      16  => 4,
      17  => 8,
      256 => 8
    }.each do |colors, bit_depth|
      it "uses a bit depth of #{bit_depth} for #{colors} colors" do
        big_palette = Array.new(colors) { |i| format("#%06x", i * 0x010101) }
        rects = Array.new(colors) { |i| [i, 0, 1, 2, i] }
        image = PngReader.read(render(colors, 2, rects, palette: big_palette))

        expect(image.bit_depth).to eq(bit_depth)
        expect(image.rows).to eq(paint(colors, 2, rects))
      end
    end

    it "rejects palettes larger than #{described_class::MAX_COLORS} colors" do
      expect { render(1, 1, [], palette: Array.new(257, "#000000")) }.to raise_error(ArgumentError, /Palette/)
    end

    it "rejects color indices outside the palette" do
      expect { render(2, 2, [[0, 0, 1, 1, 6]]) }.to raise_error(ArgumentError, /outside the palette/)
    end

    it "rejects empty images" do
      expect { render(0, 3, []) }.to raise_error(ArgumentError, /Width and height/)
    end

    it "rejects malformed rectangles" do
      expect { render(2, 2, [[0, 0, 1, 1]]) }.to raise_error(TypeError)
    end
  end
end
