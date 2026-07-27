//! Resolution of a source-level [`TextValueExpression`] into the `TextValue` a
//! runtime consumer compares or writes.
//!
//! Every `TextValue` consumer resolves through [`ResolveTextValue`] instead of
//! branching on the expression's variant itself, so raw text, a direct binding
//! reference, and an interpolated literal reach `write` and the text matchers
//! by one path. After resolution a consumer holds a [`ResolvedTextValue`] and
//! never re-inspects the source expression: the provenance it needs for
//! diagnostics and artifacts is already attached.
//!
//! Resolution takes a context and can fail, rather than being an `AsRef` /
//! `Deref` / `Into` conversion: it needs the binding environment, and it must
//! be able to report which binding was unavailable.
//!
//! See docs/adr/20260726T060000Z_interpolated-text-literal.md.

use std::collections::HashMap;

use crate::diagnostic::DiagnosticCode;
use crate::model::{
    Binding, BindingSource, BoundValue, InterpolatedTextForm, InterpolatedTextSegment, LocatedSpan,
    TextLiteral, TextValue, TextValueExpression,
};

/// The environment a text value expression resolves against.
pub struct TextResolutionContext<'a> {
    bindings: &'a HashMap<String, Binding>,
}

impl<'a> TextResolutionContext<'a> {
    pub fn new(bindings: &'a HashMap<String, Binding>) -> Self {
        Self { bindings }
    }
}

/// A resolved text value: the evaluated `TextValue` plus where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTextValue {
    value: TextValue,
    provenance: TextValueProvenance,
}

impl ResolvedTextValue {
    /// The resolved value alone, for a consumer that records no provenance of
    /// its own (a `write` step, whose evidence is the written file).
    pub fn into_value(self) -> TextValue {
        self.value
    }

    pub fn into_parts(self) -> (TextValue, TextValueProvenance) {
        (self.value, self.provenance)
    }
}

/// Where a resolved `TextValue` came from, for diagnostics and artifacts.
///
/// This is the source-form record runtime consumers read after resolution, so
/// none of them has to look back at the expression. It never carries a
/// resolved interpolation result in full: the literal segments are already in
/// the script source, and the substituted binding values are described by
/// reference rather than reproduced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextValueProvenance {
    /// A raw `"..."` string literal.
    Quoted(String),
    /// A raw heredoc literal, already dedented.
    Heredoc(String),
    /// A direct `&name` binding reference.
    Binding { name: String, source: BindingSource },
    /// An interpolated text literal, described by its source form, its span,
    /// and the bindings it substituted.
    Interpolated {
        form: InterpolatedTextForm,
        span: LocatedSpan,
        references: Vec<ResolvedBindingReference>,
    },
}

impl TextValueProvenance {
    /// A bounded description of an interpolated literal, used wherever an
    /// expected value is named rather than compared.
    ///
    /// Names the form, the literal's source line, and the bindings involved,
    /// so a script with several interpolated literals over the same binding
    /// stays unambiguous. It deliberately does not reproduce the resolved
    /// value, which mixes script text with captured process output; a
    /// mismatch's own bounded, escaped context window remains the only place
    /// any of that value is shown. Every renderer shares this one description
    /// so the human, artifact, and JSON surfaces cannot disagree about what
    /// they name.
    pub fn describe_interpolated(
        form: InterpolatedTextForm,
        span: LocatedSpan,
        references: &[ResolvedBindingReference],
    ) -> String {
        let form = match form {
            InterpolatedTextForm::String => "interpolated string literal",
            InterpolatedTextForm::Heredoc => "interpolated heredoc literal",
        };
        let line = span.line;
        if references.is_empty() {
            return format!("<{form} at line {line}>");
        }
        let names = references
            .iter()
            .map(|reference| format!("&{}", reference.name))
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{form} at line {line} referencing {names}>")
    }
}

/// One binding an interpolated literal substituted: which binding, where the
/// reference is written, and where the binding's value was captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBindingReference {
    pub name: String,
    pub reference_span: LocatedSpan,
    pub source: BindingSource,
}

/// A text value expression that could not be resolved against its context.
///
/// Distinct from a write error and from an assertion mismatch: this is a
/// failure to produce the expected value at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextResolutionError {
    pub message: String,
    pub diagnostic_code: DiagnosticCode,
}

/// Resolves a source-level text expression into the value a consumer uses.
pub trait ResolveTextValue {
    fn resolve_text_value(
        &self,
        context: &TextResolutionContext<'_>,
    ) -> Result<ResolvedTextValue, TextResolutionError>;
}

impl ResolveTextValue for TextValueExpression {
    fn resolve_text_value(
        &self,
        context: &TextResolutionContext<'_>,
    ) -> Result<ResolvedTextValue, TextResolutionError> {
        match self {
            TextValueExpression::Raw(literal) => Ok(ResolvedTextValue {
                value: literal.to_text_value(),
                provenance: match literal {
                    TextLiteral::Quoted(value) => TextValueProvenance::Quoted(value.clone()),
                    TextLiteral::Heredoc(value) => TextValueProvenance::Heredoc(value.clone()),
                },
            }),
            TextValueExpression::Binding(reference) => {
                let binding = lookup(context, &reference.name)?;
                let BoundValue::Text(value) = &binding.value;
                Ok(ResolvedTextValue {
                    value: value.clone(),
                    provenance: TextValueProvenance::Binding {
                        name: reference.name.clone(),
                        source: binding.source,
                    },
                })
            }
            TextValueExpression::Interpolated(text) => {
                let mut value = String::new();
                let mut references = Vec::new();
                for segment in text.segments() {
                    match segment {
                        InterpolatedTextSegment::Literal(literal) => {
                            value.push_str(literal.as_str())
                        }
                        InterpolatedTextSegment::Binding(reference) => {
                            let binding = lookup(context, &reference.name)?;
                            let BoundValue::Text(bound) = &binding.value;
                            // The binding's exact UTF-8 text, with no escaping,
                            // quoting, indentation, newline normalization, or
                            // re-interpolation of what it happens to contain.
                            value.push_str(bound.as_str());
                            references.push(ResolvedBindingReference {
                                name: reference.name.clone(),
                                reference_span: reference.reference_span,
                                source: binding.source,
                            });
                        }
                    }
                }
                Ok(ResolvedTextValue {
                    value: TextValue::new(value),
                    provenance: TextValueProvenance::Interpolated {
                        form: text.form(),
                        span: text.span(),
                        references,
                    },
                })
            }
        }
    }
}

/// Looks up a binding that scope validation already proved is in scope.
///
/// Reaching the error arm means the validated test definition and the runtime
/// environment disagree, so it is reported as the internal invariant violation
/// it is rather than as an ordinary user-facing resolution failure.
fn lookup<'a>(
    context: &TextResolutionContext<'a>,
    name: &str,
) -> Result<&'a Binding, TextResolutionError> {
    context
        .bindings
        .get(name)
        .ok_or_else(|| TextResolutionError {
            message: format!(
                "internal invariant violation: binding '{name}' passed definition-time validation \
             but is absent at evaluation"
            ),
            diagnostic_code: DiagnosticCode::SemanticBindingUndefined,
        })
}
