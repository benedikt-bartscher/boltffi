use boltffi_ffi_rules::callable::ExecutionKind;

use crate::{
    ir::{
        AbiCallbackInvocation, AbiCallbackMethod, CallbackId, CallbackKind, CallbackMethodDef,
        CallbackTraitDef, ParamRole, Transport,
    },
    render::dart::{
        DartCallback, DartCallbackMethod, DartFFIFunctionParamSig, DartFFIFunctionSig,
        DartFFIIntType, DartFFIParamValue, DartFFIReturnsPassing, DartFFIType, DartFFIValuePassing,
        DartFunctionReturns, DartFunctionSig, DartReturnType, NamingConvention, emit,
    },
};

impl<'a> super::DartLowerer<'a> {
    fn abi_callback_for(&self, id: &CallbackId) -> Option<&AbiCallbackInvocation> {
        self.abi.callbacks.iter().find(|cb| cb.callback_id == *id)
    }

    fn lower_callback_method(
        &self,
        cb_meth: &CallbackMethodDef,
        abi_meth: &AbiCallbackMethod,
    ) -> DartCallbackMethod {
        // skip callback handle param def
        let cb_params = &abi_meth.params[1..];

        let input_cb_params = cb_params
            .iter()
            .filter(|p| matches!(p.role, ParamRole::Input { .. }))
            .cloned()
            .collect::<Vec<_>>();
        assert!(cb_meth.params.len() == input_cb_params.len());

        let mut ffi_args = vec![DartFFIFunctionParamSig {
            name: "_p$handle".to_string(),
            ty: DartFFIType::Int(DartFFIIntType::Uint64),
        }];

        ffi_args.extend(
            abi_meth.params[1..]
                .iter()
                .map(DartFFIFunctionParamSig::from_abi_param),
        );

        match abi_meth.execution_kind {
            ExecutionKind::Sync => {
                ffi_args.push(DartFFIFunctionParamSig {
                    name: "_p$outStatus".to_string(),
                    ty: DartFFIType::Pointer(Box::new(DartFFIType::Status)),
                });
            }
            ExecutionKind::Async => {
                let mut callback_params = vec![
                    // async context handle
                    DartFFIFunctionParamSig {
                        name: String::new(),
                        ty: DartFFIType::Int(DartFFIIntType::Uint64),
                    },
                ];

                if let Some(transport) = &abi_meth.returns.transport {
                    match transport {
                        Transport::Scalar(scalar) => {
                            callback_params.extend([
                                // direct primitive value
                                DartFFIFunctionParamSig {
                                    name: String::new(),
                                    ty: DartFFIType::from_primitive(scalar.primitive()),
                                },
                            ]);
                        }
                        Transport::Composite(..) | Transport::Span(..) => {
                            callback_params.extend([
                                // result bytes ptr
                                DartFFIFunctionParamSig {
                                    name: String::new(),
                                    ty: DartFFIType::Pointer(Box::new(DartFFIType::Int(
                                        DartFFIIntType::Uint8,
                                    ))),
                                },
                                // result bytes len
                                DartFFIFunctionParamSig {
                                    name: String::new(),
                                    ty: DartFFIType::Int(DartFFIIntType::UintPtr),
                                },
                            ]);
                        }
                        Transport::Handle { .. } => todo!(),
                        Transport::Callback { .. } => todo!(),
                    }
                }

                callback_params.push(
                    // This should be FFIStatus but we choose i32 as it's a valid repr
                    DartFFIFunctionParamSig {
                        name: String::new(),
                        ty: DartFFIType::Int(DartFFIIntType::Int32),
                    },
                );

                ffi_args.extend([
                    DartFFIFunctionParamSig {
                        name: "_p$callback".to_string(),
                        ty: DartFFIType::NativeFunction {
                            sig: Box::new(DartFFIFunctionSig {
                                args: callback_params,
                                ret: DartFFIType::Void,
                            }),
                        },
                    },
                    DartFFIFunctionParamSig {
                        name: "_p$asyncCtx".to_string(),
                        ty: DartFFIType::Int(DartFFIIntType::Uint64),
                    },
                ]);
            }
        };

        DartCallbackMethod {
            name: NamingConvention::function_name(cb_meth.id.as_str()),
            sig: DartFunctionSig::from_params_return_def(
                &cb_meth.params,
                &cb_meth.returns,
                &self.ffi.catalog,
            ),
            ffi_sig: DartFFIFunctionSig {
                args: ffi_args,
                ret: DartFFIType::from_return_shape_and_error_transport(
                    &abi_meth.returns,
                    &abi_meth.error,
                ),
            },
            params: self.lower_function_params(&cb_meth.params, &input_cb_params),
            kind: cb_meth.execution_kind,
            returns: DartFunctionReturns {
                ty: DartReturnType::from_return_def(&cb_meth.returns, &self.ffi.catalog),
                passing: match &abi_meth.returns.transport {
                    Some(transport) => match DartFFIReturnsPassing::Passing(
                        self.value_passing_from_transport(transport),
                    ) {
                        DartFFIReturnsPassing::Passing(
                            DartFFIValuePassing::Value(DartFFIParamValue::Record(..))
                            | DartFFIValuePassing::Bytes(..),
                        ) => DartFFIReturnsPassing::Passing(DartFFIValuePassing::WireEncoded),
                        passing => passing,
                    },
                    None => DartFFIReturnsPassing::Void,
                },
                read_seq: abi_meth.returns.decode_ops.clone(),
                write_seq: abi_meth
                    .returns
                    .encode_ops
                    .clone()
                    .map(emit::remap_write_seq),
            },
            doc: cb_meth.doc.clone(),
        }
    }

    fn lower_one_callback(&self, cb_def: &CallbackTraitDef) -> DartCallback {
        let abi_cb = self.abi_callback_for(&cb_def.id).unwrap();

        let class_name = NamingConvention::class_name(cb_def.id.as_str());
        let impl_class_name = format!("_I${}", class_name);
        let vtable_struct_name = format!(
            "_I${}",
            NamingConvention::class_name(abi_cb.vtable_type.as_str())
        );

        let methods = std::iter::zip(&cb_def.methods, &abi_cb.methods)
            .map(|(meth_def, abi_meth)| self.lower_callback_method(meth_def, abi_meth))
            .collect();

        DartCallback {
            class_name,
            impl_class_name,
            vtable_struct_name,
            methods,
            doc: cb_def.doc.clone(),
        }
    }

    pub(super) fn lower_callbacks(&self) -> Vec<DartCallback> {
        self.ffi
            .catalog
            .all_callbacks()
            .filter(|cb| matches!(cb.kind, CallbackKind::Trait))
            .map(|cb| self.lower_one_callback(cb))
            .collect()
    }
}
