use boltffi_ffi_rules::transport::ParamValueStrategy;

use crate::{
    ir::{
        AbiParam, AbiType, CallbackKind, ErrorTransport, ParamDef, ParamRole, PrimitiveType,
        RecordId, ReturnDef, ReturnShape, SpanContent, Transport, TypeCatalog, TypeExpr,
    },
    render::dart::NamingConvention,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartFFIIntType {
    Uint8,
    Int8,
    Uint16,
    Int16,
    Uint32,
    Int32,
    Uint64,
    Int64,
    UintPtr,
    IntPtr,
}

impl DartFFIIntType {
    pub fn native_type(&self) -> String {
        match self {
            Self::Uint8 => "$$ffi.Uint8".to_string(),
            Self::Int8 => "$$ffi.Int8".to_string(),
            Self::Uint16 => "$$ffi.Uint16".to_string(),
            Self::Int16 => "$$ffi.Int16".to_string(),
            Self::Uint32 => "$$ffi.Uint32".to_string(),
            Self::Int32 => "$$ffi.Int32".to_string(),
            Self::Uint64 => "$$ffi.Uint64".to_string(),
            Self::Int64 => "$$ffi.Int64".to_string(),
            Self::UintPtr => "$$ffi.UintPtr".to_string(),
            Self::IntPtr => "$$ffi.IntPtr".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartFFIFloatType {
    Float32,
    Float64,
}

impl DartFFIFloatType {
    pub fn native_type(&self) -> String {
        match self {
            DartFFIFloatType::Float32 => "$$ffi.Float".to_string(),
            DartFFIFloatType::Float64 => "$$ffi.Double".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DartFFIPrimitiveType {
    Bool,
    Int(super::DartFFIIntType),
    Float(super::DartFFIFloatType),
}

impl DartFFIPrimitiveType {
    pub fn from_primitive(primitive: PrimitiveType) -> Self {
        match primitive {
            PrimitiveType::Bool => DartFFIPrimitiveType::Bool,
            PrimitiveType::I8 => DartFFIPrimitiveType::Int(DartFFIIntType::Int8),
            PrimitiveType::U8 => DartFFIPrimitiveType::Int(DartFFIIntType::Uint8),
            PrimitiveType::I16 => DartFFIPrimitiveType::Int(DartFFIIntType::Int16),
            PrimitiveType::U16 => DartFFIPrimitiveType::Int(DartFFIIntType::Uint16),
            PrimitiveType::I32 => DartFFIPrimitiveType::Int(DartFFIIntType::Int32),
            PrimitiveType::U32 => DartFFIPrimitiveType::Int(DartFFIIntType::Uint32),
            PrimitiveType::I64 => DartFFIPrimitiveType::Int(DartFFIIntType::Int64),
            PrimitiveType::U64 => DartFFIPrimitiveType::Int(DartFFIIntType::Uint64),
            PrimitiveType::ISize => DartFFIPrimitiveType::Int(DartFFIIntType::IntPtr),
            PrimitiveType::USize => DartFFIPrimitiveType::Int(DartFFIIntType::UintPtr),
            PrimitiveType::F32 => DartFFIPrimitiveType::Float(DartFFIFloatType::Float32),
            PrimitiveType::F64 => DartFFIPrimitiveType::Float(DartFFIFloatType::Float64),
        }
    }

    pub fn native_type(&self) -> String {
        match self {
            DartFFIPrimitiveType::Bool => String::from("$$ffi.Bool"),
            DartFFIPrimitiveType::Int(int) => int.native_type(),
            DartFFIPrimitiveType::Float(float) => float.native_type(),
        }
    }

    pub fn sub_type(&self) -> String {
        match self {
            DartFFIPrimitiveType::Bool => String::from("bool"),
            DartFFIPrimitiveType::Int(..) => String::from("int"),
            DartFFIPrimitiveType::Float(..) => String::from("double"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DartFunctionParamSig {
    pub name: String,
    pub ty: super::DartType,
}

#[derive(Debug, Clone)]
pub struct DartFunctionSig {
    pub args: Vec<DartFunctionParamSig>,
    pub ret: super::DartReturnType,
}

impl DartFunctionSig {
    pub fn from_params_return_def(
        params: &[ParamDef],
        returns: &ReturnDef,
        type_catalog: &TypeCatalog,
    ) -> Self {
        DartFunctionSig {
            args: params
                .iter()
                .map(|p| DartFunctionParamSig {
                    name: NamingConvention::param_name(p.name.as_str()),
                    ty: DartType::from_type_expr(&p.type_expr, type_catalog),
                })
                .collect(),
            ret: DartReturnType {
                inner: DartType::from_return_def(returns, type_catalog),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartFFIFunctionParamSig {
    pub name: String,
    pub ty: super::DartFFIType,
}

impl DartFFIFunctionParamSig {
    pub fn from_abi_param(p: &AbiParam) -> Self {
        let name = match &p.role {
            ParamRole::Input { contract, .. } => match contract.value_strategy() {
                ParamValueStrategy::DirectBuffer(..)
                | ParamValueStrategy::WireEncoded(..)
                | ParamValueStrategy::Utf8String => {
                    format!("{}Ptr", NamingConvention::param_name(p.name.as_str()))
                }
                _ => NamingConvention::param_name(p.name.as_str()),
            },
            ParamRole::OutDirect => String::from("_p$outPtr"),
            ParamRole::OutLen { .. } => String::from("_p$outLen"),
            _ => NamingConvention::param_name(p.name.as_str()),
        };
        DartFFIFunctionParamSig {
            name,
            ty: DartFFIType::from_abi_param(p),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartFFIFunctionSig {
    pub args: Vec<DartFFIFunctionParamSig>,
    pub ret: super::DartFFIType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DartFFIType {
    Void,
    Bool,
    Int(DartFFIIntType),
    Float(DartFFIFloatType),
    Struct(String),
    Pointer(Box<DartFFIType>),
    NativeFunction {
        sig: Box<DartFFIFunctionSig>,
        // params: Vec<DartFFIClosureParam>,
        // returns: DartFFIReturnsRecv,
    },
    Status,
    Buf,
    CallbackHandle(bool),
}

impl DartFFIType {
    pub fn from_primitive(primitive: PrimitiveType) -> Self {
        match primitive {
            PrimitiveType::Bool => Self::Bool,
            PrimitiveType::I8 => Self::Int(DartFFIIntType::Int8),
            PrimitiveType::U8 => Self::Int(DartFFIIntType::Uint8),
            PrimitiveType::I16 => Self::Int(DartFFIIntType::Int16),
            PrimitiveType::U16 => Self::Int(DartFFIIntType::Uint16),
            PrimitiveType::I32 => Self::Int(DartFFIIntType::Int32),
            PrimitiveType::U32 => Self::Int(DartFFIIntType::Uint32),
            PrimitiveType::I64 => Self::Int(DartFFIIntType::Int64),
            PrimitiveType::U64 => Self::Int(DartFFIIntType::Uint64),
            PrimitiveType::ISize => Self::Int(DartFFIIntType::IntPtr),
            PrimitiveType::USize => Self::Int(DartFFIIntType::UintPtr),
            PrimitiveType::F32 => Self::Float(DartFFIFloatType::Float32),
            PrimitiveType::F64 => Self::Float(DartFFIFloatType::Float64),
        }
    }

    pub fn from_abi_type(abi_type: &AbiType) -> Self {
        match abi_type {
            AbiType::Void => Self::Void,
            AbiType::Bool => Self::Bool,
            AbiType::I8 => Self::Int(DartFFIIntType::Int8),
            AbiType::U8 => Self::Int(DartFFIIntType::Uint8),
            AbiType::I16 => Self::Int(DartFFIIntType::Int16),
            AbiType::U16 => Self::Int(DartFFIIntType::Uint16),
            AbiType::I32 => Self::Int(DartFFIIntType::Int32),
            AbiType::U32 => Self::Int(DartFFIIntType::Uint32),
            AbiType::I64 => Self::Int(DartFFIIntType::Int64),
            AbiType::U64 => Self::Int(DartFFIIntType::Uint64),
            AbiType::ISize => Self::Int(DartFFIIntType::IntPtr),
            AbiType::USize => Self::Int(DartFFIIntType::UintPtr),
            AbiType::F32 => Self::Float(DartFFIFloatType::Float32),
            AbiType::F64 => Self::Float(DartFFIFloatType::Float64),
            AbiType::Pointer(primitive) => {
                Self::Pointer(Box::new(Self::from_primitive(*primitive)))
            }
            AbiType::OwnedBuffer => Self::Buf,
            AbiType::InlineCallbackFn {
                params,
                return_type,
            } => Self::NativeFunction {
                sig: Box::new(DartFFIFunctionSig {
                    args: std::iter::chain(
                        // userdata pointer
                        std::iter::once(DartFFIFunctionParamSig {
                            name: "_p$ud".to_string(),
                            ty: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                        }),
                        params
                            .iter()
                            .enumerate()
                            .map(|(i, ty)| DartFFIFunctionParamSig {
                                name: format!("_p$arg{}", i),
                                ty: DartFFIType::from_abi_type(ty),
                            }),
                    )
                    .collect(),
                    ret: DartFFIType::from_abi_type(return_type),
                }),
            },
            AbiType::Handle(_) => Self::Pointer(Box::new(Self::Void)),
            AbiType::CallbackHandle => Self::CallbackHandle(false),
            AbiType::Struct(record_id) => {
                Self::Struct(NamingConvention::record_struct_name(record_id.as_str()))
            }
        }
    }

    pub fn from_abi_param(abi_param: &AbiParam) -> Self {
        if let ParamRole::CallbackContext { .. } = &abi_param.role {
            return Self::Pointer(Box::new(Self::Void));
        }

        if let ParamRole::StatusOut = &abi_param.role {
            return Self::Pointer(Box::new(Self::Status));
        }

        if let ParamRole::Input { contract, .. } = &abi_param.role
            && !matches!(&abi_param.abi_type, AbiType::InlineCallbackFn { .. })
            && let ParamValueStrategy::CallbackHandle { nullable, .. } = contract.value_strategy()
        {
            return Self::CallbackHandle(nullable);
        }

        let native_type = Self::from_abi_type(&abi_param.abi_type);

        match &abi_param.role {
            ParamRole::OutDirect | ParamRole::OutLen { .. } => Self::Pointer(Box::new(native_type)),
            _ => native_type,
        }
    }

    pub fn from_transport(transport: &Transport) -> Self {
        match transport {
            Transport::Scalar(origin) => Self::from_primitive(origin.primitive()),
            Transport::Composite(layout) => Self::Struct(NamingConvention::record_struct_name(
                layout.record_id.as_str(),
            )),
            Transport::Span(content) => match content {
                SpanContent::Scalar(origin) => Self::from_primitive(origin.primitive()),
                SpanContent::Composite(layout) => Self::Struct(layout.record_id.to_string()),
                SpanContent::Utf8 | SpanContent::Encoded(..) => Self::Buf,
            },
            Transport::Handle { .. } => Self::Pointer(Box::new(Self::Void)),
            Transport::Callback { nullable, .. } => Self::CallbackHandle(*nullable),
        }
    }

    pub fn from_return_shape_and_error_transport(
        return_shape: &ReturnShape,
        error_transport: &ErrorTransport,
    ) -> Self {
        if let Some(Transport::Handle { class_id, .. }) = &return_shape.transport {
            return Self::from_abi_type(&AbiType::Handle(class_id.clone()));
        }

        if matches!(return_shape.transport, Some(Transport::Callback { .. })) {
            return Self::from_abi_type(&AbiType::CallbackHandle);
        }

        if matches!(error_transport, ErrorTransport::Encoded { .. }) {
            return Self::from_abi_type(&AbiType::OwnedBuffer);
        }

        match &return_shape.transport {
            None => {
                if matches!(error_transport, ErrorTransport::StatusCode) {
                    Self::Status
                } else {
                    Self::from_abi_type(&AbiType::Void)
                }
            }
            Some(Transport::Scalar(origin)) => Self::from_primitive(origin.primitive()),
            Some(Transport::Composite(layout)) => {
                Self::from_abi_type(&AbiType::Struct(layout.record_id.clone()))
            }
            Some(Transport::Span(_)) => Self::from_abi_type(&AbiType::OwnedBuffer),
            Some(Transport::Handle { .. } | Transport::Callback { .. }) => unreachable!(),
        }
    }

    pub fn native_type(&self) -> String {
        match self {
            Self::Void => "$$ffi.Void".to_string(),
            Self::Bool => "$$ffi.Bool".to_string(),
            Self::Int(ty) => ty.native_type(),
            Self::Float(ty) => ty.native_type(),
            Self::Struct(record) => record.to_string(),
            Self::NativeFunction { sig } => format!(
                "$$ffi.Pointer<$$ffi.NativeFunction<{} Function({})>>",
                sig.ret.native_type(),
                sig.args
                    .iter()
                    .map(|p| p.ty.native_type())
                    .reduce(|acc, ty| acc + ", " + ty.as_str())
                    .unwrap_or_default()
            ),
            Self::Pointer(inner) => format!("$$ffi.Pointer<{}>", inner.native_type()),
            Self::Buf => "_$$BoltFFIBuf".to_string(),
            Self::CallbackHandle(_) => "_$$BoltCallbackHandle".to_string(),
            Self::Status => "_$$BoltFFIStatus".to_string(),
        }
    }

    pub fn sub_type(&self) -> String {
        match self {
            Self::Void => "void".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Int(..) => "int".to_string(),
            Self::Float(..) => "double".to_string(),
            Self::Struct(record) => record.to_string(),
            o @ (Self::Pointer(..)
            | Self::Buf
            | Self::CallbackHandle(_)
            | Self::Status
            | Self::NativeFunction { .. }) => o.native_type(),
        }
    }

    pub fn struct_field_annotation(&self) -> String {
        match self {
            DartFFIType::Void
            | DartFFIType::Struct(_)
            | DartFFIType::Pointer(..)
            | DartFFIType::NativeFunction { .. }
            | DartFFIType::Status
            | DartFFIType::Buf
            | DartFFIType::CallbackHandle(_) => String::new(),
            o @ (DartFFIType::Bool | DartFFIType::Int(..) | DartFFIType::Float(..)) => {
                format!("@{}()", o.native_type())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DartClosureParamType {
    pub name: String,
    pub ty: DartType,
}

#[derive(Debug, Clone)]
pub struct DartClosureType {
    pub arg_ty: Vec<DartClosureParamType>,
    pub ret_ty: DartReturnType,
}

#[derive(Debug, Clone)]
pub struct DartReturnType {
    pub(crate) inner: DartType,
}

impl DartReturnType {
    pub fn from_return_def(return_def: &ReturnDef, type_catalog: &TypeCatalog) -> Self {
        DartReturnType {
            inner: DartType::from_return_def(return_def, type_catalog),
        }
    }

    pub fn dart_type(&self) -> String {
        match &self.inner {
            DartType::Result { ok, .. } => ok.dart_type(),
            _ => self.inner.dart_type(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DartType {
    Void,
    Bool,
    Int(DartFFIIntType),
    Double(DartFFIFloatType),
    String,
    Option(Box<DartType>),
    List(Box<DartType>),
    Bytes,
    Closure(Box<DartClosureType>),
    Result {
        ok: Box<DartType>,
        err: Box<DartType>,
    },
    Record(String),
    Enum(String),
    Class(String),
    Callback(String),
    Builtin(String),
    Custom(String),
}

impl DartType {
    pub fn from_primitive(primitive: PrimitiveType) -> Self {
        match primitive {
            PrimitiveType::Bool => DartType::Bool,
            PrimitiveType::I8 => DartType::Int(DartFFIIntType::Int8),
            PrimitiveType::U8 => DartType::Int(DartFFIIntType::Uint8),
            PrimitiveType::I16 => DartType::Int(DartFFIIntType::Int16),
            PrimitiveType::U16 => DartType::Int(DartFFIIntType::Uint16),
            PrimitiveType::I32 => DartType::Int(DartFFIIntType::Int32),
            PrimitiveType::U32 => DartType::Int(DartFFIIntType::Uint32),
            PrimitiveType::I64 => DartType::Int(DartFFIIntType::Int64),
            PrimitiveType::U64 => DartType::Int(DartFFIIntType::Uint64),
            PrimitiveType::ISize => DartType::Int(DartFFIIntType::IntPtr),
            PrimitiveType::USize => DartType::Int(DartFFIIntType::UintPtr),
            PrimitiveType::F32 => DartType::Double(DartFFIFloatType::Float32),
            PrimitiveType::F64 => DartType::Double(DartFFIFloatType::Float64),
        }
    }

    fn from_result(ok: &TypeExpr, err: &TypeExpr, type_catalog: &TypeCatalog) -> Self {
        let mapped_err = match err {
            TypeExpr::String => TypeExpr::Record(RecordId::new("$$BoltException")),
            err => err.clone(),
        };
        DartType::Result {
            ok: Box::new(Self::from_type_expr(ok, type_catalog)),
            err: Box::new(Self::from_type_expr(&mapped_err, type_catalog)),
        }
    }

    pub fn from_type_expr(type_expr: &TypeExpr, type_catalog: &TypeCatalog) -> Self {
        match type_expr {
            TypeExpr::Void => DartType::Void,
            TypeExpr::Primitive(primitive) => Self::from_primitive(*primitive),
            TypeExpr::String | TypeExpr::Str => DartType::String,
            TypeExpr::Vec(inner) => match inner.as_ref() {
                TypeExpr::Primitive(PrimitiveType::U8) => DartType::Bytes,
                _ => DartType::List(Box::new(Self::from_type_expr(inner, type_catalog))),
            },
            TypeExpr::Option(inner) => {
                DartType::Option(Box::new(Self::from_type_expr(inner, type_catalog)))
            }
            TypeExpr::Result { ok, err } => Self::from_result(ok, err, type_catalog),
            TypeExpr::Record(record_id) => DartType::Record(record_id.to_string()),
            TypeExpr::Enum(enum_id) => DartType::Enum(enum_id.to_string()),
            TypeExpr::Callback(callback_id) => {
                let callback_def = type_catalog.resolve_callback(callback_id).unwrap();
                match callback_def.kind {
                    CallbackKind::Trait => DartType::Callback(callback_id.to_string()),
                    CallbackKind::Closure => {
                        let call_method = &callback_def.methods[0];
                        assert!(call_method.id.as_str() == "call");
                        DartType::Closure(Box::new(DartClosureType {
                            arg_ty: call_method
                                .params
                                .iter()
                                .map(|p| DartClosureParamType {
                                    name: p.name.to_string(),
                                    ty: Self::from_type_expr(&p.type_expr, type_catalog),
                                })
                                .collect(),
                            ret_ty: DartReturnType {
                                inner: Self::from_return_def(&call_method.returns, type_catalog),
                            },
                        }))
                    }
                }
            }
            TypeExpr::Custom(custom_type_id) => DartType::Custom(custom_type_id.to_string()),
            TypeExpr::Builtin(builtin_id) => DartType::Builtin(match builtin_id.as_str() {
                "Duration" => String::from("Duration"),
                "SystemTime" => String::from("DateTime"),
                "Uuid" => String::from("$$BoltUUIDValue"),
                "Url" => String::from("Uri"),
                b => panic!("Unknown builtin: {b}"),
            }),
            TypeExpr::Handle(class_id) => DartType::Class(class_id.to_string()),
        }
    }

    pub fn from_return_def(return_def: &ReturnDef, type_catalog: &TypeCatalog) -> Self {
        match return_def {
            ReturnDef::Void => DartType::Void,
            ReturnDef::Value(ty) => DartType::from_type_expr(ty, type_catalog),
            ReturnDef::Result { ok, err } => DartType::from_result(ok, err, type_catalog),
        }
    }

    pub fn dart_type(&self) -> String {
        match self {
            DartType::Void => "void".to_string(),
            DartType::Bool => "bool".to_string(),
            DartType::Int(..) => "int".to_string(),
            DartType::Double(..) => "double".to_string(),
            DartType::String => "String".to_string(),
            DartType::Option(inner) => format!("{}?", inner.dart_type()),
            DartType::Result { ok, err } => {
                format!("$$BoltResult<{}, {}>", ok.dart_type(), err.dart_type())
            }
            DartType::Bytes => "$$typed_data.Uint8List".to_string(),
            DartType::List(inner) => match inner.as_ref() {
                DartType::Bool => "$$BoltBoolList".to_string(),
                DartType::Int(int) => match int {
                    DartFFIIntType::Uint8 => "$$typed_data.Uint8List".to_string(),
                    DartFFIIntType::Int8 => "$$typed_data.Int8List".to_string(),
                    DartFFIIntType::Uint16 => "$$typed_data.Uint16List".to_string(),
                    DartFFIIntType::Int16 => "$$typed_data.Int16List".to_string(),
                    DartFFIIntType::Uint32 => "$$typed_data.Uint32List".to_string(),
                    DartFFIIntType::Int32 => "$$typed_data.Int32List".to_string(),
                    DartFFIIntType::Uint64 | DartFFIIntType::UintPtr => {
                        "$$typed_data.Uint64List".to_string()
                    }
                    DartFFIIntType::Int64 | DartFFIIntType::IntPtr => {
                        "$$typed_data.Int64List".to_string()
                    }
                },
                DartType::Double(float) => match float {
                    DartFFIFloatType::Float32 => "$$typed_data.Float32List".to_string(),
                    DartFFIFloatType::Float64 => "$$typed_data.Float64List".to_string(),
                },
                _ => format!("List<{}>", inner.dart_type()),
            },
            DartType::Closure(sig) => format!(
                "{} Function({})",
                sig.ret_ty.dart_type(),
                sig.arg_ty
                    .iter()
                    .map(|arg| arg.ty.dart_type())
                    .reduce(|acc, ty| acc + ", " + ty.as_str())
                    .unwrap_or_default()
            ),
            DartType::Class(class_id) => class_id.to_string(),
            DartType::Record(record_id) => record_id.to_string(),
            DartType::Enum(enum_id) => enum_id.to_string(),
            DartType::Callback(callback_id) => callback_id.to_string(),
            DartType::Custom(custom_type_id) => custom_type_id.to_string(),
            DartType::Builtin(builtin_id) => builtin_id.to_string(),
        }
    }

    pub fn is_inner_void(&self) -> bool {
        match self {
            DartType::Option(ty) | DartType::List(ty) | DartType::Result { ok: ty, .. } => {
                matches!(ty.as_ref(), DartType::Void)
            }
            _ => false,
        }
    }

    pub fn hash_expr(&self, name: &str) -> String {
        match self {
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
            | DartType::Custom(..) => format!("{}.hashCode", name),
            DartType::Option(ty) => format!(
                "({} == null ? null.hashCode : {})",
                name,
                ty.hash_expr(&format!("{}!", name))
            ),
            DartType::List(ty) => format!(
                "_$$BoltUtil.listHash({}, (_p$item) => {})",
                name,
                ty.hash_expr("_p$item")
            ),
            DartType::Bytes => format!(
                "_$$BoltUtil.listHash({}, (_p$item) => _p$item.hashCode)",
                name
            ),
            DartType::Result { ok, err } => format!(
                "switch ({}) {{ $$BoltResult$Ok(value: final _l$value) => {}, $$BoltResult$Err(value: final _l$value) => {} }}",
                name,
                ok.hash_expr("_l$value"),
                err.hash_expr("_l$value")
            ),
            DartType::Class(_) => todo!(),
        }
    }
}
