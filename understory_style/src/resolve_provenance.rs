// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::matcher::WinningStyleSource;
use crate::resolve::ResolvedSource;
use crate::{ResourceKey, StyleExpressionId, StyleOrigin};
use crate::{Selector, Specificity};

#[derive(Clone, Debug)]
pub(super) enum CascadeSourceMetadata {
    Direct {
        origin: StyleOrigin,
        source_index: usize,
    },
    Rule {
        origin: StyleOrigin,
        selector: Selector,
        specificity: Specificity,
        source_index: usize,
        order: u32,
    },
}

impl CascadeSourceMetadata {
    pub(super) fn into_resolved_source(
        self,
        resource: Option<ResourceKey>,
        expression: Option<StyleExpressionId>,
    ) -> ResolvedSource {
        match self {
            Self::Direct {
                origin,
                source_index,
            } => ResolvedSource::CascadeDirect {
                origin,
                source_index,
                resource,
                expression,
            },
            Self::Rule {
                origin,
                selector,
                specificity,
                source_index,
                order,
            } => ResolvedSource::CascadeRule {
                origin,
                selector,
                specificity,
                source_index,
                order,
                resource,
                expression,
            },
        }
    }
}

pub(super) fn source_metadata(source: &WinningStyleSource<'_>) -> CascadeSourceMetadata {
    match source {
        WinningStyleSource::Direct {
            origin,
            source_index,
            ..
        } => CascadeSourceMetadata::Direct {
            origin: *origin,
            source_index: *source_index,
        },
        WinningStyleSource::Rule(rule) => CascadeSourceMetadata::Rule {
            origin: rule.origin(),
            selector: rule.selector().clone(),
            specificity: rule.selector().specificity(),
            source_index: rule.source_index(),
            order: rule.order(),
        },
    }
}
