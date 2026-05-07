#![no_std]

pub use lvgl_font_bridge_macros::lvgl_font;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    pixelcolor::PixelColor,
    prelude::Pixel,
    primitives::Rectangle,
    text::{
        Baseline, DecorationColor,
        renderer::{CharacterStyle, TextMetrics, TextRenderer},
    },
};

/// Glyph metrics translated from LVGL's `lv_font_fmt_txt_glyph_dsc_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphMetrics {
    pub bitmap_index: u32,
    pub adv_w: u32,
    pub box_w: u32,
    pub box_h: u32,
    pub ofs_x: i32,
    pub ofs_y: i32,
}

impl GlyphMetrics {
    /// Creates glyph metrics with LVGL-compatible fields.
    pub const fn new(
        bitmap_index: u32,
        adv_w: u32,
        box_w: u32,
        box_h: u32,
        ofs_x: i32,
        ofs_y: i32,
    ) -> Self {
        Self {
            bitmap_index,
            adv_w,
            box_w,
            box_h,
            ofs_x,
            ofs_y,
        }
    }
}

/// Default horizontal settings for a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontLayout {
    pub half_width: u32,
    pub full_width: u32,
}

impl FontLayout {
    /// Creates default horizontal settings for a font.
    pub const fn new(half_width: u32, full_width: u32) -> Self {
        Self {
            half_width,
            full_width,
        }
    }
}

/// Vertical metrics for a font's native bitmap size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontVerticalMetrics {
    pub native_size: u32,
    pub line_height: u32,
    pub baseline: u32,
}

impl FontVerticalMetrics {
    /// Creates vertical metrics for the font's native bitmap size.
    pub const fn new(native_size: u32, line_height: u32, baseline: u32) -> Self {
        Self {
            native_size,
            line_height,
            baseline,
        }
    }
}

/// Static font data derived from an LVGL-generated font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontData<'a> {
    pub bitmap: &'a [u8],
    pub symbols: &'a str,
    pub metrics: &'a [GlyphMetrics],
    pub layout: FontLayout,
    pub vertical_metrics: FontVerticalMetrics,
}

impl<'a> FontData<'a> {
    /// Creates a new immutable font payload.
    pub const fn new(
        bitmap: &'a [u8],
        symbols: &'a str,
        metrics: &'a [GlyphMetrics],
        layout: FontLayout,
        vertical_metrics: FontVerticalMetrics,
    ) -> Self {
        assert!(utf8_char_count(symbols) == metrics.len());

        Self {
            bitmap,
            symbols,
            metrics,
            layout,
            vertical_metrics,
        }
    }

    /// Returns the default horizontal settings.
    pub const fn layout(&self) -> FontLayout {
        self.layout
    }

    /// Returns the font's native vertical metrics.
    pub const fn vertical_metrics(&self) -> FontVerticalMetrics {
        self.vertical_metrics
    }

    /// Finds a glyph by a single Unicode scalar value.
    pub fn glyph_for_char(&self, character: char) -> Option<&GlyphMetrics> {
        for (index, candidate) in self.symbols.chars().enumerate() {
            if candidate == character {
                return self.metrics.get(index);
            }
        }

        None
    }
}

/// Base preset data shared by all text styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontPreset<'a> {
    pub font: &'a FontData<'a>,
}

impl<'a> FontPreset<'a> {
    /// Creates a preset from parsed font data.
    pub const fn new(font: &'a FontData<'a>) -> Self {
        Self { font }
    }

    /// Returns the underlying font data.
    pub const fn font_data(&self) -> &FontData<'a> {
        self.font
    }

    /// Returns the preset ASCII width.
    pub const fn ascii_width(&self) -> u32 {
        self.font.layout.half_width
    }

    /// Returns the preset non-ASCII width.
    pub const fn non_ascii_width(&self) -> u32 {
        self.font.layout.full_width
    }

    /// Returns the preset default logical height derived from `full_width`.
    pub const fn height(&self) -> u32 {
        self.font.layout.full_width
    }

    /// Builds a text style using the preset default logical height.
    pub const fn default_text_style<C>(&'a self, text_color: C) -> EgTextStyle<'a, C>
    where
        C: PixelColor,
    {
        EgTextStyle::new(self, text_color)
    }
}

/// `embedded-graphics` text style backed by [`FontPreset`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EgTextStyle<'a, C> {
    pub preset: &'a FontPreset<'a>,
    pub text_color: C,
    pub background_color: Option<C>,
    text_transparent: bool,
    #[cfg(feature = "scaling")]
    scale_numerator: u32,
    #[cfg(feature = "scaling")]
    scale_denominator: u32,
}

impl<'a, C> EgTextStyle<'a, C>
where
    C: PixelColor,
{
    /// Creates a text style with a transparent background.
    pub const fn new(preset: &'a FontPreset<'a>, text_color: C) -> Self {
        Self {
            preset,
            text_color,
            background_color: None,
            text_transparent: false,
            #[cfg(feature = "scaling")]
            scale_numerator: 1,
            #[cfg(feature = "scaling")]
            scale_denominator: 1,
        }
    }

    /// Creates a text style with an explicit background color.
    pub const fn with_background(
        preset: &'a FontPreset<'a>,
        text_color: C,
        background_color: C,
    ) -> Self {
        Self {
            preset,
            text_color,
            background_color: Some(background_color),
            text_transparent: false,
            #[cfg(feature = "scaling")]
            scale_numerator: 1,
            #[cfg(feature = "scaling")]
            scale_denominator: 1,
        }
    }

    /// Returns the effective font data used by this style.
    pub const fn font_data(&self) -> &FontData<'a> {
        self.preset.font
    }

    #[cfg(feature = "scaling")]
    /// Sets the proportional scaling ratio for this style.
    pub const fn with_scale_ratio(mut self, numerator: u32, denominator: u32) -> Self {
        assert!(numerator != 0);
        assert!(denominator != 0);
        self.scale_numerator = numerator;
        self.scale_denominator = denominator;
        self
    }

    #[cfg(feature = "scaling")]
    /// Returns the proportional scaling ratio used by this style.
    pub const fn scale_ratio(&self) -> (u32, u32) {
        (self.scale_numerator, self.scale_denominator)
    }

    #[cfg(feature = "scaling")]
    /// Returns whether proportional scaling is active for this style.
    pub const fn is_scaled(&self) -> bool {
        self.scale_numerator != 1 || self.scale_denominator != 1
    }

    #[cfg(not(feature = "scaling"))]
    fn character_width(&self, character: char) -> u32 {
        if character.is_ascii() {
            self.preset.ascii_width()
        } else {
            self.preset.non_ascii_width()
        }
    }

    #[cfg(feature = "scaling")]
    fn character_width(&self, character: char) -> u32 {
        let base = if character.is_ascii() {
            self.preset.ascii_width()
        } else {
            self.preset.non_ascii_width()
        };

        scale_ratio_nonzero(base, self.scale_numerator, self.scale_denominator)
    }

    #[cfg(not(feature = "scaling"))]
    fn logical_height(&self) -> u32 {
        self.preset.height()
    }

    #[cfg(feature = "scaling")]
    fn logical_height(&self) -> u32 {
        scale_ratio_nonzero(
            self.preset.height(),
            self.scale_numerator,
            self.scale_denominator,
        )
    }

    #[cfg(not(feature = "scaling"))]
    fn rendered_glyph_box_w(&self, glyph: &GlyphMetrics) -> u32 {
        glyph.box_w
    }

    #[cfg(feature = "scaling")]
    fn rendered_glyph_box_w(&self, glyph: &GlyphMetrics) -> u32 {
        if self.is_scaled() {
            scale_ratio_nonzero(glyph.box_w, self.scale_numerator, self.scale_denominator)
        } else {
            glyph.box_w
        }
    }

    #[cfg(not(feature = "scaling"))]
    fn rendered_glyph_box_h(&self, glyph: &GlyphMetrics) -> u32 {
        glyph.box_h
    }

    #[cfg(feature = "scaling")]
    fn rendered_glyph_box_h(&self, glyph: &GlyphMetrics) -> u32 {
        if self.is_scaled() {
            scale_ratio_nonzero(glyph.box_h, self.scale_numerator, self.scale_denominator)
        } else {
            glyph.box_h
        }
    }

    #[cfg(not(feature = "scaling"))]
    fn rendered_glyph_ofs_y(&self, glyph: &GlyphMetrics) -> i32 {
        glyph.ofs_y
    }

    #[cfg(feature = "scaling")]
    fn rendered_glyph_ofs_y(&self, glyph: &GlyphMetrics) -> i32 {
        if self.is_scaled() {
            scale_i32_ratio(glyph.ofs_y, self.scale_numerator, self.scale_denominator)
        } else {
            glyph.ofs_y
        }
    }

    fn scaled_vertical_metric(&self, value: u32) -> u32 {
        let native = non_zero_or_one(self.font_data().vertical_metrics.native_size);
        let scaled = scale_u32(value, self.logical_height(), native);

        if value > 0 { scaled.max(1) } else { 0 }
    }

    fn scaled_line_height(&self) -> u32 {
        self.scaled_vertical_metric(self.font_data().vertical_metrics.line_height)
    }

    fn scaled_baseline_from_bottom(&self) -> u32 {
        scale_u32(
            self.font_data().vertical_metrics.baseline,
            self.logical_height(),
            non_zero_or_one(self.font_data().vertical_metrics.native_size),
        )
    }

    fn scaled_alphabetic_baseline_offset(&self) -> u32 {
        self.scaled_line_height()
            .saturating_sub(self.scaled_baseline_from_bottom())
    }

    fn baseline_offset(&self, baseline: Baseline) -> i32 {
        let line_height = self.scaled_line_height();

        u32_to_i32_sat(match baseline {
            Baseline::Top => 0,
            Baseline::Bottom => line_height.saturating_sub(1),
            Baseline::Middle => line_height.saturating_sub(1) / 2,
            Baseline::Alphabetic => self.scaled_alphabetic_baseline_offset(),
        })
    }

    fn glyph_x_offset(&self, character: char, glyph: &GlyphMetrics) -> i32 {
        let cell_width = self.character_width(character) as i32;
        let glyph_width = self.rendered_glyph_box_w(glyph) as i32;

        (cell_width - glyph_width) / 2
    }

    fn draw_background<D>(&self, width: u32, origin: Point, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        if width == 0 {
            return Ok(());
        }

        if let Some(color) = self.background_color {
            target.fill_solid(
                &Rectangle::new(origin, Size::new(width, self.scaled_line_height())),
                color,
            )?;
        }

        Ok(())
    }

    fn glyph_origin(&self, character: char, cell_origin: Point, glyph: &GlyphMetrics) -> Point {
        let y = self
            .scaled_alphabetic_baseline_offset()
            .saturating_sub(self.rendered_glyph_box_h(glyph));

        cell_origin
            + Point::new(
                self.glyph_x_offset(character, glyph),
                u32_to_i32_sat(y).saturating_sub(self.rendered_glyph_ofs_y(glyph)),
            )
    }

    #[cfg(not(feature = "scaling"))]
    fn glyph_bit(&self, glyph: &GlyphMetrics, x: u32, y: u32) -> bool {
        glyph_bitmap_bit(self.font_data(), glyph, x, y)
    }

    #[cfg(feature = "scaling")]
    fn glyph_bit(&self, glyph: &GlyphMetrics, x: u32, y: u32) -> bool {
        if self.is_scaled() {
            interpolated_glyph_bit_for_ratio(
                self.font_data(),
                glyph,
                self.scale_numerator,
                self.scale_denominator,
                x,
                y,
            )
        } else {
            glyph_bitmap_bit(self.font_data(), glyph, x, y)
        }
    }

    fn draw_glyph<D>(
        &self,
        character: char,
        glyph: &GlyphMetrics,
        cell_origin: Point,
        target: &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        if self.text_transparent {
            return Ok(());
        }

        let color = self.text_color;
        let origin = self.glyph_origin(character, cell_origin, glyph);
        let glyph_w = self.rendered_glyph_box_w(glyph);
        let glyph_h = self.rendered_glyph_box_h(glyph);

        for y in 0..glyph_h {
            for x in 0..glyph_w {
                if self.glyph_bit(glyph, x, y) {
                    target.draw_iter(core::iter::once(Pixel(
                        origin + Point::new(u32_to_i32_sat(x), u32_to_i32_sat(y)),
                        color,
                    )))?;
                }
            }
        }

        Ok(())
    }
}

impl<C> TextRenderer for EgTextStyle<'_, C>
where
    C: PixelColor,
{
    type Color = C;

    fn draw_string<D>(
        &self,
        text: &str,
        position: Point,
        baseline: Baseline,
        target: &mut D,
    ) -> Result<Point, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let top_left = position - Point::new(0, self.baseline_offset(baseline));
        let mut cursor_x = 0_u32;

        for character in text.chars() {
            let width = self.character_width(character);
            let cell_origin = top_left + Point::new(u32_to_i32_sat(cursor_x), 0);

            self.draw_background(width, cell_origin, target)?;

            if let Some(glyph) = self.font_data().glyph_for_char(character) {
                self.draw_glyph(character, glyph, cell_origin, target)?;
            }

            cursor_x = cursor_x.saturating_add(width);
        }

        Ok(position + Point::new(u32_to_i32_sat(cursor_x), 0))
    }

    fn draw_whitespace<D>(
        &self,
        width: u32,
        position: Point,
        baseline: Baseline,
        target: &mut D,
    ) -> Result<Point, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let top_left = position - Point::new(0, self.baseline_offset(baseline));
        self.draw_background(width, top_left, target)?;

        Ok(position + Point::new(u32_to_i32_sat(width), 0))
    }

    fn measure_string(&self, text: &str, position: Point, baseline: Baseline) -> TextMetrics {
        let width = text.chars().fold(0_u32, |acc, ch| {
            acc.saturating_add(self.character_width(ch))
        });

        let line_height = self.scaled_line_height();
        let top_left = position - Point::new(0, self.baseline_offset(baseline));

        TextMetrics {
            bounding_box: Rectangle::new(top_left, Size::new(width, line_height)),
            next_position: position + Point::new(u32_to_i32_sat(width), 0),
        }
    }

    fn line_height(&self) -> u32 {
        self.scaled_line_height()
    }
}

impl<C> CharacterStyle for EgTextStyle<'_, C>
where
    C: PixelColor,
{
    type Color = C;

    fn set_text_color(&mut self, text_color: Option<Self::Color>) {
        if let Some(text_color) = text_color {
            self.text_color = text_color;
            self.text_transparent = false;
        } else {
            self.text_transparent = true;
        }
    }

    fn set_background_color(&mut self, background_color: Option<Self::Color>) {
        self.background_color = background_color;
    }

    fn set_underline_color(&mut self, _underline_color: DecorationColor<Self::Color>) {}

    fn set_strikethrough_color(&mut self, _strikethrough_color: DecorationColor<Self::Color>) {}
}

const fn scale_u32(value: u32, size: u32, native_size: u32) -> u32 {
    let numerator = value as u64 * size as u64 + (native_size as u64 / 2);
    (numerator / native_size as u64) as u32
}

#[cfg(feature = "scaling")]
const fn scale_ratio_nonzero(value: u32, numerator: u32, denominator: u32) -> u32 {
    if value == 0 {
        0
    } else {
        let scaled = scale_u32(value, numerator, denominator);

        if scaled == 0 { 1 } else { scaled }
    }
}

const fn non_zero_or_one(value: u32) -> u32 {
    if value == 0 { 1 } else { value }
}

#[cfg(feature = "scaling")]
const fn scale_i32_ratio(value: i32, numerator: u32, denominator: u32) -> i32 {
    if value == 0 {
        0
    } else {
        let sign = if value < 0 { -1 } else { 1 };
        let abs = value.abs() as i64;
        let scaled = (abs * numerator as i64 + (denominator as i64 / 2)) / denominator as i64;
        let signed = scaled * sign;

        if signed > i32::MAX as i64 {
            i32::MAX
        } else if signed < i32::MIN as i64 {
            i32::MIN
        } else {
            signed as i32
        }
    }
}

fn glyph_bitmap_bit(font: &FontData<'_>, glyph: &GlyphMetrics, x: u32, y: u32) -> bool {
    if x >= glyph.box_w || y >= glyph.box_h {
        return false;
    }

    let bit_index = y as usize * glyph.box_w as usize + x as usize;
    let byte_index = glyph.bitmap_index as usize + bit_index / 8;

    let Some(byte) = font.bitmap.get(byte_index) else {
        return false;
    };

    let shift = 7 - (bit_index % 8);

    (byte >> shift) & 1 != 0
}

#[cfg(feature = "scaling")]
fn interpolated_glyph_bit_for_ratio(
    font: &FontData<'_>,
    glyph: &GlyphMetrics,
    numerator: u32,
    denominator: u32,
    x: u32,
    y: u32,
) -> bool {
    let dst_w = scale_ratio_nonzero(glyph.box_w, numerator, denominator);
    let dst_h = scale_ratio_nonzero(glyph.box_h, numerator, denominator);

    if dst_w == 0 || dst_h == 0 || x >= dst_w || y >= dst_h {
        return false;
    }

    let (src_x, frac_x) = scaled_source_coordinate(x, glyph.box_w, dst_w);
    let (src_y, frac_y) = scaled_source_coordinate(y, glyph.box_h, dst_h);
    let src_x1 = (src_x + 1).min(glyph.box_w.saturating_sub(1));
    let src_y1 = (src_y + 1).min(glyph.box_h.saturating_sub(1));

    let v00 = if glyph_bitmap_bit(font, glyph, src_x, src_y) {
        255_u32
    } else {
        0
    };
    let v10 = if glyph_bitmap_bit(font, glyph, src_x1, src_y) {
        255_u32
    } else {
        0
    };
    let v01 = if glyph_bitmap_bit(font, glyph, src_x, src_y1) {
        255_u32
    } else {
        0
    };
    let v11 = if glyph_bitmap_bit(font, glyph, src_x1, src_y1) {
        255_u32
    } else {
        0
    };

    let top = v00 * (256 - frac_x) + v10 * frac_x;
    let bottom = v01 * (256 - frac_x) + v11 * frac_x;
    let interpolated = (top * (256 - frac_y) + bottom * frac_y + 32_768) >> 16;

    interpolated >= 128
}

#[cfg(feature = "scaling")]
fn scaled_source_coordinate(dst: u32, src_len: u32, dst_len: u32) -> (u32, u32) {
    if src_len <= 1 || dst_len == 0 {
        return (0, 0);
    }

    let max_fp = (src_len.saturating_sub(1) as i64) * 256;
    let pos_fp = ((((dst as i64) * 2 + 1) * src_len as i64 * 256) / ((dst_len as i64) * 2)) - 128;
    let clamped = pos_fp.clamp(0, max_fp);
    let base = (clamped / 256) as u32;
    let frac = (clamped % 256) as u32;

    (base, frac)
}

const fn utf8_char_count(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut count = 0;

    while index < bytes.len() {
        if (bytes[index] & 0b1100_0000) != 0b1000_0000 {
            count += 1;
        }

        index += 1;
    }

    count
}

fn u32_to_i32_sat(value: u32) -> i32 {
    value.min(i32::MAX as u32) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::{
        mock_display::MockDisplay,
        pixelcolor::BinaryColor,
        prelude::*,
        text::{Baseline, Text},
    };

    const HELLO_BITMAP: &[u8] = &[
        0x30, 0xa0, 0x40, 0x81, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x81, 0x02, 0x3f, 0x80, 0x80,
        0x60, 0x18, 0x06, 0x01, 0x80, 0x60, 0x18, 0x07, 0xff, 0x80, 0x60, 0x18, 0x06, 0x01, 0x80,
        0x60, 0x18, 0x04, 0x00, 0x20, 0x00, 0x18, 0x3e, 0x0a, 0x11, 0x09, 0x08, 0x88, 0x44, 0x44,
        0x12, 0x24, 0x05, 0x15, 0xf9, 0x88, 0x00, 0x44, 0x00, 0x22, 0x7f, 0x91, 0x20, 0x4f, 0x90,
        0x24, 0x08, 0x12, 0x04, 0x08, 0x02, 0x04, 0x01, 0xfe, 0x00, 0x81, 0x00,
    ];

    const HELLO_SYMBOLS: &str = "1H哈";
    const HELLO_METRICS: &[GlyphMetrics] = &[
        GlyphMetrics::new(0, 167, 7, 15, 2, 0),
        GlyphMetrics::new(14, 223, 10, 15, 2, 0),
        GlyphMetrics::new(33, 320, 17, 18, 2, -1),
    ];

    const HELLO_FONT: FontData<'static> = FontData::new(
        HELLO_BITMAP,
        HELLO_SYMBOLS,
        HELLO_METRICS,
        FontLayout::new(8, 16),
        FontVerticalMetrics::new(20, 18, 1),
    );
    const HELLO_PRESET: FontPreset<'static> = FontPreset::new(&HELLO_FONT);
    const HELLO_STYLE: EgTextStyle<'static, BinaryColor> =
        EgTextStyle::new(&HELLO_PRESET, BinaryColor::On);

    const SIMPLE_BITMAP: &[u8] = &[0b1100_0000];
    const SIMPLE_SYMBOLS: &str = "A哈";
    const SIMPLE_METRICS: &[GlyphMetrics] = &[
        GlyphMetrics::new(0, 3, 1, 1, 0, 0),
        GlyphMetrics::new(0, 5, 1, 1, 1, 0),
    ];
    const SIMPLE_FONT: FontData<'static> = FontData::new(
        SIMPLE_BITMAP,
        SIMPLE_SYMBOLS,
        SIMPLE_METRICS,
        FontLayout::new(3, 5),
        FontVerticalMetrics::new(10, 10, 2),
    );
    const SIMPLE_PRESET: FontPreset<'static> = FontPreset::new(&SIMPLE_FONT);

    #[cfg(feature = "scaling")]
    const UNIT_BITMAP: &[u8] = &[0b1000_0000];
    #[cfg(feature = "scaling")]
    const UNIT_SYMBOLS: &str = "A";
    #[cfg(feature = "scaling")]
    const UNIT_METRICS: &[GlyphMetrics] = &[GlyphMetrics::new(0, 1, 1, 1, 0, 0)];
    #[cfg(feature = "scaling")]
    const UNIT_FONT: FontData<'static> = FontData::new(
        UNIT_BITMAP,
        UNIT_SYMBOLS,
        UNIT_METRICS,
        FontLayout::new(1, 1),
        FontVerticalMetrics::new(1, 1, 0),
    );
    #[cfg(feature = "scaling")]
    const UNIT_PRESET: FontPreset<'static> = FontPreset::new(&UNIT_FONT);

    const BASELINE_FONT: FontData<'static> = FontData::new(
        &[],
        "",
        &[],
        FontLayout::new(4, 20),
        FontVerticalMetrics::new(10, 10, 3),
    );
    const BASELINE_PRESET: FontPreset<'static> = FontPreset::new(&BASELINE_FONT);

    #[test]
    fn const_construction_works() {
        const STYLE: EgTextStyle<'static, BinaryColor> =
            EgTextStyle::new(&HELLO_PRESET, BinaryColor::On);

        assert_eq!(STYLE.preset.font.vertical_metrics.native_size, 20);
        assert_eq!(STYLE.preset.ascii_width(), 8);
        assert_eq!(STYLE.preset.non_ascii_width(), 16);
        assert_eq!(STYLE.preset.height(), 16);
    }

    #[test]
    #[should_panic]
    fn font_data_requires_matching_lengths() {
        let _ = FontData::new(
            &[],
            "AB",
            &HELLO_METRICS[..1],
            FontLayout::new(8, 16),
            FontVerticalMetrics::new(20, 18, 1),
        );
    }

    #[test]
    fn font_preset_uses_default_dimensions() {
        let style = HELLO_PRESET.default_text_style(BinaryColor::On);

        assert_eq!(style.preset.ascii_width(), 8);
        assert_eq!(style.preset.non_ascii_width(), 16);
        assert_eq!(style.preset.height(), 16);
        assert_eq!(style.preset.font.symbols, HELLO_FONT.symbols);
    }

    #[test]
    fn glyph_lookup_supports_utf8() {
        assert_eq!(
            HELLO_FONT.glyph_for_char('1').map(|g| g.bitmap_index),
            Some(0)
        );
        assert_eq!(
            HELLO_FONT.glyph_for_char('H').map(|g| g.bitmap_index),
            Some(14)
        );
        assert_eq!(
            HELLO_FONT.glyph_for_char('哈').map(|g| g.bitmap_index),
            Some(33)
        );
        assert_eq!(HELLO_FONT.glyph_for_char('Z'), None);
    }

    #[test]
    fn measure_string_uses_ascii_and_non_ascii_widths() {
        let metrics = HELLO_STYLE.measure_string("1哈字", Point::new(4, 5), Baseline::Top);

        assert_eq!(metrics.bounding_box.top_left, Point::new(4, 5));
        assert_eq!(metrics.bounding_box.size, Size::new(40, 14));
        assert_eq!(metrics.next_position, Point::new(44, 5));
    }

    #[test]
    fn missing_glyphs_still_advance_by_category_width() {
        let metrics = HELLO_STYLE.measure_string("Z字", Point::zero(), Baseline::Top);

        assert_eq!(metrics.bounding_box.size.width, 24);
        assert_eq!(metrics.next_position, Point::new(24, 0));
    }

    #[test]
    fn preset_default_height_comes_from_full_width() {
        const WIDE_FONT: FontData<'static> = FontData::new(
            &[],
            "",
            &[],
            FontLayout::new(6, 12),
            FontVerticalMetrics::new(10, 10, 3),
        );
        const WIDE_PRESET: FontPreset<'static> = FontPreset::new(&WIDE_FONT);
        let style = EgTextStyle::new(&WIDE_PRESET, BinaryColor::On);
        let metrics = style.measure_string("A哈", Point::new(3, 20), Baseline::Alphabetic);

        assert_eq!(style.preset.height(), 12);
        assert_eq!(metrics.bounding_box.top_left, Point::new(3, 12));
        assert_eq!(metrics.bounding_box.size, Size::new(18, 12));
        assert_eq!(metrics.next_position, Point::new(21, 20));
    }

    #[test]
    fn baseline_offsets_are_respected() {
        let style = EgTextStyle::new(&BASELINE_PRESET, BinaryColor::On);
        let top = style.measure_string("A", Point::new(10, 30), Baseline::Top);
        let bottom = style.measure_string("A", Point::new(10, 30), Baseline::Bottom);
        let middle = style.measure_string("A", Point::new(10, 30), Baseline::Middle);
        let alphabetic = style.measure_string("A", Point::new(10, 30), Baseline::Alphabetic);

        assert_eq!(top.bounding_box.top_left, Point::new(10, 30));
        assert_eq!(bottom.bounding_box.top_left, Point::new(10, 11));
        assert_eq!(middle.bounding_box.top_left, Point::new(10, 21));
        assert_eq!(alphabetic.bounding_box.top_left, Point::new(10, 16));
    }

    #[test]
    fn draw_string_renders_ascii_and_non_ascii_glyphs() {
        let style = EgTextStyle::new(&SIMPLE_PRESET, BinaryColor::On);
        let mut display = MockDisplay::new();
        display.set_allow_out_of_bounds_drawing(true);

        let next = style
            .draw_string("A哈", Point::zero(), Baseline::Top, &mut display)
            .unwrap();

        assert_eq!(next, Point::new(8, 0));
        assert_eq!(display.get_pixel(Point::new(1, 3)), Some(BinaryColor::On));
        assert_eq!(display.get_pixel(Point::new(5, 3)), Some(BinaryColor::On));
    }

    #[test]
    fn draw_string_with_text_object_uses_renderer() {
        let style = EgTextStyle::new(&SIMPLE_PRESET, BinaryColor::On);
        let mut display = MockDisplay::new();
        display.set_allow_out_of_bounds_drawing(true);

        Text::with_baseline("A哈", Point::new(1, 2), style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        assert_eq!(display.get_pixel(Point::new(2, 5)), Some(BinaryColor::On));
        assert_eq!(display.get_pixel(Point::new(6, 5)), Some(BinaryColor::On));
    }

    #[test]
    fn draw_whitespace_fills_background() {
        let style = EgTextStyle::with_background(&SIMPLE_PRESET, BinaryColor::On, BinaryColor::Off);
        let mut display = MockDisplay::new();
        display.set_allow_out_of_bounds_drawing(true);

        let next = style
            .draw_whitespace(4, Point::new(2, 3), Baseline::Top, &mut display)
            .unwrap();

        assert_eq!(next, Point::new(6, 3));
        assert_eq!(
            display.affected_area(),
            Rectangle::new(Point::new(2, 3), Size::new(4, 5))
        );
    }

    #[cfg(feature = "scaling")]
    #[test]
    fn style_scale_ratio_scales_layout_proportionally() {
        let style = EgTextStyle::new(&HELLO_PRESET, BinaryColor::On).with_scale_ratio(1, 2);
        let metrics = style.measure_string("1哈", Point::new(4, 5), Baseline::Top);

        assert_eq!(style.scale_ratio(), (1, 2));
        assert!(style.is_scaled());
        assert_eq!(metrics.bounding_box.size, Size::new(12, 7));
        assert_eq!(metrics.next_position, Point::new(16, 5));
    }

    #[cfg(feature = "scaling")]
    #[test]
    fn style_scale_ratio_supports_fractional_ratio() {
        let style = EgTextStyle::new(&HELLO_PRESET, BinaryColor::On).with_scale_ratio(3, 2);
        let metrics = style.measure_string("1哈", Point::new(4, 5), Baseline::Top);

        assert_eq!(style.scale_ratio(), (3, 2));
        assert_eq!(metrics.bounding_box.size, Size::new(36, 22));
        assert_eq!(metrics.next_position, Point::new(40, 5));
    }

    #[cfg(feature = "scaling")]
    #[test]
    #[should_panic]
    fn style_scale_ratio_zero_denominator_panics() {
        let _ = EgTextStyle::new(&HELLO_PRESET, BinaryColor::On).with_scale_ratio(3, 0);
    }

    #[cfg(feature = "scaling")]
    #[test]
    fn style_scale_ratio_interpolates_glyph_bitmap() {
        let style = EgTextStyle::new(&UNIT_PRESET, BinaryColor::On).with_scale_ratio(2, 1);
        let mut display = MockDisplay::new();
        display.set_allow_out_of_bounds_drawing(true);

        let next = style
            .draw_string("A", Point::zero(), Baseline::Top, &mut display)
            .unwrap();

        assert_eq!(next, Point::new(2, 0));
        assert_eq!(display.get_pixel(Point::new(0, 0)), Some(BinaryColor::On));
        assert_eq!(display.get_pixel(Point::new(1, 0)), Some(BinaryColor::On));
        assert_eq!(display.get_pixel(Point::new(0, 1)), Some(BinaryColor::On));
        assert_eq!(display.get_pixel(Point::new(1, 1)), Some(BinaryColor::On));
    }
}
