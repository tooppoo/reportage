//! The location kinds snapshot normalization talks about: where something sits in the schema
//! document, which part of an instance an instruction applies to, and where in one concrete
//! document the application of an instruction actually got to.
//!
//! They are separate types because they answer different questions and must not be interchanged.
//! A schema location always denotes exactly one node of one document and is therefore an RFC 6901
//! JSON Pointer. An instance location denotes a *set* of instance positions, because an annotation
//! under `items` applies to every element of the array, so it is not a pointer at all. An instance
//! pointer is one position of one document, which is what a diagnostic about a document has to
//! name.

use std::fmt;

/// A node of the schema document, as an RFC 6901 JSON Pointer from the document root.
///
/// Reference cycle detection compares resolved targets by this value, so equality must stay
/// structural: two locations are the same node exactly when their decoded token sequences match.
/// Storing decoded tokens and encoding only on output keeps that true for definition names that
/// contain `/` or `~`, whose escaped spelling would otherwise depend on how the location was built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaLocation {
    tokens: Vec<String>,
}

impl SchemaLocation {
    pub fn root() -> SchemaLocation {
        SchemaLocation { tokens: Vec::new() }
    }

    /// The location of `token` inside this node, where `token` is the raw (decoded) member name.
    pub fn child(&self, token: impl Into<String>) -> SchemaLocation {
        let mut tokens = self.tokens.clone();
        tokens.push(token.into());
        SchemaLocation { tokens }
    }

    pub fn is_document_root(&self) -> bool {
        self.tokens.is_empty()
    }

    /// The RFC 6901 pointer text. The document root is the empty string, as the RFC specifies.
    pub fn as_pointer(&self) -> String {
        self.tokens
            .iter()
            .map(|token| format!("/{}", encode_pointer_token(token)))
            .collect()
    }
}

/// Renders the document root as a visible marker instead of as nothing at all, so a diagnostic
/// about the whole document does not read as a diagnostic with its location missing.
/// [`SchemaLocation::as_pointer`] is the machine-facing form.
impl fmt::Display for SchemaLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_document_root() {
            formatter.write_str("<document root>")
        } else {
            formatter.write_str(&self.as_pointer())
        }
    }
}

/// One step of an instance location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstanceSegment {
    /// Descend into an object member.
    Property(String),
    /// Descend into every element of an array.
    ///
    /// Homogeneous `items` describes all elements with one schema, so an annotation below it
    /// selects a set of positions rather than one index.
    ArrayElement,
}

/// The instance positions one normalization instruction applies to.
///
/// Two instructions target the same positions exactly when their segments are equal; identity is
/// structural and never the rendered string, because [`fmt::Display`] is lossy (see its note).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceLocation {
    segments: Vec<InstanceSegment>,
}

impl InstanceLocation {
    pub fn root() -> InstanceLocation {
        InstanceLocation {
            segments: Vec::new(),
        }
    }

    pub fn property(&self, name: impl Into<String>) -> InstanceLocation {
        self.extended(InstanceSegment::Property(name.into()))
    }

    pub fn array_element(&self) -> InstanceLocation {
        self.extended(InstanceSegment::ArrayElement)
    }

    pub fn segments(&self) -> &[InstanceSegment] {
        &self.segments
    }

    fn extended(&self, segment: InstanceSegment) -> InstanceLocation {
        let mut segments = self.segments.clone();
        segments.push(segment);
        InstanceLocation { segments }
    }
}

/// Renders a pointer-shaped path with `*` for "every element of this array", such as
/// `/tests/*/name`.
///
/// This is a diagnostic rendering, not a JSON Pointer: `*` is a legal member name, so the text is
/// ambiguous for a property literally called `*`. Comparisons must use the value, not this string.
impl fmt::Display for InstanceLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.segments.is_empty() {
            return formatter.write_str(INSTANCE_ROOT_MARKER);
        }
        for segment in &self.segments {
            match segment {
                InstanceSegment::Property(name) => {
                    write!(formatter, "/{}", encode_pointer_token(name))?;
                }
                InstanceSegment::ArrayElement => formatter.write_str("/*")?,
            }
        }
        Ok(())
    }
}

/// One step of an instance pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstanceToken {
    Property(String),
    /// The array index the walk took, which an [`InstanceSegment::ArrayElement`] never names.
    Index(usize),
}

/// One position of one instance document, as an RFC 6901 JSON Pointer.
///
/// This is what an instruction's [`InstanceLocation`] becomes once a document is walked: the array
/// segments that stood for "every element" are the indices actually visited. A diagnostic about a
/// document needs this and not the pattern, because "an element of `/tests` has the wrong shape"
/// does not say which one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstancePointer {
    tokens: Vec<InstanceToken>,
}

impl InstancePointer {
    pub fn root() -> InstancePointer {
        InstancePointer { tokens: Vec::new() }
    }

    pub fn property(&self, name: impl Into<String>) -> InstancePointer {
        self.extended(InstanceToken::Property(name.into()))
    }

    pub fn index(&self, index: usize) -> InstancePointer {
        self.extended(InstanceToken::Index(index))
    }

    pub fn tokens(&self) -> &[InstanceToken] {
        &self.tokens
    }

    pub fn is_document_root(&self) -> bool {
        self.tokens.is_empty()
    }

    /// The RFC 6901 pointer text. The whole document is the empty string, as the RFC specifies.
    pub fn as_pointer(&self) -> String {
        self.tokens
            .iter()
            .map(|token| match token {
                InstanceToken::Property(name) => format!("/{}", encode_pointer_token(name)),
                InstanceToken::Index(index) => format!("/{index}"),
            })
            .collect()
    }

    fn extended(&self, token: InstanceToken) -> InstancePointer {
        let mut tokens = self.tokens.clone();
        tokens.push(token);
        InstancePointer { tokens }
    }
}

/// Renders the whole document as a visible marker instead of as nothing at all, so a diagnostic
/// about the document does not read as a diagnostic with its location missing.
/// [`InstancePointer::as_pointer`] is the machine-facing form.
///
/// The marker is [`InstanceLocation`]'s and deliberately not [`SchemaLocation`]'s: an application
/// diagnostic prints an instance position and a schema position side by side, and one name for both
/// roots would undo the reason it carries both.
impl fmt::Display for InstancePointer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_document_root() {
            formatter.write_str(INSTANCE_ROOT_MARKER)
        } else {
            formatter.write_str(&self.as_pointer())
        }
    }
}

/// How both instance location kinds render the whole document.
const INSTANCE_ROOT_MARKER: &str = "<instance root>";

/// Escapes a raw member name into an RFC 6901 reference token.
///
/// `~` must be escaped before `/`, otherwise the `~` introduced for `/` would be escaped again.
pub fn encode_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Decodes an RFC 6901 reference token, or returns `None` when an escape is malformed.
///
/// A single left-to-right pass rather than two sequential string replacements: `~01` must decode to
/// `~1`, which only holds if the `~` a `~0` produces is never re-read as the start of an escape.
pub fn decode_pointer_token(token: &str) -> Option<String> {
    let mut decoded = String::with_capacity(token.len());
    let mut characters = token.chars();
    while let Some(character) = characters.next() {
        if character != '~' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('0') => decoded.push('~'),
            Some('1') => decoded.push('/'),
            _ => return None,
        }
    }
    Some(decoded)
}
