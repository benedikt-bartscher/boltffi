use boltffi_ffi_rules::transport::{ParamContract, ParamPassingStrategy};

use crate::{
    ir::{
        AbiCall, AbiContract, AbiParam, CallId, CallMode, CallbackKind, ConstructorDef,
        FfiContract, FunctionId, MethodDef, ParamDef, ParamRole, ReadSeq, ReturnDef, SpanContent,
        Transport, TypeExpr, WriteSeq,
    },
    render::dart::{
        DartConstructorKind, DartFFIClosureDef, DartFFIFunctionDef, DartFFIFunctionParamSig,
        DartFFIFunctionSig, DartFFIParamBytes, DartFFIParamValue, DartFFIReturnsPassing,
        DartFFIType, DartFFIValuePassing, DartFunction, DartFunctionCallOwner, DartFunctionMode,
        DartFunctionParam, DartFunctionParamWriteBack, DartFunctionReturns, DartFunctionSig,
        DartFunctionType, DartLibrary, DartMethodReceiver, DartReturnType, DartType,
        NamingConvention, emit,
    },
};

mod callback;
mod class;
mod custom_type;
mod enumeration;
mod record;

pub struct DartLowerer<'a> {
    ffi: &'a FfiContract,
    abi: &'a AbiContract,
    package_name: &'a str,
}

impl<'a> DartLowerer<'a> {
    pub fn new(ffi: &'a FfiContract, abi: &'a AbiContract, package_name: &'a str) -> Self {
        Self {
            ffi,
            abi,
            package_name,
        }
    }

    pub fn abi_call_for_function(&self, function: &FunctionId) -> &AbiCall {
        self.abi
            .calls
            .iter()
            .find(|c| match &c.id {
                CallId::Function(id) => id == function,
                _ => false,
            })
            .unwrap()
    }

    pub fn abi_call_for_call_id(&self, call_id: &CallId) -> &AbiCall {
        self.abi.calls.iter().find(|c| &c.id == call_id).unwrap()
    }

    fn value_passing_from_transport(&self, transport: &Transport) -> DartFFIValuePassing {
        match transport {
            Transport::Scalar(origin) => DartFFIValuePassing::primitive_value(origin.primitive()),
            Transport::Composite(layout) => {
                DartFFIValuePassing::record_value(layout.record_id.to_string())
            }
            Transport::Span(span_content) => match span_content {
                SpanContent::Scalar(origin) => {
                    DartFFIValuePassing::primitive_bytes(origin.primitive())
                }
                SpanContent::Composite(layout) => {
                    DartFFIValuePassing::record_bytes(layout.record_id.to_string())
                }
                SpanContent::Utf8 => DartFFIValuePassing::utf8_bytes(),
                SpanContent::Encoded(..) => DartFFIValuePassing::WireEncoded,
            },
            Transport::Handle { class_id, nullable } => DartFFIValuePassing::ClassHandle {
                class: class_id.to_string(),
                nullable: *nullable,
            },
            Transport::Callback {
                callback_id,
                nullable,
                ..
            } => {
                let cb_def = self.ffi.catalog.resolve_callback(callback_id).unwrap();
                match cb_def.kind {
                    CallbackKind::Trait => DartFFIValuePassing::CallbackHandle {
                        class: callback_id.to_string(),
                        nullable: *nullable,
                    },
                    CallbackKind::Closure => {
                        let abi_cb = self
                            .abi
                            .callbacks
                            .iter()
                            .find(|abi_cb| &abi_cb.callback_id == callback_id)
                            .unwrap();
                        let abi_call = &abi_cb.methods[0];
                        let meth_def = &cb_def.methods[0];
                        assert!(abi_call.id.as_str() == "call");

                        // skip callback handle param def
                        let closure_params = &abi_call.params[1..];

                        let input_closure_params = closure_params
                            .iter()
                            .filter(|p| matches!(p.role, ParamRole::Input { .. }))
                            .cloned()
                            .collect::<Vec<_>>();
                        assert!(meth_def.params.len() == input_closure_params.len());

                        DartFFIValuePassing::Closure(Box::new(DartFFIClosureDef {
                            sig: super::DartFFIFunctionSig {
                                args: std::iter::chain(
                                    // userdata ptr
                                    std::iter::once(super::DartFFIFunctionParamSig {
                                        name: String::from("_p$ud"),
                                        ty: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                                    }),
                                    closure_params
                                        .iter()
                                        .filter(|p| {
                                            // closure output is always returned directly
                                            match &p.role {
                                                ParamRole::Input { .. } => true,
                                                ParamRole::SyntheticLen { .. } => true,
                                                ParamRole::CallbackContext { .. } => false,
                                                ParamRole::OutLen { .. } => false,
                                                ParamRole::OutDirect => false,
                                                ParamRole::StatusOut => false,
                                            }
                                        })
                                        .map(super::DartFFIFunctionParamSig::from_abi_param),
                                )
                                .collect(),
                                ret: DartFFIType::from_return_shape_and_error_transport(
                                    &abi_call.returns,
                                    &abi_call.error,
                                ),
                            },
                            params: self
                                .lower_function_params(&meth_def.params, &input_closure_params),
                            returns: DartFunctionReturns {
                                ty: DartReturnType::from_return_def(
                                    &meth_def.returns,
                                    &self.ffi.catalog,
                                ),
                                passing: match &abi_call.returns.transport {
                                    Some(transport) => DartFFIReturnsPassing::Passing(
                                        self.value_passing_from_transport(transport),
                                    ),
                                    None => DartFFIReturnsPassing::Void,
                                },
                                write_seq: abi_call.returns.encode_ops.clone(),
                                read_seq: abi_call.returns.decode_ops.clone(),
                            },
                        }))
                    }
                }
            }
        }
    }

    fn value_passing_from_param_contract_transport(
        &self,
        contract: &ParamContract,
        transport: &Transport,
    ) -> DartFFIValuePassing {
        match transport {
            Transport::Span(SpanContent::Composite(layout)) => match contract.passing_strategy() {
                ParamPassingStrategy::ByValue => {
                    DartFFIValuePassing::record_value_list(layout.record_id.to_string())
                }
                ParamPassingStrategy::SharedRef | ParamPassingStrategy::MutableRef => {
                    DartFFIValuePassing::record_bytes(layout.record_id.to_string())
                }
            },
            _ => self.value_passing_from_transport(transport),
        }
    }

    fn lower_param(
        &self,
        param: &ParamDef,
        contract: &ParamContract,
        transport: &Transport,
        encode_ops: &Option<WriteSeq>,
        decode_ops: &Option<ReadSeq>,
    ) -> DartFunctionParam {
        let passing = self.value_passing_from_param_contract_transport(contract, transport);
        let ty = DartType::from_type_expr(&param.type_expr, &self.ffi.catalog);
        let writeback = match contract.passing_strategy() {
            ParamPassingStrategy::ByValue | ParamPassingStrategy::SharedRef => None,
            ParamPassingStrategy::MutableRef => match passing {
                DartFFIValuePassing::Bytes(DartFFIParamBytes::Array(
                    DartFFIParamValue::Primitive(primitive),
                )) => Some(DartFunctionParamWriteBack::PrimitiveBytes(primitive)),
                _ => None,
            },
        };

        DartFunctionParam {
            name: NamingConvention::param_name(param.name.as_str()),
            passing,
            write_seq: encode_ops.clone().map(emit::remap_write_seq),
            read_seq: decode_ops.clone(),
            ty,
            writeback,
        }
    }

    fn lower_function_params(
        &self,
        param_defs: &[ParamDef],
        abi_params: &[AbiParam],
    ) -> Vec<DartFunctionParam> {
        let input_abi_params = abi_params
            .iter()
            .filter(|p| matches!(p.role, ParamRole::Input { .. }))
            .collect::<Vec<_>>();

        std::iter::zip(param_defs, input_abi_params)
            .map(|(param_def, abi_param)| {
                let ParamRole::Input {
                    contract,
                    transport,
                    encode_ops,
                    decode_ops,
                    ..
                } = &abi_param.role
                else {
                    unreachable!();
                };

                self.lower_param(param_def, contract, transport, encode_ops, decode_ops)
            })
            .collect()
    }

    fn lower_function(
        &self,
        ty: DartFunctionType,
        abi_call: &AbiCall,
        abi_params_start: usize,
        param_defs: &[ParamDef],
        return_def: &ReturnDef,
        doc: Option<String>,
    ) -> DartFunction {
        let mode = match &abi_call.mode {
            CallMode::Sync => DartFunctionMode::Sync,
            CallMode::Async(async_call) => {
                DartFunctionMode::Async(Box::new(super::DartFFIAsyncFunctionDef {
                    poll_symbol: async_call.poll.to_string(),
                    complete_symbol: async_call.complete.to_string(),
                    complete_ty: DartFFIType::from_return_shape_and_error_transport(
                        &async_call.result,
                        &async_call.error,
                    ),
                    cancel_symbol: async_call.cancel.to_string(),
                    free_symbol: async_call.free.to_string(),
                }))
            }
        };

        let returns = match &abi_call.mode {
            CallMode::Sync => DartFunctionReturns {
                ty: DartReturnType::from_return_def(return_def, &self.ffi.catalog),
                passing: match return_def {
                    // returned strings are always wire encoded
                    ReturnDef::Value(TypeExpr::String) => {
                        DartFFIReturnsPassing::Passing(DartFFIValuePassing::WireEncoded)
                    }
                    _ => match &abi_call.returns.transport {
                        Some(transport) => DartFFIReturnsPassing::Passing(
                            self.value_passing_from_transport(transport),
                        ),
                        None => DartFFIReturnsPassing::Void,
                    },
                },
                read_seq: abi_call.returns.decode_ops.clone(),
                write_seq: abi_call.returns.encode_ops.clone(),
            },
            CallMode::Async(async_call) => DartFunctionReturns {
                ty: DartReturnType::from_return_def(return_def, &self.ffi.catalog),
                passing: match return_def {
                    // returned strings are always wire encoded
                    ReturnDef::Value(TypeExpr::String) => {
                        DartFFIReturnsPassing::Passing(DartFFIValuePassing::WireEncoded)
                    }
                    _ => match &async_call.result.transport {
                        Some(async_transport) => DartFFIReturnsPassing::Passing(
                            self.value_passing_from_transport(async_transport),
                        ),
                        None => DartFFIReturnsPassing::Void,
                    },
                },
                read_seq: async_call.result.decode_ops.clone(),
                write_seq: async_call.result.encode_ops.clone(),
            },
        };

        DartFunction {
            mode,
            ty,
            params: self.lower_function_params(param_defs, &abi_call.params[abi_params_start..]),
            ffi_def: DartFFIFunctionDef {
                symbol: abi_call.symbol.to_string(),
                sig: DartFFIFunctionSig {
                    args: abi_call
                        .params
                        .iter()
                        .map(DartFFIFunctionParamSig::from_abi_param)
                        .collect(),
                    ret: match &abi_call.mode {
                        CallMode::Sync => DartFFIType::from_return_shape_and_error_transport(
                            &abi_call.returns,
                            &abi_call.error,
                        ),
                        // RustFutureHandle
                        CallMode::Async(..) => DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                    },
                },
                is_leaf: false,
            },
            sig: DartFunctionSig::from_params_return_def(param_defs, return_def, &self.ffi.catalog),
            returns,
            doc,
        }
    }

    fn lower_constructor(&self, ctor: &ConstructorDef, id: CallId) -> DartFunction {
        let abi_call = self.abi_call_for_call_id(&id);

        let ret_type_expr = match &id {
            CallId::Constructor { class_id, .. } => TypeExpr::Handle(class_id.clone()),
            CallId::RecordConstructor { record_id, .. } => TypeExpr::Record(record_id.clone()),
            CallId::EnumConstructor { enum_id, .. } => TypeExpr::Enum(enum_id.clone()),
            _ => panic!("expected constructor id"),
        };

        let owner = match id {
            CallId::Constructor { class_id, .. } => DartFunctionCallOwner::Class(class_id),
            CallId::RecordConstructor { record_id, .. } => DartFunctionCallOwner::Record(record_id),
            CallId::EnumConstructor { enum_id, .. } => DartFunctionCallOwner::Enum(enum_id),
            _ => panic!("expected constructor id"),
        };

        let ctor_params = ctor.params().into_iter().cloned().collect::<Vec<_>>();
        self.lower_function(
            DartFunctionType::Constructor {
                owner,
                kind: match ctor {
                    ConstructorDef::Default { .. } => DartConstructorKind::Default,
                    ConstructorDef::NamedFactory { name, .. }
                    | ConstructorDef::NamedInit { name, .. } => DartConstructorKind::Named {
                        name: NamingConvention::function_name(name.as_str()),
                    },
                },
                is_fallible: ctor.is_fallible(),
                is_optional: ctor.is_optional(),
            },
            abi_call,
            0,
            &ctor_params,
            &ReturnDef::Value(ret_type_expr),
            ctor.doc().map(|doc| doc.to_string()),
        )
    }

    fn lower_method(&self, meth: &MethodDef, id: CallId) -> DartFunction {
        let abi_call = self.abi_call_for_call_id(&id);

        let receiver;
        let abi_param_start;

        match meth.receiver {
            crate::ir::Receiver::Static => {
                abi_param_start = 0;
                receiver = DartMethodReceiver::Static;
            }
            crate::ir::Receiver::RefSelf
            | crate::ir::Receiver::RefMutSelf
            | crate::ir::Receiver::OwnedSelf => {
                abi_param_start = 1;
                let abi_param = &abi_call.params[0];
                let ParamRole::Input { transport, .. } = &abi_param.role else {
                    unreachable!()
                };
                let passing = self.value_passing_from_transport(transport);

                receiver = DartMethodReceiver::ReceiverPassing(passing);
            }
        };

        let owner = match id {
            CallId::Method { class_id, .. } => DartFunctionCallOwner::Class(class_id),
            CallId::RecordMethod { record_id, .. } => DartFunctionCallOwner::Record(record_id),
            CallId::EnumMethod { enum_id, .. } => DartFunctionCallOwner::Enum(enum_id),
            _ => panic!("expected method id"),
        };

        self.lower_function(
            DartFunctionType::Method {
                name: NamingConvention::function_name(meth.id.as_str()),
                receiver,
                owner,
            },
            abi_call,
            abi_param_start,
            &meth.params,
            &meth.returns,
            meth.doc.clone(),
        )
    }

    fn lower_functions(&self) -> Vec<DartFunction> {
        self.ffi
            .functions
            .iter()
            .map(|func_def| {
                let abi_call = self.abi_call_for_function(&func_def.id);
                self.lower_function(
                    DartFunctionType::TopLevel {
                        name: NamingConvention::function_name(func_def.id.as_str()),
                    },
                    abi_call,
                    0,
                    &func_def.params,
                    &func_def.returns,
                    func_def.doc.clone(),
                )
            })
            .collect()
    }

    pub fn library(&self) -> DartLibrary {
        let custom_types = self.lower_custom_types();
        let records = self.lower_records();
        let enums = self.lower_enums();
        let callbacks = self.lower_callbacks();
        let classes = self.lower_classes();
        let functions = self.lower_functions();

        DartLibrary {
            custom_types,
            records,
            enums,
            callbacks,
            classes,
            functions,
        }
    }
}

#[cfg(test)]
mod tests {
    use boltffi_ffi_rules::callable::ExecutionKind;
    use rstest::rstest;

    use crate::{
        ir::{
            CallbackId, CallbackKind, CallbackMethodDef, CallbackTraitDef, FunctionDef, FunctionId,
            MethodId, ParamDef, ParamName, ParamPassing, PrimitiveType, ReturnDef, TypeExpr,
        },
        render::dart::{DartFFIFloatType, DartFFIIntType, test},
    };

    use super::*;

    #[rstest]
    #[case(
        TypeExpr::Primitive(PrimitiveType::U8),
        DartFFIType::Int(DartFFIIntType::Uint8),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::I8),
        DartFFIType::Int(DartFFIIntType::Int8),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::U16),
        DartFFIType::Int(DartFFIIntType::Uint16),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::I16),
        DartFFIType::Int(DartFFIIntType::Int16),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::U32),
        DartFFIType::Int(DartFFIIntType::Uint32),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::I32),
        DartFFIType::Int(DartFFIIntType::Int32),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::U64),
        DartFFIType::Int(DartFFIIntType::Uint64),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::I64),
        DartFFIType::Int(DartFFIIntType::Int64),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::F32),
        DartFFIType::Float(DartFFIFloatType::Float32),
        "double"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::F64),
        DartFFIType::Float(DartFFIFloatType::Float64),
        "double"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::ISize),
        DartFFIType::Int(DartFFIIntType::IntPtr),
        "int"
    )]
    #[case(
        TypeExpr::Primitive(PrimitiveType::USize),
        DartFFIType::Int(DartFFIIntType::UintPtr),
        "int"
    )]
    pub fn native_function_primitive_in(
        #[case] type_expr_primitive: TypeExpr,
        #[case] dart_ty_primitive: DartFFIType,
        #[case] dart_sub_ty: &str,
    ) {
        let mut ffi = test::empty_contract();
        ffi.functions.insert(
            0,
            FunctionDef {
                id: FunctionId::new("echo_u64"),
                params: vec![ParamDef {
                    name: ParamName::new("v"),
                    type_expr: type_expr_primitive.clone(),
                    passing: ParamPassing::Value,
                    doc: None,
                }],
                returns: ReturnDef::Value(type_expr_primitive),
                execution_kind: ExecutionKind::Sync,
                doc: None,
                deprecated: None,
            },
        );

        let library = test::lower(&ffi);

        assert_eq!(
            library.functions[0].ffi_def.sig.args[0].ty,
            dart_ty_primitive,
        );
        assert_eq!(library.functions[0].ffi_def.sig.ret, dart_ty_primitive);

        assert_eq!(
            &library.functions[0].ffi_def.sig.args[0].ty.sub_type(),
            dart_sub_ty
        );
        assert_eq!(
            &library.functions[0].ffi_def.sig.ret.sub_type(),
            dart_sub_ty
        );
    }

    #[test]
    pub fn native_function_closure_in() {
        let mut ffi = test::empty_contract();
        ffi.catalog.insert_callback(CallbackTraitDef {
            id: CallbackId::new("ClosureCb"),
            methods: vec![CallbackMethodDef {
                params: vec![],
                id: MethodId::new("call"),
                returns: ReturnDef::Void,
                execution_kind: ExecutionKind::Sync,
                doc: None,
            }],
            kind: CallbackKind::Closure,
            doc: None,
        });
        ffi.functions.insert(
            0,
            FunctionDef {
                id: FunctionId::new("function_with_callback"),
                params: vec![ParamDef {
                    name: ParamName::new("cb"),
                    type_expr: TypeExpr::Callback(CallbackId::new("ClosureCb")),
                    passing: ParamPassing::ImplTrait,
                    doc: None,
                }],
                returns: ReturnDef::Void,
                execution_kind: ExecutionKind::Sync,
                doc: None,
                deprecated: None,
            },
        );
        let library = test::lower(&ffi);

        assert!(
            library.functions[0].ffi_def.sig.args[0]
                .ty
                .native_type()
                .contains("$$ffi.Pointer<$$ffi.NativeFunction<")
        );
        assert!(!library.functions[0].ffi_def.is_leaf);
    }

    #[test]
    pub fn native_function_async() {
        let mut ffi = test::empty_contract();
        ffi.functions.push(FunctionDef {
            id: FunctionId::new("async_add"),
            params: vec![
                ParamDef {
                    name: ParamName::new("a"),
                    type_expr: TypeExpr::Primitive(PrimitiveType::I32),
                    passing: ParamPassing::Value,
                    doc: None,
                },
                ParamDef {
                    name: ParamName::new("b"),
                    type_expr: TypeExpr::Primitive(PrimitiveType::I32),
                    passing: ParamPassing::Value,
                    doc: None,
                },
            ],
            returns: ReturnDef::Value(TypeExpr::Primitive(PrimitiveType::I32)),
            execution_kind: ExecutionKind::Async,
            deprecated: None,
            doc: None,
        });

        let library = test::lower(&ffi);

        let func = &library.functions[0];
        match &func.mode {
            DartFunctionMode::Sync => panic!("CallMode should be async"),
            DartFunctionMode::Async(async_def) => {
                assert_eq!(&async_def.poll_symbol, "boltffi_async_add_poll");
                assert_eq!(&async_def.complete_symbol, "boltffi_async_add_complete");
                assert!(matches!(
                    &async_def.complete_ty,
                    DartFFIType::Int(DartFFIIntType::Int32)
                ));
                assert_eq!(&async_def.complete_symbol, "boltffi_async_add_complete");
                assert_eq!(&async_def.cancel_symbol, "boltffi_async_add_cancel");
                assert_eq!(&async_def.free_symbol, "boltffi_async_add_free");
            }
        };
    }
}
