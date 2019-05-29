use pretty_assertions::assert_eq;
use serde_json::Value;
use std::io::Cursor;
use std::path::PathBuf;

#[test]
fn it_can_calculate_loc_on_files() {
    let root = PathBuf::from("./tests/data/simple/");

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let result = lati_scanner::run(root, vec!["loc".to_string()], out);

    assert!(!result.is_err());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice()).unwrap();

    let expected =
        std::fs::read_to_string(PathBuf::from("./tests/expected/simple_files_with_loc.json"))
            .unwrap();
    let parsed_expected: Value = serde_json::from_str(&expected).unwrap();

    // TODO: how can we use test_helpers??
    assert_eq!(parsed_result, parsed_expected);
}
