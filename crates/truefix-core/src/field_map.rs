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
            if let Member::Field(existing) = m {
                if existing.tag() == field.tag() {
                    *existing = field;
                    return;
                }
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

    /// Internal: ordered members, for the encoder.
    pub(crate) fn members(&self) -> &[Member] {
        &self.members
    }
}
