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
        _ => return None,
    })
}
