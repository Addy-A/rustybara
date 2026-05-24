//! Character-code -> glyph-ID resolution for PDF simple fonts.
//!
//! Resolution priority:
//! 1. `/ToUnicode` CMap stream -> Unicode scalar -> `Face::glyph_index`
//! 2. `/Encoding` /Differences -> Postscript glyph name -> `Face::glyph_index_by_name`
//! 3. `/Encoding` named -> WinAnsi / MacRoman table -> `Face::glyph_index`
//! 4. Passthrough -> treat bytes as Latin-1 codepoint -> `Face::glyph_index`
//!
//! CIDFont / Type0 composite fonts are not handled here; callers get `None`
//! on every lookup and fall back to the bbox placeholder.

use crate::objects::tree::{deref, ref_id};
use lopdf::{Document, Object, ObjectId};
use std::sync::OnceLock;
use std::{char, collections::HashMap};
use ttf_parser::{Face, GlyphId};

pub enum NamedEncoding {
    WinAnsi,
    MacRoman,
    Standard,
}

pub enum EncodingSlot {
    Name(String),
    Char(char),
}

pub enum FontEncoding {
    ToUnicode(HashMap<u8, char>),
    Differences(HashMap<u8, EncodingSlot>),
    Named(NamedEncoding),
    Passthrough,
}

impl FontEncoding {
    pub fn resolve(&self, charcode: u8, face: &Face<'_>) -> Option<GlyphId> {
        match self {
            Self::ToUnicode(map) => face.glyph_index(*map.get(&charcode)?),
            Self::Differences(slots) => match slots.get(&charcode) {
                Some(EncodingSlot::Name(n)) => face
                    .glyph_index_by_name(n)
                    .or_else(|| adobe_name_to_char(n).and_then(|c| face.glyph_index(c))),
                Some(EncodingSlot::Char(c)) => face.glyph_index(*c),
                None => face.glyph_index(char::from_u32(charcode as u32)?),
            },
            Self::Named(enc) => face.glyph_index(enc.to_char(charcode)?),
            Self::Passthrough => face.glyph_index(char::from_u32(charcode as u32)?),
        }
    }
}

impl NamedEncoding {
    pub fn to_char(&self, code: u8) -> Option<char> {
        match self {
            Self::WinAnsi => win_ansi_char(code),
            Self::MacRoman => {
                if (0x20..=0x7E).contains(&code) {
                    char::from_u32(code as u32)
                } else {
                    None
                }
            }
            Self::Standard => {
                if (0x20..=0x7E).contains(&code) {
                    char::from_u32(code as u32)
                } else {
                    None
                }
            }
        }
    }
}

static WIN_ANSI: OnceLock<[Option<char>; 256]> = OnceLock::new();

fn win_ansi_char(code: u8) -> Option<char> {
    WIN_ANSI.get_or_init(|| {
        let mut t = [None::<char>; 256];
        for c in 0x20u8..=0x7Eu8 {
            t[c as usize] = char::from_u32(c as u32);
        }
        let ext: &[(u8, char)] = &[
            (0x80, '€'),
            (0x82, '‚'),
            (0x83, 'ƒ'),
            (0x84, '„'),
            (0x85, '…'),
            (0x86, '†'),
            (0x87, '‡'),
            (0x88, 'ˆ'),
            (0x89, '‰'),
            (0x8A, 'Š'),
            (0x8B, '‹'),
            (0x8C, 'Œ'),
            (0x8E, 'Ž'),
            (0x91, '\u{2018}'),
            (0x92, '\u{2019}'),
            (0x93, '\u{201C}'),
            (0x94, '\u{201D}'),
            (0x95, '•'),
            (0x96, '–'),
            (0x97, '—'),
            (0x98, '˜'),
            (0x99, '™'),
            (0x9A, 'š'),
            (0x9B, '›'),
            (0x9C, 'œ'),
            (0x9E, 'ž'),
            (0x9F, 'Ÿ'),
        ];
        for &(b, c) in ext {
            t[b as usize] = Some(c);
        }
        for c in 0xA0u8..=0xFFu8 {
            t[c as usize] = char::from_u32(c as u32);
        }
        t
    })[code as usize]
}

pub fn build_encoding(doc: &Document, page_id: ObjectId, font_name: &[u8]) -> FontEncoding {
    let Some(font_dict) = get_font_dict(doc, page_id, font_name) else {
        return FontEncoding::Passthrough;
    };

    // 1. /ToUnicode
    if let Ok(tu_val) = font_dict.get(b"ToUnicode") {
        if let Some(id) = ref_id(tu_val) {
            if let Ok(Object::Stream(s)) = doc.get_object(id) {
                if let Ok(bytes) = s.decompressed_content() {
                    return FontEncoding::ToUnicode(parse_to_unicode(&bytes));
                }
            }
        }
    }

    // 2. /Encoding
    if let Ok(enc_val) = font_dict.get(b"Encoding") {
        let enc_obj = deref(doc, enc_val).clone();
        match enc_obj {
            Object::Name(name) => {
                return match name.as_slice() {
                    b"WinAnsiEncoding" => FontEncoding::Named(NamedEncoding::WinAnsi),
                    b"MacRomanEncoding" => FontEncoding::Named(NamedEncoding::MacRoman),
                    b"StandardEncoding" => FontEncoding::Named(NamedEncoding::Standard),
                    _ => FontEncoding::Passthrough,
                };
            }
            Object::Dictionary(d) => {
                let mut slots: HashMap<u8, EncodingSlot> = HashMap::new();
                if let Ok(Object::Array(arr)) = d.get(b"Differences") {
                    let mut code: u8 = 0;
                    for item in arr {
                        match item {
                            Object::Integer(n) => code = *n as u8,
                            Object::Name(gname) => {
                                slots.insert(
                                    code,
                                    EncodingSlot::Name(String::from_utf8_lossy(gname).into_owned()),
                                );
                                code = code.wrapping_add(1);
                            }
                            _ => {}
                        }
                    }
                }
                if !slots.is_empty() {
                    return FontEncoding::Differences(slots);
                }
            }
            _ => {}
        }
    }

    FontEncoding::Passthrough
}

fn parse_to_unicode(bytes: &[u8]) -> HashMap<u8, char> {
    let text = String::from_utf8_lossy(bytes);
    let mut map: HashMap<u8, char> = HashMap::new();
    let mut mode = 0u8;

    for line in text.lines() {
        let line = line.trim();
        match line {
            "beginbfchar" => {
                mode = 1;
                continue;
            }
            "endbfchar" => {
                mode = 0;
                continue;
            }
            "beginbfrange" => {
                mode = 2;
                continue;
            }
            "endbfrange" => {
                mode = 0;
                continue;
            }
            _ => {}
        }
        let tokens = hex_tokens(line);
        match mode {
            1 if tokens.len() >= 2 => {
                if let Some(c) = char::from_u32(tokens[1]) {
                    map.insert(tokens[0] as u8, c);
                }
            }
            2 if tokens.len() >= 3 => {
                let start = tokens[0] as u8;
                let end = tokens[1] as u8;
                let base = tokens[2];
                for offset in 0u32..=(end.wrapping_sub(start) as u32) {
                    let code = start.wrapping_add(offset as u8);
                    if let Some(c) = char::from_u32(base + offset) {
                        map.insert(code, c);
                    }
                }
            }
            _ => {}
        }
    }
    map
}

fn hex_tokens(line: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            let start = i + 1;
            i += 1;
            while i < b.len() && b[i] != b'>' {
                i += 1;
            }
            if let Ok(v) = u32::from_str_radix(&line[start..i], 16) {
                out.push(v);
            }
        }
        i += 1;
    }
    out
}

// ── Adobe Glyph List (minimal) ────────────────────────────────────────────────

fn adobe_name_to_char(name: &str) -> Option<char> {
    if let Some(rest) = name.strip_prefix("uni") {
        if rest.len() == 4 {
            return u32::from_str_radix(rest, 16).ok().and_then(char::from_u32);
        }
    }
    match name {
        "space" => ' ',
        "exclam" => '!',
        "quotedbl" => '"',
        "numbersign" => '#',
        "dollar" => '$',
        "percent" => '%',
        "ampersand" => '&',
        "quotesingle" => '\'',
        "parenleft" => '(',
        "parenright" => ')',
        "asterisk" => '*',
        "plus" => '+',
        "comma" => ',',
        "hyphen" => '-',
        "period" => '.',
        "slash" => '/',
        "colon" => ':',
        "semicolon" => ';',
        "less" => '<',
        "equal" => '=',
        "greater" => '>',
        "question" => '?',
        "at" => '@',
        "bracketleft" => '[',
        "backslash" => '\\',
        "bracketright" => ']',
        "asciicircum" => '^',
        "underscore" => '_',
        "grave" => '`',
        "braceleft" => '{',
        "bar" => '|',
        "braceright" => '}',
        "asciitilde" => '~',
        "endash" => '\u{2013}',
        "emdash" => '\u{2014}',
        "quotedblleft" => '\u{201C}',
        "quotedblright" => '\u{201D}',
        "quoteleft" => '\u{2018}',
        "quoteright" => '\u{2019}',
        "Euro" => '€',
        "bullet" => '•',
        "ellipsis" => '…',
        "trademark" => '™',
        "fi" => '\u{FB01}',
        "fl" => '\u{FB02}',
        _ => return None,
    }
    .into()
}

fn get_font_dict(doc: &Document, page_id: ObjectId, font_name: &[u8]) -> Option<lopdf::Dictionary> {
    let page = doc.get_object(page_id).ok()?;
    let page_dict = page.as_dict().ok()?;
    let res_val = page_dict.get(b"Resources").ok()?;
    let res_dict = deref(doc, res_val).as_dict().ok()?;
    let font_val = res_dict.get(b"Font").ok()?;
    let font_map = deref(doc, font_val).as_dict().ok()?;
    let fv = font_map.get(font_name).ok()?;
    let fid = ref_id(fv)?;
    doc.get_object(fid).ok()?.as_dict().ok().cloned()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── win_ansi_char ─────────────────────────────────────────────────────────

    #[test]
    fn win_ansi_ascii_printable() {
        assert_eq!(win_ansi_char(0x20), Some(' '));
        assert_eq!(win_ansi_char(0x41), Some('A'));
        assert_eq!(win_ansi_char(0x61), Some('a'));
        assert_eq!(win_ansi_char(0x7E), Some('~'));
    }

    #[test]
    fn win_ansi_control_codes_are_none() {
        assert_eq!(win_ansi_char(0x00), None);
        assert_eq!(win_ansi_char(0x01), None);
        assert_eq!(win_ansi_char(0x1F), None);
        assert_eq!(win_ansi_char(0x7F), None);
    }

    #[test]
    fn win_ansi_windows_extension_chars() {
        assert_eq!(win_ansi_char(0x80), Some('€'));
        assert_eq!(win_ansi_char(0x99), Some('™'));
        assert_eq!(win_ansi_char(0x9F), Some('Ÿ'));
        assert_eq!(win_ansi_char(0x8A), Some('Š'));
    }

    #[test]
    fn win_ansi_undefined_slots_are_none() {
        // Slots 0x81, 0x8D, 0x8F, 0x90, 0x9D are undefined in Windows-1252.
        assert_eq!(win_ansi_char(0x81), None);
        assert_eq!(win_ansi_char(0x8D), None);
        assert_eq!(win_ansi_char(0x8F), None);
        assert_eq!(win_ansi_char(0x90), None);
        assert_eq!(win_ansi_char(0x9D), None);
    }

    #[test]
    fn win_ansi_latin1_supplement() {
        assert_eq!(win_ansi_char(0xA0), Some('\u{00A0}')); // non-breaking space
        assert_eq!(win_ansi_char(0xA9), Some('©'));
        assert_eq!(win_ansi_char(0xE9), Some('é'));
        assert_eq!(win_ansi_char(0xFF), Some('ÿ'));
    }

    // ── NamedEncoding ─────────────────────────────────────────────────────────

    #[test]
    fn named_win_ansi_ascii_and_extensions() {
        let enc = NamedEncoding::WinAnsi;
        assert_eq!(enc.to_char(0x41), Some('A'));
        assert_eq!(enc.to_char(0x20), Some(' '));
        assert_eq!(enc.to_char(0x80), Some('€'));
        assert_eq!(enc.to_char(0x81), None);
    }

    #[test]
    fn named_mac_roman_ascii_only() {
        let enc = NamedEncoding::MacRoman;
        assert_eq!(enc.to_char(0x41), Some('A'));
        // Upper range not covered in first-pass; returns None.
        assert_eq!(enc.to_char(0x80), None);
    }

    #[test]
    fn named_standard_ascii_only() {
        let enc = NamedEncoding::Standard;
        assert_eq!(enc.to_char(0x41), Some('A'));
        assert_eq!(enc.to_char(0x80), None);
    }

    // ── hex_tokens ────────────────────────────────────────────────────────────

    #[test]
    fn hex_tokens_single_value() {
        assert_eq!(hex_tokens("<41>"), vec![0x41u32]);
    }

    #[test]
    fn hex_tokens_two_values() {
        assert_eq!(hex_tokens("<41> <0042>"), vec![0x41, 0x42]);
    }

    #[test]
    fn hex_tokens_three_values() {
        assert_eq!(hex_tokens("<41> <43> <0041>"), vec![0x41, 0x43, 0x41]);
    }

    #[test]
    fn hex_tokens_empty_line() {
        assert!(hex_tokens("").is_empty());
    }

    #[test]
    fn hex_tokens_no_angle_brackets() {
        assert!(hex_tokens("beginbfchar").is_empty());
    }

    // ── parse_to_unicode ──────────────────────────────────────────────────────

    #[test]
    fn parse_bfchar_single() {
        let cmap = b"beginbfchar\n<41> <0041>\nendbfchar\n";
        let map = parse_to_unicode(cmap);
        assert_eq!(map.get(&0x41), Some(&'A'));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn parse_bfchar_multiple() {
        let cmap = b"beginbfchar\n<41> <0041>\n<42> <0042>\nendbfchar\n";
        let map = parse_to_unicode(cmap);
        assert_eq!(map.get(&0x41), Some(&'A'));
        assert_eq!(map.get(&0x42), Some(&'B'));
    }

    #[test]
    fn parse_bfrange_linear() {
        let cmap = b"beginbfrange\n<41> <43> <0041>\nendbfrange\n";
        let map = parse_to_unicode(cmap);
        assert_eq!(map.get(&0x41), Some(&'A'));
        assert_eq!(map.get(&0x42), Some(&'B'));
        assert_eq!(map.get(&0x43), Some(&'C'));
    }

    #[test]
    fn parse_ignores_text_outside_blocks() {
        let cmap = b"preamble\n<41> <0041>\nbeginbfchar\n<42> <0042>\nendbfchar\n";
        let map = parse_to_unicode(cmap);
        assert!(!map.contains_key(&0x41), "entry outside block must be ignored");
        assert_eq!(map.get(&0x42), Some(&'B'));
    }

    #[test]
    fn parse_empty_stream() {
        assert!(parse_to_unicode(b"").is_empty());
    }

    #[test]
    fn parse_mixed_bfchar_and_bfrange() {
        let cmap = b"beginbfchar\n<20> <0020>\nendbfchar\nbeginbfrange\n<41> <42> <0041>\nendbfrange\n";
        let map = parse_to_unicode(cmap);
        assert_eq!(map.get(&0x20), Some(&' '));
        assert_eq!(map.get(&0x41), Some(&'A'));
        assert_eq!(map.get(&0x42), Some(&'B'));
    }

    // ── adobe_name_to_char ────────────────────────────────────────────────────

    #[test]
    fn adobe_name_uni_four_hex_digits() {
        assert_eq!(adobe_name_to_char("uni0041"), Some('A'));
        assert_eq!(adobe_name_to_char("uni20AC"), Some('€'));
        assert_eq!(adobe_name_to_char("uni2013"), Some('\u{2013}'));
    }

    #[test]
    fn adobe_name_uni_wrong_length_falls_through() {
        // "uni" + != 4 hex chars is not the uniXXXX convention; falls to match table.
        assert_eq!(adobe_name_to_char("uni41"), None);
    }

    #[test]
    fn adobe_name_known_glyph_names() {
        assert_eq!(adobe_name_to_char("space"), Some(' '));
        assert_eq!(adobe_name_to_char("hyphen"), Some('-'));
        assert_eq!(adobe_name_to_char("endash"), Some('\u{2013}'));
        assert_eq!(adobe_name_to_char("Euro"), Some('€'));
        assert_eq!(adobe_name_to_char("fi"), Some('\u{FB01}'));
    }

    #[test]
    fn adobe_name_unknown_returns_none() {
        assert_eq!(adobe_name_to_char("notarealname"), None);
        assert_eq!(adobe_name_to_char(""), None);
    }
}
