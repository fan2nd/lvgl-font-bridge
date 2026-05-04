# lvgl-font-bridge

`lvgl-font-bridge` is a small `no_std` helper crate for rendering LVGL-generated bitmap fonts with `embedded-graphics`.

It keeps the original 1bpp glyph bitmap data, but replaces LVGL's numeric character mapping with direct UTF-8 symbols stored in a single `&str`.

## Features

- `FontData` stores:
  - raw bitmap bytes
  - UTF-8 symbol string
  - glyph metrics
  - native size, line height, and baseline
- `EgTextStyle` implements:
  - `embedded_graphics::text::renderer::TextRenderer`
  - `embedded_graphics::text::renderer::CharacterStyle`
- Horizontal advance is controlled by the user:
  - one width for ASCII characters
  - one width for non-ASCII characters
- Vertical layout scales with `size`
- Scaled presets use interpolated bitmap rendering at runtime
- `FontData::new` contains a const assertion that checks:
  - `symbols.chars().count() == metrics.len()`

## Data Model

`symbols` is a single UTF-8 string. Each character in that string corresponds to one entry in `metrics` at the same index.

```rust
use lvgl_font_bridge::{FontData, GlyphMetrics};

const BITMAP: &[u8] = &[0x00, 0x00];
const SYMBOLS: &str = "1H哈";
const METRICS: &[GlyphMetrics] = &[
    GlyphMetrics::new(0, 167, 7, 15, 2, 0),
    GlyphMetrics::new(14, 223, 10, 15, 2, 0),
    GlyphMetrics::new(33, 320, 17, 18, 2, -1),
];

const FONT: FontData<'static> = FontData::new(BITMAP, SYMBOLS, METRICS, 20, 18, 1);
```

## Rendering

Create an `EgTextStyle` with:

- a `FontPreset`
- a text color
- a logical `size`

```rust
use embedded_graphics::pixelcolor::BinaryColor;
use lvgl_font_bridge::{EgTextStyle, FontPreset};

# use lvgl_font_bridge::{FontData, GlyphMetrics};
# const BITMAP: &[u8] = &[0x00];
# const SYMBOLS: &str = "A哈";
# const METRICS: &[GlyphMetrics] = &[
#     GlyphMetrics::new(0, 3, 1, 1, 0, 0),
#     GlyphMetrics::new(0, 5, 1, 1, 1, 0),
# ];
# const FONT: FontData<'static> = FontData::new(BITMAP, SYMBOLS, METRICS, 10, 10, 2);
# const PRESET: FontPreset<'static> = FontPreset::new(FONT, 8, 16, 20);
let style = EgTextStyle::new(&PRESET, BinaryColor::On, 20);
```

Then use it with `embedded-graphics::text::Text`.

## Proportional Scaling Wrapper

If you want half-width, full-width, and height to scale together based on the original preset, use `FontPreset::scaled_ratio(numerator, denominator)`.

```rust
use embedded_graphics::pixelcolor::BinaryColor;
use lvgl_font_bridge::{FontData, FontPreset, GlyphMetrics};

# const BITMAP: &[u8] = &[0x00];
# const SYMBOLS: &str = "A哈";
# const METRICS: &[GlyphMetrics] = &[
#     GlyphMetrics::new(0, 3, 1, 1, 0, 0),
#     GlyphMetrics::new(0, 5, 1, 1, 1, 0),
# ];
# const FONT: FontData<'static> = FontData::new(BITMAP, SYMBOLS, METRICS, 10, 10, 2);
const PRESET: FontPreset<'static> = FontPreset::new(FONT, 8, 16, 20);

let scaled = PRESET.scaled_ratio(1, 2);
let style = scaled.default_text_style(BinaryColor::On);
```

This keeps the original proportions:

- `half_width`
- `full_width`
- `height`

When a style is created from `scaled_ratio(...)`, glyph rendering also uses interpolated bitmap scaling instead of only changing layout metrics.

`EgTextStyle` can be constructed directly from the merged preset type:

```rust
# use embedded_graphics::pixelcolor::BinaryColor;
# use lvgl_font_bridge::{EgTextStyle, FontData, FontPreset, GlyphMetrics};
# const BITMAP: &[u8] = &[0x00];
# const SYMBOLS: &str = "A哈";
# const METRICS: &[GlyphMetrics] = &[
#     GlyphMetrics::new(0, 3, 1, 1, 0, 0),
#     GlyphMetrics::new(0, 5, 1, 1, 1, 0),
# ];
# const FONT: FontData<'static> = FontData::new(BITMAP, SYMBOLS, METRICS, 10, 10, 2);
# const PRESET: FontPreset<'static> = FontPreset::new(FONT, 8, 16, 20);
let fixed = EgTextStyle::from_preset(&PRESET, BinaryColor::On, 20);
let scaled = PRESET.scaled_ratio(3, 2);
let proportional = EgTextStyle::from_preset(&scaled, BinaryColor::On, 0);

assert_eq!(fixed.scale_ratio(), None);
assert_eq!(proportional.scale_ratio(), Some((3, 2)));
```

For non-integer ratios:

```rust
# use lvgl_font_bridge::{FontData, FontPreset, GlyphMetrics};
# const BITMAP: &[u8] = &[0x00];
# const SYMBOLS: &str = "A哈";
# const METRICS: &[GlyphMetrics] = &[
#     GlyphMetrics::new(0, 3, 1, 1, 0, 0),
#     GlyphMetrics::new(0, 5, 1, 1, 1, 0),
# ];
# const FONT: FontData<'static> = FontData::new(BITMAP, SYMBOLS, METRICS, 10, 10, 2);
const PRESET: FontPreset<'static> = FontPreset::new(FONT, 8, 16, 20);

let scaled = PRESET.scaled_ratio(3, 2);
```

This scales the preset by `1.5x`.

## Compile-Time Macro

Use `lvgl_font!` to read an LVGL-generated `.c` file at compile time and expand it into Rust `FontData`.

```rust
use lvgl_font_bridge::{FontData, FontPreset, lvgl_font};

const FONT: FontData<'static> = lvgl_font!(path = "hello.c");
const PRESET: FontPreset<'static> = FontPreset::new(FONT, 6, 12, 12).with_scaled_height(16);
```

The macro returns `FontData`, which contains:

- parsed `FontData`

Convert it to `FontPreset` with `const` functions:

- `FontPreset::new(font, half_width, full_width, height)`
- `.with_scaled_height(height)`
- `.scaled_ratio(numerator, denominator)`

Create a text style from the preset:

```rust
use embedded_graphics::pixelcolor::BinaryColor;

# use lvgl_font_bridge::{FontData, FontPreset, lvgl_font};
# const FONT: FontData<'static> = lvgl_font!(path = "hello.c");
# const PRESET: FontPreset<'static> = FontPreset::new(FONT, 6, 12, 12).with_scaled_height(16);
let style = PRESET.default_text_style(BinaryColor::On);
let bigger = PRESET.text_style(BinaryColor::On, 18);
```

## Behavior Notes

- Glyph lookup is done by iterating `symbols.chars()`
- Missing glyphs are not drawn
- Missing glyphs still advance:
  - ASCII characters use `ascii_width`
  - all other characters use `non_ascii_width`
- `size == 0` falls back to the font's `native_size`
- `adv_w` is preserved from LVGL data, but current horizontal layout uses the user-specified widths instead
- `with_scaled_height(...)` only changes bitmap interpolation ratio; it does not auto-resize `half_width`, `full_width`, or `height`

## Limitations

- 1bpp bitmap fonts only
- no kerning
- no ligatures
- no fallback font chain
- the compile-time macro currently expects the common LVGL sparse font layout used by files like `hello.c`
