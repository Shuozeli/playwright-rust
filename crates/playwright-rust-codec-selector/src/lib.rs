//! Generated selector wire values shared by independently installable codecs.
//!
//! This crate has no command root, capability, transport, runtime, browser
//! backend, YAML parser, or build script.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU32;
use std::str::FromStr;

use playwright_rust_selector_model::{
    AriaRole, AriaTriState, CssSelector, RegexMatcher, RoleOptions, RolePatternSelector,
    RoleSelector, Selector, TestIdAttributes, TextMatcher, UserFacingAttribute,
};

#[doc(hidden)]
#[rustfmt::skip]
mod generated_schema;

pub use generated_schema::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorModelDecodeError(String);

impl SelectorModelDecodeError {
    fn new(error: impl Display) -> Self {
        Self(error.to_string())
    }
}

impl Display for SelectorModelDecodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SelectorModelDecodeError {}

/// Borrows one validated semantic selector into the generated reusable wire
/// model without copying its strings.
pub fn selector_to_wire(selector: &Selector) -> PlaywrightRustLocatorSelectorRef<'_> {
    match selector {
        Selector::Css(value) => PlaywrightRustLocatorSelectorRef::Css {
            value: value.as_str(),
        },
        Selector::Nth { base, index } => PlaywrightRustLocatorSelectorRef::Nth {
            base: Box::new(selector_to_wire(base)),
            index: *index,
        },
        Selector::FilterText { base, text, exact } => {
            PlaywrightRustLocatorSelectorRef::FilterText {
                base: Box::new(selector_to_wire(base)),
                text,
                exact: *exact,
            }
        }
        Selector::And { left, right } => PlaywrightRustLocatorSelectorRef::And {
            left: Box::new(selector_to_wire(left)),
            right: Box::new(selector_to_wire(right)),
        },
        Selector::Or { left, right } => PlaywrightRustLocatorSelectorRef::Or {
            left: Box::new(selector_to_wire(left)),
            right: Box::new(selector_to_wire(right)),
        },
        Selector::Text(matcher) => PlaywrightRustLocatorSelectorRef::Text {
            matcher: text_matcher_to_wire(matcher),
        },
        Selector::LegacyText(matcher) => PlaywrightRustLocatorSelectorRef::LegacyText {
            matcher: text_matcher_to_wire(matcher),
        },
        Selector::Label(matcher) => PlaywrightRustLocatorSelectorRef::Label {
            matcher: text_matcher_to_wire(matcher),
        },
        Selector::Role(selector) => PlaywrightRustLocatorSelectorRef::Role {
            selector: role_selector_to_wire(selector),
        },
        Selector::Within { base, descendant } => PlaywrightRustLocatorSelectorRef::Within {
            base: Box::new(selector_to_wire(base)),
            descendant: Box::new(selector_to_wire(descendant)),
        },
        Selector::FilterNotText { base, text, exact } => {
            PlaywrightRustLocatorSelectorRef::FilterNotText {
                base: Box::new(selector_to_wire(base)),
                text,
                exact: *exact,
            }
        }
        Selector::FilterHas { base, descendant } => PlaywrightRustLocatorSelectorRef::FilterHas {
            base: Box::new(selector_to_wire(base)),
            descendant: Box::new(selector_to_wire(descendant)),
        },
        Selector::FilterHasNot { base, descendant } => {
            PlaywrightRustLocatorSelectorRef::FilterHasNot {
                base: Box::new(selector_to_wire(base)),
                descendant: Box::new(selector_to_wire(descendant)),
            }
        }
        Selector::FilterVisible { base, visible } => {
            PlaywrightRustLocatorSelectorRef::FilterVisible {
                base: Box::new(selector_to_wire(base)),
                visible: *visible,
            }
        }
        Selector::TextRegex(matcher) => PlaywrightRustLocatorSelectorRef::TextRegex {
            matcher: regex_matcher_to_wire(matcher),
        },
        Selector::LabelRegex(matcher) => PlaywrightRustLocatorSelectorRef::LabelRegex {
            matcher: regex_matcher_to_wire(matcher),
        },
        Selector::FilterTextRegex { base, matcher } => {
            PlaywrightRustLocatorSelectorRef::FilterTextRegex {
                base: Box::new(selector_to_wire(base)),
                matcher: regex_matcher_to_wire(matcher),
            }
        }
        Selector::FilterNotTextRegex { base, matcher } => {
            PlaywrightRustLocatorSelectorRef::FilterNotTextRegex {
                base: Box::new(selector_to_wire(base)),
                matcher: regex_matcher_to_wire(matcher),
            }
        }
        Selector::RolePattern(selector) => PlaywrightRustLocatorSelectorRef::RolePattern {
            selector: role_pattern_selector_to_wire(selector),
        },
        Selector::AttributeText { attribute, matcher } => {
            PlaywrightRustLocatorSelectorRef::AttributeText {
                attribute: user_facing_attribute_to_wire(*attribute),
                matcher: text_matcher_to_wire(matcher),
            }
        }
        Selector::AttributeRegex { attribute, matcher } => {
            PlaywrightRustLocatorSelectorRef::AttributeRegex {
                attribute: user_facing_attribute_to_wire(*attribute),
                matcher: regex_matcher_to_wire(matcher),
            }
        }
        Selector::TestIdText { attributes, value } => {
            PlaywrightRustLocatorSelectorRef::TestIdText {
                attributes: attributes.as_csv(),
                value,
            }
        }
        Selector::TestIdRegex {
            attributes,
            matcher,
        } => PlaywrightRustLocatorSelectorRef::TestIdRegex {
            attributes: attributes.as_csv(),
            matcher: regex_matcher_to_wire(matcher),
        },
    }
}

/// Validates and reconstructs the portable selector model after bounded wire
/// decoding.
pub fn selector_from_wire(
    selector: PlaywrightRustLocatorSelector,
) -> Result<Selector, SelectorModelDecodeError> {
    match selector {
        PlaywrightRustLocatorSelector::Css { value } => CssSelector::new(value)
            .map(Selector::Css)
            .map_err(SelectorModelDecodeError::new),
        PlaywrightRustLocatorSelector::Nth { base, index } => Ok(Selector::Nth {
            base: Box::new(selector_from_wire(*base)?),
            index,
        }),
        PlaywrightRustLocatorSelector::FilterText { base, text, exact } => {
            Ok(Selector::FilterText {
                base: Box::new(selector_from_wire(*base)?),
                text,
                exact,
            })
        }
        PlaywrightRustLocatorSelector::And { left, right } => Ok(Selector::And {
            left: Box::new(selector_from_wire(*left)?),
            right: Box::new(selector_from_wire(*right)?),
        }),
        PlaywrightRustLocatorSelector::Or { left, right } => Ok(Selector::Or {
            left: Box::new(selector_from_wire(*left)?),
            right: Box::new(selector_from_wire(*right)?),
        }),
        PlaywrightRustLocatorSelector::Text { matcher } => {
            Ok(Selector::Text(text_matcher_from_wire(matcher)))
        }
        PlaywrightRustLocatorSelector::LegacyText { matcher } => {
            Ok(Selector::LegacyText(text_matcher_from_wire(matcher)))
        }
        PlaywrightRustLocatorSelector::Label { matcher } => {
            Ok(Selector::Label(text_matcher_from_wire(matcher)))
        }
        PlaywrightRustLocatorSelector::Role { selector } => {
            role_selector_from_wire(selector).map(Selector::Role)
        }
        PlaywrightRustLocatorSelector::Within { base, descendant } => Ok(Selector::Within {
            base: Box::new(selector_from_wire(*base)?),
            descendant: Box::new(selector_from_wire(*descendant)?),
        }),
        PlaywrightRustLocatorSelector::FilterNotText { base, text, exact } => {
            Ok(Selector::FilterNotText {
                base: Box::new(selector_from_wire(*base)?),
                text,
                exact,
            })
        }
        PlaywrightRustLocatorSelector::FilterHas { base, descendant } => Ok(Selector::FilterHas {
            base: Box::new(selector_from_wire(*base)?),
            descendant: Box::new(selector_from_wire(*descendant)?),
        }),
        PlaywrightRustLocatorSelector::FilterHasNot { base, descendant } => {
            Ok(Selector::FilterHasNot {
                base: Box::new(selector_from_wire(*base)?),
                descendant: Box::new(selector_from_wire(*descendant)?),
            })
        }
        PlaywrightRustLocatorSelector::FilterVisible { base, visible } => {
            Ok(Selector::FilterVisible {
                base: Box::new(selector_from_wire(*base)?),
                visible,
            })
        }
        PlaywrightRustLocatorSelector::TextRegex { matcher } => {
            regex_matcher_from_wire(matcher).map(Selector::TextRegex)
        }
        PlaywrightRustLocatorSelector::LabelRegex { matcher } => {
            regex_matcher_from_wire(matcher).map(Selector::LabelRegex)
        }
        PlaywrightRustLocatorSelector::FilterTextRegex { base, matcher } => {
            Ok(Selector::FilterTextRegex {
                base: Box::new(selector_from_wire(*base)?),
                matcher: regex_matcher_from_wire(matcher)?,
            })
        }
        PlaywrightRustLocatorSelector::FilterNotTextRegex { base, matcher } => {
            Ok(Selector::FilterNotTextRegex {
                base: Box::new(selector_from_wire(*base)?),
                matcher: regex_matcher_from_wire(matcher)?,
            })
        }
        PlaywrightRustLocatorSelector::RolePattern { selector } => {
            role_pattern_selector_from_wire(selector).map(Selector::RolePattern)
        }
        PlaywrightRustLocatorSelector::AttributeText { attribute, matcher } => {
            Ok(Selector::AttributeText {
                attribute: user_facing_attribute_from_wire(attribute),
                matcher: text_matcher_from_wire(matcher),
            })
        }
        PlaywrightRustLocatorSelector::AttributeRegex { attribute, matcher } => {
            Ok(Selector::AttributeRegex {
                attribute: user_facing_attribute_from_wire(attribute),
                matcher: regex_matcher_from_wire(matcher)?,
            })
        }
        PlaywrightRustLocatorSelector::TestIdText { attributes, value } => {
            Ok(Selector::TestIdText {
                attributes: TestIdAttributes::new(attributes)
                    .map_err(SelectorModelDecodeError::new)?,
                value,
            })
        }
        PlaywrightRustLocatorSelector::TestIdRegex {
            attributes,
            matcher,
        } => Ok(Selector::TestIdRegex {
            attributes: TestIdAttributes::new(attributes).map_err(SelectorModelDecodeError::new)?,
            matcher: regex_matcher_from_wire(matcher)?,
        }),
    }
}

fn text_matcher_to_wire(matcher: &TextMatcher) -> PlaywrightRustLocatorTextMatcherRef<'_> {
    PlaywrightRustLocatorTextMatcherRef {
        value: matcher.value(),
        exact: matcher.exact(),
    }
}

fn text_matcher_from_wire(matcher: PlaywrightRustLocatorTextMatcher) -> TextMatcher {
    TextMatcher::new(matcher.value, matcher.exact)
}

fn regex_matcher_to_wire(matcher: &RegexMatcher) -> PlaywrightRustLocatorRegexMatcherRef<'_> {
    PlaywrightRustLocatorRegexMatcherRef {
        source: matcher.source(),
        flags: matcher.flags(),
    }
}

fn regex_matcher_from_wire(
    matcher: PlaywrightRustLocatorRegexMatcher,
) -> Result<RegexMatcher, SelectorModelDecodeError> {
    RegexMatcher::new(matcher.source, matcher.flags).map_err(SelectorModelDecodeError::new)
}

fn role_selector_to_wire(selector: &RoleSelector) -> PlaywrightRustLocatorRoleSelectorRef<'_> {
    let options = selector.options();
    PlaywrightRustLocatorRoleSelectorRef {
        role: selector.role().as_str(),
        name: options.name.as_ref().map(text_matcher_to_wire),
        description: options.description.as_ref().map(text_matcher_to_wire),
        checked: optional_tri_state_to_wire(options.checked),
        disabled: optional_bool_to_wire(options.disabled),
        expanded: optional_bool_to_wire(options.expanded),
        include_hidden: options.include_hidden,
        level: options.level.map(NonZeroU32::get),
        pressed: optional_tri_state_to_wire(options.pressed),
        selected: optional_bool_to_wire(options.selected),
    }
}

fn role_pattern_selector_to_wire(
    selector: &RolePatternSelector,
) -> PlaywrightRustLocatorRolePatternSelectorRef<'_> {
    let options = selector.options();
    PlaywrightRustLocatorRolePatternSelectorRef {
        role: selector.role().as_str(),
        name: text_pattern_to_wire(options.name.as_ref(), options.name_regex.as_ref()),
        description: text_pattern_to_wire(
            options.description.as_ref(),
            options.description_regex.as_ref(),
        ),
        checked: optional_tri_state_to_wire(options.checked),
        disabled: optional_bool_to_wire(options.disabled),
        expanded: optional_bool_to_wire(options.expanded),
        include_hidden: options.include_hidden,
        level: options.level.map(NonZeroU32::get),
        pressed: optional_tri_state_to_wire(options.pressed),
        selected: optional_bool_to_wire(options.selected),
    }
}

fn text_pattern_to_wire<'a>(
    text: Option<&'a TextMatcher>,
    regex: Option<&'a RegexMatcher>,
) -> Option<PlaywrightRustLocatorTextPatternRef<'a>> {
    match (text, regex) {
        (Some(matcher), None) => Some(PlaywrightRustLocatorTextPatternRef::String {
            matcher: text_matcher_to_wire(matcher),
        }),
        (None, Some(matcher)) => Some(PlaywrightRustLocatorTextPatternRef::Regex {
            matcher: regex_matcher_to_wire(matcher),
        }),
        (None, None) => None,
        (Some(_), Some(_)) => unreachable!("validated role options choose one matcher kind"),
    }
}

fn role_selector_from_wire(
    selector: PlaywrightRustLocatorRoleSelector,
) -> Result<RoleSelector, SelectorModelDecodeError> {
    let role = AriaRole::from_str(&selector.role).map_err(SelectorModelDecodeError::new)?;
    let options = RoleOptions {
        name: selector.name.map(text_matcher_from_wire),
        description: selector.description.map(text_matcher_from_wire),
        name_regex: None,
        description_regex: None,
        checked: optional_tri_state_from_wire(selector.checked),
        disabled: optional_bool_from_wire(selector.disabled),
        expanded: optional_bool_from_wire(selector.expanded),
        include_hidden: selector.include_hidden,
        level: optional_level_from_wire(selector.level)?,
        pressed: optional_tri_state_from_wire(selector.pressed),
        selected: optional_bool_from_wire(selector.selected),
    };
    RoleSelector::new(role, options).map_err(SelectorModelDecodeError::new)
}

fn role_pattern_selector_from_wire(
    selector: PlaywrightRustLocatorRolePatternSelector,
) -> Result<RolePatternSelector, SelectorModelDecodeError> {
    let role = AriaRole::from_str(&selector.role).map_err(SelectorModelDecodeError::new)?;
    let (name, name_regex) = text_pattern_from_wire(selector.name)?;
    let (description, description_regex) = text_pattern_from_wire(selector.description)?;
    let options = RoleOptions {
        name,
        description,
        name_regex,
        description_regex,
        checked: optional_tri_state_from_wire(selector.checked),
        disabled: optional_bool_from_wire(selector.disabled),
        expanded: optional_bool_from_wire(selector.expanded),
        include_hidden: selector.include_hidden,
        level: optional_level_from_wire(selector.level)?,
        pressed: optional_tri_state_from_wire(selector.pressed),
        selected: optional_bool_from_wire(selector.selected),
    };
    RolePatternSelector::new(role, options).map_err(SelectorModelDecodeError::new)
}

fn text_pattern_from_wire(
    pattern: Option<PlaywrightRustLocatorTextPattern>,
) -> Result<(Option<TextMatcher>, Option<RegexMatcher>), SelectorModelDecodeError> {
    match pattern {
        None => Ok((None, None)),
        Some(PlaywrightRustLocatorTextPattern::String { matcher }) => {
            Ok((Some(text_matcher_from_wire(matcher)), None))
        }
        Some(PlaywrightRustLocatorTextPattern::Regex { matcher }) => {
            Ok((None, Some(regex_matcher_from_wire(matcher)?)))
        }
    }
}

fn optional_level_from_wire(
    level: Option<u32>,
) -> Result<Option<NonZeroU32>, SelectorModelDecodeError> {
    level
        .map(|value| {
            NonZeroU32::new(value)
                .ok_or_else(|| SelectorModelDecodeError("ARIA level must be non-zero".to_string()))
        })
        .transpose()
}

const fn optional_bool_to_wire(value: Option<bool>) -> PlaywrightRustLocatorOptionalBool {
    match value {
        None => PlaywrightRustLocatorOptionalBool::Absent,
        Some(false) => PlaywrightRustLocatorOptionalBool::False,
        Some(true) => PlaywrightRustLocatorOptionalBool::True,
    }
}

const fn optional_bool_from_wire(value: PlaywrightRustLocatorOptionalBool) -> Option<bool> {
    match value {
        PlaywrightRustLocatorOptionalBool::Absent => None,
        PlaywrightRustLocatorOptionalBool::False => Some(false),
        PlaywrightRustLocatorOptionalBool::True => Some(true),
    }
}

const fn optional_tri_state_to_wire(
    value: Option<AriaTriState>,
) -> PlaywrightRustLocatorOptionalAriaTriState {
    match value {
        None => PlaywrightRustLocatorOptionalAriaTriState::Absent,
        Some(AriaTriState::False) => PlaywrightRustLocatorOptionalAriaTriState::False,
        Some(AriaTriState::True) => PlaywrightRustLocatorOptionalAriaTriState::True,
        Some(AriaTriState::Mixed) => PlaywrightRustLocatorOptionalAriaTriState::Mixed,
    }
}

const fn optional_tri_state_from_wire(
    value: PlaywrightRustLocatorOptionalAriaTriState,
) -> Option<AriaTriState> {
    match value {
        PlaywrightRustLocatorOptionalAriaTriState::Absent => None,
        PlaywrightRustLocatorOptionalAriaTriState::False => Some(AriaTriState::False),
        PlaywrightRustLocatorOptionalAriaTriState::True => Some(AriaTriState::True),
        PlaywrightRustLocatorOptionalAriaTriState::Mixed => Some(AriaTriState::Mixed),
    }
}

const fn user_facing_attribute_to_wire(
    value: UserFacingAttribute,
) -> PlaywrightRustLocatorUserFacingAttribute {
    match value {
        UserFacingAttribute::Placeholder => PlaywrightRustLocatorUserFacingAttribute::Placeholder,
        UserFacingAttribute::Alt => PlaywrightRustLocatorUserFacingAttribute::Alt,
        UserFacingAttribute::Title => PlaywrightRustLocatorUserFacingAttribute::Title,
    }
}

const fn user_facing_attribute_from_wire(
    value: PlaywrightRustLocatorUserFacingAttribute,
) -> UserFacingAttribute {
    match value {
        PlaywrightRustLocatorUserFacingAttribute::Placeholder => UserFacingAttribute::Placeholder,
        PlaywrightRustLocatorUserFacingAttribute::Alt => UserFacingAttribute::Alt,
        PlaywrightRustLocatorUserFacingAttribute::Title => UserFacingAttribute::Title,
    }
}

#[cfg(test)]
mod tests {
    use playwright_rust_codec_binary::{
        BinaryDecode, BinaryEncode, BinaryReader, BinaryValueContext, BinaryValueLimits,
        BinaryWriter,
    };
    use playwright_rust_codec_locator::{LOCATOR_SCHEMA_CATALOG, LOCATOR_V2_SCHEMA_CATALOG};
    use playwright_rust_selector_model::{AriaRole, RegexMatcher, RoleOptions, Selector};

    use super::{CATALOG, PlaywrightRustLocatorSelector, selector_from_wire, selector_to_wire};

    #[test]
    fn selector_library_is_valid_and_matches_published_locator_catalog_identities() {
        // Arrange
        let v1 = LOCATOR_SCHEMA_CATALOG.types();
        let v2 = LOCATOR_V2_SCHEMA_CATALOG.types();

        // Act
        // Assert
        assert_eq!(CATALOG.validate(), Ok(()));
        assert_eq!(CATALOG.types().len(), 9);
        for selector_type in CATALOG.types() {
            let v1_type = v1
                .iter()
                .find(|candidate| candidate.id() == selector_type.id())
                .unwrap_or_else(|| panic!("locator 1.x is missing {}", selector_type.id()));
            assert_eq!(v1_type.kind(), selector_type.kind());
            assert_eq!(
                v1_type.layout_fingerprint(),
                selector_type.layout_fingerprint()
            );

            let v2_type = v2
                .iter()
                .find(|candidate| candidate.id() == selector_type.id())
                .unwrap_or_else(|| panic!("locator 2.x is missing {}", selector_type.id()));
            assert_eq!(v2_type.kind(), selector_type.kind());
        }
    }

    #[test]
    fn semantic_selector_round_trips_through_the_generated_library_codec() {
        // Arrange
        let selector = Selector::role(
            AriaRole::Button,
            RoleOptions::new().with_name_regex(
                RegexMatcher::new("save|submit", "i").expect("regex should validate"),
            ),
        )
        .expect("role selector should validate")
        .filter_visible(true)
        .nth(2);
        let limits = BinaryValueLimits::new(1_024, 0, 0, 64, 128);
        let mut writer = BinaryWriter::new();
        let mut encode_context = BinaryValueContext::new(limits);

        // Act
        selector_to_wire(&selector)
            .encode_binary(&mut writer, &mut encode_context)
            .expect("selector should encode");
        let bytes = writer.into_bytes();
        let mut reader = BinaryReader::new(&bytes);
        let mut decode_context = BinaryValueContext::new(limits);
        let wire = PlaywrightRustLocatorSelector::decode_binary(&mut reader, &mut decode_context)
            .expect("selector should decode");
        let decoded = selector_from_wire(wire).expect("semantic selector should validate");

        // Assert
        reader.finish().expect("selector bytes should be exhausted");
        assert_eq!(decoded, selector);
    }

    #[test]
    fn legacy_text_selector_round_trips_without_becoming_semantic_text() {
        // Arrange
        let selector = Selector::legacy_text("save");
        let limits = BinaryValueLimits::new(1_024, 0, 0, 64, 128);
        let mut writer = BinaryWriter::new();
        let mut encode_context = BinaryValueContext::new(limits);

        // Act
        selector_to_wire(&selector)
            .encode_binary(&mut writer, &mut encode_context)
            .expect("legacy text selector should encode");
        let bytes = writer.into_bytes();
        let mut reader = BinaryReader::new(&bytes);
        let mut decode_context = BinaryValueContext::new(limits);
        let wire = PlaywrightRustLocatorSelector::decode_binary(&mut reader, &mut decode_context)
            .expect("legacy text selector should decode");
        let decoded = selector_from_wire(wire).expect("legacy text selector should validate");

        // Assert
        reader
            .finish()
            .expect("legacy text selector bytes should be exhausted");
        assert_eq!(decoded, selector);
        assert!(matches!(decoded, Selector::LegacyText(_)));
    }
}
