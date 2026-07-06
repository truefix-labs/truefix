#![cfg(feature = "dict-tooling")]

use truefix_dict::orchestra::convert;

#[test]
fn orchestra_maps_every_type_supported_by_the_fix_repository_converter() {
    let cases = [
        ("PRICEOFFSET", "PRICEOFFSET"),
        ("TIME", "TIME"),
        ("UTCDATE", "UTCDATE"),
        ("DAYOFMONTH", "DAYOFMONTH"),
        ("CURRENCY", "CURRENCY"),
        ("EXCHANGE", "EXCHANGE"),
        ("MULTIPLEVALUESTRING", "MULTIPLEVALUESTRING"),
        ("MULTIPLESTRINGVALUE", "MULTIPLESTRINGVALUE"),
        ("MULTIPLECHARVALUE", "MULTIPLECHARVALUE"),
        ("COUNTRY", "COUNTRY"),
        ("LOCALMKTDATE", "LOCALMKTDATE"),
    ];

    for (index, (orchestra_type, normalized_type)) in cases.into_iter().enumerate() {
        let tag = 10_000 + index;
        let xml = format!(
            r#"<fixr:repository version="FIX.Latest">
                <fixr:fields>
                    <fixr:field id="{tag}" name="Field{tag}" type="{orchestra_type}"/>
                </fixr:fields>
            </fixr:repository>"#
        );

        let converted = convert(&xml).unwrap_or_else(|error| {
            panic!("failed to map Orchestra type {orchestra_type}: {error}")
        });
        assert!(
            converted.contains(&format!("field {tag} Field{tag} {normalized_type}\n")),
            "{orchestra_type} must map to the distinct normalized type {normalized_type}:\n\
             {converted}"
        );
    }
}
