/// A format-agnostic decoded value tree.
///
/// `Map` is `Vec<(String, Document)>`, not a `HashMap`/`BTreeMap`: it
/// preserves the original key order and makes duplicate keys structurally
/// observable, which matters for validation diagnostics that need to point
/// at, say, "the second `score` key". Canonical key ordering is
/// `obserde-canonical`'s explicit job, not something baked into storage
/// here.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Document {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Document>),
    Map(Vec<(String, Document)>),
}

impl Document {
    pub fn type_name(&self) -> &'static str {
        match self {
            Document::Null => "null",
            Document::Bool(_) => "bool",
            Document::Integer(_) => "integer",
            Document::Float(_) => "float",
            Document::String(_) => "string",
            Document::Bytes(_) => "bytes",
            Document::List(_) => "list",
            Document::Map(_) => "map",
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Document::Null)
    }

    pub fn as_map(&self) -> Option<&[(String, Document)]> {
        match self {
            Document::Map(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Document]> {
        match self {
            Document::List(items) => Some(items),
            _ => None,
        }
    }

    /// Looks up the first entry with the given key, if this is a `Map`.
    pub fn get(&self, key: &str) -> Option<&Document> {
        self.as_map()?.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_name_per_variant() {
        assert_eq!(Document::Null.type_name(), "null");
        assert_eq!(Document::Bool(true).type_name(), "bool");
        assert_eq!(Document::Integer(1).type_name(), "integer");
        assert_eq!(Document::Float(1.0).type_name(), "float");
        assert_eq!(Document::String("s".into()).type_name(), "string");
        assert_eq!(Document::Bytes(vec![]).type_name(), "bytes");
        assert_eq!(Document::List(vec![]).type_name(), "list");
        assert_eq!(Document::Map(vec![]).type_name(), "map");
    }

    #[test]
    fn is_null() {
        assert!(Document::Null.is_null());
        assert!(!Document::Bool(false).is_null());
    }

    #[test]
    fn map_get_finds_first_matching_key() {
        let doc = Document::Map(vec![
            ("a".to_string(), Document::Integer(1)),
            ("a".to_string(), Document::Integer(2)),
        ]);
        assert_eq!(doc.get("a"), Some(&Document::Integer(1)));
        assert_eq!(doc.get("missing"), None);
    }

    #[test]
    fn as_map_and_as_list_are_variant_specific() {
        let map = Document::Map(vec![]);
        let list = Document::List(vec![]);
        assert!(map.as_map().is_some());
        assert!(map.as_list().is_none());
        assert!(list.as_list().is_some());
        assert!(list.as_map().is_none());
    }
}
