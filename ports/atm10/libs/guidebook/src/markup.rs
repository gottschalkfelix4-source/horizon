//! Patchouli's text markup: `$(br)` line break, `$(br2)` paragraph, `$(l:url)...$()` link,
//! `$(item)`, `$(thing)`, `$(k:key)` key name, `$(#rrggbb)` colour, `$(o)` obfuscated, `$(b)`,
//! `$(i)`, `$(s)`, `$(u)`, and `$()` to reset. Macros a pack defines are expanded by the caller.

/// A run of text in one style.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub obfuscated: bool,
    /// 0xRRGGBB, when set.
    pub color: Option<u32>,
    /// A link target (`https://...` or an entry id).
    pub link: Option<String>,
    /// A key binding's id, shown as its current key.
    pub key: Option<String>,
}

/// One piece of a text: a span, or a break.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Span(Span),
    /// A new line; `blank` adds an empty line after it (`$(br2)`).
    Break { blank: bool },
}

pub fn parse(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut style = Span::default();
    let mut buf = String::new();
    let mut rest = text;
    let flush = |buf: &mut String, style: &Span, out: &mut Vec<Piece>| {
        if !buf.is_empty() {
            out.push(Piece::Span(Span { text: std::mem::take(buf), ..style.clone() }));
        }
    };
    while let Some(i) = rest.find("$(") {
        buf.push_str(&rest[..i]);
        let Some(end) = rest[i..].find(')') else {
            buf.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let code = &rest[i + 2..i + end];
        rest = &rest[i + end + 1..];
        flush(&mut buf, &style, &mut out);
        match code {
            "" => style = Span::default(),
            "br" => out.push(Piece::Break { blank: false }),
            "br2" | "p" => out.push(Piece::Break { blank: true }),
            "b" | "bold" => style.bold = true,
            "i" | "italic" | "em" => style.italic = true,
            "u" | "underline" => style.underline = true,
            "s" | "strike" => style.strike = true,
            "o" | "obf" => style.obfuscated = true,
            c if c.starts_with("l:") => {
                style.link = Some(c[2..].to_owned());
                style.underline = true;
                style.color.get_or_insert(0x0000EE);
            }
            c if c.starts_with("k:") => style.key = Some(c[2..].to_owned()),
            c if c.starts_with('#') => {
                if let Ok(v) = u32::from_str_radix(&c[1..], 16) {
                    style.color = Some(v);
                }
            }
            // `$(item)` and `$(thing)`: Patchouli's highlight colours.
            "item" => style.color = Some(0x0000B0),
            "thing" => style.color = Some(0x0B6623),
            _ => {}
        }
    }
    buf.push_str(rest);
    flush(&mut buf, &style, &mut out);
    out
}

/// One line of a page: spans laid out left to right.
pub type Line = Vec<Span>;

/// Cuts `pieces` into pages of at most `lines_per_page` lines, each line at most `width` wide as
/// `measure` says (the game's own text width, so the result matches what is drawn). A word that
/// is wider than a line stands alone on it. A `blank` break costs an empty line.
pub fn paginate(pieces: &[Piece], width: f32, lines_per_page: usize, measure: &dyn Fn(&Span) -> f32) -> Vec<Vec<Line>> {
    let mut lines: Vec<Line> = vec![Vec::new()];
    let mut used = 0.0f32;
    for piece in pieces {
        match piece {
            Piece::Break { blank } => {
                lines.push(Vec::new());
                if *blank {
                    lines.push(Vec::new());
                }
                used = 0.0;
            }
            Piece::Span(span) => {
                // Words keep their trailing space so a span that ends mid-sentence joins the next.
                for word in span.text.split_inclusive(' ') {
                    let piece = Span { text: word.to_owned(), ..span.clone() };
                    let w = measure(&piece);
                    if used + w > width && used > 0.0 {
                        lines.push(Vec::new());
                        used = 0.0;
                    }
                    let line = lines.last_mut().expect("a line exists");
                    // A line starts after a space: drop the leading one of a wrapped word.
                    line.push(piece);
                    used += w;
                }
            }
        }
    }
    if lines_per_page == 0 {
        return vec![lines];
    }
    lines.chunks(lines_per_page).map(<[Line]>::to_vec).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(pieces: &[Piece]) -> String {
        pieces
            .iter()
            .map(|p| match p {
                Piece::Span(s) => s.text.clone(),
                Piece::Break { .. } => "|".into(),
            })
            .collect()
    }

    #[test]
    fn styles_links_and_breaks() {
        let p = parse("Use $(l:https://a.b)the site$() or $(#ff0000)red$(br)$(item)Rod$(br2)end");
        assert_eq!(text(&p), "Use the site or red|Rod|end");
        let Piece::Span(link) = &p[1] else { panic!() };
        assert_eq!(link.link.as_deref(), Some("https://a.b"));
        assert!(link.underline);
        let Piece::Span(plain) = &p[2] else { panic!() };
        assert_eq!(plain.link, None);
        let Piece::Span(red) = &p[3] else { panic!() };
        assert_eq!(red.color, Some(0xFF0000));
    }

    #[test]
    fn an_open_code_is_text() {
        assert_eq!(text(&parse("price $(5")), "price $(5");
    }

    #[test]
    fn lines_wrap_by_measure_and_pages_by_line_count() {
        let p = parse("aa bb cc dd ee ff");
        // every character is 1 wide: "aa bb " is 6, so 7 wide fits two words per line
        let pages = paginate(&p, 7.0, 2, &|s| s.text.chars().count() as f32);
        let flat: Vec<Vec<String>> = pages.iter().map(|pg| pg.iter().map(|l| l.iter().map(|s| s.text.clone()).collect()).collect()).collect();
        assert_eq!(flat, vec![vec!["aa bb ".to_owned(), "cc dd ".to_owned()], vec!["ee ff".to_owned()]]);
    }
}
