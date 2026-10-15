//! Shared logic used by `hello-cli` and, via WASM, the Zola site: the
//! primes for its home page and the note descriptions for its piano page.

mod notes;
mod primes;

pub use notes::{MeiError, NoteIndex};
pub use primes::random_consecutive_primes;

/// WASM bindings, compiled only when targeting the browser.
#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::{JsError, wasm_bindgen};

    /// Browser-callable wrapper around [`crate::random_consecutive_primes`].
    ///
    /// Returns bare numbers (`[first, second]`), not a display-ready sentence — the
    /// page's own JS builds whatever text it wants around them.
    #[wasm_bindgen(js_name = randomConsecutivePrimes)]
    pub fn random_consecutive_primes() -> Vec<u64> {
        let (first, second) = crate::random_consecutive_primes();
        vec![first, second]
    }

    /// Browser-side handle on a [`crate::NoteIndex`]: built once from the MEI
    /// Verovio produces (`toolkit.getMEI()`), then asked about clicked notes.
    ///
    /// Unlike the primes, this returns display-ready text: describing a note
    /// in words is the point of it, and keeping that in Rust lets
    /// `cargo test` check it.
    #[wasm_bindgen]
    pub struct NoteIndex(crate::NoteIndex);

    #[wasm_bindgen]
    impl NoteIndex {
        /// Throws if `mei` isn't well-formed XML.
        #[wasm_bindgen(constructor)]
        pub fn new(mei: &str) -> Result<NoteIndex, JsError> {
            Ok(Self(crate::NoteIndex::from_mei(mei)?))
        }

        /// See [`crate::NoteIndex::describe_note`]; `undefined` for an
        /// unknown id.
        #[wasm_bindgen(js_name = describeNote)]
        pub fn describe_note(&self, id: &str) -> Option<String> {
            self.0.describe_note(id)
        }
    }
}

/// Build a greeting for `name`, falling back to "World" when blank.
///
/// ```
/// assert_eq!(hello_core::greet("Ada"), "Hello, Ada!");
/// assert_eq!(hello_core::greet("  "), "Hello, World!");
/// ```
pub fn greet(name: &str) -> String {
    let name = name.trim();
    let name = if name.is_empty() { "World" } else { name };
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use super::greet;

    #[test]
    fn greets_by_name() {
        assert_eq!(greet("Ferris"), "Hello, Ferris!");
    }

    #[test]
    fn blank_defaults_to_world() {
        assert_eq!(greet(""), "Hello, World!");
    }
}
