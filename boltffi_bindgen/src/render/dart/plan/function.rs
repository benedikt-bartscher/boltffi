use crate::{
    ir::{ClassId, EnumId, PrimitiveType, ReadOp, ReadSeq, RecordId, ValueExpr, WriteSeq},
    render::dart::{DartFFIPrimitiveType, emit},
};

#[derive(Debug, Clone)]
pub enum DartFFIParamValue {
    Primitive(super::DartFFIPrimitiveType),
    Record(String),
}

impl DartFFIParamValue {
    pub fn from_primitive(primitive: PrimitiveType) -> Self {
        Self::Primitive(super::DartFFIPrimitiveType::from_primitive(primitive))
    }
}

#[derive(Debug, Clone)]
pub enum DartFFIParamBytes {
    Array(DartFFIParamValue),
    Record(String),
    UTF8,
}

#[derive(Debug, Clone)]
pub struct DartFFIClosureDef {
    pub sig: super::DartFFIFunctionSig,
    pub params: Vec<DartFunctionParam>,
    pub returns: DartFunctionReturns,
}

impl DartFFIClosureDef {
    pub fn exceptional_return(&self) -> Option<&'static str> {
        match self.returns.ty.inner {
            super::DartType::Bool => Some("false"),
            super::DartType::Int(..) => Some("0"),
            super::DartType::Double(..) => Some("0.0"),
            super::DartType::Enum(_) => {
                if let DartFFIReturnsPassing::Passing(DartFFIValuePassing::Value(
                    DartFFIParamValue::Primitive(..),
                )) = &self.returns.passing
                {
                    Some("0")
                } else {
                    None
                }
            }
            super::DartType::Void
            | super::DartType::String
            | super::DartType::Option(..)
            | super::DartType::List(..)
            | super::DartType::Bytes
            | super::DartType::Closure(..)
            | super::DartType::Result { .. }
            | super::DartType::Record(_)
            | super::DartType::Class(_)
            | super::DartType::Callback(_)
            | super::DartType::Builtin(_)
            | super::DartType::Custom(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DartFFIValuePassing {
    /// ints, floats, bools, records, enums, ...
    Value(DartFFIParamValue),
    /// enums, records, strings, lists
    WireEncoded,
    /// owned list of ints, floats, bools, records, ...
    List(DartFFIParamValue),
    /// ref arrays of (ints, floats, bools, records, ...), strings, records
    Bytes(DartFFIParamBytes),
    /// closures
    Closure(Box<DartFFIClosureDef>),
    /// class handle
    ClassHandle { class: String, nullable: bool },
    /// callback handle
    CallbackHandle { class: String, nullable: bool },
}

impl DartFFIValuePassing {
    pub fn primitive_value(primitive: PrimitiveType) -> Self {
        DartFFIValuePassing::Value(DartFFIParamValue::from_primitive(primitive))
    }

    pub fn record_value(record: String) -> Self {
        DartFFIValuePassing::Value(DartFFIParamValue::Record(record))
    }

    pub fn primitive_bytes(primitive: PrimitiveType) -> Self {
        DartFFIValuePassing::Bytes(DartFFIParamBytes::Array(DartFFIParamValue::from_primitive(
            primitive,
        )))
    }

    pub fn record_bytes(record: String) -> Self {
        DartFFIValuePassing::Bytes(DartFFIParamBytes::Array(DartFFIParamValue::Record(record)))
    }

    pub fn utf8_bytes() -> Self {
        DartFFIValuePassing::Bytes(DartFFIParamBytes::UTF8)
    }

    pub fn record_value_list(record: String) -> Self {
        DartFFIValuePassing::List(super::DartFFIParamValue::Record(record))
    }
}

#[derive(Debug, Clone)]
pub enum DartFFIReturnsRecv {
    /// ints, floats, bools, records, closures, ...
    Value,
}

#[derive(Debug, Clone)]
pub struct DartFFIFunctionReturns {
    pub ty: super::DartType,
    pub passing: DartFFIReturnsRecv,
}

#[derive(Debug, Clone)]
pub enum DartFunctionParamWriteBack {
    PrimitiveBytes(DartFFIPrimitiveType),
}

#[derive(Debug, Clone)]
pub struct DartFunctionParam {
    pub name: String,
    pub ty: super::DartType,
    pub passing: DartFFIValuePassing,
    pub write_seq: Option<WriteSeq>,
    pub read_seq: Option<ReadSeq>,
    pub writeback: Option<DartFunctionParamWriteBack>,
}

impl DartFunctionParam {
    pub fn buf_name(&self) -> String {
        format!("l${}Buf", self.name)
    }

    pub fn wire_name(&self) -> String {
        format!("l${}Wire", self.name)
    }

    pub fn reader_name(&self) -> String {
        format!("l${}Reader", self.name)
    }

    pub fn writer_name(&self) -> String {
        format!("l${}Writer", self.name)
    }

    pub fn storage_name(&self) -> String {
        format!("l${}Storage", self.name)
    }

    pub fn bytes_name(&self) -> String {
        format!("l${}Bytes", self.name)
    }

    pub fn ffi_param_ptr_name(&self) -> String {
        format!("{}Ptr", self.name)
    }

    pub fn ffi_param_len_name(&self) -> String {
        format!("{}Len", self.name)
    }

    pub fn callable_name(&self) -> String {
        format!("l${}Callable", self.name)
    }

    pub fn wire_read_expr(&self) -> String {
        emit::emit_reader_read(
            self.read_seq.as_ref().expect("ffi buffer parts"),
            &self.wire_name(),
            self.ty.is_inner_void(),
        )
    }

    pub fn wire_write_expr(&self) -> String {
        emit::emit_writer_write(
            self.write_seq.as_ref().expect("wire encoded"),
            &self.wire_name(),
            &self.name,
        )
    }

    pub fn wire_size_expr(&self) -> String {
        let w = self.write_seq.as_ref().expect("wire encoded");
        emit::emit_size_expr(&emit::remap_size_expr_value_expr(
            &w.size,
            ValueExpr::Named(self.name.clone()),
        ))
    }

    pub fn bytes_read_expr(&self) -> String {
        let DartFFIValuePassing::Bytes(bytes) = &self.passing else {
            panic!("bytes passsing")
        };

        match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(primitive) => match primitive {
                    super::DartFFIPrimitiveType::Bool => {
                        format!(
                            "_$$BoltBoolList._m$fromUint8List({}.asTypedList({}).sublist(0).buffer.asUint8List())",
                            self.ffi_param_ptr_name(),
                            self.ffi_param_len_name()
                        )
                    }
                    super::DartFFIPrimitiveType::Int(int) => match int {
                        super::DartFFIIntType::Uint8 => {
                            format!(
                                "{}.asTypedList({}).sublist(0).buffer.asUint8List()",
                                self.ffi_param_ptr_name(),
                                self.ffi_param_len_name()
                            )
                        }
                        super::DartFFIIntType::Int8 => {
                            format!(
                                "{}.asTypedList({}).sublist(0).buffer.asInt8List()",
                                self.ffi_param_ptr_name(),
                                self.ffi_param_len_name()
                            )
                        }
                        super::DartFFIIntType::Uint16 => {
                            format!(
                                "{}.asTypedList({}).sublist(0).buffer.asUint16List()",
                                self.ffi_param_ptr_name(),
                                self.ffi_param_len_name()
                            )
                        }
                        super::DartFFIIntType::Int16 => {
                            format!(
                                "{}.asTypedList({}).sublist(0).buffer.asInt16List()",
                                self.ffi_param_ptr_name(),
                                self.ffi_param_len_name()
                            )
                        }
                        super::DartFFIIntType::Uint32 => {
                            format!(
                                "{}.asTypedList({}).sublist(0).buffer.asUint32List()",
                                self.ffi_param_ptr_name(),
                                self.ffi_param_len_name()
                            )
                        }
                        super::DartFFIIntType::Int32 => {
                            format!(
                                "{}.asTypedList({}).sublist(0).buffer.asInt32List()",
                                self.ffi_param_ptr_name(),
                                self.ffi_param_len_name()
                            )
                        }
                        super::DartFFIIntType::Uint64 | super::DartFFIIntType::UintPtr => {
                            format!(
                                "{}.asTypedList({}).sublist(0).buffer.asUint64List()",
                                self.ffi_param_ptr_name(),
                                self.ffi_param_len_name()
                            )
                        }
                        super::DartFFIIntType::Int64 | super::DartFFIIntType::IntPtr => {
                            format!(
                                "{}.asTypedList({}).sublist(0).buffer.asInt64List()",
                                self.ffi_param_ptr_name(),
                                self.ffi_param_len_name()
                            )
                        }
                    },
                    super::DartFFIPrimitiveType::Float(float) => match float {
                        super::DartFFIFloatType::Float32 => format!(
                            "{}.asTypedList({}).sublist(0).buffer.asFloat32List()",
                            self.ffi_param_ptr_name(),
                            self.ffi_param_len_name()
                        ),
                        super::DartFFIFloatType::Float64 => format!(
                            "{}.asTypedList({}).sublist(0).buffer.asFloat64List()",
                            self.ffi_param_ptr_name(),
                            self.ffi_param_len_name()
                        ),
                    },
                },
                DartFFIParamValue::Record(record) => format!(
                    "{}._m$blittableReadList({len}, _$$BoltBufReader.fromSpan({ptr}, {len}))",
                    record,
                    ptr = self.ffi_param_ptr_name(),
                    len = self.ffi_param_len_name(),
                ),
            },
            DartFFIParamBytes::Record(record) => format!(
                "{}._m$blittableRead(_$$BoltBufReader.fromSpan({}, {}))",
                record,
                self.ffi_param_ptr_name(),
                self.ffi_param_len_name(),
            ),
            DartFFIParamBytes::UTF8 => format!(
                "$$convert.utf8.decode({}.asTypedList({}))",
                self.ffi_param_ptr_name(),
                self.ffi_param_len_name()
            ),
        }
    }

    pub fn bytes_write_expr(&self) -> String {
        let DartFFIValuePassing::Bytes(bytes) = &self.passing else {
            panic!("bytes passsing")
        };

        let write = match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(..) => {
                    format!("{}.writeBytes", self.writer_name())
                }
                DartFFIParamValue::Record(record) => {
                    format!("{}._m$blittableWriteList", record)
                }
            },
            DartFFIParamBytes::Record(record) => format!("{}._m$blittableWrite", record),
            DartFFIParamBytes::UTF8 => {
                format!("{}.writeBytes", self.writer_name())
            }
        };

        let args = match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(primitive) => match primitive {
                    super::DartFFIPrimitiveType::Bool => {
                        vec![format!("{}._bytes", self.name), String::from("0")]
                    }
                    super::DartFFIPrimitiveType::Int(..)
                    | super::DartFFIPrimitiveType::Float(..) => {
                        vec![self.name.clone(), String::from("0")]
                    }
                },
                DartFFIParamValue::Record(..) => vec![self.name.clone(), self.writer_name()],
            },
            DartFFIParamBytes::Record(..) => vec![self.name.clone(), self.writer_name()],
            DartFFIParamBytes::UTF8 => vec![self.bytes_name(), String::from("0")],
        };

        format!(
            "{}({});",
            write,
            args.into_iter()
                .reduce(|acc, s| acc + ", " + s.as_str())
                .unwrap()
        )
    }

    pub fn bytes_create_expr(&self) -> Option<String> {
        let DartFFIValuePassing::Bytes(bytes) = &self.passing else {
            panic!("bytes passsing")
        };

        let create = match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(..) | DartFFIParamValue::Record(..) => return None,
            },
            DartFFIParamBytes::Record(..) => return None,
            DartFFIParamBytes::UTF8 => "$$convert.utf8.encode".to_string(),
        };

        let var = match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(primitive) => match primitive {
                    super::DartFFIPrimitiveType::Bool
                    | super::DartFFIPrimitiveType::Int(..)
                    | super::DartFFIPrimitiveType::Float(..) => self.name.clone(),
                },
                DartFFIParamValue::Record(record) => {
                    format!("{}, {}._k$structSize", self.name, record)
                }
            },
            DartFFIParamBytes::Record(record) => format!("{}, {}._k$structSize", self.name, record),
            DartFFIParamBytes::UTF8 => self.name.clone(),
        };

        Some(format!("{}({})", create, var))
    }

    pub fn bytes_len_expr(&self) -> String {
        let DartFFIValuePassing::Bytes(bytes) = &self.passing else {
            panic!("bytes passsing")
        };

        match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(..) => {
                    format!("{}.lengthInBytes", self.name)
                }
                DartFFIParamValue::Record(record) => {
                    format!("{}.length * {}._k$structSize", self.name, record)
                }
            },
            DartFFIParamBytes::Record(record) => {
                format!("{}.length * {}._k$structSize", self.name, record)
            }
            DartFFIParamBytes::UTF8 => format!("{}.lengthInBytes", self.bytes_name()),
        }
    }

    pub fn list_bytes_write_expr(&self) -> String {
        let DartFFIValuePassing::List(value) = &self.passing else {
            panic!("bytes passsing")
        };

        let write = match value {
            DartFFIParamValue::Primitive(..) => {
                format!("{}.writeBytes", self.writer_name())
            }
            DartFFIParamValue::Record(record) => {
                format!("{}._m$blittableWriteList", record)
            }
        };

        let args = match value {
            DartFFIParamValue::Primitive(primitive) => match primitive {
                super::DartFFIPrimitiveType::Bool => {
                    vec![format!("{}._bytes", self.name), String::from("0")]
                }
                super::DartFFIPrimitiveType::Int(..) | super::DartFFIPrimitiveType::Float(..) => {
                    vec![self.name.clone(), String::from("0")]
                }
            },
            DartFFIParamValue::Record(..) => vec![self.name.clone(), self.writer_name()],
        };

        format!(
            "{}({});",
            write,
            args.into_iter()
                .reduce(|acc, s| acc + ", " + s.as_str())
                .unwrap()
        )
    }

    pub fn list_bytes_len_expr(&self) -> String {
        let DartFFIValuePassing::List(value) = &self.passing else {
            panic!("bytes passsing")
        };

        match value {
            DartFFIParamValue::Primitive(..) => format!("{}.lengthInBytes", self.name),
            DartFFIParamValue::Record(record) => {
                format!("{}.length * {}._k$structSize", self.name, record)
            }
        }
    }

    pub fn writeback_expr(&self) -> Option<String> {
        let writeback = self.writeback.as_ref()?;

        match writeback {
            DartFunctionParamWriteBack::PrimitiveBytes(primitive) => {
                let value_bytes = match primitive {
                    DartFFIPrimitiveType::Bool => format!("{}._bytes", self.name),
                    DartFFIPrimitiveType::Int(..) | DartFFIPrimitiveType::Float(..) => {
                        self.name.clone()
                    }
                };

                Some(format!(
                    "{value_bytes}.setRange(0, {name}.length, {storage}.ptr.cast<{native_type}>().asTypedList({name}.length));",
                    name = self.name,
                    storage = self.storage_name(),
                    native_type = primitive.native_type(),
                ))
            }
        }
    }

    pub fn get_ffi_param(&self) -> Vec<String> {
        match &self.passing {
            DartFFIValuePassing::Value(value) => match value {
                DartFFIParamValue::Primitive(..) => match &self.ty {
                    super::DartType::Bool
                    | super::DartType::Int(..)
                    | super::DartType::Double(..) => {
                        vec![self.name.clone()]
                    }
                    super::DartType::Enum(_) => vec![format!("{}.value", self.name)],
                    super::DartType::Custom(_) => todo!(),
                    _ => unreachable!(),
                },
                DartFFIParamValue::Record(..) => vec![format!("{}._m$toStruct()", self.name)],
            },
            DartFFIValuePassing::WireEncoded => {
                vec![
                    format!("{}.ptr", self.storage_name()),
                    format!("{}.len", self.wire_name()),
                ]
            }
            DartFFIValuePassing::List(value) => match value {
                DartFFIParamValue::Primitive(primitive) => vec![
                    format!(
                        "{}.ptr.cast<{}>()",
                        self.storage_name(),
                        primitive.native_type()
                    ),
                    format!("{}.length", self.name),
                ],
                DartFFIParamValue::Record(..) => vec![
                    format!("{}.ptr", self.storage_name()),
                    format!("{}.len", self.writer_name()),
                ],
            },
            DartFFIValuePassing::Bytes(bytes) => match bytes {
                DartFFIParamBytes::Array(value) => match value {
                    DartFFIParamValue::Primitive(primitive) => vec![
                        format!(
                            "{}.ptr.cast<{}>()",
                            self.storage_name(),
                            primitive.native_type()
                        ),
                        format!("{}.length", self.name),
                    ],
                    DartFFIParamValue::Record(_) => vec![
                        format!("{}.ptr", self.storage_name()),
                        format!("{}.length", self.name),
                    ],
                },
                DartFFIParamBytes::Record(record) => vec![
                    format!("{}.ptr.cast()", self.storage_name()),
                    format!("{}._k$structSize", record),
                ],
                DartFFIParamBytes::UTF8 => vec![
                    format!("{}.ptr", self.storage_name()),
                    format!("{}.lengthInBytes", self.bytes_name()),
                ],
            },
            DartFFIValuePassing::Closure(..) => {
                vec![
                    format!("{}.nativeFunction", self.callable_name()),
                    String::from("$$ffi.nullptr"),
                ]
            }
            DartFFIValuePassing::ClassHandle { .. } => vec![format!("{}._handle", self.name)],
            DartFFIValuePassing::CallbackHandle { class, nullable } => {
                if *nullable {
                    vec![format!(
                        "({name} == null) ? _k$BoltCallbackHandleNull : _I${class}.createCallbackHandle({name})",
                        name = self.name
                    )]
                } else {
                    vec![format!("_I${}.createCallbackHandle({})", class, self.name)]
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DartFFIAsyncFunctionDef {
    pub poll_symbol: String,
    pub complete_symbol: String,
    pub complete_ty: super::DartFFIType,
    pub cancel_symbol: String,
    pub free_symbol: String,
}

#[derive(Debug, Clone)]
pub enum DartFunctionMode {
    Sync,
    Async(Box<DartFFIAsyncFunctionDef>),
}

#[derive(Debug, Clone)]
pub enum DartFFIReturnsPassing {
    Void,
    Passing(DartFFIValuePassing),
}

#[derive(Debug, Clone)]
pub struct DartFunctionReturns {
    pub ty: super::DartReturnType,
    pub passing: DartFFIReturnsPassing,
    pub read_seq: Option<ReadSeq>,
    pub write_seq: Option<WriteSeq>,
}

impl DartFunctionReturns {
    pub fn res_name(&self) -> &'static str {
        "_l$res"
    }

    pub fn storage_name(&self) -> &'static str {
        "_l$resStorage"
    }

    pub fn buf_name(&self) -> &'static str {
        "_l$resBuf"
    }

    pub fn bytes_name(&self) -> &'static str {
        "_l$resBytes"
    }

    pub fn reader_name(&self) -> &'static str {
        "_l$resReader"
    }

    pub fn writer_name(&self) -> &'static str {
        "_l$resWriter"
    }

    pub fn wire_name(&self) -> &'static str {
        "_l$resWire"
    }

    pub fn wire_size_expr(&self) -> String {
        let w = self.write_seq.as_ref().expect("wire encoded");
        emit::emit_size_expr(&emit::remap_size_expr_value_expr(
            &w.size,
            ValueExpr::Named(self.res_name().to_string()),
        ))
    }

    pub fn wire_encode_expr(&self) -> String {
        emit::emit_writer_write(
            self.write_seq.as_ref().expect("wire"),
            self.wire_name(),
            self.res_name(),
        )
    }

    pub fn wire_decode_expr(&self) -> String {
        emit::emit_reader_read(
            self.read_seq.as_ref().expect("wire"),
            self.wire_name(),
            self.ty.inner.is_inner_void(),
        )
    }

    pub fn bytes_read_expr(&self) -> String {
        let DartFFIReturnsPassing::Passing(DartFFIValuePassing::Bytes(bytes)) = &self.passing
        else {
            panic!("bytes passsing")
        };

        match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(primitive) => match primitive {
                    crate::render::dart::DartFFIPrimitiveType::Bool => format!(
                        "$$BoltBoolList._m$fromUint8List({reader}.readUint8List({buf}.len, 0))",
                        reader = self.reader_name(),
                        buf = self.buf_name(),
                    ),
                    crate::render::dart::DartFFIPrimitiveType::Int(int) => {
                        let verb = match int {
                            super::DartFFIIntType::Uint8 => "Uint8",
                            super::DartFFIIntType::Int8 => "Int8",
                            super::DartFFIIntType::Uint16 => "Uint16",
                            super::DartFFIIntType::Int16 => "Int16",
                            super::DartFFIIntType::Uint32 => "Uint32",
                            super::DartFFIIntType::Int32 => "Int32",
                            super::DartFFIIntType::Uint64 | super::DartFFIIntType::UintPtr => {
                                "Uint64"
                            }
                            super::DartFFIIntType::Int64 | super::DartFFIIntType::IntPtr => "Int64",
                        };

                        format!(
                            "{reader}.read{verb}List({buf}.len, 0)",
                            reader = self.reader_name(),
                            buf = self.buf_name(),
                        )
                    }
                    crate::render::dart::DartFFIPrimitiveType::Float(float) => {
                        let verb = match float {
                            super::DartFFIFloatType::Float32 => "Float32",
                            super::DartFFIFloatType::Float64 => "Float64",
                        };

                        format!(
                            "{reader}.read{verb}List({buf}.len, 0)",
                            reader = self.reader_name(),
                            buf = self.buf_name(),
                        )
                    }
                },
                DartFFIParamValue::Record(record) => {
                    format!(
                        "{record}._m$blittableReadList({buf}.len, {reader})",
                        reader = self.reader_name(),
                        buf = self.buf_name(),
                    )
                }
            },
            DartFFIParamBytes::Record(record) => format!(
                "{record}._m$blittableRead({reader})",
                reader = self.reader_name()
            ),
            DartFFIParamBytes::UTF8 => {
                format!(
                    "$$convert.utf8.decode({reader}.readBytes({buf}.len, 0))",
                    reader = self.reader_name(),
                    buf = self.buf_name(),
                )
            }
        }
    }

    pub fn bytes_create_expr(&self) -> Option<String> {
        let DartFFIReturnsPassing::Passing(DartFFIValuePassing::Bytes(bytes)) = &self.passing
        else {
            panic!("bytes passsing")
        };

        let create = match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(primitive) => match primitive {
                    super::DartFFIPrimitiveType::Bool => "$$BoltBoolList.fromList".to_string(),
                    super::DartFFIPrimitiveType::Int(int) => match int {
                        super::DartFFIIntType::Uint8 => {
                            "$$typed_data.Uint8List.fromList".to_string()
                        }
                        super::DartFFIIntType::Int8 => "$$typed_data.Int8List.fromList".to_string(),
                        super::DartFFIIntType::Uint16 => {
                            "$$typed_data.Uint16List.fromList".to_string()
                        }
                        super::DartFFIIntType::Int16 => {
                            "$$typed_data.Int16List.fromList".to_string()
                        }
                        super::DartFFIIntType::Uint32 => {
                            "$$typed_data.Uint32List.fromList".to_string()
                        }
                        super::DartFFIIntType::Int32 => {
                            "$$typed_data.Int32List.fromList".to_string()
                        }
                        super::DartFFIIntType::Uint64 | super::DartFFIIntType::UintPtr => {
                            "$$typed_data.Uint64List.fromList".to_string()
                        }
                        super::DartFFIIntType::Int64 | super::DartFFIIntType::IntPtr => {
                            "$$typed_data.Int64List.fromList".to_string()
                        }
                    },
                    super::DartFFIPrimitiveType::Float(float) => match float {
                        super::DartFFIFloatType::Float32 => {
                            "$$typed_data.Float32List.fromList".to_string()
                        }
                        super::DartFFIFloatType::Float64 => {
                            "$$typed_data.Float64List.fromList".to_string()
                        }
                    },
                },
                DartFFIParamValue::Record(..) => return None,
            },
            DartFFIParamBytes::Record(..) => return None,
            DartFFIParamBytes::UTF8 => "$$convert.utf8.encode".to_string(),
        };

        let var = match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(primitive) => match primitive {
                    super::DartFFIPrimitiveType::Bool
                    | super::DartFFIPrimitiveType::Int(..)
                    | super::DartFFIPrimitiveType::Float(..) => self.res_name().to_string(),
                },
                DartFFIParamValue::Record(record) => {
                    format!("{}, {}._k$structSize", self.res_name(), record)
                }
            },
            DartFFIParamBytes::Record(record) => {
                format!("{}, {}._k$structSize", self.res_name(), record)
            }
            DartFFIParamBytes::UTF8 => self.res_name().to_string(),
        };

        Some(format!("{}({})", create, var))
    }

    pub fn bytes_write_expr(&self) -> String {
        let DartFFIReturnsPassing::Passing(DartFFIValuePassing::Bytes(bytes)) = &self.passing
        else {
            panic!("bytes passing")
        };

        let write = match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(..) => {
                    format!("{}.writeBytes", self.writer_name())
                }
                DartFFIParamValue::Record(record) => {
                    format!("{}._m$blittableWriteList", record)
                }
            },
            DartFFIParamBytes::Record(record) => format!("{}._m$blittableWrite", record),
            DartFFIParamBytes::UTF8 => {
                format!("{}.writeBytes", self.writer_name())
            }
        };

        let args = match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(primitive) => match primitive {
                    super::DartFFIPrimitiveType::Bool => {
                        vec![format!("{}._bytes", self.res_name()), String::from("0")]
                    }
                    super::DartFFIPrimitiveType::Int(..)
                    | super::DartFFIPrimitiveType::Float(..) => {
                        vec![self.res_name().to_string(), String::from("0")]
                    }
                },
                DartFFIParamValue::Record(..) => {
                    vec![self.res_name().to_string(), self.writer_name().to_string()]
                }
            },
            DartFFIParamBytes::Record(..) => {
                vec![self.res_name().to_string(), self.writer_name().to_string()]
            }
            DartFFIParamBytes::UTF8 => vec![self.bytes_name().to_string(), String::from("0")],
        };

        format!(
            "{}({})",
            write,
            args.into_iter()
                .reduce(|acc, s| acc + ", " + s.as_str())
                .unwrap()
        )
    }

    pub fn bytes_len_expr(&self) -> String {
        let DartFFIReturnsPassing::Passing(DartFFIValuePassing::Bytes(bytes)) = &self.passing
        else {
            panic!("bytes passsing")
        };

        match bytes {
            DartFFIParamBytes::Array(value) => match value {
                DartFFIParamValue::Primitive(..) => {
                    format!("{}.lengthInBytes", self.res_name())
                }
                DartFFIParamValue::Record(record) => {
                    format!("{}.length * {}._k$structSize", self.res_name(), record)
                }
            },
            DartFFIParamBytes::Record(record) => {
                format!("{}.length * {}._k$structSize", self.res_name(), record)
            }
            DartFFIParamBytes::UTF8 => format!("{}.lengthInBytes", self.bytes_name()),
        }
    }

    pub fn value_write_expr(&self) -> String {
        let DartFFIReturnsPassing::Passing(DartFFIValuePassing::Value(value)) = &self.passing
        else {
            panic!("bytes passsing")
        };

        match value {
            DartFFIParamValue::Primitive(..) => panic!("unexpected primitive value write"),
            DartFFIParamValue::Record(..) => format!("{}._m$toStruct()", self.res_name()),
        }
    }

    pub fn value_read_expr(&self) -> String {
        let DartFFIReturnsPassing::Passing(DartFFIValuePassing::Value(value)) = &self.passing
        else {
            panic!("bytes passsing")
        };

        match value {
            DartFFIParamValue::Primitive(..) => panic!("unexpected primitive value read"),
            DartFFIParamValue::Record(record) => format!(
                "{record}._m$fromStruct({reader})",
                reader = self.reader_name()
            ),
        }
    }

    pub fn is_fallible(&self) -> bool {
        if let Some(seq) = &self.read_seq {
            let read_op = seq.ops.first().expect("read op");
            matches!(read_op, ReadOp::Result { .. })
        } else {
            matches!(
                self.passing,
                DartFFIReturnsPassing::Passing(DartFFIValuePassing::ClassHandle {
                    nullable: true,
                    ..
                })
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct DartFunction {
    pub mode: DartFunctionMode,
    pub ty: DartFunctionType,
    pub ffi_def: DartFFIFunctionDef,
    pub sig: super::DartFunctionSig,
    pub params: Vec<DartFunctionParam>,
    pub returns: DartFunctionReturns,
    pub doc: Option<String>,
}

impl DartFunction {
    pub fn is_async(&self) -> bool {
        matches!(self.mode, DartFunctionMode::Async { .. })
    }

    pub fn is_fallible(&self) -> bool {
        match &self.ty {
            DartFunctionType::TopLevel { .. } | DartFunctionType::Method { .. } => {
                matches!(self.returns.ty.inner, super::DartType::Result { .. })
            }
            DartFunctionType::Constructor { is_fallible, .. } => *is_fallible,
        }
    }

    pub fn self_storage_name(&self) -> String {
        String::from("l$$selfStorage")
    }

    pub fn self_wire_name(&self) -> String {
        String::from("l$$selfWire")
    }

    pub fn self_wire_size(&self) -> Option<String> {
        match &self.ty {
            DartFunctionType::TopLevel { .. } | DartFunctionType::Constructor { .. } => None,
            DartFunctionType::Method {
                receiver, owner, ..
            } => {
                match receiver {
                    DartMethodReceiver::Static => {
                        return None;
                    }
                    DartMethodReceiver::ReceiverPassing(passing) => {
                        if !matches!(passing, DartFFIValuePassing::WireEncoded) {
                            return None;
                        }
                    }
                }

                match owner {
                    DartFunctionCallOwner::Class(..) => None,
                    DartFunctionCallOwner::Record(..) | DartFunctionCallOwner::Enum(..) => {
                        Some(String::from("_m$wireEncodedSize()"))
                    }
                }
            }
        }
    }

    pub fn self_wire_encode_expr(&self) -> Option<String> {
        match &self.ty {
            DartFunctionType::TopLevel { .. } | DartFunctionType::Constructor { .. } => None,
            DartFunctionType::Method {
                receiver, owner, ..
            } => {
                match receiver {
                    DartMethodReceiver::Static => {
                        return None;
                    }
                    DartMethodReceiver::ReceiverPassing(passing) => {
                        if !matches!(passing, DartFFIValuePassing::WireEncoded) {
                            return None;
                        }
                    }
                }

                match owner {
                    DartFunctionCallOwner::Class(..) => None,
                    DartFunctionCallOwner::Record(..) | DartFunctionCallOwner::Enum(..) => {
                        Some(format!("_m$wireEncode({});", self.self_wire_name()))
                    }
                }
            }
        }
    }

    pub fn get_ffi_params(&self) -> impl Iterator<Item = String> {
        std::iter::chain(
            match &self.ty {
                DartFunctionType::TopLevel { .. } => vec![],
                DartFunctionType::Method {
                    receiver, owner, ..
                } => match receiver {
                    DartMethodReceiver::Static => vec![],
                    DartMethodReceiver::ReceiverPassing(recv_passing) => match recv_passing {
                        DartFFIValuePassing::Value(value) => match value {
                            DartFFIParamValue::Primitive(..) => match owner {
                                DartFunctionCallOwner::Class(..) => unreachable!(),
                                DartFunctionCallOwner::Record(..) => unreachable!(),
                                DartFunctionCallOwner::Enum(..) => vec![String::from("this.value")],
                            },
                            DartFFIParamValue::Record(..) => vec![String::from("_m$toStruct()")],
                        },
                        DartFFIValuePassing::WireEncoded => vec![
                            format!("{}.ptr", self.self_storage_name()),
                            format!("{}.len", self.self_wire_name()),
                        ],
                        DartFFIValuePassing::ClassHandle { .. } => vec![String::from("_handle")],
                        DartFFIValuePassing::List(..)
                        | DartFFIValuePassing::Bytes(..)
                        | DartFFIValuePassing::Closure(..)
                        | DartFFIValuePassing::CallbackHandle { .. } => unreachable!(),
                    },
                },
                DartFunctionType::Constructor { .. } => vec![],
            },
            self.params.iter().flat_map(|p| p.get_ffi_param()),
        )
    }
}

#[derive(Debug, Clone)]
pub enum DartFunctionCallOwner {
    Class(ClassId),
    Record(RecordId),
    Enum(EnumId),
}

impl DartFunctionCallOwner {
    pub fn name(&self) -> &str {
        match self {
            DartFunctionCallOwner::Class(s) => s.as_str(),
            DartFunctionCallOwner::Record(s) => s.as_str(),
            DartFunctionCallOwner::Enum(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DartConstructorKind {
    Default,
    Named { name: String },
}

#[derive(Debug, Clone)]
pub enum DartFunctionType {
    TopLevel {
        name: String,
    },
    Method {
        name: String,
        receiver: DartMethodReceiver,
        owner: DartFunctionCallOwner,
    },
    Constructor {
        kind: super::DartConstructorKind,
        owner: DartFunctionCallOwner,
        is_fallible: bool,
        is_optional: bool,
    },
}

#[derive(Debug, Clone)]
pub enum DartMethodReceiver {
    Static,
    ReceiverPassing(DartFFIValuePassing),
}

#[derive(Debug, Clone)]
pub struct DartFFIFunctionDef {
    pub sig: super::DartFFIFunctionSig,
    pub symbol: String,
    pub is_leaf: bool,
}
