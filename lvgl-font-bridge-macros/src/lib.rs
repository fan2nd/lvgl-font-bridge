use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use regex::Regex;
use std::{fs, path::PathBuf};
use syn::{
    Ident, LitInt, LitStr, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

struct FontMacroInput {
    path: LitStr,
    half_width: LitInt,
    full_width: LitInt,
    height: LitInt,
}

impl Parse for FontMacroInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut path = None;
        let mut half_width = None;
        let mut full_width = None;
        let mut height = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "path" => path = Some(input.parse()?),
                "half_width" => half_width = Some(input.parse()?),
                "full_width" => full_width = Some(input.parse()?),
                "height" => height = Some(input.parse()?),
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "expected one of: path, half_width, full_width, height",
                    ));
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            path: path.ok_or_else(|| syn::Error::new(Span::call_site(), "missing `path`"))?,
            half_width: half_width
                .ok_or_else(|| syn::Error::new(Span::call_site(), "missing `half_width`"))?,
            full_width: full_width
                .ok_or_else(|| syn::Error::new(Span::call_site(), "missing `full_width`"))?,
            height: height.ok_or_else(|| syn::Error::new(Span::call_site(), "missing `height`"))?,
        })
    }
}

struct ParsedFont {
    bitmap: Vec<u8>,
    metrics: Vec<GlyphMetricValue>,
    symbols: String,
    native_size: u32,
    line_height: u32,
    baseline: u32,
}

struct GlyphMetricValue {
    bitmap_index: u32,
    adv_w: u32,
    box_w: u32,
    box_h: u32,
    ofs_x: i32,
    ofs_y: i32,
}

#[proc_macro]
pub fn lvgl_font(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as FontMacroInput);

    match expand_font_macro(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_font_macro(input: FontMacroInput) -> Result<TokenStream2> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| syn::Error::new(Span::call_site(), "CARGO_MANIFEST_DIR is not set"))?;
    let path = PathBuf::from(manifest_dir).join(input.path.value());

    let source = fs::read_to_string(&path).map_err(|error| {
        syn::Error::new(
            input.path.span(),
            format!("failed to read `{}`: {error}", path.display()),
        )
    })?;

    let parsed = parse_lvgl_font(&source)?;
    let crate_path = bridge_crate_path()?;

    let bitmap = parsed.bitmap.iter();
    let symbols = LitStr::new(&parsed.symbols, Span::call_site());
    let half_width = &input.half_width;
    let full_width = &input.full_width;
    let height = &input.height;
    let native_size = parsed.native_size;
    let line_height = parsed.line_height;
    let baseline = parsed.baseline;

    let metrics = parsed.metrics.iter().map(|metric| {
        let bitmap_index = metric.bitmap_index;
        let adv_w = metric.adv_w;
        let box_w = metric.box_w;
        let box_h = metric.box_h;
        let ofs_x = metric.ofs_x;
        let ofs_y = metric.ofs_y;

        quote! {
            #crate_path::GlyphMetrics::new(
                #bitmap_index,
                #adv_w,
                #box_w,
                #box_h,
                #ofs_x,
                #ofs_y,
            )
        }
    });

    Ok(quote! {{
        const BITMAP: &[u8] = &[#(#bitmap),*];
        const SYMBOLS: &str = #symbols;
        const METRICS: &[#crate_path::GlyphMetrics] = &[#(#metrics),*];

        #crate_path::FontData::new(
            BITMAP,
            SYMBOLS,
            METRICS,
            #half_width,
            #full_width,
            #height,
            #native_size,
            #line_height,
            #baseline,
        )
    }})
}

fn bridge_crate_path() -> Result<TokenStream2> {
    let found = crate_name("lvgl-font-bridge")
        .map_err(|error| syn::Error::new(Span::call_site(), format!("crate resolution failed: {error}")))?;

    Ok(match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
    })
}

fn parse_lvgl_font(source: &str) -> Result<ParsedFont> {
    let bitmap_body = capture_section(source, r"glyph_bitmap\[\]\s*=\s*\{(?s)(.*?)\};")?;
    let glyph_dsc_body = capture_section(source, r"glyph_dsc\[\]\s*=\s*\{(?s)(.*?)\};")?;
    let unicode_list_body = capture_section(source, r"unicode_list_0\[\]\s*=\s*\{(?s)(.*?)\};")?;

    let bitmap = parse_u8_list(&strip_c_comments(&bitmap_body)?)?;
    let unicode_list = parse_u32_list(&strip_c_comments(&unicode_list_body)?)?;
    let range_start = capture_single_u32(source, r"\.range_start\s*=\s*(\d+)")?;
    let native_size = capture_single_u32(source, r"Size:\s*(\d+)\s*px")?;
    let line_height = capture_single_u32(source, r"\.line_height\s*=\s*(\d+)")?;
    let baseline = capture_single_u32(source, r"\.base_line\s*=\s*(\d+)")?;

    let glyph_re = Regex::new(
        r"\{\s*\.bitmap_index\s*=\s*(\d+),\s*\.adv_w\s*=\s*(\d+),\s*\.box_w\s*=\s*(\d+),\s*\.box_h\s*=\s*(\d+),\s*\.ofs_x\s*=\s*(-?\d+),\s*\.ofs_y\s*=\s*(-?\d+)\s*\}",
    )
    .map_err(regex_to_syn_error)?;

    let mut metrics = Vec::new();
    for captures in glyph_re.captures_iter(&glyph_dsc_body).skip(1) {
        metrics.push(GlyphMetricValue {
            bitmap_index: parse_capture_u32(&captures[1])?,
            adv_w: parse_capture_u32(&captures[2])?,
            box_w: parse_capture_u32(&captures[3])?,
            box_h: parse_capture_u32(&captures[4])?,
            ofs_x: captures[5]
                .parse()
                .map_err(|_| syn::Error::new(Span::call_site(), "invalid ofs_x value"))?,
            ofs_y: captures[6]
                .parse()
                .map_err(|_| syn::Error::new(Span::call_site(), "invalid ofs_y value"))?,
        });
    }

    if metrics.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "no glyph metrics found in glyph_dsc[]",
        ));
    }

    if unicode_list.len() != metrics.len() {
        return Err(syn::Error::new(
            Span::call_site(),
            format!(
                "unicode_list length ({}) does not match glyph metrics length ({})",
                unicode_list.len(),
                metrics.len()
            ),
        ));
    }

    let mut symbols = String::new();
    for offset in unicode_list {
        let codepoint = range_start
            .checked_add(offset)
            .ok_or_else(|| syn::Error::new(Span::call_site(), "codepoint overflow"))?;
        let character = char::from_u32(codepoint).ok_or_else(|| {
            syn::Error::new(Span::call_site(), format!("invalid Unicode codepoint U+{codepoint:04X}"))
        })?;
        symbols.push(character);
    }

    Ok(ParsedFont {
        bitmap,
        metrics,
        symbols,
        native_size,
        line_height,
        baseline,
    })
}

fn capture_section(source: &str, pattern: &str) -> Result<String> {
    let re = Regex::new(pattern).map_err(regex_to_syn_error)?;
    let captures = re
        .captures(source)
        .ok_or_else(|| syn::Error::new(Span::call_site(), format!("section not found: {pattern}")))?;
    Ok(captures
        .get(1)
        .ok_or_else(|| syn::Error::new(Span::call_site(), "missing capture group"))?
        .as_str()
        .to_string())
}

fn capture_single_u32(source: &str, pattern: &str) -> Result<u32> {
    let re = Regex::new(pattern).map_err(regex_to_syn_error)?;
    let captures = re
        .captures(source)
        .ok_or_else(|| syn::Error::new(Span::call_site(), format!("value not found: {pattern}")))?;
    parse_capture_u32(
        captures
            .get(1)
            .ok_or_else(|| syn::Error::new(Span::call_site(), "missing capture group"))?
            .as_str(),
    )
}

fn parse_u8_list(section: &str) -> Result<Vec<u8>> {
    let number_re = Regex::new(r"0x[0-9A-Fa-f]+|\d+").map_err(regex_to_syn_error)?;
    let mut values = Vec::new();

    for matched in number_re.find_iter(section) {
        values.push(parse_capture_u8(matched.as_str())?);
    }

    if values.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "expected at least one numeric value",
        ));
    }

    Ok(values)
}

fn parse_u32_list(section: &str) -> Result<Vec<u32>> {
    let number_re = Regex::new(r"0x[0-9A-Fa-f]+|\d+").map_err(regex_to_syn_error)?;
    let mut values = Vec::new();

    for matched in number_re.find_iter(section) {
        values.push(parse_capture_u32(matched.as_str())?);
    }

    if values.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "expected at least one numeric value",
        ));
    }

    Ok(values)
}

fn strip_c_comments(section: &str) -> Result<String> {
    let block_re = Regex::new(r"(?s)/\*.*?\*/").map_err(regex_to_syn_error)?;
    let without_block = block_re.replace_all(section, "");
    let line_re = Regex::new(r"//[^\n\r]*").map_err(regex_to_syn_error)?;

    Ok(line_re.replace_all(&without_block, "").into_owned())
}

fn parse_capture_u32(value: &str) -> Result<u32> {
    if let Some(hex) = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
            .map_err(|_| syn::Error::new(Span::call_site(), format!("invalid hex value `{value}`")))
    } else {
        value
            .parse()
            .map_err(|_| syn::Error::new(Span::call_site(), format!("invalid integer value `{value}`")))
    }
}

fn parse_capture_u8(value: &str) -> Result<u8> {
    let parsed = parse_capture_u32(value)?;

    u8::try_from(parsed).map_err(|_| {
        syn::Error::new(
            Span::call_site(),
            format!("bitmap value `{value}` is out of range for u8"),
        )
    })
}

fn regex_to_syn_error(error: regex::Error) -> syn::Error {
    syn::Error::new(Span::call_site(), format!("regex error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_comments_before_parsing_bitmap_values() {
        let section = r#"
            /* U+0021 "!" */
            0xfc, 0x80,

            /* U+0072 "r" */
            0xd2, 0x49, 0x20
        "#;

        let stripped = strip_c_comments(section).unwrap();
        let values = parse_u8_list(&stripped).unwrap();

        assert_eq!(values, vec![0xfc, 0x80, 0xd2, 0x49, 0x20]);
    }

    #[test]
    fn bitmap_values_must_fit_in_u8() {
        let error = parse_u8_list("0x100").unwrap_err();

        assert!(error.to_string().contains("out of range for u8"));
    }
}
