use pulldown_cmark::Event;

/// Non-breaking space (U+00A0), French spacing before punctuation and inside guillemets.
const NO_BREAK_SPACE: char = '\u{00A0}';
/// Narrow non-breaking space (U+202F), French spacing around em dashes.
const NARROW_NO_BREAK_SPACE: char = '\u{202F}';

/// Applies French typographic conventions to pulldown-cmark's smart
/// punctuation: guillemets, non-breaking spaces and spaced em dashes.
#[derive(Debug, Default)]
pub(crate) struct Typography {
    french: bool,
    /// Quote nesting depth: 1 is `« … »`, deeper levels stay `“ … ”`.
    depth: u32,
    /// Spaces held until the next character.
    pending_spaces: usize,
    /// Last visible character.
    last_char: Option<char>,
    /// Previous character was a dash.
    after_dash: bool,
    /// Dash held until the next character.
    pending_dash: Option<char>,
    /// Held dash was preceded by a space.
    space_before_dash: bool,
}

impl Typography {
    pub(crate) fn new(french: bool) -> Self {
        Self { french, ..Self::default() }
    }

    pub(crate) fn is_french(&self) -> bool {
        self.french
    }

    /// Transforms one text event; call [`Self::take_pending`] to flush held characters.
    pub(crate) fn text(&mut self, text: &str) -> String {
        if !self.french {
            return text.to_string();
        }

        let mut out = String::with_capacity(text.len() + 8);
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                ' ' | NO_BREAK_SPACE => {
                    self.flush_dash(&mut out, true);
                    self.pending_spaces += 1;
                }
                '\u{2013}' | '\u{2014}' => {
                    self.flush_dash(&mut out, false);
                    if self.pending_spaces > 0 {
                        // A dash preceded by a space is an incise, not a range.
                        self.pending_spaces = 0;
                        self.ensure_space(&mut out, NARROW_NO_BREAK_SPACE);
                        self.space_before_dash = true;
                    } else {
                        self.space_before_dash = false;
                    }
                    self.pending_dash = Some(c);
                }
                ';' | '!' | '?' | ':' => {
                    self.flush_dash(&mut out, false);
                    let next = chars.peek().copied();
                    let no_space = match c {
                        // URL schemes and Windows drive letters.
                        ':' => next.is_some_and(|n| n == '/' || n == '\\'),
                        _ => false,
                    } || (c == ':'
                        && self.last_char.is_some_and(|p| p.is_ascii_digit())
                        && next.is_some_and(|n| n.is_ascii_digit()));
                    if !no_space
                        && self.last_char.is_some_and(|p| {
                            !p.is_whitespace() && !matches!(p, ';' | ':' | '!' | '?')
                        })
                    {
                        self.pending_spaces = 0;
                        self.ensure_space(&mut out, NO_BREAK_SPACE);
                        self.emit(&mut out, c);
                    } else {
                        self.flush_spaces(&mut out);
                        self.emit(&mut out, c);
                    }
                }
                '\u{201C}' => {
                    self.flush_dash(&mut out, false);
                    self.flush_spaces(&mut out);
                    self.depth += 1;
                    if self.depth == 1 {
                        out.push('«');
                        self.emit(&mut out, NO_BREAK_SPACE);
                    } else {
                        self.emit(&mut out, c);
                    }
                }
                '\u{201D}' => {
                    self.flush_dash(&mut out, false);
                    if self.depth <= 1 {
                        self.pending_spaces = 0;
                        self.ensure_space(&mut out, NO_BREAK_SPACE);
                        self.emit(&mut out, '»');
                        self.depth = 0;
                    } else {
                        self.flush_spaces(&mut out);
                        self.emit(&mut out, c);
                        self.depth -= 1;
                    }
                }
                _ => {
                    self.flush_dash(&mut out, false);
                    self.flush_spaces(&mut out);
                    self.emit(&mut out, c);
                }
            }
        }
        out
    }

    /// Flush held spaces and dash, if any, as a `Text` event.
    pub(crate) fn take_pending<'a>(&mut self) -> Option<Event<'a>> {
        let mut out = String::new();
        self.flush_dash(&mut out, false);
        self.flush_spaces(&mut out);
        if out.is_empty() { None } else { Some(Event::Text(out.into())) }
    }

    /// Forget quote pairing and pending characters at a block boundary.
    pub(crate) fn reset(&mut self) {
        self.depth = 0;
        self.pending_spaces = 0;
        self.last_char = None;
        self.after_dash = false;
        self.pending_dash = None;
        self.space_before_dash = false;
    }

    fn emit(&mut self, out: &mut String, c: char) {
        out.push(c);
        self.last_char = Some(c);
        self.after_dash = matches!(c, '\u{2013}' | '\u{2014}');
    }

    /// Push a non-breaking space unless one was just emitted.
    fn ensure_space(&mut self, out: &mut String, space: char) {
        let already = match out.chars().next_back() {
            Some(c) => c == space,
            None => self.last_char == Some(space),
        };
        if !already {
            out.push(space);
        }
        self.last_char = Some(space);
        self.after_dash = false;
    }

    fn flush_spaces(&mut self, out: &mut String) {
        for i in 0..self.pending_spaces {
            out.push(if i == 0 && self.after_dash { NARROW_NO_BREAK_SPACE } else { ' ' });
        }
        self.pending_spaces = 0;
        self.after_dash = false;
    }

    fn flush_dash(&mut self, out: &mut String, followed_by_space: bool) {
        if let Some(dash) = self.pending_dash.take() {
            let em_dash = self.space_before_dash || followed_by_space;
            if em_dash
                && !self.space_before_dash
                && self.last_char.is_some_and(|p| !p.is_whitespace())
            {
                self.ensure_space(out, NARROW_NO_BREAK_SPACE);
            }
            self.emit(out, if em_dash { '\u{2014}' } else { dash });
        }
        self.space_before_dash = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn french(text: &str) -> String {
        let mut typography = Typography::new(true);
        let mut out = typography.text(text);
        if let Some(Event::Text(pending)) = typography.take_pending() {
            out.push_str(&pending);
        }
        out
    }

    #[test]
    fn adds_non_breaking_spaces_before_punctuation() {
        assert_eq!(french("Bonjour!"), "Bonjour\u{00A0}!");
        assert_eq!(french("Vraiment ?"), "Vraiment\u{00A0}?");
        assert_eq!(french("Attention; voici"), "Attention\u{00A0}; voici");
        assert_eq!(french("Donc: oui"), "Donc\u{00A0}: oui");
        assert_eq!(french("Quoi?!"), "Quoi\u{00A0}?!");
    }

    #[test]
    fn keeps_spaces_where_french_has_none() {
        assert_eq!(french("12:30"), "12:30");
        assert_eq!(french("https://example.com"), "https://example.com");
    }

    #[test]
    fn uses_guillemets_with_non_breaking_spaces() {
        assert_eq!(french("\u{201C}Bonjour\u{201D}"), "«\u{00A0}Bonjour\u{00A0}»");
    }

    #[test]
    fn keeps_english_quotes_for_nested_quotes() {
        assert_eq!(
            french("\u{201C}a \u{201C}b\u{201D} c\u{201D}"),
            "«\u{00A0}a \u{201C}b\u{201D} c\u{00A0}»"
        );
    }

    #[test]
    fn turns_spaced_dashes_into_em_dashes() {
        assert_eq!(french("une incise \u{2013} ici"), "une incise\u{202F}—\u{202F}ici");
        assert_eq!(french("une incise \u{2014} ici"), "une incise\u{202F}—\u{202F}ici");
        assert_eq!(french("une incise\u{2013} ici"), "une incise\u{202F}—\u{202F}ici");
    }

    #[test]
    fn keeps_unspaced_dashes_for_ranges() {
        assert_eq!(french("2\u{2013}3"), "2\u{2013}3");
    }

    #[test]
    fn keeps_spaces_across_events() {
        let mut typography = Typography::new(true);
        let mut out = typography.text("Bonjour ");
        out.push_str(&typography.text("!"));
        assert_eq!(out, "Bonjour\u{00A0}!");
    }
}
