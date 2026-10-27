use toon_format::encode_default;

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy)]
pub enum ToonValue<'a> {
    Str(&'a str),
    Int(i64),
    Block(&'a str),
}

pub fn toon_doc(fields: &[(&str, ToonValue<'_>)]) -> String {
    let mut map = Map::with_capacity(fields.len());
    for (key, value) in fields {
        map.insert(
            (*key).to_string(),
            match value {
                ToonValue::Str(text) => Value::String((*text).to_string()),
                ToonValue::Int(number) => Value::Number((*number).into()),
                ToonValue::Block(text) => Value::String((*text).to_string()),
            },
        );
    }
    let object = Value::Object(map);
    match encode_default(&object) {
        Ok(text) => text,
        // A tool result that cannot be encoded must not take the app down.
        Err(_) => serde_json::to_string(&object)
            .unwrap_or_else(|_| "error: tool result could not be encoded".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_object_with_strings_and_ints() {
        let text = toon_doc(&[
            ("path", ToonValue::Str("src/foo:bar")),
            ("startLine", ToonValue::Int(3)),
        ]);
        assert!(text.contains("path: "));
        assert!(text.contains("startLine: 3"));
    }
}
