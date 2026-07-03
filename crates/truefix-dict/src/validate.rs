//! Runtime message validation against a [`DataDictionary`].

use truefix_core::{Field, GroupSpec, Message};

use crate::model::{DataDictionary, GroupDef, RejectReason, ValidationError, ValidationOptions};

impl GroupSpec for DataDictionary {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        self.groups
            .get(&count_tag)
            .map(|g| (g.delimiter, g.members.as_slice()))
    }
}

/// The first user-defined-field tag (UDFs occupy 5000–9999 in FIX).
const UDF_START: u32 = 5000;

impl DataDictionary {
    /// Validate `message` against this dictionary using `opts`.
    ///
    /// Returns the first failure found. Field-membership/format/required failures are
    /// session-level rejects; an unknown-but-structural MsgType is a business-level reject
    /// (FR-C4 two rejection layers).
    pub fn validate(
        &self,
        message: &Message,
        opts: &ValidationOptions,
    ) -> Result<(), ValidationError> {
        if !opts.validate_incoming_message {
            return Ok(());
        }

        // GAP-32/FR-029 (feature 005): a no-op when this dictionary has no `version-meta`
        // (spec.md Edge Case) or the message's BeginString doesn't have the plain "FIX.M.N" shape
        // (e.g. FIXT.1.1, whose version is instead resolved via ApplVerID — a separate mechanism,
        // see `fixt.rs` — not this per-message BeginString check).
        if let Some(vm) = self.version_meta {
            if let Some(bs) = message.header.get(8).and_then(|f| f.as_str().ok()) {
                if let Some((major, minor)) = parse_fix_begin_string(bs) {
                    if major != vm.major || minor != vm.minor {
                        return Err(ValidationError::session(
                            RejectReason::ValueIsIncorrect,
                            Some(8),
                            "BeginString does not match the loaded dictionary's version",
                        ));
                    }
                }
            }
        }

        if opts.validate_fields_out_of_order && message.fields_out_of_order() {
            return Err(ValidationError::session(
                RejectReason::TagOutOfRequiredOrder,
                None,
                "header/body/trailer fields are out of wire-sectioning order",
            ));
        }

        let poss_dup = message.header.get(43).and_then(|f| f.as_str().ok()) == Some("Y");
        if poss_dup {
            if !opts.allow_pos_dup {
                return Err(ValidationError::session(
                    RejectReason::ValueIsIncorrect,
                    Some(43),
                    "PossDup messages are not accepted (AllowPosDup=N)",
                ));
            }
            if opts.requires_orig_sending_time && message.header.get(122).is_none() {
                return Err(ValidationError::session(
                    RejectReason::RequiredTagMissing,
                    Some(122),
                    "PossDup message missing OrigSendingTime",
                ));
            }
        }

        let msg_type = message.msg_type().ok_or_else(|| {
            ValidationError::session(RejectReason::InvalidMsgType, Some(35), "missing MsgType")
        })?;
        let mdef = self.message(msg_type).ok_or_else(|| {
            ValidationError::business(
                RejectReason::InvalidMsgType,
                Some(35),
                "MsgType not defined in dictionary",
            )
        })?;

        let mut seen_once: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();

        for field in message
            .header
            .fields()
            .chain(message.body.fields())
            .chain(message.trailer.fields())
        {
            let tag = field.tag();

            // Tags that are members of a repeating group legitimately repeat once per entry; only
            // flag a repeat for tags that are not part of any group in this message.
            if opts.check_repeated_tags
                && !mdef.member_tags.contains(&tag)
                && !seen_once.insert(tag)
            {
                return Err(ValidationError::session(
                    RejectReason::TagAppearsMoreThanOnce,
                    Some(tag),
                    "tag appears more than once",
                ));
            }

            if opts.validate_fields_have_values && field.value_bytes().is_empty() {
                return Err(ValidationError::session(
                    RejectReason::TagSpecifiedWithoutValue,
                    Some(tag),
                    "field has no value",
                ));
            }

            match self.field(tag) {
                None => {
                    let is_udf = tag >= UDF_START;
                    if is_udf && !opts.validate_user_defined_fields {
                        continue;
                    }
                    if opts.allow_unknown_msg_fields {
                        continue;
                    }
                    return Err(ValidationError::session(
                        RejectReason::InvalidTagNumber,
                        Some(tag),
                        "tag not defined in dictionary",
                    ));
                }
                Some(fdef) => {
                    // GAP-33/FR-031 (feature 005): BeginString(8)/CheckSum(10) are pure envelope
                    // framing fields, always managed by the codec layer (`truefix_core::decode`
                    // already validates/computes both before a message ever reaches `validate()`)
                    // — never re-checked against the dictionary's own per-field type here,
                    // regardless of what type the dictionary happens to declare for them. This
                    // matters concretely for real QuickFIX data: FIX 4.0/4.1's own bundled XML
                    // dictionaries misclassify both as `CHAR` (a documented upstream quirk —
                    // `CheckSum`'s real value is a 3-digit string like `"000"`, `BeginString`'s is
                    // a multi-character version string like `"FIX.4.0"`; neither is ever a single
                    // character) — enforcing that literally would reject every real message for
                    // those two versions.
                    let is_envelope_framing_field = tag == truefix_core::tags::BEGIN_STRING
                        || tag == truefix_core::tags::CHECK_SUM;
                    if opts.check_field_types && !is_envelope_framing_field {
                        if !fdef.field_type.value_ok(field) {
                            return Err(ValidationError::session(
                                RejectReason::IncorrectDataFormat,
                                Some(tag),
                                "value has incorrect data format",
                            ));
                        }
                        if let Ok(value) = field.as_str() {
                            if !fdef.allows(value) {
                                return Err(ValidationError::session(
                                    RejectReason::ValueIsIncorrect,
                                    Some(tag),
                                    "value is not an allowed enumeration",
                                ));
                            }
                        }
                    }

                    let belongs =
                        self.is_header(tag) || self.is_trailer(tag) || mdef.contains_member(tag);
                    if !belongs && !opts.allow_unknown_msg_fields {
                        return Err(ValidationError::session(
                            RejectReason::TagNotDefinedForMessageType,
                            Some(tag),
                            "tag not defined for this message type",
                        ));
                    }
                }
            }
        }

        if opts.check_groups {
            self.validate_groups(message, opts)?;
        }

        if opts.check_required_fields {
            for &tag in &mdef.required {
                if !present(message, tag) {
                    return Err(ValidationError::session(
                        RejectReason::RequiredTagMissing,
                        Some(tag),
                        "required field missing",
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate repeating-group structure in the (flat, wire-ordered) message body: the NoXxx count
    /// must match the number of delimiter-led entries, each entry must begin with its delimiter
    /// (FirstFieldInGroupIsDelimiter), and members must be in order (ValidateUnorderedGroupFields).
    /// Nested groups are validated recursively (FR-004/FR-005).
    fn validate_groups(
        &self,
        message: &Message,
        opts: &ValidationOptions,
    ) -> Result<(), ValidationError> {
        let body: Vec<&Field> = message.body.fields().collect();
        let mut pos = 0usize;
        while let Some(f) = body.get(pos) {
            let tag = f.tag();
            if self.groups.contains_key(&tag) {
                self.validate_group(&body, &mut pos, tag, opts)?;
            } else {
                pos += 1;
            }
        }
        Ok(())
    }

    fn validate_group(
        &self,
        body: &[&Field],
        pos: &mut usize,
        count_tag: u32,
        opts: &ValidationOptions,
    ) -> Result<(), ValidationError> {
        let Some(gdef) = self.groups.get(&count_tag) else {
            *pos += 1;
            return Ok(());
        };
        let declared = body.get(*pos).and_then(|f| f.as_int().ok()).unwrap_or(-1);
        *pos += 1; // consume the count field

        let mut found = 0i64;
        while let Some(f) = body.get(*pos) {
            let tag = f.tag();
            if tag != gdef.delimiter {
                // A group member where the delimiter was expected means the entry is malformed.
                if opts.first_field_in_group_is_delimiter
                    && found < declared
                    && (gdef.members.contains(&tag) || self.groups.contains_key(&tag))
                {
                    return Err(ValidationError::session(
                        RejectReason::RepeatingGroupFieldsOutOfOrder,
                        Some(gdef.delimiter),
                        "group entry does not begin with its delimiter",
                    ));
                }
                break; // end of this group
            }
            // GAP-24/FR-024 (feature 005): `FieldMap::fields()` (used by the top-level field
            // loop in `validate()`) skips group members entirely, so this is the only place a
            // group entry's own delimiter field gets type/enum-checked — via the group's `child`
            // dictionary when present (falling back to this dictionary's own field registry,
            // which `child` is itself derived from).
            if opts.check_field_types {
                self.check_group_field_value(gdef, f)?;
            }
            found += 1;
            *pos += 1; // consume delimiter

            let mut last_idx = 0usize; // the delimiter is members[0]
            while let Some(mf) = body.get(*pos) {
                let t = mf.tag();
                if t == gdef.delimiter {
                    break; // next entry
                }
                if !gdef.members.contains(&t) {
                    break; // field belongs to an enclosing scope
                }
                let idx = gdef
                    .members
                    .iter()
                    .position(|&m| m == t)
                    .unwrap_or(usize::MAX);
                if opts.validate_unordered_group_fields && idx < last_idx {
                    return Err(ValidationError::session(
                        RejectReason::RepeatingGroupFieldsOutOfOrder,
                        Some(t),
                        "repeating-group fields out of order",
                    ));
                }
                last_idx = idx;
                if self.groups.contains_key(&t) {
                    self.validate_group(body, pos, t, opts)?; // nested group
                } else {
                    // GAP-24/FR-024 (feature 005): same per-field type/enum check as the
                    // delimiter above, for every other member field of this group entry.
                    if opts.check_field_types {
                        self.check_group_field_value(gdef, mf)?;
                    }
                    *pos += 1;
                }
            }
        }

        if declared != found {
            return Err(ValidationError::session(
                RejectReason::IncorrectNumInGroupCount,
                Some(count_tag),
                "NoXxx count does not match the number of group entries",
            ));
        }
        Ok(())
    }

    /// Type/enum-check one field of a group entry (GAP-24/FR-024, feature 005), consulting the
    /// group's `child` dictionary when present (falling back to this dictionary's own field
    /// registry — `child` is derived from it, so both answer identically when both have the tag;
    /// the fallback only matters for the defensive `child: None` case).
    fn check_group_field_value(
        &self,
        gdef: &GroupDef,
        field: &Field,
    ) -> Result<(), ValidationError> {
        let tag = field.tag();
        let Some(fdef) = gdef
            .child
            .as_deref()
            .and_then(|c| c.field(tag))
            .or_else(|| self.field(tag))
        else {
            return Ok(()); // unknown-tag policy is the top-level loop's job, not this one's
        };
        if !fdef.field_type.value_ok(field) {
            return Err(ValidationError::session(
                RejectReason::IncorrectDataFormat,
                Some(tag),
                "group field value has incorrect data format",
            ));
        }
        if let Ok(value) = field.as_str() {
            if !fdef.allows(value) {
                return Err(ValidationError::session(
                    RejectReason::ValueIsIncorrect,
                    Some(tag),
                    "group field value is not an allowed enumeration",
                ));
            }
        }
        Ok(())
    }
}

/// Parse a plain `"FIX.<major>.<minor>[SP<n>][EP<n>]"` BeginString into `(major, minor)`. Returns
/// `None` for shapes this doesn't recognize (e.g. `"FIXT.1.1"`), matching FR-029's own
/// no-op-when-not-applicable framing (see this function's caller).
fn parse_fix_begin_string(bs: &str) -> Option<(u8, u8)> {
    let rest = bs.strip_prefix("FIX.")?;
    let mut parts = rest.splitn(2, '.');
    let major: u8 = parts.next()?.parse().ok()?;
    let minor_token = parts.next()?;
    let minor_digits: String = minor_token
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let minor: u8 = minor_digits.parse().ok()?;
    Some((major, minor))
}

fn present(message: &Message, tag: u32) -> bool {
    message.header.get(tag).is_some()
        || message.body.get(tag).is_some()
        || message.trailer.get(tag).is_some()
}
