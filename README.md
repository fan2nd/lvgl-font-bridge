# lvgl-font-bridge

`lvgl-font-bridge` is a small `no_std` helper crate for rendering LVGL-generated bitmap fonts with `embedded-graphics`.

It keeps the original 1bpp glyph bitmap data, but replaces LVGL's numeric character mapping with direct UTF-8 symbols stored in a single `&str`.

## Features

- `FontData` stores:
  - raw bitmap bytes
  - UTF-8 symbol string
  - glyph metrics
  - `FontLayout`
  - `FontVerticalMetrics`
- `EgTextStyle` implements:
  - `embedded_graphics::text::renderer::TextRenderer`
  - `embedded_graphics::text::renderer::CharacterStyle`
- Horizontal advance is controlled by the user:
  - one width for ASCII characters
  - one width for non-ASCII characters
- Vertical layout uses the preset logical height derived from `full_width`
- `FontData::new` contains a const assertion that checks:
  - `symbols.chars().count() == metrics.len()`

Optional feature:

- `scaling`
  - enables proportional bitmap scaling on `EgTextStyle`
  - keeps default mode and scaling mode clearly separated

## Data Model

`symbols` is a single UTF-8 string. Each character in that string corresponds to one entry in `metrics` at the same index.

```rust
use lvgl_font_bridge::{FontData, FontLayout, FontVerticalMetrics, GlyphMetrics};

const BITMAP: &[u8] = &[0x00, 0x00];
const SYMBOLS: &str = "1H哈";
const METRICS: &[GlyphMetrics] = &[
    GlyphMetrics::new(0, 167, 7, 15, 2, 0),
    GlyphMetrics::new(14, 223, 10, 15, 2, 0),
    GlyphMetrics::new(33, 320, 17, 18, 2, -1),
];

const FONT: FontData<'static> = FontData::new(
    BITMAP,
    SYMBOLS,
    METRICS,
    FontLayout::new(8, 16),
    FontVerticalMetrics::new(20, 18, 1),
);
```

## Default Mode

In the default build, `FontPreset` only carries base font information and layout defaults. `EgTextStyle` renders without proportional bitmap scaling.

```rust
use embedded_graphics::pixelcolor::BinaryColor;
use lvgl_font_bridge::{EgTextStyle, FontLayout, FontPreset, FontVerticalMetrics};

# use lvgl_font_bridge::{FontData, GlyphMetrics};
# const BITMAP: &[u8] = &[0x00];
# const SYMBOLS: &str = "A哈";
# const METRICS: &[GlyphMetrics] = &[
#     GlyphMetrics::new(0, 3, 1, 1, 0, 0),
#     GlyphMetrics::new(0, 5, 1, 1, 1, 0),
# ];
# const FONT: FontData<'static> = FontData::new(
#     BITMAP,
#     SYMBOLS,
#     METRICS,
#     FontLayout::new(8, 16),
#     FontVerticalMetrics::new(10, 10, 2),
# );
# const PRESET: FontPreset<'static> = FontPreset::new(&FONT);
let style = EgTextStyle::new(&PRESET, BinaryColor::On);
```

Then use it with `embedded-graphics::text::Text`.

## Scaling Feature

Enable the `scaling` feature if you want proportional layout scaling and interpolated bitmap rendering on the style itself.

```toml
lvgl-font-bridge = { version = "...", features = ["scaling"] }
```

With `scaling` enabled, the boundary is:

- `FontPreset`
  - still only stores base font data
- `EgTextStyle`
  - optionally applies a scale ratio for this render style

```rust
use embedded_graphics::pixelcolor::BinaryColor;
use lvgl_font_bridge::{EgTextStyle, FontData, FontLayout, FontPreset, FontVerticalMetrics, GlyphMetrics};

# const BITMAP: &[u8] = &[0x00];
# const SYMBOLS: &str = "A哈";
# const METRICS: &[GlyphMetrics] = &[
#     GlyphMetrics::new(0, 3, 1, 1, 0, 0),
#     GlyphMetrics::new(0, 5, 1, 1, 1, 0),
# ];
# const FONT: FontData<'static> = FontData::new(
#     BITMAP,
#     SYMBOLS,
#     METRICS,
#     FontLayout::new(8, 16),
#     FontVerticalMetrics::new(10, 10, 2),
# );
const PRESET: FontPreset<'static> = FontPreset::new(&FONT);

let fixed = EgTextStyle::new(&PRESET, BinaryColor::On);
let scaled = EgTextStyle::new(&PRESET, BinaryColor::On).with_scale_ratio(3, 2);
```

This scales:

- ASCII width
- non-ASCII width
- logical height derived from `full_width`
- glyph bitmap rendering

For non-integer ratios:

```rust
# use embedded_graphics::pixelcolor::BinaryColor;
# use lvgl_font_bridge::{EgTextStyle, FontData, FontLayout, FontPreset, FontVerticalMetrics, GlyphMetrics};
# const BITMAP: &[u8] = &[0x00];
# const SYMBOLS: &str = "A哈";
# const METRICS: &[GlyphMetrics] = &[
#     GlyphMetrics::new(0, 3, 1, 1, 0, 0),
#     GlyphMetrics::new(0, 5, 1, 1, 1, 0),
# ];
# const FONT: FontData<'static> = FontData::new(
#     BITMAP,
#     SYMBOLS,
#     METRICS,
#     FontLayout::new(8, 16),
#     FontVerticalMetrics::new(10, 10, 2),
# );
# const PRESET: FontPreset<'static> = FontPreset::new(&FONT);
let scaled = EgTextStyle::new(&PRESET, BinaryColor::On).with_scale_ratio(3, 2);
```

This scales by `1.5x`.

## Compile-Time Macro

Use `lvgl_font!` to read an LVGL-generated `.c` file at compile time and expand it into Rust `FontData`.

```rust
use lvgl_font_bridge::{FontData, FontPreset, lvgl_font};

const FONT: FontData<'static> = lvgl_font!(
    path = "hello.c",
    half_width = 6,
    full_width = 12,
);
const PRESET: FontPreset<'static> = FontPreset::new(&FONT);
```

The macro returns `FontData`, which contains:

- parsed `FontData`
- `half_width`
- `full_width`
- native vertical metrics

Create a text style from the preset:

```rust
use embedded_graphics::pixelcolor::BinaryColor;

# use lvgl_font_bridge::{FontData, FontPreset, lvgl_font};
# const FONT: FontData<'static> = lvgl_font!(
#     path = "hello.c",
#     half_width = 6,
#     full_width = 12,
# );
# const PRESET: FontPreset<'static> = FontPreset::new(&FONT);
let style = PRESET.default_text_style(BinaryColor::On);
```

## Behavior Notes

- Glyph lookup is done by iterating `symbols.chars()`
- Missing glyphs are not drawn
- Missing glyphs still advance:
  - ASCII characters use `half_width`
  - all other characters use `full_width`
- `adv_w` is preserved from LVGL data, but current horizontal layout uses the user-specified widths instead
- `EgTextStyle::new(...)` uses `full_width` as its default logical height through the preset
- `with_scale_ratio(...)` is only available with the `scaling` feature

## Limitations

- 1bpp bitmap fonts only
- no kerning
- no ligatures
- no fallback font chain
- the compile-time macro currently expects the common LVGL sparse font layout used by files like `hello.c`
