//! The normalized dictionary model and validation types.

use std::collections::{BTreeMap, BTreeSet};

use truefix_core::Field;

/// A FIX field's value type (drives format validation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// Integer.
    Int,
    /// Length.
    Length,
    /// Sequence number.
    SeqNum,
    /// Repeating-group count.
    NumInGroup,
    /// Float.
    Float,
    /// Price.
    Price,
    /// Quantity.
    Qty,
    /// Amount.
    Amt,
    /// Percentage.
    Percentage,
    /// Single character.
    Char,
    /// Boolean (`Y`/`N`).
    Boolean,
    /// String.
    String,
    /// Binary data.
    Data,
    /// UTC timestamp.
    UtcTimestamp,
    /// UTC time-only.
    UtcTimeOnly,
    /// UTC date-only.
    UtcDateOnly,
    /// Month-year.
    MonthYear,
}

impl FieldType {
    /// Parse a normalized type token (e.g. `"PRICE"`).
    pub fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "INT" => Self::Int,
            "LENGTH" => Self::Length,
            "SEQNUM" => Self::SeqNum,
            "NUMINGROUP" => Self::NumInGroup,
            "FLOAT" => Self::Float,
            "PRICE" => Self::Price,
            "QTY" => Self::Qty,
            "AMT" => Self::Amt,
            "PERCENTAGE" => Self::Percentage,
            "CHAR" => Self::Char,
            "BOOLEAN" => Self::Boolean,
            "STRING" => Self::String,
            "DATA" => Self::Data,
            "UTCTIMESTAMP" => Self::UtcTimestamp,
            "UTCTIMEONLY" => Self::UtcTimeOnly,
            "UTCDATEONLY" => Self::UtcDateOnly,
            "MONTHYEAR" => Self::MonthYear,
            _ => return None,
        })
    }

    /// Whether `field`'s value is well-formed for this type.
    pub fn value_ok(self, field: &Field) -> bool {
        match self {
            Self::Int | Self::Length | Self::SeqNum | Self::NumInGroup => field.as_int().is_ok(),
            Self::Float | Self::Price | Self::Qty | Self::Amt | Self::Percentage => {
                field.as_decimal().is_ok()
            }
            Self::Char => field.as_char().is_ok(),
            Self::Boolean => field.as_bool().is_ok(),
            Self::UtcTimestamp => field.as_utc_timestamp().is_ok(),
            // String/Data/date/time-only are accepted as-is at this layer.
            _ => true,
        }
    }
}

/// A field definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    /// Tag number.
    pub tag: u32,
    /// Field name.
    pub name: String,
    /// Value type.
    pub field_type: FieldType,
    /// Allowed enumerated values (empty = any value of the type).
    pub values: Vec<String>,
}

impl FieldDef {
    /// Whether `value` is allowed (always true when the field is not enumerated).
    pub fn allows(&self, value: &str) -> bool {
        self.values.is_empty() || self.values.iter().any(|v| v == value)
    }
}

/// A message definition: required and optional field tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDef {
    /// MsgType value (e.g. `"D"`).
    pub msg_type: String,
    /// Message name (e.g. `"NewOrderSingle"`).
    pub name: String,
    /// Required body field tags.
    pub required: Vec<u32>,
    /// Optional body field tags.
    pub optional: Vec<u32>,
    /// All valid body tags, including transitively-nested repeating-group member tags. Computed by
    /// the parser once groups are known; used for the "tag defined for message type" check.
    pub member_tags: BTreeSet<u32>,
}

impl MessageDef {
    /// Whether `tag` is a directly required or optional body field of this message.
    pub fn allows_tag(&self, tag: u32) -> bool {
        self.required.contains(&tag) || self.optional.contains(&tag)
    }

    /// Whether `tag` is valid anywhere in this message's body, including within its repeating
    /// groups (transitively).
    pub fn contains_member(&self, tag: u32) -> bool {
        self.allows_tag(tag) || self.member_tags.contains(&tag)
    }
}

/// A named, reusable group of field/group member tags, referenced from one or more message or
/// group definitions via a `component:<Name>` token (FR-009). `members` is already fully expanded
/// (nested `component:` references resolved recursively) by the time a `DataDictionary` exists —
/// decode/validate code never has to know components exist; they see the same flat, transitively-
/// expanded tag lists a hand-inlined dictionary would have produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDef {
    /// The component's name, as referenced by `component:<Name>` tokens.
    pub name: String,
    /// The fully-expanded, ordered member tags (a tag that is itself a group's count_tag denotes a
    /// nested group, exactly like a message's or group's own member list).
    pub members: Vec<u32>,
}

/// A parsed FIX data dictionary for one version.
#[derive(Debug, Clone)]
pub struct DataDictionary {
    pub(crate) version: String,
    pub(crate) fields: BTreeMap<u32, FieldDef>,
    pub(crate) field_by_name: BTreeMap<String, u32>,
    pub(crate) messages: BTreeMap<String, MessageDef>,
    pub(crate) header: BTreeSet<u32>,
    pub(crate) trailer: BTreeSet<u32>,
    pub(crate) groups: BTreeMap<u32, GroupDef>,
    pub(crate) components: BTreeMap<String, ComponentDef>,
    pub(crate) hash: u64,
}

impl DataDictionary {
    /// The dictionary version (BeginString).
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The content hash of the source this dictionary was parsed from.
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Look up a field definition by tag.
    pub fn field(&self, tag: u32) -> Option<&FieldDef> {
        self.fields.get(&tag)
    }

    /// Look up a field's tag by name.
    pub fn field_by_name(&self, name: &str) -> Option<u32> {
        self.field_by_name.get(name).copied()
    }

    /// Look up a message definition by MsgType.
    pub fn message(&self, msg_type: &str) -> Option<&MessageDef> {
        self.messages.get(msg_type)
    }

    /// Look up a repeating-group definition by its NoXxx count tag.
    pub fn group(&self, count_tag: u32) -> Option<&GroupDef> {
        self.groups.get(&count_tag)
    }

    /// Look up a component definition by name.
    pub fn component(&self, name: &str) -> Option<&ComponentDef> {
        self.components.get(name)
    }

    /// Whether `tag` is a repeating-group count tag in this dictionary.
    pub fn is_group_count(&self, tag: u32) -> bool {
        self.groups.contains_key(&tag)
    }

    /// Whether `tag` is a header field.
    pub fn is_header(&self, tag: u32) -> bool {
        self.header.contains(&tag)
    }

    /// Whether `tag` is a trailer field.
    pub fn is_trailer(&self, tag: u32) -> bool {
        self.trailer.contains(&tag)
    }

    /// Number of defined fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Number of defined messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Merge `other` into `self` (FR-010): fields/messages/groups/components present only in
    /// `other` are added; a key present in both with an *identical* definition is a no-op
    /// (idempotent — merging the same extension twice is safe); a key present in both with a
    /// *different* definition is a [`DictMergeConflict`] and aborts the merge, leaving `self`
    /// completely unmodified (checked in a dry-run pass before any mutation). `header`/`trailer`
    /// membership has no "conflict" concept — an extension's header/trailer tags are simply unioned
    /// in. `hash` is left unchanged: it identifies the base (bundled, dual-track) source, which
    /// `extend()` deliberately sits outside of (Principle IV; a runtime extension is not part of the
    /// codegen↔runtime provenance the hash proves).
    pub fn extend(&mut self, other: &DataDictionary) -> Result<(), DictMergeConflict> {
        for (tag, def) in &other.fields {
            if let Some(existing) = self.fields.get(tag) {
                if existing != def {
                    return Err(DictMergeConflict {
                        kind: "field",
                        key: tag.to_string(),
                    });
                }
            }
        }
        for (msg_type, def) in &other.messages {
            if let Some(existing) = self.messages.get(msg_type) {
                if existing != def {
                    return Err(DictMergeConflict {
                        kind: "message",
                        key: msg_type.clone(),
                    });
                }
            }
        }
        for (count_tag, def) in &other.groups {
            if let Some(existing) = self.groups.get(count_tag) {
                if existing != def {
                    return Err(DictMergeConflict {
                        kind: "group",
                        key: count_tag.to_string(),
                    });
                }
            }
        }
        for (name, def) in &other.components {
            if let Some(existing) = self.components.get(name) {
                if existing != def {
                    return Err(DictMergeConflict {
                        kind: "component",
                        key: name.clone(),
                    });
                }
            }
        }

        // No conflicts found — apply the merge.
        for (tag, def) in &other.fields {
            self.fields.entry(*tag).or_insert_with(|| def.clone());
            self.field_by_name.entry(def.name.clone()).or_insert(*tag);
        }
        for (msg_type, def) in &other.messages {
            self.messages
                .entry(msg_type.clone())
                .or_insert_with(|| def.clone());
        }
        for (count_tag, def) in &other.groups {
            self.groups.entry(*count_tag).or_insert_with(|| def.clone());
        }
        for (name, def) in &other.components {
            self.components
                .entry(name.clone())
                .or_insert_with(|| def.clone());
        }
        self.header.extend(other.header.iter().copied());
        self.trailer.extend(other.trailer.iter().copied());
        Ok(())
    }
}

/// A conflicting redefinition found while [`DataDictionary::extend`]ing (FR-010): `other` redefines
/// `key` (of kind `kind`) differently than `self` already defines it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("conflicting redefinition of {kind} {key:?}")]
pub struct DictMergeConflict {
    /// Which map the conflict is in: `"field"`, `"message"`, `"group"`, or `"component"`.
    pub kind: &'static str,
    /// The conflicting key (a tag number or name, stringified).
    pub key: String,
}

/// FIX SessionRejectReason / business reject reason for a validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Tag number is not defined in the dictionary (0).
    InvalidTagNumber,
    /// A required tag is missing (1).
    RequiredTagMissing,
    /// Tag not defined for this message type (2).
    TagNotDefinedForMessageType,
    /// Undefined tag (3).
    UndefinedTag,
    /// Tag specified without a value (4).
    TagSpecifiedWithoutValue,
    /// Value is incorrect / out of range for the field (5).
    ValueIsIncorrect,
    /// Incorrect data format for the value (6).
    IncorrectDataFormat,
    /// Invalid or unsupported MsgType (11).
    InvalidMsgType,
    /// A tag appears more than once outside a repeating group (13).
    TagAppearsMoreThanOnce,
    /// A tag is out of the required order — e.g. the standard header's first fields, or
    /// header/body/trailer sectioning (14).
    TagOutOfRequiredOrder,
    /// A repeating group's fields are out of order / delimiter missing (15).
    RepeatingGroupFieldsOutOfOrder,
    /// The NoXxx count does not match the number of group entries (16).
    IncorrectNumInGroupCount,
}

impl RejectReason {
    /// The numeric SessionRejectReason (tag 373) code.
    pub fn code(self) -> u32 {
        match self {
            Self::InvalidTagNumber => 0,
            Self::RequiredTagMissing => 1,
            Self::TagNotDefinedForMessageType => 2,
            Self::UndefinedTag => 3,
            Self::TagSpecifiedWithoutValue => 4,
            Self::ValueIsIncorrect => 5,
            Self::IncorrectDataFormat => 6,
            Self::InvalidMsgType => 11,
            Self::TagAppearsMoreThanOnce => 13,
            Self::TagOutOfRequiredOrder => 14,
            Self::RepeatingGroupFieldsOutOfOrder => 15,
            Self::IncorrectNumInGroupCount => 16,
        }
    }
}

/// A repeating-group definition: the NoXxx count tag, the entry delimiter (first field), and the
/// ordered member tags (a member that is itself a count tag denotes a nested group).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupDef {
    /// The NoXxx count tag (e.g. 453 NoPartyIDs).
    pub count_tag: u32,
    /// The delimiter tag that starts each entry (e.g. 448 PartyID).
    pub delimiter: u32,
    /// The ordered member tags of each entry (including the delimiter first).
    pub members: Vec<u32>,
}

/// A validation failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason:?} (tag {ref_tag:?}): {text}")]
pub struct ValidationError {
    /// Why the message was rejected.
    pub reason: RejectReason,
    /// The offending tag, if applicable.
    pub ref_tag: Option<u32>,
    /// `true` => business-level reject; `false` => session-level reject.
    pub business: bool,
    /// Human-readable detail.
    pub text: String,
}

impl ValidationError {
    pub(crate) fn session(reason: RejectReason, ref_tag: Option<u32>, text: &str) -> Self {
        Self {
            reason,
            ref_tag,
            business: false,
            text: text.to_owned(),
        }
    }

    pub(crate) fn business(reason: RejectReason, ref_tag: Option<u32>, text: &str) -> Self {
        Self {
            reason,
            ref_tag,
            business: true,
            text: text.to_owned(),
        }
    }
}

/// Which validation checks to apply (maps to the FIX `Validate*` config toggles).
#[derive(Debug, Clone, Copy)]
pub struct ValidationOptions {
    /// Reject fields present with an empty value.
    pub validate_fields_have_values: bool,
    /// Validate user-defined fields (tags ≥ 5000) rather than skipping them.
    pub validate_user_defined_fields: bool,
    /// Permit fields not defined for the message type / dictionary.
    pub allow_unknown_msg_fields: bool,
    /// Check that required fields are present.
    pub check_required_fields: bool,
    /// Check field value formats and enumerations.
    pub check_field_types: bool,
    /// Validate repeating-group structure (count matches entries; delimiter; ordering).
    pub check_groups: bool,
    /// Require each group entry to begin with its delimiter (FirstFieldInGroupIsDelimiter).
    pub first_field_in_group_is_delimiter: bool,
    /// Reject out-of-order fields within a group entry (ValidateUnorderedGroupFields).
    pub validate_unordered_group_fields: bool,
    /// Reject a tag that appears more than once outside a repeating group.
    pub check_repeated_tags: bool,
    /// Reject messages whose header/body/trailer fields violate wire-sectioning order, or whose
    /// third field isn't MsgType(35) (`ValidateFieldsOutOfOrder`; FR-006). Default `false` —
    /// matches today's lenient behaviour; the underlying violation is detected at decode time
    /// regardless (see [`truefix_core::Message::fields_out_of_order`]), this toggle only governs
    /// whether `validate()` rejects on it.
    pub validate_fields_out_of_order: bool,
    /// Documents QuickFIX/J's `ValidateChecksum` config key for parity purposes. TrueFix's decoder
    /// always validates the wire checksum unconditionally (a bad checksum is a decode-time error,
    /// handled via `RejectGarbledMessage` before a message ever reaches `validate()`) — this flag
    /// does not weaken that enforcement; a `false` value is accepted but not honored, by design
    /// (Constitution Principle I/II: checksum validation is not an optional safety property).
    pub validate_checksum: bool,
    /// Master switch: when `false`, `validate()` skips all other checks and returns `Ok(())`
    /// unconditionally (`ValidateIncomingMessage`). Default `true` (today's behaviour).
    pub validate_incoming_message: bool,
    /// Whether a message carrying `PossDupFlag(43)=Y` is accepted at all (`AllowPosDup`). Default
    /// `true` — matches today's behaviour (PossDup messages are not otherwise rejected by
    /// `validate()`).
    pub allow_pos_dup: bool,
    /// Require `OrigSendingTime(122)` on any message carrying `PossDupFlag(43)=Y`
    /// (`RequiresOrigSendingTime`). Default `false` — matches today's behaviour (not required).
    pub requires_orig_sending_time: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            validate_fields_have_values: true,
            validate_user_defined_fields: false,
            allow_unknown_msg_fields: false,
            check_required_fields: true,
            check_field_types: true,
            check_groups: true,
            first_field_in_group_is_delimiter: true,
            validate_unordered_group_fields: true,
            check_repeated_tags: true,
            validate_fields_out_of_order: false,
            validate_checksum: true,
            validate_incoming_message: true,
            allow_pos_dup: true,
            requires_orig_sending_time: false,
        }
    }
}
