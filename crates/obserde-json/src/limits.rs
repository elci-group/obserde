/// Limits applied while decoding JSON text into a `Document`.
///
/// **Honest scope note**: `max_input_bytes` is checked before any parsing
/// happens and is the real, allocation-bounding resource-exhaustion
/// defense — `serde_json::from_str::<serde_json::Value>` fully
/// materializes the parsed tree in memory before `max_string_len` or
/// `max_collection_len` are ever consulted, so those two are *post-parse*
/// shape/policy rejections (useful for catching "small on the wire but
/// absurd in shape" payloads a schema wouldn't want), not independent
/// pre-allocation guards. Plain JSON has no entity-expansion mechanism
/// (unlike, say, XML entities), so the amplification factor between wire
/// bytes and parsed-tree memory is small and constant — `max_input_bytes`
/// alone already gives a real, fixed worst-case memory bound regardless
/// of the other two knobs.
///
/// `max_depth` *is* enforced cheaply, before any large allocation, because
/// `serde_json`'s own `Deserializer` aborts a maximally-nested payload
/// around its own hard-coded recursion guard almost immediately. That
/// guard is fixed at 128 by the `serde_json` version this crate depends
/// on and cannot be raised without enabling its `unbounded_depth`
/// feature — which must never be enabled, since that feature exists
/// specifically to opt out of the DoS guard `max_depth` here relies on.
/// A `max_depth` above 128 is therefore unreachable in practice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Maximum size, in bytes, of the raw JSON input. Checked before
    /// parsing begins.
    pub max_input_bytes: usize,
    /// Maximum nesting depth (each `List`/`Map` level counts as one).
    pub max_depth: usize,
    /// Maximum string length, in Unicode scalar values
    /// (`str::chars().count()`, not bytes).
    pub max_string_len: usize,
    /// Maximum number of entries in a single `List` or `Map`.
    pub max_collection_len: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 10 * 1024 * 1024,
            max_depth: 64,
            max_string_len: 1_000_000,
            max_collection_len: 100_000,
        }
    }
}
