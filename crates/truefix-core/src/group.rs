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
}

impl Group {
    /// Create an empty group for the given NoXxx count tag.
    pub fn new(count_tag: u32) -> Self {
        Self {
            count_tag,
            entries: Vec::new(),
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

    /// Consume the group into `(count_tag, entries)`.
    pub(crate) fn into_parts(self) -> (u32, Vec<FieldMap>) {
        (self.count_tag, self.entries)
    }
}
