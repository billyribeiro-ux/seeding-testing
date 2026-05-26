//! hello-cli library — counts lines, words, and characters in a string.
//!
//! Kept in a library crate (separate from `main.rs`) so each function is unit-testable
//! without spawning the binary. This is the *first* enterprise habit we teach: business
//! logic lives in `lib.rs`, CLI plumbing lives in `main.rs`.

use std::fmt::{self, Write as _};

/// Counts of lines, words, and characters in an input string.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub lines: usize,
    pub words: usize,
    pub chars: usize,
}

impl Counts {
    /// Count the lines, words, and characters in `input`.
    ///
    /// - `lines`: number of newline-terminated lines (matches the behaviour of `str::lines`).
    /// - `words`: whitespace-separated tokens.
    /// - `chars`: Unicode scalar values (not bytes).
    pub fn count(input: &str) -> Self {
        Self {
            lines: input.lines().count(),
            words: input.split_whitespace().count(),
            chars: input.chars().count(),
        }
    }
}

impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:>7} {:>7} {:>7}", self.lines, self.words, self.chars)
    }
}

/// Which categories to print. Used by `main.rs` to drive output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    pub lines: bool,
    pub words: bool,
    pub chars: bool,
}

impl Selection {
    /// If the user passed no flags, default to "show all".
    #[must_use]
    pub fn or_default(self) -> Self {
        if self.lines || self.words || self.chars {
            self
        } else {
            Self {
                lines: true,
                words: true,
                chars: true,
            }
        }
    }

    pub fn render(self, c: Counts) -> String {
        let mut out = String::new();
        if self.lines {
            write!(out, "{:>7}", c.lines).expect("writing to a String never fails");
        }
        if self.words {
            if !out.is_empty() {
                out.push(' ');
            }
            write!(out, "{:>7}", c.words).expect("writing to a String never fails");
        }
        if self.chars {
            if !out.is_empty() {
                out.push(' ');
            }
            write!(out, "{:>7}", c.chars).expect("writing to a String never fails");
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_has_zero_of_everything() {
        let c = Counts::count("");
        assert_eq!(
            c,
            Counts {
                lines: 0,
                words: 0,
                chars: 0
            }
        );
    }

    #[test]
    fn single_line_no_trailing_newline() {
        let c = Counts::count("hello world");
        assert_eq!(c.lines, 1);
        assert_eq!(c.words, 2);
        assert_eq!(c.chars, "hello world".chars().count());
    }

    #[test]
    fn multiple_lines_with_trailing_newline() {
        let c = Counts::count("a\nb\nc\n");
        // `str::lines` doesn't count a trailing empty line — that's the canonical wc behaviour.
        assert_eq!(c.lines, 3);
        assert_eq!(c.words, 3);
        assert_eq!(c.chars, 6);
    }

    #[test]
    fn whitespace_only_has_zero_words() {
        let c = Counts::count("   \t  \n  ");
        assert_eq!(c.words, 0);
    }

    #[test]
    fn unicode_is_counted_per_scalar_not_byte() {
        // "café" is 4 scalars but 5 bytes (é is 2 bytes in UTF-8).
        let c = Counts::count("café");
        assert_eq!(c.chars, 4);
        assert_eq!(c.words, 1);
    }

    #[test]
    fn display_pads_each_field_to_seven_columns() {
        let c = Counts {
            lines: 1,
            words: 2,
            chars: 3,
        };
        assert_eq!(format!("{c}"), "      1       2       3");
    }

    #[test]
    fn selection_or_default_turns_on_everything_when_empty() {
        let s = Selection::default().or_default();
        assert!(s.lines && s.words && s.chars);
    }

    #[test]
    fn selection_render_respects_chosen_columns() {
        let c = Counts {
            lines: 4,
            words: 5,
            chars: 6,
        };
        let s = Selection {
            lines: true,
            words: false,
            chars: true,
        };
        assert_eq!(s.render(c), "      4       6");
    }
}

#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// `Counts::count` must agree with `str::chars().count()` for every UTF-8 input.
        #[test]
        fn chars_matches_string(s in ".{0,256}") {
            let c = Counts::count(&s);
            prop_assert_eq!(c.chars, s.chars().count());
        }

        /// Word count is bounded above by the number of whitespace tokens.
        #[test]
        fn words_le_split_whitespace(s in ".{0,256}") {
            let c = Counts::count(&s);
            prop_assert_eq!(c.words, s.split_whitespace().count());
        }
    }
}
