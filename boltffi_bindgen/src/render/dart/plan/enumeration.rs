use crate::{
    ir::{PrimitiveType, ReadSeq, SizeExpr, ValueExpr, WriteSeq},
    render::dart::emit,
};

#[derive(Debug, Clone, Copy)]
pub enum DartEnumKind {
    Enhanced,
    SealedClass,
}

#[derive(Debug, Clone)]
pub struct DartEnumField {
    pub name: String,
    pub ty: super::DartType,
    pub read_seq: ReadSeq,
    pub write_seq: WriteSeq,
}

impl DartEnumField {
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
        emit::emit_cmp_expr(&self.name, &format!("{other}.{}", self.name), &self.ty)
    }
}

#[derive(Debug, Clone)]
pub struct DartEnumVariant {
    pub name: String,
    pub class_name: String,
    pub discriminant: i128,
    pub fields: Vec<DartEnumField>,
}

#[derive(Debug, Clone)]
pub struct DartEnum {
    pub name: String,
    pub kind: DartEnumKind,
    pub tag_type: PrimitiveType,
    pub variants: Vec<DartEnumVariant>,
    pub size_expr: SizeExpr,
    pub is_error: bool,
    pub constructors: Vec<super::DartFunction>,
    pub methods: Vec<super::DartFunction>,
    pub doc: Option<String>,
}

impl DartEnum {
    pub fn tag_reader_read(&self, reader_name: &str) -> String {
        format!(
            "{reader_name}.{}()",
            emit::primitive_read_method(self.tag_type)
        )
    }

    pub fn tag_dart_type(&self) -> String {
        emit::primitive_dart_type(self.tag_type)
    }

    pub fn wire_encoded_size_expr(&self) -> String {
        emit::emit_size_expr(&self.size_expr)
    }
}
