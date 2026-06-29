//! Runtime message validation against a [`DataDictionary`].

use truefix_core::Message;

use crate::model::{DataDictionary, RejectReason, ValidationError, ValidationOptions};

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

        for field in message
            .header
            .fields()
            .chain(message.body.fields())
            .chain(message.trailer.fields())
        {
            let tag = field.tag();

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
                    if opts.check_field_types {
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
                        self.is_header(tag) || self.is_trailer(tag) || mdef.allows_tag(tag);
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
}

fn present(message: &Message, tag: u32) -> bool {
    message.header.get(tag).is_some()
        || message.body.get(tag).is_some()
        || message.trailer.get(tag).is_some()
}
