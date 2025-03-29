# frozen_string_literal: true

require "rmagick"

module Squarecraft
  class Generator
    attr_reader :seed,
                :background, :colors,
                :rows, :cols, :size, :gap, :margin, :multiplier,
                :distribution, :sequence, :epoch, :picture

    DEFAULTS = {
      seed:       "3A8EF7B1",
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
      @seed = args[:seed] || DEFAULTS[:seed]
    end

    def setup_colors!(args)
      @background = args[:background] || DEFAULTS[:background]
      @colors     = args[:colors]     || DEFAULTS[:colors]
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
      bg  = Magick::SolidFill.new(background)
      img = Magick::Image.new(*picture_size, bg)

      sequence.chars.each_with_index do |c, i|
        gc = Magick::Draw.new
        gc.fill(colors[c.to_i])
        gc.rectangle(*coords(i))
        gc.draw(img)
      end

      img
    end

    def picture_size
      width  = (margin + (rows * size) + ((rows - 1) * gap) + margin) * multiplier
      height = (margin + (cols * size) + ((cols - 1) * gap) + margin) * multiplier

      [width, height]
    end

    def coords(index)
      mod_row = (index % rows)
      mod_col = (index / cols)

      x = margin + size * mod_row + gap * mod_row
      y = margin + size * mod_col + gap * mod_col

      [x, y, (x + size - gap), (y + size - gap)].map { |i| i * multiplier }
    end
  end
end
