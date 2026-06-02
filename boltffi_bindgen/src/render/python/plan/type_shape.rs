use crate::ir::types::PrimitiveType;
use crate::render::python::primitives::PythonScalarTypeExt as _;

use super::{PythonEnumType, PythonRecordType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PythonSequenceType {
    Bytes,
    PrimitiveVec(PrimitiveType),
    CStyleEnumVec(PythonEnumType),
}

impl PythonSequenceType {
    pub fn parameter_annotation(&self) -> String {
        match self {
            Self::Bytes => "bytes".to_string(),
            Self::PrimitiveVec(PrimitiveType::U8) => "bytes | Sequence[int]".to_string(),
            Self::PrimitiveVec(primitive) => {
                format!("Sequence[{}]", primitive.python_annotation())
            }
            Self::CStyleEnumVec(enum_type) => format!("Sequence[{}]", enum_type.type_literal()),
        }
    }

    pub fn return_annotation(&self) -> String {
        match self {
            Self::Bytes | Self::PrimitiveVec(PrimitiveType::U8) => "bytes".to_string(),
            Self::PrimitiveVec(primitive) => {
                format!("list[{}]", primitive.python_annotation())
            }
            Self::CStyleEnumVec(enum_type) => format!("list[{}]", enum_type.type_literal()),
        }
    }

    pub fn primitive_element(&self) -> Option<PrimitiveType> {
        match self {
            Self::Bytes => None,
            Self::PrimitiveVec(primitive) => Some(*primitive),
            Self::CStyleEnumVec(_) => None,
        }
    }

    pub fn enum_element(&self) -> Option<&PythonEnumType> {
        match self {
            Self::CStyleEnumVec(enum_type) => Some(enum_type),
            _ => None,
        }
    }

    pub fn is_bytes(&self) -> bool {
        matches!(self, Self::Bytes)
    }

    pub fn is_byte_like(&self) -> bool {
        matches!(self, Self::Bytes | Self::PrimitiveVec(PrimitiveType::U8))
    }

    pub fn is_primitive_vector(&self) -> bool {
        matches!(self, Self::PrimitiveVec(_))
    }

    pub fn is_c_style_enum_vector(&self) -> bool {
        matches!(self, Self::CStyleEnumVec(_))
    }

    pub fn uses_buffer_input(&self) -> bool {
        matches!(
            self,
            Self::Bytes | Self::PrimitiveVec(_) | Self::CStyleEnumVec(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PythonType {
    Void,
    Primitive(PrimitiveType),
    Record(PythonRecordType),
    CStyleEnum(PythonEnumType),
    String,
    Sequence(PythonSequenceType),
    /// A `Result<T, E>` return. The Ok payload is surfaced as the Python return
    /// value; the Err payload is raised as an `FfiException`. Only valid in
    /// return position (never as a parameter).
    Result {
        ok: Box<PythonType>,
        err: Box<PythonType>,
    },
}

impl PythonType {
    pub fn parameter_annotation(&self) -> String {
        match self {
            Self::Void => "None".to_string(),
            Self::Primitive(primitive) => primitive.python_annotation().to_string(),
            Self::Record(record_type) => record_type.type_literal(),
            Self::CStyleEnum(enum_type) => enum_type.type_literal(),
            Self::String => "str".to_string(),
            Self::Sequence(sequence) => sequence.parameter_annotation(),
            // Result is never a parameter; defensively annotate as the ok type.
            Self::Result { ok, .. } => ok.parameter_annotation(),
        }
    }

    pub fn return_annotation(&self) -> String {
        match self {
            Self::Void => "None".to_string(),
            Self::Primitive(primitive) => primitive.python_annotation().to_string(),
            Self::Record(record_type) => record_type.type_literal(),
            Self::CStyleEnum(enum_type) => enum_type.type_literal(),
            Self::String => "str".to_string(),
            Self::Sequence(sequence) => sequence.return_annotation(),
            // A fallible function returns its Ok type and raises on Err.
            Self::Result { ok, .. } => ok.return_annotation(),
        }
    }

    pub fn native_primitive(&self) -> Option<PrimitiveType> {
        match self {
            Self::Void => None,
            Self::Primitive(primitive) => Some(*primitive),
            Self::Record(_) => None,
            Self::CStyleEnum(enum_type) => Some(enum_type.tag_type),
            Self::String => None,
            Self::Sequence(sequence) => sequence.primitive_element(),
            Self::Result { .. } => None,
        }
    }

    pub fn record(&self) -> Option<&PythonRecordType> {
        match self {
            Self::Record(record_type) => Some(record_type),
            _ => None,
        }
    }

    pub fn c_style_enum(&self) -> Option<&PythonEnumType> {
        match self {
            Self::CStyleEnum(enum_type) => Some(enum_type),
            _ => None,
        }
    }

    pub fn sequence_c_style_enum(&self) -> Option<&PythonEnumType> {
        match self {
            Self::Sequence(sequence) => sequence.enum_element(),
            _ => None,
        }
    }

    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String)
    }

    pub fn is_record(&self) -> bool {
        matches!(self, Self::Record(_))
    }

    pub fn is_c_style_enum(&self) -> bool {
        matches!(self, Self::CStyleEnum(_))
    }

    pub fn is_bytes(&self) -> bool {
        matches!(self, Self::Sequence(PythonSequenceType::Bytes))
    }

    pub fn is_byte_like(&self) -> bool {
        matches!(self, Self::Sequence(sequence) if sequence.is_byte_like())
    }

    pub fn is_primitive_vector(&self) -> bool {
        matches!(self, Self::Sequence(PythonSequenceType::PrimitiveVec(_)))
    }

    pub fn is_c_style_enum_vector(&self) -> bool {
        matches!(self, Self::Sequence(PythonSequenceType::CStyleEnumVec(_)))
    }

    pub fn uses_buffer_input(&self) -> bool {
        matches!(self, Self::Sequence(sequence) if sequence.uses_buffer_input())
    }

    pub fn is_owned_buffer(&self) -> bool {
        // A Result crosses the boundary as an owned `[tag][payload]` buffer too.
        matches!(self, Self::String | Self::Sequence(_) | Self::Result { .. })
    }

    pub fn is_result(&self) -> bool {
        matches!(self, Self::Result { .. })
    }

    pub fn result_ok(&self) -> Option<&PythonType> {
        match self {
            Self::Result { ok, .. } => Some(ok),
            _ => None,
        }
    }

    pub fn result_err(&self) -> Option<&PythonType> {
        match self {
            Self::Result { err, .. } => Some(err),
            _ => None,
        }
    }

    /// All primitive types reachable through this type, flattening `Result`
    /// into its ok/err payloads. Used so the C extension emits the boxer
    /// helpers for primitives that only appear inside a `Result`.
    pub fn contained_primitives(&self) -> Vec<PrimitiveType> {
        match self {
            Self::Result { ok, err } => {
                let mut primitives = ok.contained_primitives();
                primitives.extend(err.contained_primitives());
                primitives
            }
            other => other.native_primitive().into_iter().collect(),
        }
    }

    /// Whether this type can appear as a `Result` ok/err payload, i.e. the C
    /// extension knows how to decode it from a `[tag][payload]` buffer at an
    /// offset. Functions whose ok/err type is unsupported are dropped during
    /// lowering rather than emitting broken bindings.
    pub fn is_supported_result_payload(&self) -> bool {
        match self {
            Self::Void | Self::Primitive(_) | Self::String | Self::Record(_) => true,
            Self::Sequence(sequence) => sequence.is_byte_like(),
            Self::CStyleEnum(_) | Self::Result { .. } => false,
        }
    }

    /// Name of the C reader that decodes a value of this type from a cursor
    /// `(const uint8_t *base, uintptr_t avail, uintptr_t *consumed)`.
    pub fn result_payload_reader_name(&self) -> String {
        match self {
            Self::Void => "boltffi_python_read_payload_void".to_string(),
            Self::Primitive(primitive) => {
                format!("boltffi_python_read_payload_{}", primitive.rust_name())
            }
            Self::String => "boltffi_python_read_payload_string".to_string(),
            Self::Sequence(_) => "boltffi_python_read_payload_bytes".to_string(),
            Self::Record(record_type) => {
                format!("boltffi_python_read_payload_{}", record_type.c_type_name)
            }
            Self::CStyleEnum(_) | Self::Result { .. } => {
                unreachable!("unsupported result payload type has no reader")
            }
        }
    }
}
