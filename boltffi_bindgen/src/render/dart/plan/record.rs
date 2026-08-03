use crate::{
    ir::{DefaultValue, PrimitiveType, ReadSeq, ValueExpr, WriteSeq},
    render::dart::{NamingConvention, emit},
};

#[derive(Debug, Clone)]
pub struct DartRecordField {
    pub name: String,
    pub offset: usize,
    pub ty: super::DartType,
    pub default_value: Option<DefaultValue>,
    pub read_seq: ReadSeq,
    pub write_seq: WriteSeq,
    pub doc: Option<String>,
}

impl DartRecordField {
    pub fn wire_decode_expr(&self, reader_name: &str) -> String {
        emit::emit_reader_read(&self.read_seq, reader_name, self.ty.is_inner_void())
    }

    pub fn wire_encode_expr(&self, writer_name: &str) -> String {
        emit::emit_writer_write(&self.write_seq, writer_name, &self.name)
    }

    pub fn wire_encoded_size_expr(&self) -> String {
        emit::emit_size_expr(&emit::remap_size_expr_value_expr(
            &self.write_seq.size,
            ValueExpr::Named(self.name.to_string()),
        ))
    }

    pub fn field_cmp_expr(&self, other: &str) -> String {
        emit::emit_cmp_expr(&self.name, &format!("{}.{}", other, self.name), &self.ty)
    }

    pub fn dart_default_value(&self) -> String {
        let default_value = self
            .default_value
            .as_ref()
            .expect("Field with default value");
        match default_value {
            DefaultValue::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            DefaultValue::Integer(i) => i.to_string(),
            DefaultValue::Float(f) => f.to_string(),
            DefaultValue::String(s) => format!("{:?}", s),
            DefaultValue::EnumVariant {
                enum_name,
                variant_name,
            } => format!(
                "{}.{}",
                NamingConvention::class_name(enum_name),
                NamingConvention::property_name(variant_name)
            ),
            DefaultValue::Null => "null".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DartBlittableLayout {
    pub struct_name: String,
    pub struct_size: usize,
    pub fields: Vec<DartBlittableField>,
}

#[derive(Debug, Clone)]
pub struct DartBlittableField {
    pub name: String,
    pub primitive: PrimitiveType,
    pub native_type: super::DartFFIType,
    pub offset_const_name: String,
    pub offset: usize,
}

impl DartBlittableField {
    pub fn primitive_write_method(&self) -> &'static str {
        emit::primitive_write_method(self.primitive)
    }

    pub fn primitive_read_method(&self) -> &'static str {
        emit::primitive_read_method(self.primitive)
    }

    pub fn blittable_read_expr(&self, bytes_name: &str) -> String {
        emit::emit_read_blittable_value(&self.offset_const_name, self.primitive, bytes_name)
    }

    pub fn blittable_write_expr(&self, bytes_name: &str) -> String {
        emit::emit_write_blittable_value(
            &self.offset_const_name,
            self.primitive,
            &self.name,
            bytes_name,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DartRecordInterface {
    Exception,
}

impl DartRecordInterface {
    pub fn name(&self) -> String {
        match self {
            DartRecordInterface::Exception => String::from("Exception"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DartRecord {
    pub name: String,
    pub interfaces: Vec<DartRecordInterface>,
    pub fields: Vec<DartRecordField>,
    pub blittable_layout: Option<DartBlittableLayout>,
    pub constructors: Vec<super::DartFunction>,
    pub methods: Vec<super::DartFunction>,
    pub doc: Option<String>,
}

impl DartRecord {
    pub fn fields_with_defaults(&self) -> impl Iterator<Item = &DartRecordField> {
        self.fields.iter().filter(|f| f.default_value.is_some())
    }

    pub fn fields_without_defaults(&self) -> impl Iterator<Item = &DartRecordField> {
        self.fields.iter().filter(|f| f.default_value.is_none())
    }
}
