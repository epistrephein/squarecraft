# Squarecraft

A small, deterministic generative-art tool written in Ruby.

It fills a grid with colored tiles drawn from a palette and renders the result
as a PNG. Every image is driven by a hexadecimal seed, so the same seed and
settings always produce the exact same picture.

It works like this:

1. A seed (any valid hex string) initializes a seeded random number generator.
2. Squarecraft walks a `rows × cols` grid and, for each cell, picks a palette
   color using that PRNG.
3. Each cell is drawn as a square with a configurable size, gap, margin, and scale.
4. The image is written to a PNG whose filename records the seed, background,
  and palette, so any picture can be traced back to the exact inputs that
  produced it.

Because the RNG is seeded, generation is fully deterministic: using the same seed,
palette and geometry will produce a byte-for-byte identical layout every time.

## Requirements

- **Ruby 4**
- **ImageMagick** (the [RMagick](https://github.com/rmagick/rmagick) gem does
  the actual drawing)

Install ImageMagick first:

```bash
# macOS
brew install imagemagick

# Debian/Ubuntu
sudo apt-get install libmagickwand-dev imagemagick
```

Then just install the gem dependencies:

```bash
bundle install
```

## Usage

Run the CLI with no arguments to generate an image using the built-in defaults:

```bash
bin/squarecraft
# => Generated: 1759483108-3a8ef7b1--#2b3240--#dbcfb0-#bfc8ad-#90b494-#718f94-#545775.png
```

Images are written to the `generated/` directory. The filename encodes the epoch,
seed, background, and palette so it can always be reproduced.

### Examples

```bash
# Use a named palette and a random seed
bin/squarecraft --palette sunset --random-seed

# Reproduce a specific image from its seed
bin/squarecraft --palette estuary --seed a1b2c3d4

# Bring your own colors and background
bin/squarecraft --background "#000000" --colors "#ff0000,#00ff00,#0000ff,#ffff00,#ffffff"

# Change the grid geometry
bin/squarecraft --palette caprese --rows 24 --cols 24 --gap 0.25
```

### Options

| Short | Long              | Argument            | Description                                        |
| ----- | ----------------- | ------------------- | -------------------------------------------------- |
| `-e`  | `--geometry`      | `NAME`              | Named geometry from `config/geometries.yml`        |
| `-p`  | `--palette`       | `NAME`              | Named palette from `config/palettes.yml`           |
| `-s`  | `--seed`          | `HEX`               | RNG seed, a hex string (e.g. `a1b2c3d4`)           |
| `-q`  | `--random-seed`   |                     | Generate a random 8-character hex seed             |
| `-b`  | `--background`    | `#HEX`              | Background color (e.g. `#000000`)                  |
| `-c`  | `--colors`        | `#HEX1,#HEX2,...`   | Comma-separated tile colors                        |
| `-r`  | `--rows`          | `INT`               | Number of rows (> 0)                               |
| `-l`  | `--cols`          | `INT`               | Number of columns (> 0)                            |
| `-z`  | `--size`          | `INT`               | Tile size (> 0)                                    |
| `-g`  | `--gap`           | `FLOAT`             | Gap between tiles                                  |
| `-m`  | `--margin`        | `INT`               | Outer margin                                       |
| `-x`  | `--multiplier`    | `INT`               | Scale multiplier for the final image (> 0)         |
| `-v`  | `--version`       |                     | Print the version                                  |
| `-h`  | `--help`          |                     | Show help                                          |

Colors and backgrounds must be six-digit hex (`#rrggbb`); seeds must be a valid hex string.  
Palette and geometry options are simply presets that fill in these same values: anything
passed explicitly on the command line overrides the preset.

## Palettes and geometries

Presets live in plain YAML files so custom values can be added without touching any code.

**`config/palettes.yml`** – each palette names a `background` and a list of `colors`:

```yaml
estuary:
  background: "#2b3240"
  colors:
    - "#dbcfb0"
    - "#bfc8ad"
    - "#90b494"
    - "#718f94"
    - "#545775"
```

**`config/geometries.yml`** – each geometry names a grid layout:

```yaml
classic:
  rows: 16
  cols: 16
  size: 2
  gap: 0.15
  margin: 8
  multiplier: 80
```

## Development

The interactive console can be started with:

```bash
bin/console
```

It can be used for experimenting with the generator directly in Ruby:

```ruby
gen = Squarecraft::Generator.new(seed: "a1b2c3d4", **Squarecraft::Config.palettes[:sunset])
gen.paint!
gen.picture.write("out.png")
```

Tests and linting can be run with:

```bash
bundle exec rspec
bundle exec rubocop
```

## License

This project is released as open source under the terms of the [MIT License](LICENSE.md).
