//! Composable ECMAScript regular-expression matching policy.
//!
//! Playwright transports JavaScript `RegExp` source and flags across its
//! protocol and reconstructs a stateful expression in the Node driver. This
//! crate preserves the observable `RegExp.test()` subset without owning a
//! browser, runtime, or language binding.

use std::{fmt, sync::Mutex};

use regress::{Flags, Regex};
use thiserror::Error;

/// A compiled ECMAScript regular expression with JavaScript `test()` state.
pub struct EcmaScriptRegex {
    source: String,
    flags: String,
    regex: Regex,
    global: bool,
    sticky: bool,
    unicode_mode: bool,
    last_index: Mutex<usize>,
}

impl fmt::Debug for EcmaScriptRegex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EcmaScriptRegex")
            .field("source", &self.source)
            .field("flags", &self.flags)
            .field("last_index", &self.last_index())
            .finish_non_exhaustive()
    }
}

impl EcmaScriptRegex {
    /// Compiles JavaScript RegExp source and flags.
    ///
    /// Flags may arrive in any order because Playwright Python emits `ism`
    /// while JavaScript's `RegExp.prototype.flags` uses canonical order.
    pub fn compile(source: String, flags: String) -> Result<Self, RegExpError> {
        let parsed = ParsedFlags::parse(&flags)?;
        let mut engine_flags = Flags::from(flags.as_str());
        // `v` is a Unicode mode in JavaScript even though regress exposes the
        // parser switches independently.
        if parsed.unicode_sets {
            engine_flags.unicode = true;
        }
        let regex = if parsed.unicode || parsed.unicode_sets {
            Regex::with_flags(&source, engine_flags)
        } else {
            let engine_source = normalize_non_unicode_word_atoms(&source);
            Regex::from_unicode(
                non_unicode_pattern_units(&engine_source).into_iter(),
                engine_flags,
            )
        }
        .map_err(|error| RegExpError::InvalidSource(error.to_string()))?;
        Ok(Self {
            source,
            flags,
            regex,
            global: parsed.global,
            sticky: parsed.sticky,
            unicode_mode: parsed.unicode || parsed.unicode_sets,
            last_index: Mutex::new(0),
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn flags(&self) -> &str {
        &self.flags
    }

    /// Applies JavaScript `RegExp.prototype.test` semantics to one string.
    pub fn test(&self, value: &str) -> bool {
        let code_units = value.encode_utf16().collect::<Vec<_>>();
        self.test_utf16(&code_units)
    }

    /// Applies JavaScript `RegExp.prototype.test` semantics to UTF-16 code units.
    ///
    /// JavaScript strings may contain unpaired surrogates, which Rust `str`
    /// cannot represent. Protocol adapters that retain raw JavaScript string
    /// units can use this entry point without lossy UTF-8 replacement.
    pub fn test_utf16(&self, code_units: &[u16]) -> bool {
        if !self.global && !self.sticky {
            return self.find(code_units, 0, false).is_some();
        }

        let mut last_index = self
            .last_index
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let start = *last_index;
        let found = self.find(code_units, start, self.sticky);
        match found {
            Some(end) => {
                *last_index = end;
                true
            }
            None => {
                *last_index = 0;
                false
            }
        }
    }

    pub fn last_index(&self) -> usize {
        *self
            .last_index
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn find(&self, code_units: &[u16], start: usize, sticky: bool) -> Option<usize> {
        let matched = if self.unicode_mode {
            self.regex.find_from_utf16(code_units, start).next()
        } else {
            self.regex.find_from_ucs2(code_units, start).next()
        }?;
        let range = matched.range();
        if sticky && range.start != start {
            return None;
        }
        Some(range.end)
    }
}

fn normalize_non_unicode_word_atoms(source: &str) -> String {
    let characters = source.chars().collect::<Vec<_>>();
    let mut normalized = String::with_capacity(source.len());
    let mut in_class = false;
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character == '\\' && index + 1 < characters.len() {
            let escaped = characters[index + 1];
            if !in_class && matches!(escaped, 'w' | 'W') {
                if escaped == 'w' {
                    normalized.push_str("(?-i:[A-Za-z0-9_])");
                } else {
                    normalized.push_str("(?-i:[^A-Za-z0-9_])");
                }
            } else {
                normalized.push(character);
                normalized.push(escaped);
            }
            index += 2;
            continue;
        }
        if character == '[' {
            in_class = true;
        } else if character == ']' && in_class {
            in_class = false;
        }
        normalized.push(character);
        index += 1;
    }
    normalized
}

fn non_unicode_pattern_units(source: &str) -> Vec<u32> {
    let characters = source.chars().collect::<Vec<_>>();
    let mut units = Vec::with_capacity(source.len());
    let mut index = 0;
    while index < characters.len() {
        let named_capture = characters[index..].starts_with(&['(', '?', '<'])
            && characters
                .get(index + 3)
                .is_some_and(|character| !matches!(character, '=' | '!'));
        let named_backreference = characters[index..].starts_with(&['\\', 'k', '<']);
        if named_capture || named_backreference {
            units.extend(characters[index..index + 3].iter().copied().map(u32::from));
            index += 3;
            while index < characters.len() {
                let character = characters[index];
                units.push(u32::from(character));
                index += 1;
                if character == '>' {
                    break;
                }
            }
            continue;
        }
        if characters[index] == '\\' && index + 1 < characters.len() {
            units.push(u32::from(characters[index]));
            index += 1;
            append_utf16_units(characters[index], &mut units);
            index += 1;
            continue;
        }
        append_utf16_units(characters[index], &mut units);
        index += 1;
    }
    units
}

fn append_utf16_units(character: char, units: &mut Vec<u32>) {
    let mut encoded = [0_u16; 2];
    units.extend(
        character
            .encode_utf16(&mut encoded)
            .iter()
            .copied()
            .map(u32::from),
    );
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ParsedFlags {
    global: bool,
    sticky: bool,
    unicode: bool,
    unicode_sets: bool,
}

impl ParsedFlags {
    fn parse(flags: &str) -> Result<Self, RegExpError> {
        let mut parsed = Self::default();
        let mut seen = 0_u16;
        for flag in flags.chars() {
            let (bit, destination) = match flag {
                'd' => (1, None),
                'g' => (2, Some(&mut parsed.global)),
                'i' => (4, None),
                'm' => (8, None),
                's' => (16, None),
                'u' => (32, Some(&mut parsed.unicode)),
                'v' => (64, Some(&mut parsed.unicode_sets)),
                'y' => (128, Some(&mut parsed.sticky)),
                unsupported => return Err(RegExpError::UnsupportedFlag(unsupported)),
            };
            if seen & bit != 0 {
                return Err(RegExpError::DuplicateFlag(flag));
            }
            seen |= bit;
            if let Some(destination) = destination {
                *destination = true;
            }
        }
        if parsed.unicode && parsed.unicode_sets {
            return Err(RegExpError::ConflictingUnicodeModes);
        }
        Ok(parsed)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RegExpError {
    #[error("unsupported ECMAScript RegExp flag {0:?}")]
    UnsupportedFlag(char),
    #[error("duplicate ECMAScript RegExp flag {0:?}")]
    DuplicateFlag(char),
    #[error("ECMAScript RegExp flags 'u' and 'v' are mutually exclusive")]
    ConflictingUnicodeModes,
    #[error("invalid ECMAScript RegExp source: {0}")]
    InvalidSource(String),
}

#[cfg(test)]
mod tests {
    use super::{EcmaScriptRegex, RegExpError};

    #[test]
    fn lookaround_and_backreferences_match_ecmascript_sources() {
        // Arrange
        let lookaround = EcmaScriptRegex::compile(r"(?<=api/)v(?=2)".into(), String::new())
            .expect("lookaround should compile");
        let backreference = EcmaScriptRegex::compile(r"([a-z])\1".into(), String::new())
            .expect("backreference should compile");

        // Act
        let lookaround_match = lookaround.test("https://example.test/api/v2");
        let backreference_match = backreference.test("https://example.test/book");

        // Assert
        assert!(lookaround_match);
        assert!(backreference_match);
    }

    #[test]
    fn global_test_updates_and_resets_utf16_last_index() {
        // Arrange
        let pattern = EcmaScriptRegex::compile("item".into(), "dg".into())
            .expect("global expression should compile");

        // Act
        let first = pattern.test("item-item");
        let first_index = pattern.last_index();
        let second = pattern.test("item-item");
        let second_index = pattern.last_index();
        let third = pattern.test("item-item");

        // Assert
        assert!(first);
        assert_eq!(first_index, 4);
        assert!(second);
        assert_eq!(second_index, 9);
        assert!(!third);
        assert_eq!(pattern.last_index(), 0);
    }

    #[test]
    fn sticky_test_requires_a_match_at_last_index() {
        // Arrange
        let pattern = EcmaScriptRegex::compile("item".into(), "y".into())
            .expect("sticky expression should compile");

        // Act
        let first = pattern.test("item-xitem");
        let second = pattern.test("item-xitem");

        // Assert
        assert!(first);
        assert!(!second);
        assert_eq!(pattern.last_index(), 0);
    }

    #[test]
    fn unicode_flag_changes_surrogate_pair_matching() {
        // Arrange
        let code_unit = EcmaScriptRegex::compile("^.$".into(), String::new())
            .expect("non-Unicode expression should compile");
        let code_point = EcmaScriptRegex::compile("^.$".into(), "u".into())
            .expect("Unicode expression should compile");

        // Act
        let code_unit_match = code_unit.test("😀");
        let code_point_match = code_point.test("😀");

        // Assert
        assert!(!code_unit_match);
        assert!(code_point_match);
    }

    #[test]
    fn non_unicode_astral_literal_matches_its_utf16_surrogate_sequence() {
        // Arrange
        let pattern = EcmaScriptRegex::compile("𠮷".into(), String::new())
            .expect("astral literal should compile");

        // Act
        let matched = pattern.test("𠮷");

        // Assert
        assert!(matched);
    }

    #[test]
    fn non_unicode_named_group_identifiers_stay_unicode_scalars() {
        // Arrange
        let pattern = EcmaScriptRegex::compile(r"(?<𝓓>dog)-\k<𝓓>".into(), String::new())
            .expect("astral named group should compile");

        // Act
        let matched = pattern.test("dog-dog");

        // Assert
        assert!(matched);
    }

    #[test]
    fn non_unicode_ignore_case_word_atoms_remain_ascii() {
        // Arrange
        let global = EcmaScriptRegex::compile(r"^\w$".into(), "i".into())
            .expect("global ignore-case word expression should compile");
        let modifier = EcmaScriptRegex::compile(r"^(?i:\w)$".into(), String::new())
            .expect("modifier word expression should compile");
        let unicode = EcmaScriptRegex::compile(r"^\w$".into(), "iu".into())
            .expect("Unicode ignore-case word expression should compile");

        // Act
        let global_long_s = global.test("ſ");
        let modifier_kelvin = modifier.test("K");
        let unicode_long_s = unicode.test("ſ");

        // Assert
        assert!(!global_long_s);
        assert!(!modifier_kelvin);
        assert!(unicode_long_s);
    }

    #[test]
    fn utf16_input_preserves_unpaired_surrogates() {
        // Arrange
        let pattern = EcmaScriptRegex::compile(r"^\uD800$".into(), String::new())
            .expect("surrogate expression should compile");

        // Act
        let matched = pattern.test_utf16(&[0xD800]);

        // Assert
        assert!(matched);
    }

    #[test]
    fn unicode_sets_flag_supports_ecmascript_class_intersection() {
        // Arrange
        let pattern = EcmaScriptRegex::compile(r"[\p{ASCII}&&\p{Letter}]".into(), "v".into())
            .expect("Unicode-sets expression should compile");

        // Act
        let ascii_letter = pattern.test("A");
        let non_ascii_letter = pattern.test("é");
        let ascii_digit = pattern.test("1");

        // Assert
        assert!(ascii_letter);
        assert!(!non_ascii_letter);
        assert!(!ascii_digit);
    }

    #[test]
    fn invalid_flags_and_sources_fail_explicitly() {
        // Arrange
        let duplicate_flags = EcmaScriptRegex::compile("a".into(), "ii".into());
        let conflicting_modes = EcmaScriptRegex::compile("a".into(), "uv".into());
        let unsupported_flag = EcmaScriptRegex::compile("a".into(), "x".into());
        let invalid_source = EcmaScriptRegex::compile("(".into(), String::new());

        // Act / Assert
        assert_eq!(
            duplicate_flags.expect_err("duplicate flags must fail"),
            RegExpError::DuplicateFlag('i')
        );
        assert_eq!(
            conflicting_modes.expect_err("conflicting Unicode modes must fail"),
            RegExpError::ConflictingUnicodeModes
        );
        assert_eq!(
            unsupported_flag.expect_err("unsupported flags must fail"),
            RegExpError::UnsupportedFlag('x')
        );
        assert!(matches!(invalid_source, Err(RegExpError::InvalidSource(_))));
    }
}
