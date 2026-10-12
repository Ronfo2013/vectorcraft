//! Hanging punctuation and optical margins: how far a character at the start or end of a line
//! stands outside the line's edge.
//!
//! Behaviour from the public documentation ("Formatting paragraphs" › Hanging punctuation,
//! <https://helpx.adobe.com/illustrator/using/formatting-paragraphs.html>):
//!
//! - **Roman Hanging Punctuation** (Paragraph panel menu, per paragraph): single and double
//!   quotation marks, hyphens, full stops and commas hang 100 % outside the margins; asterisks,
//!   tildes, ellipses, en dashes, em dashes, colons and semicolons hang 50 %.
//! - **Optical Margin Alignment** (Type menu, for every paragraph of a type object): punctuation
//!   marks and the edges of letters such as W and A fall outside the text margins, so the type
//!   looks aligned.
//!
//! The documentation gives no amounts for letters. VectorCraft's are its own, derived from the
//! shapes: a letter whose side is a diagonal stroke (A, V, W, Y, X, v, w, y, x) has a wedge of
//! empty space at that edge about a fifth of its advance wide at the x-height, and half of that
//! wedge reads as empty, so it hangs a tenth of its advance; a letter whose edge is a bar set
//! over a centred stem (T, and the left of J) about half that; a round side (O, C, G, Q, D, o,
//! c, e…) only its overshoot, a thirtieth. Optical Margin Alignment hangs the punctuation the
//! documentation lists by the same amounts as Roman Hanging Punctuation. Amounts are fractions of
//! the advance the character takes on the line, so a mark hanging wholly outside the start leaves
//! the next character on the edge.

/// Which edge of the line a character stands at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The line's start (left in horizontal type, top in vertical type).
    Start,
    /// The line's end.
    End,
}

/// Punctuation hanging wholly outside: quotation marks (straight, curly, low-9 and angle),
/// hyphens, the full stop and the comma.
fn full(c: char) -> bool {
    matches!(
        c,
        '\'' | '"'
            | '\u{2018}'
            | '\u{2019}'
            | '\u{201A}'
            | '\u{201B}'
            | '\u{201C}'
            | '\u{201D}'
            | '\u{201E}'
            | '\u{201F}'
            | '\u{2039}'
            | '\u{203A}'
            | '\u{00AB}'
            | '\u{00BB}'
            | '-'
            | '\u{00AD}'
            | '\u{2010}'
            | '\u{2011}'
            | '.'
            | ','
    )
}

/// Punctuation hanging half outside: asterisk, tilde, ellipsis, en and em dash, colon, semicolon.
fn half(c: char) -> bool {
    matches!(c, '*' | '~' | '\u{2026}' | '\u{2013}' | '\u{2014}' | ':' | ';')
}

/// The fraction of a letter's advance Optical Margin Alignment hangs at `edge` (0 for anything
/// else).
fn letter(c: char, edge: Edge) -> f64 {
    const DIAGONAL: f64 = 0.1;
    const BAR: f64 = 0.05;
    const ROUND: f64 = 0.03;
    match (c, edge) {
        ('A' | 'V' | 'W' | 'X' | 'Y' | 'v' | 'w' | 'x' | 'y', _) => DIAGONAL,
        ('T', _) | ('J', Edge::Start) => BAR,
        ('O' | 'Q' | 'o', _) => ROUND,
        ('C' | 'G' | 'c' | 'e' | 'd' | 'q', Edge::Start) => ROUND,
        ('D' | 'b' | 'p', Edge::End) => ROUND,
        _ => 0.0,
    }
}

/// The fraction of the advance of character `c` that hangs outside a line's `edge`, with Roman
/// Hanging Punctuation (`roman`) and Optical Margin Alignment (`optical`) as given (0: none).
pub fn ratio(c: char, edge: Edge, roman: bool, optical: bool) -> f64 {
    if !roman && !optical {
        return 0.0;
    }
    let punct = if full(c) {
        1.0
    } else if half(c) {
        0.5
    } else {
        0.0
    };
    if punct > 0.0 {
        return punct;
    }
    if optical { letter(c, edge) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_punctuation_hangs_whole_or_half_and_letters_only_optically() {
        for c in ['"', '\'', '\u{201C}', '\u{2019}', '-', '.', ','] {
            assert_eq!(ratio(c, Edge::Start, true, false), 1.0, "{c}");
            assert_eq!(ratio(c, Edge::End, true, false), 1.0, "{c}");
        }
        for c in ['*', '~', '\u{2026}', '\u{2013}', '\u{2014}', ':', ';'] {
            assert_eq!(ratio(c, Edge::End, true, false), 0.5, "{c}");
        }
        assert_eq!(ratio('W', Edge::Start, true, false), 0.0);
        assert!(ratio('W', Edge::Start, false, true) > 0.0);
        assert!(ratio('A', Edge::End, false, true) > 0.0);
        assert_eq!(ratio('.', Edge::End, false, true), 1.0);
        assert_eq!(ratio('m', Edge::End, true, true), 0.0);
        assert_eq!(ratio('.', Edge::End, false, false), 0.0);
        // A round side hangs less than a diagonal one; only the round side hangs.
        assert!(ratio('O', Edge::Start, false, true) < ratio('V', Edge::Start, false, true));
        assert_eq!(ratio('D', Edge::Start, false, true), 0.0);
        assert!(ratio('D', Edge::End, false, true) > 0.0);
    }
}
