//! An ordered collection of fields and nested repeating groups.

use crate::field::Field;
use crate::group::Group;

/// One member of a [`FieldMap`]: either a plain field or a repeating group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Member {
    /// A plain field.
    Field(Field),
    /// A repeating group: its count tag and ordered entries.
    Group {
        /// The NoXxx count tag.
        count_tag: u32,
        /// The group entries (each a `FieldMap`).
        entries: Vec<FieldMap>,
    },
}

/// An ordered map of FIX fields (and nested groups), preserving wire order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldMap {
    members: Vec<Member>,
}

impl FieldMap {
    /// Create an empty field map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a field, preserving insertion order (used by the decoder).
    pub fn add_field(&mut self, field: Field) {
        self.members.push(Member::Field(field));
    }

    /// Set a top-level field, replacing an existing one with the same tag, else appending.
    pub fn set(&mut self, field: Field) {
        for m in &mut self.members {
            if let Member::Field(existing) = m
                && existing.tag() == field.tag()
            {
                *existing = field;
                return;
            }
        }
        self.members.push(Member::Field(field));
    }

    /// Get the first top-level field with `tag`.
    pub fn get(&self, tag: u32) -> Option<&Field> {
        self.members.iter().find_map(|m| match m {
            Member::Field(f) if f.tag() == tag => Some(f),
            _ => None,
        })
    }

    /// Returns `true` if a top-level field with `tag` is present.
    pub fn contains(&self, tag: u32) -> bool {
        self.get(tag).is_some()
    }

    /// Append a repeating group.
    pub fn add_group(&mut self, group: Group) {
        let (count_tag, entries) = group.into_parts();
        self.members.push(Member::Group { count_tag, entries });
    }

    /// Get the entries of the first group with `count_tag`.
    pub fn group(&self, count_tag: u32) -> Option<&[FieldMap]> {
        self.members.iter().find_map(|m| match m {
            Member::Group {
                count_tag: ct,
                entries,
            } if *ct == count_tag => Some(entries.as_slice()),
            _ => None,
        })
    }

    /// Get one entry (by 0-based `index`) of the first group with `count_tag` (US9, feature 005,
    /// FR-024/FR-025). `None` if the group doesn't exist or `index` is out of range.
    pub fn get_group(&self, count_tag: u32, index: usize) -> Option<&FieldMap> {
        self.group(count_tag).and_then(|entries| entries.get(index))
    }

    /// Replace one entry (by 0-based `index`) of the first group with `count_tag` (US9, feature
    /// 005, FR-024/FR-025). No-op if the group doesn't exist or `index` is out of range.
    pub fn replace_group(&mut self, count_tag: u32, index: usize, entry: FieldMap) {
        if let Some(Member::Group {
            count_tag: ct,
            entries,
        }) = self
            .members
            .iter_mut()
            .find(|m| matches!(m, Member::Group { count_tag: ct, .. } if *ct == count_tag))
        {
            debug_assert_eq!(*ct, count_tag);
            if let Some(slot) = entries.get_mut(index) {
                *slot = entry;
            }
        }
    }

    /// Remove one entry (by 0-based `index`) of the first group with `count_tag` (US9, feature
    /// 005, FR-024/FR-025), shifting later entries down. No-op if the group doesn't exist or
    /// `index` is out of range.
    pub fn remove_group(&mut self, count_tag: u32, index: usize) {
        if let Some(Member::Group {
            count_tag: ct,
            entries,
        }) = self
            .members
            .iter_mut()
            .find(|m| matches!(m, Member::Group { count_tag: ct, .. } if *ct == count_tag))
        {
            debug_assert_eq!(*ct, count_tag);
            if index < entries.len() {
                entries.remove(index);
            }
        }
    }

    /// Iterate the top-level fields (skipping repeating groups), in order.
    pub fn fields(&self) -> impl Iterator<Item = &Field> {
        self.members.iter().filter_map(|m| match m {
            Member::Field(f) => Some(f),
            Member::Group { .. } => None,
        })
    }

    /// Internal: ordered members, for the encoder.
    pub(crate) fn members(&self) -> &[Member] {
        &self.members
    }
}
