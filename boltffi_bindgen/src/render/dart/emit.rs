use askama::Template as _;

use crate::{
    ir::{
        BuiltinId, EnumLayout, PrimitiveType, ReadOp, ReadSeq, RecordId, ReturnDef, SizeExpr,
        TypeExpr, ValueExpr, VecLayout, WireSizeOwner, WriteOp, WriteSeq,
    },
    render::dart::{
        DartLibrary, DartType, NamingConvention,
        templates::{
            BuildHookTemplate, CallableTemplate, CallbackTemplate, ClassTemplate,
            CustomTypesTemplate, EnhancedEnumTemplate, ExternFunctionTemplate, PreludeTemplate,
            PubspecTemplate, RecordTemplate, SealedClassEnumTemplate,
        },
    },
};

pub struct DartPackage {
    pub pubspec: String,
    pub lib: String,
    pub build: String,
}

pub struct DartEmitter {}

impl DartEmitter {
    pub fn emit(library: &DartLibrary, artifact_name: &str) -> DartPackage {
        let output = std::iter::once(PreludeTemplate {}.render().unwrap())
            .chain(std::iter::once(
                CustomTypesTemplate {
                    custom_types: &library.custom_types,
                }
                .render()
                .unwrap(),
            ))
            .chain(
                library
                    .records
                    .iter()
                    .map(|r| RecordTemplate { record: r }.render().unwrap()),
            )
            .chain(library.enums.iter().map(|e| match &e.kind {
                super::DartEnumKind::Enhanced => {
                    EnhancedEnumTemplate { dart_enum: e }.render().unwrap()
                }
                super::DartEnumKind::SealedClass => {
                    SealedClassEnumTemplate { dart_enum: e }.render().unwrap()
                }
            }))
            .chain(
                library
                    .callbacks
                    .iter()
                    .map(|cb| CallbackTemplate { cb }.render().unwrap()),
            )
            .chain(
                library
                    .classes
                    .iter()
                    .map(|class| ClassTemplate { class }.render().unwrap()),
            )
            .chain(library.functions.iter().flat_map(|func| {
                [
                    ExternFunctionTemplate { func }.render().unwrap(),
                    CallableTemplate { func }.render().unwrap(),
                ]
            }))
            .reduce(|acc, s| acc + "\n" + s.as_str())
            .unwrap_or_default();

        DartPackage {
            pubspec: PubspecTemplate {
                artifact_name,
                description: None,
                version: None,
                repository: None,
            }
            .render()
            .unwrap(),
            lib: output,
            build: BuildHookTemplate { artifact_name }.render().unwrap(),
        }
    }
}

pub fn primitive_dart_type(primitive: PrimitiveType) -> String {
    match primitive {
        PrimitiveType::Bool => "bool".to_string(),
        PrimitiveType::I8
        | PrimitiveType::U8
        | PrimitiveType::I16
        | PrimitiveType::U16
        | PrimitiveType::I32
        | PrimitiveType::U32
        | PrimitiveType::I64
        | PrimitiveType::U64
        | PrimitiveType::ISize
        | PrimitiveType::USize => "int".to_string(),
        PrimitiveType::F32 | PrimitiveType::F64 => "double".to_string(),
    }
}

pub fn primitive_native_type(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::Bool => "$$ffi.Bool",
        PrimitiveType::I8 => "$$ffi.Int8",
        PrimitiveType::I16 => "$$ffi.Int16",
        PrimitiveType::I32 => "$$ffi.Int32",
        PrimitiveType::I64 => "$$ffi.Int64",
        PrimitiveType::U8 => "$$ffi.Uint8",
        PrimitiveType::U16 => "$$ffi.Uint16",
        PrimitiveType::U32 => "$$ffi.Uint32",
        PrimitiveType::U64 => "$$ffi.Uint64",
        PrimitiveType::ISize => "$$ffi.IntPtr",
        PrimitiveType::USize => "$$ffi.UintPtr",
        PrimitiveType::F32 => "$$ffi.Float",
        PrimitiveType::F64 => "$$ffi.Double",
    }
}

fn render_type_name(name: &str) -> String {
    NamingConvention::class_name(name)
}

pub fn render_value(expr: &ValueExpr) -> String {
    match expr {
        ValueExpr::Instance => String::new(),
        ValueExpr::Var(name) => name.clone(),
        ValueExpr::Named(name) => name.to_string(),
        ValueExpr::Field(parent, field) => {
            let parent_str = render_value(parent);
            if parent_str.is_empty() {
                field.to_string()
            } else {
                format!("{}.{}", parent_str, field.as_str())
            }
        }
    }
}

pub fn type_expr_dart_type(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Primitive(p) => primitive_dart_type(*p),
        TypeExpr::String | TypeExpr::Str => "String".to_string(),
        TypeExpr::Vec(inner) => match inner.as_ref() {
            TypeExpr::Primitive(primitive) => match primitive {
                PrimitiveType::I32 => "$$typed_data.Int32List".to_string(),
                PrimitiveType::U32 => "$$typed_data.Uint32List".to_string(),
                PrimitiveType::I16 => "$$typed_data.Int16List".to_string(),
                PrimitiveType::U16 => "$$typed_data.Uint16List".to_string(),
                PrimitiveType::I64 => "$$typed_data.Int64List".to_string(),
                PrimitiveType::U64 => "$$typed_data.Uint64List".to_string(),
                PrimitiveType::ISize => "$$typed_data.Int64List".to_string(),
                PrimitiveType::USize => "$$typed_data.Uint64List".to_string(),
                PrimitiveType::F32 => "$$typed_data.Float32List".to_string(),
                PrimitiveType::F64 => "$$typed_data.Float64List".to_string(),
                PrimitiveType::U8 => "$$typed_data.Uint8List".to_string(),
                PrimitiveType::I8 => "$$typed_data.Int8List".to_string(),
                PrimitiveType::Bool => "$$BoltFFIBoolList".to_string(),
            },
            _ => format!("List<{}>", type_expr_dart_type(inner)),
        },
        TypeExpr::Option(inner) => format!("{}?", type_expr_dart_type(inner)),
        TypeExpr::Result { ok, err } => {
            format!(
                "$$BoltResult<{}, {}>",
                type_expr_dart_type(ok),
                match err.as_ref() {
                    TypeExpr::String => "$$BoltException".to_string(),
                    _ => type_expr_dart_type(err),
                },
            )
        }
        TypeExpr::Record(id) => render_type_name(id.as_str()),
        TypeExpr::Enum(id) => render_type_name(id.as_str()),
        TypeExpr::Custom(id) => render_type_name(id.as_str()),
        TypeExpr::Builtin(id) => match id.as_str() {
            "Duration" => "Duration".to_string(),
            "SystemTime" => "Datetime".to_string(),
            "Uuid" => "String".to_string(),
            "Url" => "Uri".to_string(),
            _ => "String".to_string(),
        },
        TypeExpr::Handle(class_id) => render_type_name(class_id.as_str()),
        TypeExpr::Callback(callback_id) => render_type_name(callback_id.as_str()),
        TypeExpr::Void => "void".to_string(),
    }
}

pub fn return_def_dart_type(return_def: &ReturnDef) -> String {
    match return_def {
        ReturnDef::Void => "void".to_string(),
        ReturnDef::Value(type_expr) => type_expr_dart_type(type_expr),
        ReturnDef::Result { ok, err } => format!(
            "$$BoltResult<{}, {}>",
            type_expr_dart_type(ok),
            type_expr_dart_type(err)
        ),
    }
}

pub fn primitive_as_num(primitive: PrimitiveType, value: &str) -> String {
    match primitive {
        PrimitiveType::Bool => format!("({} ? 1 : 0)", value),
        PrimitiveType::I8
        | PrimitiveType::U8
        | PrimitiveType::I16
        | PrimitiveType::U16
        | PrimitiveType::I32
        | PrimitiveType::U32
        | PrimitiveType::I64
        | PrimitiveType::U64
        | PrimitiveType::ISize
        | PrimitiveType::USize
        | PrimitiveType::F32
        | PrimitiveType::F64 => value.to_string(),
    }
}

pub fn num_as_primitive(primitive: PrimitiveType, value: &str) -> String {
    match primitive {
        PrimitiveType::Bool => format!("({} == 1)", value),
        PrimitiveType::I8
        | PrimitiveType::U8
        | PrimitiveType::I16
        | PrimitiveType::U16
        | PrimitiveType::I32
        | PrimitiveType::U32
        | PrimitiveType::I64
        | PrimitiveType::U64
        | PrimitiveType::ISize
        | PrimitiveType::USize
        | PrimitiveType::F32
        | PrimitiveType::F64 => value.to_string(),
    }
}

pub fn primitive_blittable_write_method(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::I8 => "setInt8",
        PrimitiveType::Bool | PrimitiveType::U8 => "setUint8",
        PrimitiveType::I16 => "setInt16",
        PrimitiveType::U16 => "setUint16",
        PrimitiveType::I32 => "setInt32",
        PrimitiveType::U32 => "setUint32",
        PrimitiveType::I64 | PrimitiveType::ISize => "setInt64",
        PrimitiveType::U64 | PrimitiveType::USize => "setUint64",
        PrimitiveType::F32 => "setFloat32",
        PrimitiveType::F64 => "setFloat64",
    }
}

pub fn emit_write_blittable_value(
    offset: &str,
    primitive: PrimitiveType,
    value: &str,
    bytes_name: &str,
) -> String {
    format!(
        "{}.{}({}, {}{})",
        bytes_name,
        primitive_blittable_write_method(primitive),
        offset,
        primitive_as_num(primitive, value),
        match primitive {
            PrimitiveType::U8 | PrimitiveType::I8 | PrimitiveType::Bool => "",
            _ => ", $$typed_data.Endian.little",
        }
    )
}

pub fn primitive_blittable_read_method(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::I8 => "getInt8",
        PrimitiveType::Bool | PrimitiveType::U8 => "getUint8",
        PrimitiveType::I16 => "getInt16",
        PrimitiveType::U16 => "getUint16",
        PrimitiveType::I32 => "getInt32",
        PrimitiveType::U32 => "getUint32",
        PrimitiveType::I64 | PrimitiveType::ISize => "getInt64",
        PrimitiveType::U64 | PrimitiveType::USize => "getUint64",
        PrimitiveType::F32 => "getFloat32",
        PrimitiveType::F64 => "getFloat64",
    }
}

pub fn emit_read_blittable_value(
    offset: &str,
    primitive: PrimitiveType,
    bytes_name: &str,
) -> String {
    num_as_primitive(
        primitive,
        format!(
            "{}.{}({}{})",
            bytes_name,
            primitive_blittable_read_method(primitive),
            offset,
            match primitive {
                PrimitiveType::U8 | PrimitiveType::I8 | PrimitiveType::Bool => "",
                _ => ", $$typed_data.Endian.little",
            }
        )
        .as_str(),
    )
}

pub fn primitive_write_method(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::Bool => "writeBool",
        PrimitiveType::I8 => "writeI8",
        PrimitiveType::U8 => "writeU8",
        PrimitiveType::I16 => "writeI16",
        PrimitiveType::U16 => "writeU16",
        PrimitiveType::I32 => "writeI32",
        PrimitiveType::U32 => "writeU32",
        PrimitiveType::I64 | PrimitiveType::ISize => "writeI64",
        PrimitiveType::U64 | PrimitiveType::USize => "writeU64",
        PrimitiveType::F32 => "writeF32",
        PrimitiveType::F64 => "writeF64",
    }
}

fn emit_write_primitive(primitive: PrimitiveType, writer_name: &str, value: &str) -> String {
    format!(
        "{}.{}({});",
        writer_name,
        primitive_write_method(primitive),
        value
    )
}

fn enum_tag_write_expr(tag_type: PrimitiveType, writer_name: &str, value: &str) -> String {
    let write_method = primitive_write_method(tag_type);

    format!("{}.{}({})", writer_name, write_method, value)
}

fn emit_write_builtin(id: &BuiltinId, writer_name: &str, value: &str) -> String {
    match id.as_str() {
        "Duration" => format!("{}.writeDuration({});", writer_name, value),
        "SystemTime" => format!("{}.writeInstant({});", writer_name, value),
        "Uuid" => format!("{}.writeUUID({});", writer_name, value),
        "Url" => format!("{}.writeUri({});", writer_name, value),
        _ => format!("{}.writeString({});", writer_name, value),
    }
}

fn write_seq_dart_type(seq: &WriteSeq) -> String {
    match seq.ops.first() {
        Some(WriteOp::Primitive { primitive, .. }) => {
            type_expr_dart_type(&TypeExpr::Primitive(*primitive))
        }
        Some(WriteOp::String { .. }) => "String".to_string(),
        Some(WriteOp::Builtin { id, .. }) => type_expr_dart_type(&TypeExpr::Builtin(id.clone())),
        Some(WriteOp::Record { id, .. }) => render_type_name(id.as_str()),
        Some(WriteOp::Enum { id, .. }) => render_type_name(id.as_str()),
        Some(WriteOp::Custom { id, .. }) => render_type_name(id.as_str()),
        Some(WriteOp::Vec { element_type, .. }) => {
            type_expr_dart_type(&TypeExpr::Vec(Box::new(element_type.clone())))
        }
        Some(WriteOp::Option { some, .. }) => format!("{}?", write_seq_dart_type(some)),
        Some(WriteOp::Result { ok, err, .. }) => format!(
            "$$BoltResult<{}, {}>",
            write_seq_dart_type(ok),
            write_seq_dart_type(err)
        ),
        _ => "dynamic".to_string(),
    }
}

fn emit_writer_vec(
    value: &str,
    element_type: &TypeExpr,
    element: &WriteSeq,
    _layout: &VecLayout,
    writer_name: &str,
) -> String {
    match element_type {
        TypeExpr::Primitive(primitive) => {
            let value = match primitive {
                PrimitiveType::Bool => format!("{value}._bytes"),
                PrimitiveType::I8
                | PrimitiveType::U8
                | PrimitiveType::I16
                | PrimitiveType::U16
                | PrimitiveType::I32
                | PrimitiveType::U32
                | PrimitiveType::I64
                | PrimitiveType::U64
                | PrimitiveType::ISize
                | PrimitiveType::USize
                | PrimitiveType::F32
                | PrimitiveType::F64 => value.to_string(),
            };

            format!("{writer_name}.writeBytes({value});")
        }
        _ => {
            let inner_write_expr = emit_writer_write(element, writer_name, "_p$item");
            format!(
                "{writer_name}.writeList({value}, (_p$item, {writer_name}) {{ {} }});",
                inner_write_expr
            )
        }
    }
}

pub fn emit_writer_write(seq: &WriteSeq, writer_name: &str, value: &str) -> String {
    match seq.ops.first() {
        Some(WriteOp::Primitive { primitive, .. }) => {
            format!(
                "{writer_name}.{}({});",
                primitive_write_method(*primitive),
                value,
            )
        }
        Some(WriteOp::String { .. }) => format!("{writer_name}.writeString({value});"),
        Some(WriteOp::Builtin { id, .. }) => emit_write_builtin(id, writer_name, value),
        Some(WriteOp::Record { .. }) => format!("{value}._m$wireEncode({writer_name});",),
        Some(WriteOp::Enum { .. }) => format!("{value}._m$wireEncode({writer_name});"),
        Some(WriteOp::Custom { underlying, .. }) => {
            emit_writer_write(underlying, writer_name, value)
        }
        Some(WriteOp::Vec {
            element_type,
            element,
            layout,
            ..
        }) => emit_writer_vec(value, element_type, element, layout, writer_name),
        Some(WriteOp::Option { some, .. }) => {
            let inner_write_expr = emit_writer_write(some, writer_name, value);

            format!(
                r#"if ({value} case final {value}?) {{ {writer_name}.writeU8(1); {inner_write_expr} }} else {{ {writer_name}.writeU8(0); }}"#
            )
        }
        Some(WriteOp::Result { ok, err, .. }) => {
            let err_op = err.ops.first().expect("write ops");

            format!(
                r#"switch ({value}) {{ case $$BoltResult$Ok(:final value): {{ {writer_name}.writeU8(0); {} }} case $$BoltResult$Err(:final value): {{ {writer_name}.writeU8(1); {} }} }}"#,
                emit_writer_write(ok, writer_name, "value"),
                match err_op {
                    WriteOp::String { .. } => format!("value._m$wireEncode({writer_name});"),
                    _ => emit_writer_write(err, writer_name, "value"),
                }
            )
        }
        _ => String::new(),
    }
}

pub fn primitive_read_method(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::Bool => "readBool",
        PrimitiveType::I8 => "readI8",
        PrimitiveType::U8 => "readU8",
        PrimitiveType::I16 => "readI16",
        PrimitiveType::U16 => "readU16",
        PrimitiveType::I32 => "readI32",
        PrimitiveType::U32 => "readU32",
        PrimitiveType::I64 | PrimitiveType::ISize => "readI64",
        PrimitiveType::U64 | PrimitiveType::USize => "readU64",
        PrimitiveType::F32 => "readF32",
        PrimitiveType::F64 => "readF64",
    }
}

fn emit_reader_vec(
    element_type: &TypeExpr,
    element: &ReadSeq,
    _layout: &VecLayout,
    reader_name: &str,
    is_void: bool,
) -> String {
    match element_type {
        TypeExpr::Primitive(primitive) => {
            let method = match primitive {
                PrimitiveType::Bool => "readBoolList",
                PrimitiveType::U8 => "readUint8List",
                PrimitiveType::I8 => "readInt8List",
                PrimitiveType::I16 => "readInt16List",
                PrimitiveType::U16 => "readUint16List",
                PrimitiveType::I32 => "readInt32List",
                PrimitiveType::U32 => "readUint32List",
                PrimitiveType::U64 | PrimitiveType::USize => "readUint64List",
                PrimitiveType::I64 | PrimitiveType::ISize => "readInt64List",
                PrimitiveType::F32 => "readFloat32List",
                PrimitiveType::F64 => "readFloat64List",
            };
            format!("{reader_name}.{}()", method)
        }
        _ => {
            let inner_read_expr = emit_reader_read(element, reader_name, is_void);
            format!("{reader_name}.readList(({reader_name}) => {inner_read_expr})")
        }
    }
}

pub fn emit_reader_read(seq: &ReadSeq, reader_name: &str, is_inner_void: bool) -> String {
    let op = seq.ops.first().expect("read ops");
    match op {
        ReadOp::Primitive { primitive, .. } => {
            format!("{reader_name}.{}()", primitive_read_method(*primitive))
        }
        ReadOp::String { .. } => format!("{reader_name}.readString()"),
        ReadOp::Record { id, .. } => {
            format!(
                "{}._m$wireDecode({reader_name})",
                NamingConvention::class_name(id.as_str())
            )
        }
        ReadOp::Enum { id, layout, .. } => match layout {
            EnumLayout::CStyle {
                is_error: false, ..
            } => {
                format!(
                    "{}._m$wireDecode({reader_name})",
                    render_type_name(id.as_str()),
                )
            }
            EnumLayout::CStyle { is_error: true, .. }
            | EnumLayout::Data { .. }
            | EnumLayout::Recursive => {
                format!(
                    "{}._m$wireDecode({reader_name})",
                    render_type_name(id.as_str())
                )
            }
        },
        ReadOp::Option { some, .. } => {
            let inner_read_expr = emit_reader_read(some, reader_name, is_inner_void);
            format!(
                r#"switch ({reader_name}.readU8()) {{ 0 => null, 1 => {inner_read_expr}, (int _l$tag) => throw $$BoltException("Invalid Optional tag: ${{_l$tag}}") }}"#
            )
        }
        ReadOp::Vec {
            element_type,
            element,
            layout,
            ..
        } => emit_reader_vec(element_type, element, layout, reader_name, is_inner_void),
        ReadOp::Result { ok, err, .. } => {
            let ok_expr = if is_inner_void {
                "null".to_string()
            } else {
                emit_reader_read(ok, reader_name, is_inner_void)
            };
            let err_op = err.ops.first().expect("read ops");

            let err_expr = match err_op {
                ReadOp::String { .. } => format!("$$BoltException._m$wireDecode({reader_name})"),
                _ => emit_reader_read(err, reader_name, is_inner_void),
            };
            format!(
                r#"(switch ({reader_name}.readU8()) {{ 0 => $$BoltResult.ok({ok_expr}), 1 => $$BoltResult.err({err_expr}), (int _l$tag) => throw $$BoltException("Invalid Result tag: ${{_l$tag}}") }})"#
            )
        }
        ReadOp::Builtin { id, .. } => match id.as_str() {
            "Duration" => format!("{reader_name}.readDuration()"),
            "SystemTime" => format!("{reader_name}.readInstant()"),
            "Uuid" => format!("{reader_name}.readUUID()"),
            "Url" => format!("{reader_name}.readUri()"),
            _ => format!("{reader_name}.readString()"),
        },
        ReadOp::Custom { underlying, .. } => {
            emit_reader_read(underlying, reader_name, is_inner_void)
        }
    }
}

pub(crate) fn remap_size_expr_value_expr(expr: &SizeExpr, v: ValueExpr) -> SizeExpr {
    match expr {
        SizeExpr::Fixed(value) => SizeExpr::Fixed(*value),
        SizeExpr::Runtime => SizeExpr::Runtime,
        SizeExpr::StringLen(..) => SizeExpr::StringLen(v),
        SizeExpr::BytesLen(..) => SizeExpr::BytesLen(v),
        SizeExpr::ValueSize(..) => SizeExpr::ValueSize(v),
        SizeExpr::WireSize { owner, .. } => SizeExpr::WireSize {
            owner: owner.clone(),
            value: v,
        },
        SizeExpr::BuiltinSize { id, .. } => SizeExpr::BuiltinSize {
            id: id.clone(),
            value: v,
        },
        SizeExpr::Sum(exprs) => SizeExpr::Sum(
            exprs
                .iter()
                .map(|s| remap_size_expr_value_expr(s, v.clone()))
                .collect(),
        ),
        SizeExpr::OptionSize { inner, .. } => SizeExpr::OptionSize {
            value: v,
            inner: inner.clone(),
        },
        SizeExpr::VecSize { inner, layout, .. } => SizeExpr::VecSize {
            value: v,
            inner: inner.clone(),
            layout: layout.clone(),
        },
        SizeExpr::ResultSize { ok, err, .. } => SizeExpr::ResultSize {
            value: v,
            ok: ok.clone(),
            err: err.clone(),
        },
    }
}

pub fn remap_write_seq(mut seq: WriteSeq) -> WriteSeq {
    match seq.ops.first_mut() {
        Some(WriteOp::Result { err: res_err, .. }) => {
            let SizeExpr::ResultSize {
                err: seq_res_err_size,
                ..
            } = &mut seq.size
            else {
                unreachable!()
            };
            let res_err_op = res_err.ops.first().expect("write op");
            if let WriteOp::String { value } = res_err_op {
                **seq_res_err_size = SizeExpr::WireSize {
                    value: value.clone(),
                    owner: Some(WireSizeOwner::Record(RecordId::new("_$$BoltException"))),
                };
                res_err.size = seq_res_err_size.as_ref().clone();
            }
            seq
        }
        _ => seq,
    }
}

fn emit_vec_size(value: &str, inner: &SizeExpr, layout: &VecLayout) -> String {
    match layout {
        VecLayout::Blittable { element_size } => {
            format!("(4 + ({}.length * {}))", value, element_size)
        }
        VecLayout::Encoded => format!(
            "{value}.fold<int>(4, (_p$sum, _p$item) => _p$sum + {})",
            emit_size_expr(&remap_size_expr_value_expr(
                inner,
                ValueExpr::Named("_p$item".to_string())
            ))
        ),
    }
}

fn emit_builtin_size(id: &BuiltinId, value: &str) -> String {
    if id.as_str() == "Url" {
        format!("{}.toString().length * 3", value)
    } else {
        format!("{}._m$wireEncodedSize()", value)
    }
}

pub fn emit_size_expr(size: &SizeExpr) -> String {
    match size {
        SizeExpr::Fixed(value) => value.to_string(),
        SizeExpr::Runtime => "0".to_string(),
        SizeExpr::StringLen(value) => format!("({}.length * 3)", render_value(value)),
        SizeExpr::BytesLen(value) => format!("{}.length", render_value(value)),
        SizeExpr::ValueSize(value) => render_value(value),
        SizeExpr::WireSize { value, .. } => format!("{}._m$wireEncodedSize()", render_value(value)),
        SizeExpr::BuiltinSize { id, value } => emit_builtin_size(id, &render_value(value)),
        SizeExpr::Sum(parts) => {
            let rendered = parts
                .iter()
                .map(emit_size_expr)
                .reduce(|acc, s| acc + " + " + s.as_str())
                .unwrap_or_else(|| String::from("0"));
            format!("({})", rendered)
        }
        SizeExpr::OptionSize { value, inner } => {
            let inner_expr = emit_size_expr(&remap_size_expr_value_expr(
                inner,
                ValueExpr::Var(format!("{}!", render_value(value))),
            ));
            format!(
                "(switch ({} == null) {{ true => 1, false => 1 + {} }})",
                render_value(value),
                inner_expr
            )
        }
        SizeExpr::VecSize {
            value,
            inner,
            layout,
        } => emit_vec_size(&render_value(value), inner, layout),
        SizeExpr::ResultSize { value, ok, err } => {
            let ok_expr = emit_size_expr(&remap_size_expr_value_expr(
                ok,
                ValueExpr::Var("value".to_string()),
            ));
            let err_expr = emit_size_expr(&remap_size_expr_value_expr(
                err,
                ValueExpr::Var("value".to_string()),
            ));
            format!(
                "1 + (switch ({}) {{ $$BoltResult$Ok(:final value) => {}, $$BoltResult$Err(:final value) => {} }})",
                render_value(value),
                ok_expr,
                err_expr
            )
        }
    }
}

pub fn emit_cmp_expr(expr_a: &str, expr_b: &str, expr_ty: &DartType) -> String {
    match expr_ty {
        DartType::Void
        | DartType::Bool
        | DartType::Int(..)
        | DartType::Double(..)
        | DartType::String
        | DartType::Closure(..)
        | DartType::Record(..)
        | DartType::Enum(..)
        | DartType::Callback(..)
        | DartType::Builtin(..)
        | DartType::Custom(..) => format!("{expr_a} == {expr_b}"),
        DartType::Option(ty) => format!(
            "_$$BoltUtil.nullableCompare({expr_a}, {expr_b}, (_l$a, _l$b) => {})",
            emit_cmp_expr("_l$a", "_l$b", ty)
        ),
        DartType::List(ty) => format!(
            "_$$BoltUtil.listCompare({}, {}, (_l$a, _l$b) => {})",
            expr_a,
            expr_b,
            emit_cmp_expr("_l$a", "_l$b", ty)
        ),
        DartType::Bytes => format!(
            "_$$BoltUtil.listCompare({}, {}, (_l$a, _l$b) => _l$a == _l$b)",
            expr_a, expr_b
        ),
        DartType::Result { ok, err } => format!(
            "_$$BoltUtil.fallibleCompare({}, {}, (_l$okA, _l$okB) => {}, (_l$errA, _l$errB) => {})",
            expr_a,
            expr_b,
            emit_cmp_expr("_l$okA", "_l$okB", ok),
            emit_cmp_expr("_l$errA", "_l$errB", err)
        ),
        DartType::Class(_) => todo!(),
    }
}
