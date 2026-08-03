use crate::{
    ir::{ReadSeq, StreamMode},
    render::dart::{DartFFIFunctionDef, DartFFIParamValue, DartFFIType, DartFFIValuePassing, emit},
};

#[derive(Debug, Clone)]
pub struct DartStream {
    pub name: String,
    pub item_ty: super::DartType,
    pub item_read_seq: ReadSeq,
    pub item_passing: super::DartFFIValuePassing,
    pub ffi_item_ty: DartFFIType,
    pub ffi_item_size: Option<usize>,
    pub subscribe_fn: DartFFIFunctionDef,
    pub poll_fn: DartFFIFunctionDef,
    pub pop_batch_fn: DartFFIFunctionDef,
    pub wait_fn: DartFFIFunctionDef,
    pub unsubscribe_fn: DartFFIFunctionDef,
    pub free_fn: DartFFIFunctionDef,
    pub mode: StreamMode,
    pub doc: Option<String>,
}

impl DartStream {
    pub fn item_wire_decode_expr(&self, wire_name: &str) -> String {
        emit::emit_reader_read(&self.item_read_seq, wire_name, self.item_ty.is_inner_void())
    }

    pub fn item_array_read_expr(&self, reader_name: &str, bytes_len: &str) -> String {
        let DartFFIValuePassing::Value(value) = &self.item_passing else {
            panic!("value passsing")
        };

        match value {
            DartFFIParamValue::Primitive(primitive) => match primitive {
                crate::render::dart::DartFFIPrimitiveType::Bool => format!(
                    "$$BoltBoolList._m$fromUint8List({reader}.readUint8List({bytes_len}, 0))",
                    reader = reader_name
                ),
                crate::render::dart::DartFFIPrimitiveType::Int(int) => {
                    let verb = match int {
                        super::DartFFIIntType::Uint8 => "Uint8",
                        super::DartFFIIntType::Int8 => "Int8",
                        super::DartFFIIntType::Uint16 => "Uint16",
                        super::DartFFIIntType::Int16 => "Int16",
                        super::DartFFIIntType::Uint32 => "Uint32",
                        super::DartFFIIntType::Int32 => "Int32",
                        super::DartFFIIntType::Uint64 | super::DartFFIIntType::UintPtr => "Uint64",
                        super::DartFFIIntType::Int64 | super::DartFFIIntType::IntPtr => "Int64",
                    };

                    let mut s = format!(
                        "{reader}.read{verb}List({bytes_len}, 0)",
                        reader = reader_name
                    );
                    if let super::DartType::Enum(class) = &self.item_ty {
                        s.push_str(format!(".map({class}._m$fromDiscriminant)").as_str());
                    }
                    s
                }
                crate::render::dart::DartFFIPrimitiveType::Float(float) => {
                    let verb = match float {
                        super::DartFFIFloatType::Float32 => "Float32",
                        super::DartFFIFloatType::Float64 => "Float64",
                    };

                    format!(
                        "{reader}.read{verb}List({bytes_len}, 0)",
                        reader = reader_name
                    )
                }
            },
            DartFFIParamValue::Record(record) => {
                format!(
                    "{record}._m$blittableReadList({bytes_len}, {reader})",
                    reader = reader_name
                )
            }
        }
    }
}
