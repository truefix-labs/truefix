//! Standard FIX tag numbers and header/trailer classification (version-agnostic core set).
//!
//! This is the minimal session-level set used to route fields into header/body/trailer when
//! decoding without a dictionary. The full per-version classification lives in `truefix-dict`.

/// SOH (Start Of Header) field separator byte.
pub const SOH: u8 = 0x01;

/// BeginString (always first).
pub const BEGIN_STRING: u32 = 8;
/// BodyLength (always second).
pub const BODY_LENGTH: u32 = 9;
/// MsgType (always third).
pub const MSG_TYPE: u32 = 35;
/// CheckSum (always last).
pub const CHECK_SUM: u32 = 10;
/// MsgSeqNum.
pub const MSG_SEQ_NUM: u32 = 34;
/// SenderCompID.
pub const SENDER_COMP_ID: u32 = 49;
/// TargetCompID.
pub const TARGET_COMP_ID: u32 = 56;
/// SendingTime.
pub const SENDING_TIME: u32 = 52;

/// Returns `true` if `tag` is a standard session-level header field.
pub fn is_header(tag: u32) -> bool {
    matches!(
        tag,
        8 | 9
            | 35
            | 49
            | 56
            | 115
            | 128
            | 90
            | 91
            | 34
            | 50
            | 142
            | 57
            | 143
            | 116
            | 144
            | 129
            | 145
            | 43
            | 97
            | 52
            | 122
            | 212
            | 213
            | 347
            | 369
            | 370
            // GAP-26/FR-032 (feature 006): the standard NoHops (627) repeating group and its
            // member fields -- the one realistic FIX standard-header group, previously entirely
            // unclassified here (so even a direct decode_with_groups caller would misroute it to
            // "body" instead of "header"). Verified against current shipped dictionary content
            // (`crates/truefix-dict/dict-src/normalized/FIX44.fixdict`'s own `group 627 NoHops
            // 628 628,629,630` and `header ... 627` lines) rather than trusted from the audit's
            // own citation, which named the wrong tag (504, actually PaymentDate).
            | 627
            | 628
            | 629
            | 630
    )
}

/// Returns `true` if `tag` is a standard trailer field.
pub fn is_trailer(tag: u32) -> bool {
    matches!(tag, 93 | 89 | 10)
}

/// If `tag` is a length field, returns the tag of the binary data field that immediately
/// follows it (whose value is exactly that many bytes and may contain SOH).
pub fn data_field_for_length(tag: u32) -> Option<u32> {
    Some(match tag {
        90 => 91,   // SecureDataLen -> SecureData
        95 => 96,   // RawDataLength -> RawData
        212 => 213, // XmlDataLen -> XmlData
        348 => 349, // EncodedIssuerLen -> EncodedIssuer
        350 => 351, // EncodedSecurityDescLen -> EncodedSecurityDesc
        352 => 353, // EncodedListExecInstLen -> EncodedListExecInst
        354 => 355, // EncodedTextLen -> EncodedText
        356 => 357, // EncodedSubjectLen -> EncodedSubject
        358 => 359, // EncodedHeadlineLen -> EncodedHeadline
        360 => 361, // EncodedAllocTextLen -> EncodedAllocText
        362 => 363, // EncodedUnderlyingIssuerLen -> EncodedUnderlyingIssuer
        364 => 365, // EncodedUnderlyingSecurityDescLen -> EncodedUnderlyingSecurityDesc
        93 => 89,   // SignatureLength -> Signature (BUG-02, feature 005): the one documented
        // exception to `lengthTag = dataTag - 1` (Signature's length field is 93, not 88).
        // B22/FR-033 (feature 006): the remaining Len/Data pairs present in the shipped
        // dictionaries (verified directly against `dict-src/normalized/FIX50SP2.fixdict`, which
        // enumerates every `EncodedXLen`/`EncodedX` field) but missing from this table — embedded
        // SOH bytes in their content would otherwise corrupt message framing. This corrects (and
        // extends) the audit's own citation, which named two incorrect tag pairs (620->621, which
        // is actually LegSecurityDesc(620)->EncodedLegSecurityDescLen(621), not a Len/Data pair at
        // all; and 1039->1040, which is UnderlyingSettlMethod->SecondaryTradeID, unrelated plain
        // STRING fields) and missed three genuine pairs entirely (1359/1397/1468's Len fields).
        445 => 446,   // EncodedListStatusTextLen -> EncodedListStatusText
        618 => 619,   // EncodedLegIssuerLen -> EncodedLegIssuer
        621 => 622,   // EncodedLegSecurityDescLen -> EncodedLegSecurityDesc
        1359 => 1360, // EncodedSymbolLen -> EncodedSymbol
        1397 => 1398, // EncodedMktSegmDescLen -> EncodedMktSegmDesc
        1468 => 1469, // EncodedSecurityListDescLen -> EncodedSecurityListDesc
        _ => return None,
    })
}
