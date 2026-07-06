//! T115 — FIX Repository repeating components exclude their NoXxx count field from group members.

#![cfg(feature = "dict-tooling")]

use truefix_dict::fix_repository::{RepositorySource, convert};

const FIELDS: &str = r#"
<Fields>
  <Field><Tag>8</Tag><Name>BeginString</Name><Type>STRING</Type></Field>
  <Field><Tag>9</Tag><Name>BodyLength</Name><Type>LENGTH</Type></Field>
  <Field><Tag>10</Tag><Name>CheckSum</Name><Type>STRING</Type></Field>
  <Field><Tag>35</Tag><Name>MsgType</Name><Type>STRING</Type></Field>
  <Field><Tag>447</Tag><Name>PartyIDSource</Name><Type>CHAR</Type></Field>
  <Field><Tag>448</Tag><Name>PartyID</Name><Type>STRING</Type></Field>
  <Field><Tag>453</Tag><Name>NoPartyIDs</Name><Type>NUMINGROUP</Type></Field>
</Fields>
"#;

const COMPONENTS: &str = r#"
<Components>
  <Component><ComponentID>1</ComponentID><ComponentType>Block</ComponentType><Name>StandardHeader</Name></Component>
  <Component><ComponentID>2</ComponentID><ComponentType>Block</ComponentType><Name>StandardTrailer</Name></Component>
  <Component><ComponentID>3</ComponentID><ComponentType>BlockRepeating</ComponentType><Name>Parties</Name></Component>
</Components>
"#;

const CONTENTS: &str = r#"
<MsgContents>
  <MsgContent><ComponentID>1</ComponentID><TagText>8</TagText><Indent>0</Indent><Position>1</Position><Reqd>1</Reqd></MsgContent>
  <MsgContent><ComponentID>1</ComponentID><TagText>9</TagText><Indent>0</Indent><Position>2</Position><Reqd>1</Reqd></MsgContent>
  <MsgContent><ComponentID>1</ComponentID><TagText>35</TagText><Indent>0</Indent><Position>3</Position><Reqd>1</Reqd></MsgContent>
  <MsgContent><ComponentID>2</ComponentID><TagText>10</TagText><Indent>0</Indent><Position>1</Position><Reqd>1</Reqd></MsgContent>
  <MsgContent><ComponentID>3</ComponentID><TagText>453</TagText><Indent>0</Indent><Position>1</Position><Reqd>0</Reqd></MsgContent>
  <MsgContent><ComponentID>3</ComponentID><TagText>448</TagText><Indent>1</Indent><Position>2</Position><Reqd>1</Reqd></MsgContent>
  <MsgContent><ComponentID>3</ComponentID><TagText>447</TagText><Indent>1</Indent><Position>3</Position><Reqd>0</Reqd></MsgContent>
</MsgContents>
"#;

#[test]
fn count_field_is_excluded_and_first_real_member_is_the_delimiter() {
    let source = RepositorySource {
        fields_xml: FIELDS,
        enums_xml: "<Enums/>",
        components_xml: COMPONENTS,
        messages_xml: "<Messages/>",
        msg_contents_xml: CONTENTS,
    };

    let generated = convert(&source, "FIX.TEST").expect("convert fixture");

    assert!(
        generated.contains("group 453 Parties 448 448,447\n"),
        "unexpected group directive:\n{generated}"
    );
    assert!(
        !generated.contains("group 453 Parties 453 453,448,447"),
        "the NoPartyIDs count field must not become its own delimiter/member"
    );
    truefix_dict::parse(&generated).expect("converted dictionary remains runtime-parseable");
}
