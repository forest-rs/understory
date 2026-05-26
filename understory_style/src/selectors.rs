// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Selector construction helpers for common style subject patterns.
//!
//! This module owns generic selector authoring conveniences; it explicitly does
//! not own application-specific type, part, class, or pseudoclass vocabularies.
//! Hosts define those IDs and pass them into these helpers.

use crate::{ClassId, PartTag, PseudoClassId, Selector, SelectorStep, TypeTag};

/// Matches any subject with `type_tag`.
#[must_use]
pub fn type_tag(type_tag: TypeTag) -> Selector {
    Selector::single(SelectorStep::type_tag(type_tag))
}

/// Matches a typed subject with `pseudo`.
///
/// This is useful for stateful style subjects such as `Button:hover` or
/// `Toggle:checked`, while keeping the pseudoclass vocabulary host-defined.
#[must_use]
pub fn type_state(type_tag: TypeTag, pseudo: PseudoClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(type_tag).with_pseudo(pseudo))
}

/// Matches an immediate owner-local part: `owner > part`.
///
/// `PartTag` values are host-defined and commonly reused by unrelated owners,
/// so anchoring parts under a [`TypeTag`] keeps selectors owner-local.
#[must_use]
pub fn part(owner: TypeTag, part: PartTag) -> Selector {
    Selector::child(SelectorStep::type_tag(owner), SelectorStep::part_tag(part))
}

/// Matches an immediate owner-local classified part: `owner > part.class`.
#[must_use]
pub fn part_with_class(owner: TypeTag, part: PartTag, class: ClassId) -> Selector {
    Selector::child(
        SelectorStep::type_tag(owner),
        SelectorStep::part_tag(part).with_class(class),
    )
}

/// Matches an immediate part below a pre-built owner step: `owner_step > part`.
///
/// Use this when the owner step carries additional class or pseudoclass
/// requirements.
#[must_use]
pub fn part_when(owner_step: SelectorStep, part: PartTag) -> Selector {
    Selector::child(owner_step, SelectorStep::part_tag(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_state_matches_typed_pseudo_subject() {
        let selector = type_state(TypeTag(1), PseudoClassId(2));
        let step = &selector.steps()[0];

        assert_eq!(selector.len(), 1);
        assert_eq!(step.type_tag, Some(TypeTag(1)));
        assert!(step.required_pseudos.as_slice().contains(&PseudoClassId(2)));
    }

    #[test]
    fn part_with_class_anchors_classified_part_under_owner() {
        let selector = part_with_class(TypeTag(1), PartTag(2), ClassId(3));

        assert_eq!(selector.len(), 2);
        assert_eq!(selector.steps()[0], SelectorStep::type_tag(TypeTag(1)));
        assert_eq!(selector.steps()[1].part_tag, Some(PartTag(2)));
        assert!(
            selector.steps()[1]
                .required_classes
                .as_slice()
                .contains(&ClassId(3))
        );
    }

    #[test]
    fn part_when_preserves_owner_step_requirements() {
        let owner = SelectorStep::type_tag(TypeTag(1)).with_pseudo(PseudoClassId(2));
        let selector = part_when(owner.clone(), PartTag(3));

        assert_eq!(selector.steps()[0], owner);
        assert_eq!(selector.steps()[1], SelectorStep::part_tag(PartTag(3)));
    }
}
