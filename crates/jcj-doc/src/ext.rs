// SPDX-License-Identifier: MIT OR Apache-2.0
//! Typed extension data on documents, paragraphs and runs.
//!
//! ```
//! use jcj_doc::{ExtKey, HasExt, Paragraph};
//!
//! // Declared once, in the app that owns the namespace.
//! const SCENE_NUMBER: ExtKey<String> = ExtKey::new("script.scene_number");
//!
//! let mut p = Paragraph::new();
//! SCENE_NUMBER.set(&mut p, &"12A".to_string()).unwrap();
//! assert_eq!(SCENE_NUMBER.get(&p).unwrap(), Some("12A".to_string()));
//! SCENE_NUMBER.remove(&mut p);
//! assert!(!SCENE_NUMBER.is_set(&p));
//! ```

use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{CharProps, Document, Ext, ExtKeyError, ParaProps, Paragraph, Run};

/// Why a typed extension value could not be read or written.
#[derive(Debug)]
pub enum ExtError {
    /// The key is not `namespace.name` (see [`crate::valid_ext_key`]).
    Key(ExtKeyError),
    /// The stored value is not the type the key says (written by another version, or by hand).
    Value { key: &'static str, error: serde_json::Error },
}

impl std::fmt::Display for ExtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtError::Key(e) => e.fmt(f),
            ExtError::Value { key, error } => write!(f, "extension value `{key}`: {error}"),
        }
    }
}

impl std::error::Error for ExtError {}

/// Something that carries extension data: a document, a paragraph, a run, or their properties.
pub trait HasExt {
    fn ext(&self) -> &Ext;
    /// Mutable access. For a [`Paragraph`] this also marks it changed, so layout caches redo it.
    fn ext_mut(&mut self) -> &mut Ext;
}

impl HasExt for Ext {
    fn ext(&self) -> &Ext {
        self
    }
    fn ext_mut(&mut self) -> &mut Ext {
        self
    }
}

impl HasExt for Document {
    fn ext(&self) -> &Ext {
        &self.ext
    }
    fn ext_mut(&mut self) -> &mut Ext {
        &mut self.ext
    }
}

impl HasExt for ParaProps {
    fn ext(&self) -> &Ext {
        &self.ext
    }
    fn ext_mut(&mut self) -> &mut Ext {
        &mut self.ext
    }
}

impl HasExt for CharProps {
    fn ext(&self) -> &Ext {
        &self.ext
    }
    fn ext_mut(&mut self) -> &mut Ext {
        &mut self.ext
    }
}

impl HasExt for Run {
    fn ext(&self) -> &Ext {
        &self.props.ext
    }
    fn ext_mut(&mut self) -> &mut Ext {
        &mut self.props.ext
    }
}

impl HasExt for Paragraph {
    fn ext(&self) -> &Ext {
        &self.props.ext
    }
    fn ext_mut(&mut self) -> &mut Ext {
        self.touch();
        &mut self.props.ext
    }
}

/// A typed, namespaced extension key. Declare one `const` per value an app keeps:
/// `const STATUS: ExtKey<Status> = ExtKey::new("manuscript.status");`
pub struct ExtKey<T> {
    key: &'static str,
    _value: PhantomData<fn() -> T>,
}

impl<T> Clone for ExtKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ExtKey<T> {}

impl<T> std::fmt::Debug for ExtKey<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExtKey({})", self.key)
    }
}

impl<T: Serialize + DeserializeOwned> ExtKey<T> {
    /// A key; `key` must be `namespace.name` (checked when a value is first written).
    pub const fn new(key: &'static str) -> ExtKey<T> {
        ExtKey { key, _value: PhantomData }
    }
    pub fn key(&self) -> &'static str {
        self.key
    }
    pub fn namespace(&self) -> &'static str {
        self.key.split_once('.').map_or(self.key, |(ns, _)| ns)
    }
    /// The value, `Ok(None)` when unset, or an error if the stored value has another shape.
    pub fn get(&self, on: &impl HasExt) -> Result<Option<T>, ExtError> {
        match on.ext().get(self.key) {
            None => Ok(None),
            Some(v) => serde_json::from_value(v.clone()).map(Some).map_err(|error| ExtError::Value { key: self.key, error }),
        }
    }
    /// The value, or `None` when unset or unreadable.
    pub fn get_or_none(&self, on: &impl HasExt) -> Option<T> {
        self.get(on).ok().flatten()
    }
    pub fn is_set(&self, on: &impl HasExt) -> bool {
        on.ext().contains(self.key)
    }
    /// Set the value. A value that serializes to JSON `null` removes the key.
    pub fn set(&self, on: &mut impl HasExt, value: &T) -> Result<(), ExtError> {
        let v = serde_json::to_value(value).map_err(|error| ExtError::Value { key: self.key, error })?;
        on.ext_mut().set(self.key, v).map_err(ExtError::Key)
    }
    pub fn remove(&self, on: &mut impl HasExt) {
        if self.is_set(on) {
            on.ext_mut().remove(self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Status {
        label: String,
        done: bool,
    }

    const STATUS: ExtKey<Status> = ExtKey::new("test.status");
    const COUNT: ExtKey<u32> = ExtKey::new("test.count");
    const BAD: ExtKey<u32> = ExtKey::new("nonamespace");

    #[test]
    fn typed_values_round_trip_on_every_carrier() {
        let s = Status { label: "Draft".into(), done: false };
        let mut d = Document::new();
        STATUS.set(&mut d, &s).unwrap();
        assert_eq!(STATUS.get(&d).unwrap(), Some(Status { label: "Draft".into(), done: false }));
        let mut c = CharProps::default();
        COUNT.set(&mut c, &3).unwrap();
        assert_eq!(COUNT.get_or_none(&c), Some(3));
        assert_eq!(STATUS.namespace(), "test");
    }

    #[test]
    fn a_wrong_shape_is_an_error_not_a_panic() {
        let mut p = ParaProps::default();
        COUNT.set(&mut p, &3).unwrap();
        let wrong: ExtKey<Status> = ExtKey::new("test.count");
        assert!(matches!(wrong.get(&p), Err(ExtError::Value { .. })));
        assert_eq!(wrong.get_or_none(&p), None);
    }

    #[test]
    fn keys_without_a_namespace_are_refused() {
        let mut p = ParaProps::default();
        assert!(matches!(BAD.set(&mut p, &1), Err(ExtError::Key(_))));
    }

    #[test]
    fn setting_a_paragraph_value_marks_it_changed() {
        let mut p = Paragraph::new();
        let before = p.rev;
        COUNT.set(&mut p, &1).unwrap();
        assert_ne!(p.rev, before);
    }
}
