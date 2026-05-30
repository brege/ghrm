pub(crate) type Pair = (String, String);

pub(crate) fn parse_bool(raw: &str) -> Option<bool> {
    match raw {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

pub(crate) fn parse_pairs(raw: &str) -> Vec<Pair> {
    if raw.is_empty() {
        return Vec::new();
    }
    form_urlencoded::parse(raw.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

pub(crate) fn encode_pairs(pairs: &[Pair]) -> String {
    let mut out = form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        out.append_pair(key, value);
    }
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bool_accepts_explicit_values() {
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("false"), Some(false));
    }

    #[test]
    fn parse_pairs_decodes_query_encoding() {
        assert_eq!(
            parse_pairs("group=docs&group=web+dev&path=odd%2Fname"),
            vec![
                ("group".to_string(), "docs".to_string()),
                ("group".to_string(), "web dev".to_string()),
                ("path".to_string(), "odd/name".to_string()),
            ]
        );
    }

    #[test]
    fn encode_pairs_preserves_empty_values() {
        assert_eq!(
            encode_pairs(&[
                ("group".to_string(), String::new()),
                ("hidden".to_string(), "1".to_string()),
            ]),
            "group=&hidden=1"
        );
    }
}
