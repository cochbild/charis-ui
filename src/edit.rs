//! Pure text-editing operations used by text inputs.

/// Cursor + selection anchor as byte offsets into the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub cursor: usize,
    pub anchor: usize,
}

impl Selection {
    pub fn caret(i: usize) -> Self {
        Self { cursor: i, anchor: i }
    }
    pub fn range(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }
    pub fn is_empty(&self) -> bool {
        self.cursor == self.anchor
    }
    pub fn clamp(&mut self, s: &str) {
        let fix = |mut i: usize| {
            i = i.min(s.len());
            while !s.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        self.cursor = fix(self.cursor);
        self.anchor = fix(self.anchor);
    }
}

pub fn prev_char(s: &str, i: usize) -> usize {
    s[..i].char_indices().next_back().map(|(j, _)| j).unwrap_or(0)
}

pub fn next_char(s: &str, i: usize) -> usize {
    s[i..].chars().next().map(|c| i + c.len_utf8()).unwrap_or(s.len())
}

/// The char starting at byte `i` (a space past the end, which is never a word char).
fn char_at(s: &str, i: usize) -> char {
    s.get(i..).and_then(|t| t.chars().next()).unwrap_or(' ')
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn prev_word(s: &str, i: usize) -> usize {
    let mut j = i;
    // skip whitespace/punctuation, then word chars
    while j > 0 {
        let p = prev_char(s, j);
        if is_word(char_at(s, p)) {
            break;
        }
        j = p;
    }
    while j > 0 {
        let p = prev_char(s, j);
        if !is_word(char_at(s, p)) {
            break;
        }
        j = p;
    }
    j
}

pub fn next_word(s: &str, i: usize) -> usize {
    let mut j = i;
    while j < s.len() && !is_word(char_at(s, j)) {
        j = next_char(s, j);
    }
    while j < s.len() && is_word(char_at(s, j)) {
        j = next_char(s, j);
    }
    j
}

/// The word around byte offset `i` (for double-click selection).
pub fn word_at(s: &str, i: usize) -> (usize, usize) {
    let i = i.min(s.len());
    let mut a = i;
    while a > 0 {
        let p = prev_char(s, a);
        if !is_word(char_at(s, p)) {
            break;
        }
        a = p;
    }
    let mut b = i;
    while b < s.len() && is_word(char_at(s, b)) {
        b = next_char(s, b);
    }
    (a, b)
}

/// Replace the selection with `ins`, returning the new text and caret.
pub fn replace(s: &str, sel: Selection, ins: &str) -> (String, Selection) {
    let (a, b) = sel.range();
    let mut out = String::with_capacity(s.len() + ins.len());
    out.push_str(&s[..a]);
    out.push_str(ins);
    out.push_str(&s[b..]);
    (out, Selection::caret(a + ins.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_motion() {
        let s = "hello  wide world";
        assert_eq!(next_word(s, 0), 5);
        assert_eq!(next_word(s, 5), 11);
        assert_eq!(prev_word(s, 11), 7);
        assert_eq!(prev_word(s, 7), 0);
        assert_eq!(word_at(s, 8), (7, 11));
    }

    #[test]
    fn unicode_safe() {
        let s = "héllo";
        assert_eq!(next_char(s, 1), 3);
        assert_eq!(prev_char(s, 3), 1);
        let (t, sel) = replace(s, Selection { cursor: 1, anchor: 3 }, "e");
        assert_eq!(t, "hello");
        assert_eq!(sel.cursor, 2);
    }
}
