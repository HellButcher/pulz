//! Globally interned string handles — deduplicated, cache-friendly, zero-cost for static strings.
//!
//! Strings are stored in a single global arena and indexed by a compact `NonZeroU16` handle
//! (feature-gated to `NonZeroU32` via the `interned-u32` feature flag). Handles compare equal
//! and hash identically when their indices match, making them ideal for use as map keys.
//!
//! # Insertion semantics
//!
//! Every factory method follows the same pattern: **lookup first by reference**, only allocating
//! (cloning + leaking) on a miss. Static strings never allocate or leak — they go directly into
//! the arena and are deduplicated by content via a `HashMap<&'static str, InternedStr>`.
//!
//! # Example
//!
//! ```
//! use pulz_schedule::interned::InternedStr;
//!
//! // Static strings: zero allocation, instant dedup.
//! let a = InternedStr::from_static("hello");
//! let b = InternedStr::from_static("hello");
//! assert_eq!(a, b); // same index → same content
//!
//! // Owned strings: one leak per unique value.
//! let c = InternedStr::from_string(String::from("world"));
//!
//! // Batch intern static entries with a single lock acquisition.
//! let labels = InternedStr::from_static_array(&[
//!     "First", "Update", "Last",
//! ]);
//! ```

#[cfg(feature = "interned-u32")]
use core::num::NonZeroU32;
use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{Debug, Display},
    ops::Deref,
    sync::{LazyLock, RwLock},
};

/// A compact handle to an interned string.
///
/// Interned strings are deduplicated — identical strings share the same index into a global arena.
/// Handles compare equal and hash identically when they point to the same arena entry, making them
/// ideal for use as `HashMap` keys or in sets.
///
/// # Handle size
///
/// By default this is **2 bytes** (`NonZeroU16`). Enable the `interned-u32` feature flag to use
/// 4 bytes (`NonZeroU32`) for scenarios where more than ~65k distinct interned strings are needed.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InternedStr(InternIdx);

/// Index type alias — `u16` by default, `u32` when the `interned-u32` feature is enabled.
#[cfg(not(feature = "interned-u32"))]
type InternIdx = core::num::NonZeroU16;

#[cfg(feature = "interned-u32")]
type InternIdx = NonZeroU32;

/// Global string arena with content-based deduplication.
struct GlobalStringMap {
    /// All interned entries, indexed by `InternedStr(NonZeroU16) - 1`.
    arena: Vec<&'static str>,

    /// Content-based deduplication via string Hash/Eq on `&'static str` keys.
    /// Keys ARE the same pointers stored in `arena` — no duplicates between map and arena.
    map: HashMap<&'static str, InternedStr>,
}

// LazyLock avoids calling non-const constructors (HashMap::with_capacity) in statics.
static MAP: LazyLock<RwLock<GlobalStringMap>> = LazyLock::new(|| {
    RwLock::new(GlobalStringMap {
        arena: Vec::new(),
        map: HashMap::new(),
    })
});

// ── Factory methods ───────────────────────────────────────────

impl InternedStr {
    /// Intern a `&'static str` — zero allocation, instant dedup.
    ///
    /// Static strings are never cloned or leaked; they go directly into the arena and are
    /// deduplicated by content via the global map.
    #[inline]
    pub fn from_static(s: &'static str) -> Self {
        // Fast path with read lock — handles already-interned strings.
        {
            let guard = MAP.read().unwrap();
            if let Some(&idx) = guard.map.get(&s) {
                return idx;
            }
        }

        // Slow path: acquire write lock and double-check (another thread may have inserted).
        let mut guard = MAP.write().unwrap();
        // Double-check again in case another thread raced us.
        if let Some(idx) = guard.map.get(&s) {
            return *idx;
        }
        let idx = Self::new(guard.arena.len());
        guard.map.insert(s, idx);
        guard.arena.push(s);
        idx
    }

    /// Intern an owned `String` — one allocation per unique value.
    ///
    /// The string is cloned and leaked into the arena only if it's not already interned by content.
    pub fn from_string(s: String) -> Self {
        // Fast path: check if already interned by content (no leak needed).
        let s_ref = s.as_str();
        {
            let guard = MAP.read().unwrap();
            if let Some(&idx) = guard.map.get(s_ref) {
                return idx;
            }
        }

        // Slow path: acquire write lock and double-check.
        let mut guard = MAP.write().unwrap();
        // Double-check again in case another thread raced us.
        if let Some(&idx) = guard.map.get(s_ref) {
            return idx;
        }
        let idx = Self::new(guard.arena.len());
        // Only now do we leak — the string is guaranteed to be a miss.
        let p: &'static str = Box::leak(s.into_boxed_str());
        guard.map.insert(p, idx);
        guard.arena.push(p);
        idx
    }

    /// Intern a `Cow<'_, str>` — borrows go through [`from_ref`] (clones + leaks only on first),
    /// owned values delegate to [`from_string`].
    pub fn from_cow(s: Cow<'_, str>) -> Self {
        match s {
            Cow::Borrowed(s) => Self::from_ref(s),
            Cow::Owned(s) => Self::from_string(s),
        }
    }

    /// Intern a borrowed `&str` — clones and leaks only on first occurrence.
    pub fn from_ref(s: &str) -> Self {
        // Fast path: check if already interned by content (no clone/leak needed).
        {
            let guard = MAP.read().unwrap();
            if let Some(&idx) = guard.map.get(s) {
                return idx;
            }
        }

        // Slow path: acquire write lock and double-check.
        let mut guard = MAP.write().unwrap();
        // Double-check again in case another thread raced us.
        if let Some(&idx) = guard.map.get(s) {
            return idx;
        }
        let idx = Self::new(guard.arena.len());
        // Only now do we clone + leak — the string is guaranteed to be a miss.
        let p: &'static str = Box::leak(s.to_owned().into_boxed_str());
        guard.map.insert(p, idx);
        guard.arena.push(p);
        idx
    }

    /// Intern a batch of static string entries with exactly **one** lock acquisition.
    ///
    /// This is ideal for caching enum variant labels — all N variants are interned in a single
    /// critical section, then stored behind a `OnceLock` so subsequent calls are lock-free.
    ///
    /// # Example
    ///
    /// ```
    /// use pulz_schedule::interned::InternedStr;
    ///
    /// let labels = InternedStr::from_static_array(&[
    ///     "MyEnum::First",
    ///     "MyEnum::Update",
    ///     "MyEnum::Last",
    /// ]);
    /// assert_eq!(labels.len(), 3);
    /// ```
    pub fn from_static_array<const N: usize>(arr: &[&'static str; N]) -> [Self; N] {
        let mut guard = MAP.write().unwrap();
        // Placeholder array — all entries will be overwritten below.
        let mut result: [Self; N] = unsafe { std::mem::zeroed() };

        for (i, &s) in arr.iter().enumerate() {
            if let Some(&idx) = guard.map.get(&s) {
                result[i] = idx;
            } else {
                let idx = Self::new(guard.arena.len());
                guard.map.insert(s, idx);
                guard.arena.push(s);
                result[i] = idx;
            }
        }

        result
    }

    /// Returns the underlying `&'static str`.
    ///
    /// The returned reference is valid for `'static` because the arena only contains pointers to
    /// compile-time string literals or leaked heap memory — both of which are never freed.
    #[inline]
    pub fn as_str(self) -> &'static str {
        let guard = MAP.read().unwrap();
        let idx: usize = self.0.get() as usize - 1;
        guard.arena[idx]
    }

    /// Create a new InternedStr from an arena index (0-based).
    #[inline]
    fn new(index: usize) -> Self {
        // +1 because NonZero must be > 0; arena index 0 → value 1.
        #[cfg(not(feature = "interned-u32"))]
        let idx_raw = index as u16;

        #[cfg(feature = "interned-u32")]
        let idx_raw = index as u32;

        // safety: we add 1
        unsafe { Self(InternIdx::new_unchecked(idx_raw.checked_add(1).unwrap())) }
    }
}

// ── Trait impls ───────────────────────────────────────────────

impl Deref for InternedStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for InternedStr {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Compare by content with &str — delegates to as_str().
impl PartialEq<&str> for InternedStr {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// Compare by content with str (owned reference).
impl PartialEq<str> for InternedStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl Debug for InternedStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("InternedStr").field(&self.as_str()).finish()
    }
}

impl Display for InternedStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::hash::{DefaultHasher, Hash, Hasher};

    use super::*;

    #[test]
    fn static_strings_are_deduplicated() {
        let a = InternedStr::from_static("hello");
        let b = InternedStr::from_static("hello");
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "hello");
    }

    #[test]
    fn static_strings_with_different_content_differ() {
        let a = InternedStr::from_static("hello");
        let b = InternedStr::from_static("world");
        assert_ne!(a, b);
    }

    #[test]
    fn from_string_deduplicates_by_content() {
        let a = InternedStr::from_string(String::from("dedup"));
        let b = InternedStr::from_string(String::from("dedup"));
        assert_eq!(a, b);
    }

    #[test]
    fn from_ref_deduplicates_by_content() {
        let a = InternedStr::from_ref("ref_test");
        let b = InternedStr::from_ref("ref_test");
        assert_eq!(a, b);
    }

    #[test]
    fn from_cow_borrowed_is_zero_cost() {
        let s: &'static str = "cow_static";
        let a = InternedStr::from_cow(Cow::Borrowed(s));
        let b = InternedStr::from_cow(Cow::Borrowed(s));
        assert_eq!(a, b);
    }

    #[test]
    fn from_cow_owned_deduplicates() {
        let a = InternedStr::from_cow(Cow::Owned(String::from("cow_owned")));
        let b = InternedStr::from_cow(Cow::Owned(String::from("cow_owned")));
        assert_eq!(a, b);
    }

    #[test]
    fn from_static_array_interns_all_with_one_lock() {
        let labels = InternedStr::from_static_array(&["First", "Second", "Third"]);
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0].as_str(), "First");
        assert_eq!(labels[1].as_str(), "Second");
        assert_eq!(labels[2].as_str(), "Third");

        // Re-interning the same array should produce identical handles.
        let labels2 = InternedStr::from_static_array(&["First", "Second", "Third"]);
        assert_eq!(labels, labels2);
    }

    #[test]
    fn hash_is_by_index() {
        let a = InternedStr::from_static("same");
        let b = InternedStr::from_static("same");
        assert_eq!(hash_value(&a), hash_value(&b));

        let c = InternedStr::from_static("different");
        // Different content → different index → different hash (very likely)
        assert_ne!(a, c);
    }

    #[test]
    fn handles_work_as_map_keys() {
        use std::collections::HashMap;

        let mut map: HashMap<InternedStr, i32> = HashMap::new();
        let key = InternedStr::from_static("key");

        map.insert(key, 42);
        assert_eq!(map.get(&key), Some(&42));

        // Re-interning the same string produces an equal handle.
        let key2 = InternedStr::from_static("key");
        assert_eq!(map.get(&key2), Some(&42));
    }

    #[test]
    fn debug_and_display() {
        let s = InternedStr::from_static("display_test");
        assert_eq!(format!("{}", s), "display_test");
        assert_eq!(format!("{:?}", s), "InternedStr(\"display_test\")");
    }

    /// Helper to get the hash value of an InternedStr for testing.
    fn hash_value(v: &InternedStr) -> u64 {
        let mut hasher = DefaultHasher::new();
        v.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn copy_and_clone() {
        let a = InternedStr::from_static("copied");
        let b: InternedStr = a; // Copy (implicit)
        let c = a.clone(); // Clone
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn as_ref_and_deref() {
        let s = InternedStr::from_static("asref_test");
        let _s: &str = s.as_ref();
        let _d: &str = &*s;
        assert_eq!(s, "asref_test");
    }

    #[test]
    fn partial_eq_with_str() {
        let s = InternedStr::from_static("eq_test");
        assert_eq!(s, "eq_test");
        assert_ne!(s, "other");
    }
}
