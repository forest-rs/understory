// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for the `understory_selection` crate.
//!
//! These exercises the core `Selection<T>` API, with a focus on how contents,
//! primary/anchor roles, and the revision counter interact.

use understory_selection::Selection;

#[test]
fn empty_selection_basics() {
    let sel = Selection::<u32>::new();
    assert!(sel.is_empty());
    assert_eq!(sel.len(), 0);
    assert_eq!(sel.primary(), None);
    assert_eq!(sel.anchor(), None);
    assert_eq!(sel.revision(), 0);
}

#[test]
fn select_only_sets_primary_anchor_and_bumps_revision() {
    let mut sel = Selection::new();
    assert!(sel.select_only(1));

    assert_eq!(sel.items(), &[1]);
    assert_eq!(sel.primary(), Some(&1));
    assert_eq!(sel.anchor(), Some(&1));
    assert_eq!(sel.revision(), 1);

    // No-op: selecting the same singleton again should not change revision.
    assert!(!sel.select_only(1));
    assert_eq!(sel.revision(), 1);
}

#[test]
fn clear_empties_and_bumps_revision_only_on_change() {
    let mut sel = Selection::new();
    assert!(!sel.clear());
    assert_eq!(sel.revision(), 0);

    assert!(sel.select_only(1));
    assert_eq!(sel.revision(), 1);

    assert!(sel.clear());
    assert!(sel.is_empty());
    assert_eq!(sel.primary(), None);
    assert_eq!(sel.anchor(), None);
    assert_eq!(sel.revision(), 2);
}

#[test]
fn replace_with_dedups_and_preserves_anchor_when_possible() {
    let mut sel = Selection::new();

    assert!(sel.replace_with([1, 2, 2, 3]));
    assert_eq!(sel.items(), &[1, 2, 3]);
    assert_eq!(sel.primary(), Some(&1));
    assert_eq!(sel.anchor(), Some(&1));

    // Set anchor explicitly to 2.
    assert!(sel.set_anchor(&2));
    let rev_after_anchor = sel.revision();
    assert_eq!(sel.anchor(), Some(&2));

    // Replace with a set that still contains 2: anchor should remain 2.
    assert!(sel.replace_with([2, 3, 4]));
    assert_eq!(sel.items(), &[2, 3, 4]);
    assert_eq!(sel.anchor(), Some(&2));
    assert!(sel.revision() > rev_after_anchor);

    // Replace with a set that does not contain the old anchor: anchor falls back to first.
    assert!(sel.replace_with([10, 11]));
    assert_eq!(sel.items(), &[10, 11]);
    assert_eq!(sel.primary(), Some(&10));
    assert_eq!(sel.anchor(), Some(&10));

    let rev_after_replace = sel.revision();
    assert!(!sel.replace_with([10, 11]));
    assert_eq!(sel.revision(), rev_after_replace);
}

#[test]
fn replace_with_roles_sets_contents_and_roles_in_one_revision() {
    let mut sel = Selection::new();
    assert!(sel.select_only(5));

    let rev_before = sel.revision();
    assert!(sel.replace_with_roles([10, 20, 20, 30], Some(&30), Some(&10)));

    assert_eq!(sel.items(), &[10, 20, 30]);
    assert_eq!(sel.primary(), Some(&30));
    assert_eq!(sel.anchor(), Some(&10));
    assert_eq!(sel.revision(), rev_before + 1);

    assert!(!sel.replace_with_roles([10, 20, 30], Some(&30), Some(&10)));
    assert_eq!(sel.revision(), rev_before + 1);
}

#[test]
fn replace_with_roles_clears_roles_that_are_absent_from_new_contents() {
    let mut sel = Selection::new();
    assert!(sel.replace_with_roles([1, 2, 3], Some(&3), Some(&1)));

    assert!(sel.replace_with_roles([4, 5], Some(&3), Some(&1)));
    assert_eq!(sel.items(), &[4, 5]);
    assert_eq!(sel.primary(), None);
    assert_eq!(sel.anchor(), None);
}

#[test]
fn replace_with_roles_treats_none_as_an_explicit_clear() {
    let mut sel = Selection::new();
    assert!(sel.replace_with([1, 2, 3]));

    assert!(sel.replace_with_roles([1, 2], None, Some(&2)));
    assert_eq!(sel.items(), &[1, 2]);
    assert_eq!(sel.primary(), None);
    assert_eq!(sel.anchor(), Some(&2));

    assert!(sel.replace_with_roles([1, 2], None, None));
    assert_eq!(sel.items(), &[1, 2]);
    assert_eq!(sel.primary(), None);
    assert_eq!(sel.anchor(), None);

    let rev_after_clear = sel.revision();
    assert!(!sel.replace_with_roles([1, 2], None, None));
    assert_eq!(sel.revision(), rev_after_clear);
}

#[test]
fn extend_with_adds_items_and_does_not_move_anchor() {
    let mut sel = Selection::new();
    assert!(sel.replace_with([1, 2]));
    assert!(!sel.set_anchor(&1));
    let rev_before = sel.revision();

    assert!(sel.extend_with([2, 3, 3, 4]));
    assert_eq!(sel.items(), &[1, 2, 3, 4]);
    assert_eq!(sel.anchor(), Some(&1));
    assert!(sel.revision() > rev_before);

    let rev_after_extend = sel.revision();
    assert!(!sel.extend_with([2, 3, 4]));
    assert_eq!(sel.revision(), rev_after_extend);
}

#[test]
fn add_and_remove_update_primary_and_revision() {
    let mut sel = Selection::new();
    assert!(sel.add(1));
    assert!(sel.add(2));
    assert_eq!(sel.items(), &[1, 2]);
    assert_eq!(sel.primary(), Some(&2));

    let rev_before = sel.revision();
    // Adding an already-selected key should only move primary.
    assert!(sel.add(1));
    assert_eq!(sel.primary(), Some(&1));
    assert!(sel.revision() > rev_before);

    let rev_before_duplicate_primary = sel.revision();
    assert!(!sel.add(1));
    assert_eq!(sel.revision(), rev_before_duplicate_primary);

    // Removing a non-existent key is a no-op.
    let rev_before_remove = sel.revision();
    assert!(!sel.remove(&99));
    assert_eq!(sel.revision(), rev_before_remove);

    // Removing an existing key updates contents and revision.
    assert!(sel.remove(&1));
    assert_eq!(sel.items(), &[2]);
    assert!(sel.revision() > rev_before_remove);
}

#[test]
fn toggle_adds_and_removes_with_revision() {
    let mut sel = Selection::new();

    assert!(sel.toggle(1));
    assert_eq!(sel.items(), &[1]);
    assert_eq!(sel.primary(), Some(&1));
    let rev_after_add = sel.revision();

    assert!(sel.toggle(1));
    assert!(sel.items().is_empty());
    assert!(sel.primary().is_none());
    assert!(sel.anchor().is_none());
    assert!(sel.revision() > rev_after_add);
}

#[test]
fn set_primary_and_anchor_are_noops_when_unchanged() {
    let mut sel = Selection::new();
    assert!(sel.replace_with([1, 2, 3]));
    assert!(sel.set_primary(&2));
    assert!(!sel.set_anchor(&1));
    let rev_after_init = sel.revision();

    // Setting to the same values should not bump revision.
    assert!(!sel.set_primary(&2));
    assert!(!sel.set_anchor(&1));
    assert!(!sel.set_primary(&99));
    assert!(!sel.set_anchor(&99));
    assert_eq!(sel.revision(), rev_after_init);
}

#[test]
fn clear_anchor_only_changes_when_anchor_is_some() {
    let mut sel = Selection::new();
    assert!(sel.replace_with([1, 2]));
    assert!(sel.set_anchor(&2));
    let rev_with_anchor = sel.revision();

    assert!(sel.clear_anchor());
    assert!(sel.anchor().is_none());
    assert!(sel.revision() > rev_with_anchor);

    let rev_without_anchor = sel.revision();
    assert!(!sel.clear_anchor());
    assert_eq!(sel.revision(), rev_without_anchor);
}
