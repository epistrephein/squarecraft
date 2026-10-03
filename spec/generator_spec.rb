# frozen_string_literal: true

RSpec.describe Squarecraft::Generator do
  describe "#initialize" do
    context "with defaults" do
      subject(:generator) { described_class.new }

      it "uses default seed" do
        expect(generator.seed).to eq("3a8ef7b1")
      end

      it "uses default background" do
        expect(generator.background).to eq("#2b3240")
      end

      it "uses default colors" do
        expect(generator.colors).to eq(["#dbcfb0", "#bfc8ad", "#90b494", "#718f94", "#545775"])
      end

      it "uses default geometry" do
        expect(generator.rows).to eq(16)
        expect(generator.cols).to eq(16)
        expect(generator.size).to eq(2)
        expect(generator.gap).to eq(0.15)
        expect(generator.margin).to eq(8)
        expect(generator.multiplier).to eq(80)
      end
    end

    context "with custom arguments" do
      it "accepts a custom seed" do
        generator = described_class.new(seed: "abcdef01")
        expect(generator.seed).to eq("abcdef01")
      end

      it "downcases the seed" do
        generator = described_class.new(seed: "ABCDEF01")
        expect(generator.seed).to eq("abcdef01")
      end

      it "accepts custom background" do
        generator = described_class.new(background: "#FF0000")
        expect(generator.background).to eq("#ff0000")
      end

      it "accepts custom colors" do
        colors = ["#ff0000", "#00ff00"]
        generator = described_class.new(colors: colors)
        expect(generator.colors).to eq(colors)
      end

      it "downcases colors" do
        generator = described_class.new(colors: ["#FF0000", "#00FF00"])
        expect(generator.colors).to eq(["#ff0000", "#00ff00"])
      end

      it "accepts custom geometry" do
        generator = described_class.new(rows: 8, cols: 8, size: 4, gap: 0.5, margin: 4, multiplier: 40)
        expect(generator.rows).to eq(8)
        expect(generator.cols).to eq(8)
        expect(generator.size).to eq(4)
        expect(generator.gap).to eq(0.5)
        expect(generator.margin).to eq(4)
        expect(generator.multiplier).to eq(40)
      end
    end

    context "with invalid arguments" do
      it "raises ArgumentError for non-hex seed" do
        expect { described_class.new(seed: "xyz!") }.to raise_error(ArgumentError, "Seed must be a hex string")
      end

      it "raises ArgumentError for invalid background color" do
        expect { described_class.new(background: "red") }.to raise_error(ArgumentError, "Background must be in hex format")
      end

      it "raises ArgumentError for invalid colors" do
        expect { described_class.new(colors: ["not-a-color"]) }.to raise_error(ArgumentError, "Colors must be in hex format")
      end
    end
  end

  describe "#compute!" do
    subject(:generator) { described_class.new(rows: 2, cols: 2, colors: ["#ff0000", "#00ff00"]) }

    before { generator.compute! }

    it "generates a sequence matching rows * cols length" do
      expect(generator.sequence.length).to eq(4)
    end

    it "generates a sequence containing only valid color indices" do
      expect(generator.sequence).to match(/\A[01]+\z/)
    end

    it "populates distribution for all colors" do
      expect(generator.distribution.keys).to eq([0, 1])
      expect(generator.distribution.values.sum).to eq(4)
    end

    it "produces deterministic results for same seed" do
      gen1 = described_class.new(seed: "aabb", rows: 4, cols: 4, colors: ["#ff0000", "#00ff00"])
      gen2 = described_class.new(seed: "aabb", rows: 4, cols: 4, colors: ["#ff0000", "#00ff00"])
      gen1.compute!
      gen2.compute!
      expect(gen1.sequence).to eq(gen2.sequence)
      expect(gen1.distribution).to eq(gen2.distribution)
    end

    it "returns self" do
      new_gen = described_class.new(rows: 2, cols: 2)
      expect(new_gen.compute!).to be(new_gen)
    end
  end

  describe "#paint!" do
    subject(:generator) { described_class.new(rows: 2, cols: 2) }

    it "sets picture to a PNG blob" do
      generator.paint!
      expect(generator.picture).to be_a(String)
      expect(generator.picture.bytesize).to be > 8
      expect(generator.picture.byteslice(0, 8)).to eq("\x89PNG\r\n\x1a\n".b)
    end

    it "encodes the expected dimensions in the PNG header" do
      generator.paint!
      width, height = generator.picture.byteslice(16, 8).unpack("NN")
      expect(width).to eq(1612)
      expect(height).to eq(1612)
    end

    it "draws every tile with its color over the background" do
      generator = described_class.new(rows: 3, cols: 4, size: 2, gap: 0.5, margin: 1, multiplier: 10)
      colors = PngReader.read(generator.paint!.picture).colors

      expect(colors.first.uniq).to eq([generator.background])
      generator.sequence.each_char.with_index do |color, index|
        x = 10 + ((index % 4) * 25)
        y = 10 + ((index / 4) * 25)

        expect(colors[y][x]).to eq(generator.colors[color.to_i])
        expect(colors[y + 15][x + 15]).to eq(generator.colors[color.to_i])
        expect(colors[y + 16][x + 16]).to eq(generator.background)
      end
    end

    it "sets epoch" do
      generator.paint!
      expect(generator.epoch).to be_a(Integer)
    end

    it "sets filename with expected format" do
      generator.paint!
      expect(generator.filename).to match(/\A\d+-#{generator.seed}--#{Regexp.escape(generator.background)}--.*\.png\z/)
    end

    it "returns self" do
      expect(generator.paint!).to be(generator)
    end

    it "is idempotent" do
      generator.paint!
      first_picture = generator.picture
      generator.paint!
      expect(generator.picture).to be(first_picture)
    end
  end

  describe "#painted?" do
    subject(:generator) { described_class.new(rows: 2, cols: 2) }

    it "returns false before painting" do
      expect(generator.painted?).to be(false)
    end

    it "returns true after painting" do
      generator.paint!
      expect(generator.painted?).to be(true)
    end
  end
end
