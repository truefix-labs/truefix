//! Runtime message validation against a [`DataDictionary`].

use truefix_core::{Field, FieldMap, GroupSpec, MemberRef, Message};

use crate::model::{
    DataDictionary, GroupDef, RejectReason, ValidationError, ValidationOptions, VersionMeta,
};

impl GroupSpec for DataDictionary {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        self.groups
            .get(&count_tag)
            .map(|g| (g.delimiter, g.members.as_slice()))
    }
}

/// The first user-defined-field tag (UDFs occupy 5000–9999 in FIX).
const UDF_START: u32 = 5000;

/// Find the first `Member::Group` in `map` whose count tag is `count_tag`, returning its entries,
/// declared wire count, and the member immediately following it in `map`'s own member sequence, if
/// any (feature 011) — the structured-group counterpart to `FieldMap::group()`, which discards
/// both `declared_count` and sibling position. The trailing sibling is what lets
/// `validate_structured_group` tell a genuine short count apart from a missing-delimiter entry
/// (see its doc).
fn find_structured_group_with_next(
    map: &FieldMap,
    count_tag: u32,
) -> Option<(&[FieldMap], Option<i64>, Option<MemberRef<'_>>)> {
    let mut members = map.members().peekable();
    while let Some(m) = members.next() {
        if let MemberRef::Group {
            count_tag: ct,
            entries,
            declared_count,
        } = m
            && ct == count_tag
        {
            return Some((entries, declared_count, members.peek().copied()));
        }
    }
    None
}

impl DataDictionary {
    /// BUG-62/FR-035 (feature 007): whether this dictionary's version is FIX.4.0/4.1 — QFJ skips
    /// `CharConverter` (single-character) validation entirely for those versions, treating `CHAR`
    /// fields as plain strings, so a multi-character value must be accepted rather than rejected.
    /// Deliberately parses `self.version` (the plain `version FIX.M.N` directive every bundled
    /// dictionary declares) rather than `self.version_meta` (the separate, optional `version-meta`
    /// directive that, as of this writing, *no* bundled dictionary actually declares — relying on
    /// it here would make this check silently inert for every real dictionary shipped today).
    fn is_legacy_char_lenient(&self) -> bool {
        parse_fix_begin_string(&self.version)
            .is_some_and(|(major, minor, _, _)| major == 4 && minor <= 1)
    }

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

        // GAP-32/FR-029 (feature 005); NEW-145 (audit 006): previously a no-op whenever this
        // dictionary had no `version-meta` directive -- true for every bundled dictionary, since
        // none declares one, making the check always skipped in practice. Falls back to parsing
        // `self.version` (the plain `version FIX.M.N` directive every bundled dictionary does
        // declare) when `version_meta` is absent, so the check actually runs for real dictionaries
        // instead of only for a hypothetical one with an explicit `version-meta` line. Still a
        // no-op when neither source parses as a plain "FIX.M.N" BeginString (e.g. FIXT.1.1, whose
        // version is instead resolved via ApplVerID — a separate mechanism, see `fixt.rs`) or the
        // message's own BeginString doesn't either (spec.md Edge Case).
        let effective_version = self.version_meta.or_else(|| {
            parse_fix_begin_string(&self.version).map(
                |(major, minor, service_pack, extension_pack)| VersionMeta {
                    major,
                    minor,
                    service_pack,
                    extension_pack,
                },
            )
        });
        if let Some(vm) = effective_version
            && let Some(bs) = message.header.get(8).and_then(|f| f.as_str().ok())
            && let Some((major, minor, service_pack, extension_pack)) = parse_fix_begin_string(bs)
            && (major != vm.major
                || minor != vm.minor
                || service_pack != vm.service_pack
                || extension_pack != vm.extension_pack)
        {
            return Err(ValidationError::session(
                RejectReason::ValueIsIncorrect,
                Some(8),
                "BeginString does not match the loaded dictionary's version",
            ));
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

            // NEW-17 (feature 009): the UDF short-circuit must run before *every* other check in
            // this loop (repeated-tag, empty-value, unknown-tag, type/enum) so
            // `ValidateUserDefinedFields=N` fully skips UDFs, matching QFJ -- previously this
            // check lived only inside the `self.field(tag) == None` arm further down, so an empty
            // or repeated UDF was still rejected by the earlier checks even with the option off.
            if tag >= UDF_START && !opts.validate_user_defined_fields {
                continue;
            }

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

            // BUG-89/FR-011 (feature 007): now that admin messages are dictionary-validated too
            // (previously skipped entirely), the six Appendix-B "ReverseRoute" routing tags need
            // an explicit exception here — FIX legitimately allows these present-but-empty
            // (`Message::reverse_route`'s own "presence — not content — governs" reversal rule,
            // exercised by the `ReverseRouteWithEmptyRoutingTags` AT scenario), unlike every other
            // field, where an empty value is a genuine `TagSpecifiedWithoutValue` violation.
            const REVERSE_ROUTE_TAGS: [u32; 6] = [115, 116, 128, 129, 144, 145];
            if opts.validate_fields_have_values
                && field.value_bytes().is_empty()
                && !REVERSE_ROUTE_TAGS.contains(&tag)
            {
                return Err(ValidationError::session(
                    RejectReason::TagSpecifiedWithoutValue,
                    Some(tag),
                    "field has no value",
                ));
            }

            match self.field(tag) {
                None => {
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
                        if !fdef
                            .field_type
                            .value_ok(field, self.is_legacy_char_lenient())
                        {
                            return Err(ValidationError::session(
                                RejectReason::IncorrectDataFormat,
                                Some(tag),
                                "value has incorrect data format",
                            ));
                        }
                        if let Ok(value) = field.as_str()
                            && !fdef.allows(value)
                        {
                            return Err(ValidationError::session(
                                RejectReason::ValueIsIncorrect,
                                Some(tag),
                                "value is not an allowed enumeration",
                            ));
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
            // NEW-127 (audit 006): QFJ's `DataDictionary.checkHasRequired()` checks required
            // fields for HEADER_ID/TRAILER_ID as well as the message body; previously this
            // dictionary only checked `mdef.required` (body fields) -- header/trailer had no
            // required/optional split at all, so this check was structurally impossible. These
            // envelope-framing tags are universally required by the FIX message-structure spec
            // (Volume 1) regardless of dictionary version, so they're checked directly here rather
            // than needing a per-dictionary required/optional split for header/trailer. Gated on
            // `check_required_envelope_fields` (default `false`, see its doc) since a message built
            // directly (bypassing `truefix_core::decode`/`encode`'s envelope construction) may
            // legitimately omit them -- e.g. this crate's and other crates' own test fixtures.
            if opts.check_required_envelope_fields {
                for &tag in self.header_required_tags() {
                    if !present(message, tag) {
                        return Err(ValidationError::session(
                            RejectReason::RequiredTagMissing,
                            Some(tag),
                            "required header field missing",
                        ));
                    }
                }
                for &tag in self.trailer_required_tags() {
                    if !present(message, tag) {
                        return Err(ValidationError::session(
                            RejectReason::RequiredTagMissing,
                            Some(tag),
                            "required trailer field missing",
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate repeating-group structure in the message body: the NoXxx count must match the
    /// number of delimiter-led entries, each entry must begin with its delimiter
    /// (FirstFieldInGroupIsDelimiter), and members must be in order (ValidateUnorderedGroupFields).
    /// Nested groups are validated recursively (FR-004/FR-005).
    ///
    /// Two independent representations of a group both reach this method and are both checked
    /// (feature 011: confirmed by running the existing flat-message-fixture test suite — see
    /// `group_validation.rs`/`header_group_validation.rs` — after attempting to remove the flat
    /// path below; several tests regressed, proving it is *not* dead code):
    /// - **Structured** (`Member::Group`, produced by `decode_with_groups`/`restructure_groups`
    ///   whenever a production receive path has a dictionary attached — now the body case too, not
    ///   just header/trailer, since feature 011 FR-003): validated via `validate_structured_group`.
    /// - **Flat** (`Member::Field`, produced by a plain `decode()` with no `GroupSpec`, or by a
    ///   caller building a `Message` directly via `FieldMap::add_field` without ever going through
    ///   `add_group`/`decode_with_groups` — `DataDictionary::validate` is a general-purpose public
    ///   API, not exclusively fed by the structured production receive path): validated via
    ///   `validate_group`'s position-scanning walk, which only ever sees content
    ///   `FieldMap::fields()` still exposes — i.e. exactly the tags the structured loop above
    ///   didn't already consume as a `Member::Group`, so the two paths never double-process the
    ///   same group occurrence.
    fn validate_groups(
        &self,
        message: &Message,
        opts: &ValidationOptions,
    ) -> Result<(), ValidationError> {
        // NEW-19 (feature 009): the standard header can itself declare a repeating group (e.g.
        // NoHops(627) — `header ... 627` / `group 627 NoHops ...` in every bundled dictionary) --
        // checked here identically to the body case.
        for &count_tag in self.groups.keys() {
            if let Some((entries, declared_count, next)) =
                find_structured_group_with_next(&message.header, count_tag)
            {
                self.validate_structured_group(entries, declared_count, next, count_tag, opts)?;
            }
            if let Some((entries, declared_count, next)) =
                find_structured_group_with_next(&message.body, count_tag)
            {
                self.validate_structured_group(entries, declared_count, next, count_tag, opts)?;
            }
        }

        let header: Vec<&Field> = message.header.fields().collect();
        let mut hpos = 0usize;
        while let Some(f) = header.get(hpos) {
            let tag = f.tag();
            if self.groups.contains_key(&tag) {
                self.validate_group(&header, &mut hpos, tag, opts)?;
            } else {
                hpos += 1;
            }
        }

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

    /// Whether `tag` is present in a structured group `entry` — a plain field lookup, except when
    /// `tag` is itself a nested group's count tag, in which case presence means that nested group
    /// has at least one entry (used by `validate_structured_group`'s required-field check).
    fn present_in_structured_entry(&self, entry: &FieldMap, tag: u32) -> bool {
        if self.groups.contains_key(&tag) {
            entry.group(tag).is_some_and(|entries| !entries.is_empty())
        } else {
            entry.get(tag).is_some()
        }
    }

    /// BUG-55/FR-034 (feature 007); count/order checks added feature 011 (see research.md R4's
    /// "known pre-existing gap" note — until this fix, a structured group's declared wire count
    /// and member order were never checked at all here, only by the flat `validate_group` path;
    /// this surfaced as 3 real AT-scenario regressions once feature 011 made body-level
    /// structuring routine, proving the gap live rather than theoretical): structural counterpart
    /// to [`Self::validate_group`] for a group that's already `Member::Group`-structured (see
    /// [`Self::validate_groups`]'s doc for when this is reachable). Checks, per entry: the
    /// declared wire count (`Group::declared_count`, via `MemberRef`) against the actual entry
    /// count (`IncorrectNumInGroupCount`); each member's position, walked in the entry's own
    /// actual field order, against `gdef.members`' declared order
    /// (`RepeatingGroupFieldsOutOfOrder`, gated on `validate_unordered_group_fields` — this also
    /// catches an entry not starting with its delimiter, since the delimiter is always index 0);
    /// each field's type/enum validity; and required-field presence (`gdef.required`, BUG-54).
    /// Nested groups recurse via the entry's own [`FieldMap::members`].
    fn validate_structured_group(
        &self,
        entries: &[FieldMap],
        declared_count: Option<i64>,
        next_sibling: Option<MemberRef<'_>>,
        count_tag: u32,
        opts: &ValidationOptions,
    ) -> Result<(), ValidationError> {
        let Some(gdef) = self.groups.get(&count_tag) else {
            return Ok(());
        };
        if let Some(declared) = declared_count
            && declared != entries.len() as i64
        {
            // QFJ934: when there are fewer entries than declared, `decode_with_groups`/
            // `restructure_groups` (`build_group`) most likely stopped collecting entries because
            // the position where the next entry should have started held a member field instead
            // of the delimiter — the wire-level "entry doesn't begin with its delimiter" violation
            // — rather than the wire genuinely just declaring the wrong count. `build_group`
            // itself discards *why* it stopped short, but the symptom survives one level up: the
            // member that would have continued this group ends up as this group's very next
            // sibling in the parent's own member sequence instead. Recognizing that reconstructs
            // QFJ's `FirstFieldInGroupIsDelimiter` signal (reason 15) instead of a generic count
            // mismatch (reason 16), matching the flat path's behavior for the identical input.
            if (declared as usize) > entries.len()
                && let Some(next) = next_sibling
            {
                let next_tag = match next {
                    MemberRef::Field(f) => f.tag(),
                    MemberRef::Group { count_tag: ct, .. } => ct,
                };
                if opts.first_field_in_group_is_delimiter && gdef.members.contains(&next_tag) {
                    return Err(ValidationError::session(
                        RejectReason::RepeatingGroupFieldsOutOfOrder,
                        Some(gdef.delimiter),
                        "group entry does not begin with its delimiter",
                    ));
                }
            }
            return Err(ValidationError::session(
                RejectReason::IncorrectNumInGroupCount,
                Some(count_tag),
                "NoXxx count does not match the number of group entries",
            ));
        }
        for entry in entries {
            let mut last_idx = 0usize;
            let mut members = entry.members().peekable();
            while let Some(member) = members.next() {
                let tag = match member {
                    MemberRef::Field(f) => f.tag(),
                    MemberRef::Group { count_tag: ct, .. } => ct,
                };
                let Some(idx) = gdef.members.iter().position(|&m| m == tag) else {
                    continue; // not declared as a member of this group (defensive; e.g. a UDF)
                };
                if opts.validate_unordered_group_fields && idx < last_idx {
                    return Err(ValidationError::session(
                        RejectReason::RepeatingGroupFieldsOutOfOrder,
                        Some(tag),
                        "repeating-group fields out of order",
                    ));
                }
                last_idx = idx;
                match member {
                    MemberRef::Group {
                        count_tag: ct,
                        entries: nested_entries,
                        declared_count: nested_declared,
                    } => {
                        // If this nested group is the last member of its enclosing entry, the
                        // true next wire-order token (needed to detect QFJ934-style deeply-nested
                        // delimiter violations) escaped this entry entirely and instead follows
                        // wherever the *enclosing* group itself ends — i.e. this call's own
                        // `next_sibling` — since `build_group` unwinds every enclosing scope in
                        // one pass once a token fails a scope's membership check. Falling back to
                        // it is always safe even when not exactly precise (e.g. more entries
                        // remain in the enclosing group): the check below only ever matches a tag
                        // that's a genuine member of *this* nested group's own definition.
                        self.validate_structured_group(
                            nested_entries,
                            nested_declared,
                            members.peek().copied().or(next_sibling),
                            ct,
                            opts,
                        )?;
                    }
                    MemberRef::Field(field)
                        if opts.check_field_types || opts.validate_fields_have_values =>
                    {
                        self.check_group_field_value(gdef, field, opts)?;
                    }
                    MemberRef::Field(_) => {}
                }
            }
            if opts.check_required_fields {
                for &req_tag in &gdef.required {
                    if !self.present_in_structured_entry(entry, req_tag) {
                        return Err(ValidationError::session(
                            RejectReason::RequiredTagMissing,
                            Some(req_tag),
                            "group entry missing a required field",
                        ));
                    }
                }
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
        let Some(count_field) = body.get(*pos) else {
            return Err(ValidationError::session(
                RejectReason::IncorrectDataFormat,
                Some(count_tag),
                "repeating-group count is missing",
            ));
        };
        let declared = count_field.as_int().map_err(|_| {
            ValidationError::session(
                RejectReason::IncorrectDataFormat,
                Some(count_tag),
                "repeating-group count has incorrect data format",
            )
        })?;
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
            if opts.check_field_types || opts.validate_fields_have_values {
                self.check_group_field_value(gdef, f, opts)?;
            }
            found += 1;
            *pos += 1; // consume delimiter

            // BUG-54/FR-034 (feature 007): tags actually present in this entry, checked against
            // `gdef.required` once the entry ends — QFJ's `checkHasRequired` recurses into each
            // group entry the same way; TrueFix previously only checked `mdef.required` at the
            // message body level, never anything within an entry (so an entry missing a required
            // member, other than the delimiter itself, was never caught as long as the NoXxx count
            // matched the number of entries actually present).
            let mut seen_in_entry: Vec<u32> = vec![gdef.delimiter];

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
                seen_in_entry.push(t);
                if self.groups.contains_key(&t) {
                    self.validate_group(body, pos, t, opts)?; // nested group
                } else {
                    // GAP-24/FR-024 (feature 005): same per-field type/enum check as the
                    // delimiter above, for every other member field of this group entry.
                    if opts.check_field_types || opts.validate_fields_have_values {
                        self.check_group_field_value(gdef, mf, opts)?;
                    }
                    *pos += 1;
                }
            }

            if opts.check_required_fields {
                for &req_tag in &gdef.required {
                    if !seen_in_entry.contains(&req_tag) {
                        return Err(ValidationError::session(
                            RejectReason::RequiredTagMissing,
                            Some(req_tag),
                            "group entry missing a required field",
                        ));
                    }
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
        opts: &ValidationOptions,
    ) -> Result<(), ValidationError> {
        let tag = field.tag();
        if opts.validate_fields_have_values && field.value_bytes().is_empty() {
            return Err(ValidationError::session(
                RejectReason::TagSpecifiedWithoutValue,
                Some(tag),
                "group field has no value",
            ));
        }
        if !opts.check_field_types {
            return Ok(());
        }
        let Some(fdef) = gdef
            .child
            .as_deref()
            .and_then(|c| c.field(tag))
            .or_else(|| self.field(tag))
        else {
            return Ok(()); // unknown-tag policy is the top-level loop's job, not this one's
        };
        if !fdef
            .field_type
            .value_ok(field, self.is_legacy_char_lenient())
        {
            return Err(ValidationError::session(
                RejectReason::IncorrectDataFormat,
                Some(tag),
                "group field value has incorrect data format",
            ));
        }
        if let Ok(value) = field.as_str()
            && !fdef.allows(value)
        {
            return Err(ValidationError::session(
                RejectReason::ValueIsIncorrect,
                Some(tag),
                "group field value is not an allowed enumeration",
            ));
        }
        Ok(())
    }
}

/// Parse `"FIX.<major>.<minor>[SP<n>][EP<n>]"` into its four version components.
fn parse_fix_begin_string(bs: &str) -> Option<(u8, u8, Option<u8>, Option<u8>)> {
    let rest = bs.strip_prefix("FIX.")?;
    let mut parts = rest.splitn(2, '.');
    let major: u8 = parts.next()?.parse().ok()?;
    let minor_token = parts.next()?;
    let minor_len = minor_token.bytes().take_while(u8::is_ascii_digit).count();
    let minor: u8 = minor_token.get(..minor_len)?.parse().ok()?;
    let mut suffix = minor_token.get(minor_len..)?;
    let service_pack = parse_version_suffix(&mut suffix, "SP")?;
    let extension_pack = parse_version_suffix(&mut suffix, "EP")?;
    if !suffix.is_empty() {
        return None;
    }
    Some((major, minor, service_pack, extension_pack))
}

fn parse_version_suffix(suffix: &mut &str, marker: &str) -> Option<Option<u8>> {
    let Some(rest) = suffix.strip_prefix(marker) else {
        return Some(None);
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let value = rest.get(..digits)?.parse().ok()?;
    *suffix = rest.get(digits..)?;
    Some(Some(value))
}

/// Whether `tag` is present at the top level of `message`'s header, body, or trailer — a plain
/// field lookup, except when `tag` is itself a group's count tag, in which case presence means
/// that group is `Member::Group`-structured in this section (feature 011: `FieldMap::get()`/
/// `contains()` only match `Member::Field`, so without this, a dictionary-required group's own
/// count tag would be invisible to `validate()`'s `mdef.required`/header/trailer-required checks
/// once that group is structured, even though the group is genuinely present).
fn present(message: &Message, tag: u32) -> bool {
    fn present_in(map: &FieldMap, tag: u32) -> bool {
        map.members().any(|m| match m {
            MemberRef::Field(f) => f.tag() == tag,
            MemberRef::Group { count_tag, .. } => count_tag == tag,
        })
    }
    present_in(&message.header, tag)
        || present_in(&message.body, tag)
        || present_in(&message.trailer, tag)
}
