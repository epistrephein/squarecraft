# frozen_string_literal: true

require "securerandom"

require_relative "png"

module Squarecraft
  class Generator
    attr_reader :seed,
                :background, :colors,
                :rows, :cols, :size, :gap, :margin, :multiplier,
                :distribution, :sequence,
                :epoch, :filename, :picture

    SEED_REGEX = /\A[0-9a-f]+\z/
    COLOR_REGEX = /\A#[0-9a-f]{6}\z/

    DEFAULTS = {
      seed:       "3a8ef7b1",
      background: "#2b3240",
      colors:     ["#dbcfb0", "#bfc8ad", "#90b494", "#718f94", "#545775"],
      rows:       16,
      cols:       16,
      size:       2,
      gap:        0.15,
      margin:     8,
      multiplier: 80
    }.freeze

    def initialize(**args)
      setup_seed!(args)
      setup_colors!(args)
      setup_geometry!(args)
    end

    def paint!
      return self if painted?

      compute!

      @picture = draw!
      @epoch = Time.now.utc.to_i
      @filename = "#{epoch}-#{seed}--#{background}--#{colors.join('-')}.png"

      self
    end

    def compute!
      @rng = Random.new(seed.hex)
      @distribution = Hash[(0...colors.size).map { |i| [i, 0] }]
      @sequence = ""

      (rows * cols).times do
        random_color_index = @rng.rand(0...colors.size)

        @distribution[random_color_index] += 1
        @sequence += random_color_index.to_s
      end

      self
    end

    def painted?
      !@picture.nil?
    end

    private

    def setup_seed!(args)
      @seed = (args[:seed] || DEFAULTS[:seed]).downcase

      raise ArgumentError, "Seed must be a hex string" unless @seed.match?(SEED_REGEX)
    end

    def setup_colors!(args)
      @background = (args[:background] || DEFAULTS[:background]).downcase
      @colors     = (args[:colors]     || DEFAULTS[:colors]).map(&:downcase)

      raise ArgumentError, "Background must be in hex format" unless background.match?(COLOR_REGEX)
      raise ArgumentError, "Colors must be in hex format" unless colors.all? { |c| c.match?(COLOR_REGEX) }
    end

    def setup_geometry!(args)
      @rows       = args[:rows]       || DEFAULTS[:rows]
      @cols       = args[:cols]       || DEFAULTS[:cols]
      @size       = args[:size]       || DEFAULTS[:size]
      @gap        = args[:gap]        || DEFAULTS[:gap]
      @margin     = args[:margin]     || DEFAULTS[:margin]
      @multiplier = args[:multiplier] || DEFAULTS[:multiplier]
    end

    def draw!
      width, height = picture_size.map(&:round)

      Png.render(width:   width,
                 height:  height,
                 palette: [background] + colors,
                 rects:   rects)
    end

    # One [x, y, width, height, palette_index] square per tile, where index 0
    # is the background.
    def rects
      side = ((size - gap) * multiplier).round + 1 # rectangle edges are inclusive

      sequence.each_char.with_index.map do |color, index|
        x, y, = coords(index)

        [x.round, y.round, side, side, color.to_i + 1]
      end
    end

    def picture_size
      width  = (margin + (cols * size) + ((cols - 1) * gap) + margin) * multiplier
      height = (margin + (rows * size) + ((rows - 1) * gap) + margin) * multiplier

      [width, height]
    end

    def coords(index)
      mod_row = (index / cols)
      mod_col = (index % cols)

      x = margin + size * mod_col + gap * mod_col
      y = margin + size * mod_row + gap * mod_row

      [x, y, (x + size - gap), (y + size - gap)].map { |i| i * multiplier }
    end
  end
end
