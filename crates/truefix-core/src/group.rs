//! Repeating-group builder.
//!
//! A group is a count tag (NoXxx) plus ordered entries; each entry is a [`FieldMap`] whose
//! first field is the group delimiter. Dictionary-driven *parsing* of groups from a flat
//! wire message is added in `truefix-dict` (Stage S4); this type lets callers *build* and
//! *encode* groups (including nested ones) today.

use crate::field_map::FieldMap;

/// Supplies repeating-group structure to the dictionary-driven decoder without `truefix-core`
/// depending on `truefix-dict` (the dictionary implements this trait). See [`crate::decode_with_groups`].
pub trait GroupSpec {
    /// If `count_tag` is a repeating-group count tag, return its `(delimiter, member_tags)`.
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])>;
}

/// A repeating group: a count tag plus its ordered entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Group {
    count_tag: u32,
    entries: Vec<FieldMap>,
    /// NEW-22 (feature 009): the count this group's `NoXxx` field declared on the wire, when
    /// decoded from one — `None` for a group built fresh (e.g. by application code constructing
    /// an outbound message), which always encodes `entries.len()`, same as before this field
    /// existed. Preserves round-trip fidelity for a decoded group whose declared count didn't
    /// match its actual entry count (previously silently "corrected" to `entries.len()` on
    /// re-encode, discarding the wire's own original — possibly malformed — declaration).
    declared_count: Option<i64>,
}

impl Group {
    /// Create an empty group for the given NoXxx count tag.
    pub fn new(count_tag: u32) -> Self {
        Self {
            count_tag,
            entries: Vec::new(),
            declared_count: None,
        }
    }

    /// Append an entry; returns `&mut self` for chaining.
    pub fn add_entry(&mut self, entry: FieldMap) -> &mut Self {
        self.entries.push(entry);
        self
    }

    /// The NoXxx count tag.
    pub fn count_tag(&self) -> u32 {
        self.count_tag
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the group has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// NEW-22 (feature 009): record the count actually declared on the wire (set by the decoder;
    /// not exposed for outbound-only construction, which has no such thing to preserve).
    pub(crate) fn set_declared_count(&mut self, n: i64) {
        self.declared_count = Some(n);
    }

    /// Consume the group into `(count_tag, entries, declared_count)`.
    pub(crate) fn into_parts(self) -> (u32, Vec<FieldMap>, Option<i64>) {
        (self.count_tag, self.entries, self.declared_count)
    }
}
