#![no_std]

#[cfg(feature = "macros")]
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

/// Static font data derived from an LVGL-generated font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontData<'a> {
    pub bitmap: &'a [u8],
    pub symbols: &'a str,
    pub metrics: &'a [GlyphMetrics],
    pub native_size: u32,
    pub line_height: u32,
    pub baseline: u32,
}

impl<'a> FontData<'a> {
    /// Creates a new immutable font payload.
    pub const fn new(
        bitmap: &'a [u8],
        symbols: &'a str,
        metrics: &'a [GlyphMetrics],
        native_size: u32,
        line_height: u32,
        baseline: u32,
    ) -> Self {
        assert!(utf8_char_count(symbols) == metrics.len());

        Self {
            bitmap,
            symbols,
            metrics,
            native_size,
            line_height,
            baseline,
        }
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

/// A font plus caller-provided width and default height settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontPreset<'a> {
    pub font: FontData<'a>,
    pub half_width: u32,
    pub full_width: u32,
    pub height: u32,
}

impl<'a> FontPreset<'a> {
    /// Creates a preset from parsed font data and width defaults.
    pub const fn new(font: FontData<'a>, half_width: u32, full_width: u32, height: u32) -> Self {
        Self {
            font,
            half_width,
            full_width,
            height,
        }
    }

    /// Returns the underlying font data.
    pub const fn font_data(&self) -> &FontData<'a> {
        &self.font
    }

    /// Builds a text style using the preset widths and the provided logical size.
    pub const fn text_style<C>(&'a self, text_color: C, size: u32) -> EgTextStyle<'a, C>
    where
        C: PixelColor,
    {
        EgTextStyle::new(
            &self.font,
            text_color,
            if size == 0 { self.height } else { size },
            self.half_width,
            self.full_width,
        )
    }

    /// Builds a text style using the preset height as the default size.
    pub const fn default_text_style<C>(&'a self, text_color: C) -> EgTextStyle<'a, C>
    where
        C: PixelColor,
    {
        self.text_style(text_color, self.height)
    }
}

/// `embedded-graphics` text style backed by [`FontData`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EgTextStyle<'a, C> {
    pub font: &'a FontData<'a>,
    pub text_color: Option<C>,
    pub background_color: Option<C>,
    pub size: u32,
    pub ascii_width: u32,
    pub non_ascii_width: u32,
}

impl<'a, C> EgTextStyle<'a, C>
where
    C: PixelColor,
{
    /// Creates a text style with a transparent background.
    pub const fn new(
        font: &'a FontData<'a>,
        text_color: C,
        size: u32,
        ascii_width: u32,
        non_ascii_width: u32,
    ) -> Self {
        Self {
            font,
            text_color: Some(text_color),
            background_color: None,
            size,
            ascii_width,
            non_ascii_width,
        }
    }

    /// Creates a text style with an explicit background color.
    pub const fn with_background(
        font: &'a FontData<'a>,
        text_color: C,
        background_color: C,
        size: u32,
        ascii_width: u32,
        non_ascii_width: u32,
    ) -> Self {
        Self {
            font,
            text_color: Some(text_color),
            background_color: Some(background_color),
            size,
            ascii_width,
            non_ascii_width,
        }
    }

    fn effective_size(&self) -> u32 {
        if self.size == 0 {
            self.font.native_size.max(1)
        } else {
            self.size
        }
    }

    fn scaled_vertical_metric(&self, value: u32) -> u32 {
        let native = self.font.native_size.max(1);
        let scaled = scale_u32(value, self.effective_size(), native);

        if value > 0 { scaled.max(1) } else { 0 }
    }

    fn scaled_line_height(&self) -> u32 {
        self.scaled_vertical_metric(self.font.line_height)
    }

    fn scaled_baseline_from_bottom(&self) -> u32 {
        scale_u32(
            self.font.baseline,
            self.effective_size(),
            self.font.native_size.max(1),
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

    fn character_width(&self, character: char) -> u32 {
        if character.is_ascii() {
            self.ascii_width
        } else {
            self.non_ascii_width
        }
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

    fn glyph_origin(&self, cell_origin: Point, glyph: &GlyphMetrics) -> Point {
        let y = self
            .scaled_alphabetic_baseline_offset()
            .saturating_sub(glyph.box_h);

        cell_origin + Point::new(glyph.ofs_x, u32_to_i32_sat(y).saturating_sub(glyph.ofs_y))
    }

    fn glyph_bit(&self, glyph: &GlyphMetrics, x: u32, y: u32) -> bool {
        if x >= glyph.box_w || y >= glyph.box_h {
            return false;
        }

        let bit_index = y as usize * glyph.box_w as usize + x as usize;
        let byte_index = glyph.bitmap_index as usize + bit_index / 8;

        let Some(byte) = self.font.bitmap.get(byte_index) else {
            return false;
        };

        let shift = 7 - (bit_index % 8);

        (byte >> shift) & 1 != 0
    }

    fn draw_glyph<D>(
        &self,
        glyph: &GlyphMetrics,
        cell_origin: Point,
        target: &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        let Some(color) = self.text_color else {
            return Ok(());
        };

        let origin = self.glyph_origin(cell_origin, glyph);

        for y in 0..glyph.box_h {
            for x in 0..glyph.box_w {
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

            if let Some(glyph) = self.font.glyph_for_char(character) {
                self.draw_glyph(glyph, cell_origin, target)?;
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
        self.text_color = text_color;
    }

    fn set_background_color(&mut self, background_color: Option<Self::Color>) {
        self.background_color = background_color;
    }

    fn set_underline_color(&mut self, _underline_color: DecorationColor<Self::Color>) {}

    fn set_strikethrough_color(&mut self, _strikethrough_color: DecorationColor<Self::Color>) {}
}

fn scale_u32(value: u32, size: u32, native_size: u32) -> u32 {
    let numerator = value as u64 * size as u64 + (native_size as u64 / 2);
    (numerator / native_size as u64) as u32
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

    const HELLO_FONT: FontData<'static> =
        FontData::new(HELLO_BITMAP, HELLO_SYMBOLS, HELLO_METRICS, 20, 18, 1);
    const HELLO_STYLE: EgTextStyle<'static, BinaryColor> =
        EgTextStyle::new(&HELLO_FONT, BinaryColor::On, 20, 8, 16);

    const SIMPLE_BITMAP: &[u8] = &[0b1100_0000];
    const SIMPLE_SYMBOLS: &str = "A哈";
    const SIMPLE_METRICS: &[GlyphMetrics] = &[
        GlyphMetrics::new(0, 3, 1, 1, 0, 0),
        GlyphMetrics::new(0, 5, 1, 1, 1, 0),
    ];
    const SIMPLE_FONT: FontData<'static> =
        FontData::new(SIMPLE_BITMAP, SIMPLE_SYMBOLS, SIMPLE_METRICS, 10, 10, 2);

    const BASELINE_FONT: FontData<'static> = FontData::new(&[], "", &[], 10, 10, 3);

    #[test]
    fn const_construction_works() {
        const STYLE: EgTextStyle<'static, BinaryColor> =
            EgTextStyle::new(&HELLO_FONT, BinaryColor::On, 20, 8, 16);

        assert_eq!(STYLE.font.native_size, 20);
        assert_eq!(STYLE.ascii_width, 8);
        assert_eq!(STYLE.non_ascii_width, 16);
    }

    #[test]
    #[should_panic]
    fn font_data_requires_matching_lengths() {
        let _ = FontData::new(&[], "AB", &HELLO_METRICS[..1], 20, 18, 1);
    }

    #[test]
    fn font_preset_uses_default_dimensions() {
        const PRESET: FontPreset<'static> = FontPreset::new(HELLO_FONT, 8, 16, 20);
        let style = PRESET.default_text_style(BinaryColor::On);

        assert_eq!(style.size, 20);
        assert_eq!(style.ascii_width, 8);
        assert_eq!(style.non_ascii_width, 16);
        assert_eq!(style.font.symbols, HELLO_FONT.symbols);
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
        let metrics = HELLO_STYLE.measure_string("1哈H", Point::new(4, 5), Baseline::Top);

        assert_eq!(metrics.bounding_box.top_left, Point::new(4, 5));
        assert_eq!(metrics.bounding_box.size, Size::new(32, 18));
        assert_eq!(metrics.next_position, Point::new(36, 5));
    }

    #[test]
    fn missing_glyphs_still_advance_by_category_width() {
        let metrics = HELLO_STYLE.measure_string("Z界", Point::zero(), Baseline::Top);

        assert_eq!(metrics.bounding_box.size.width, 24);
        assert_eq!(metrics.next_position, Point::new(24, 0));
    }

    #[test]
    fn size_changes_vertical_metrics_only() {
        let small = EgTextStyle::new(&BASELINE_FONT, BinaryColor::On, 10, 6, 12);
        let large = EgTextStyle::new(&BASELINE_FONT, BinaryColor::On, 30, 6, 12);

        let small_metrics = small.measure_string("A哈", Point::new(3, 20), Baseline::Alphabetic);
        let large_metrics = large.measure_string("A哈", Point::new(3, 20), Baseline::Alphabetic);

        assert_eq!(small_metrics.bounding_box.size.width, 18);
        assert_eq!(large_metrics.bounding_box.size.width, 18);
        assert_eq!(small_metrics.next_position.x, large_metrics.next_position.x);
        assert_eq!(small_metrics.bounding_box.size.height, 10);
        assert_eq!(large_metrics.bounding_box.size.height, 30);
    }

    #[test]
    fn baseline_offsets_are_respected() {
        let style = EgTextStyle::new(&BASELINE_FONT, BinaryColor::On, 20, 4, 8);
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
        let style = EgTextStyle::new(&SIMPLE_FONT, BinaryColor::On, 10, 3, 5);
        let mut display = MockDisplay::new();
        display.set_allow_out_of_bounds_drawing(true);

        let next = style
            .draw_string("A哈", Point::zero(), Baseline::Top, &mut display)
            .unwrap();

        assert_eq!(next, Point::new(8, 0));
        assert_eq!(display.get_pixel(Point::new(0, 7)), Some(BinaryColor::On));
        assert_eq!(display.get_pixel(Point::new(4, 7)), Some(BinaryColor::On));
    }

    #[test]
    fn draw_string_with_text_object_uses_renderer() {
        let style = EgTextStyle::new(&SIMPLE_FONT, BinaryColor::On, 10, 3, 5);
        let mut display = MockDisplay::new();
        display.set_allow_out_of_bounds_drawing(true);

        Text::with_baseline("A哈", Point::new(1, 2), style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        assert_eq!(display.get_pixel(Point::new(1, 9)), Some(BinaryColor::On));
        assert_eq!(display.get_pixel(Point::new(5, 9)), Some(BinaryColor::On));
    }

    #[test]
    fn draw_whitespace_fills_background() {
        let style =
            EgTextStyle::with_background(&SIMPLE_FONT, BinaryColor::On, BinaryColor::Off, 10, 3, 5);
        let mut display = MockDisplay::new();
        display.set_allow_out_of_bounds_drawing(true);

        let next = style
            .draw_whitespace(4, Point::new(2, 3), Baseline::Top, &mut display)
            .unwrap();

        assert_eq!(next, Point::new(6, 3));
        assert_eq!(
            display.affected_area(),
            Rectangle::new(Point::new(2, 3), Size::new(4, 10))
        );
    }
}
