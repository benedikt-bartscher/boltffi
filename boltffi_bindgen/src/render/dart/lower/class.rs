use boltffi_ffi_rules::naming;

use crate::{
    ir::{AbiStream, CallId, ClassDef, ClassId, StreamDef, StreamId, StreamItemTransport},
    render::dart::{
        DartClass, DartFFIFunctionDef, DartFFIFunctionParamSig, DartFFIFunctionSig, DartFFIIntType,
        DartFFIType, DartStream, DartType, NamingConvention,
    },
};

impl<'a> super::DartLowerer<'a> {
    fn abi_stream_for(&self, id: &StreamId, class_id: &ClassId) -> Option<&AbiStream> {
        self.abi
            .streams
            .iter()
            .find(|s| &s.stream_id == id && &s.class_id == class_id)
    }

    fn lower_one_stream(&self, stream: &StreamDef, class_id: &ClassId) -> DartStream {
        let abi_stream = self.abi_stream_for(&stream.id, class_id).unwrap();

        let StreamItemTransport::WireEncoded { decode_ops } = &abi_stream.item;

        let mut pop_batch_params = vec![DartFFIFunctionParamSig {
            name: "handle".to_string(),
            ty: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
        }];
        match abi_stream.item_size {
            Some(_) => {
                pop_batch_params.extend([
                    DartFFIFunctionParamSig {
                        name: "output_ptr".to_string(),
                        ty: DartFFIType::Pointer(Box::new(DartFFIType::from_transport(
                            &abi_stream.item_transport,
                        ))),
                    },
                    DartFFIFunctionParamSig {
                        name: "output_capacity".to_string(),
                        ty: DartFFIType::Int(DartFFIIntType::UintPtr),
                    },
                ]);
            }
            None => pop_batch_params.extend([DartFFIFunctionParamSig {
                name: "max_count".to_string(),
                ty: DartFFIType::Int(DartFFIIntType::UintPtr),
            }]),
        }
        let pop_batch_return_type = match abi_stream.item_size {
            Some(_) => DartFFIType::Int(DartFFIIntType::UintPtr),
            None => DartFFIType::Buf,
        };
        let pop_batch_fn = DartFFIFunctionDef {
            symbol: abi_stream.pop_batch.to_string(),
            sig: DartFFIFunctionSig {
                args: pop_batch_params,
                ret: pop_batch_return_type,
            },
            is_leaf: true,
        };

        DartStream {
            name: NamingConvention::function_name(stream.id.as_str()),
            item_ty: DartType::from_type_expr(&stream.item_type, &self.ffi.catalog),
            item_read_seq: decode_ops.clone(),
            item_passing: self.value_passing_from_transport(&abi_stream.item_transport),
            ffi_item_ty: DartFFIType::from_transport(&abi_stream.item_transport),
            ffi_item_size: abi_stream.item_size,
            subscribe_fn: DartFFIFunctionDef {
                symbol: abi_stream.subscribe.to_string(),
                sig: DartFFIFunctionSig {
                    args: vec![DartFFIFunctionParamSig {
                        name: "handle".to_string(),
                        ty: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                    }],
                    ret: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                },
                is_leaf: true,
            },
            poll_fn: DartFFIFunctionDef {
                symbol: abi_stream.poll.to_string(),
                sig: DartFFIFunctionSig {
                    args: vec![
                        DartFFIFunctionParamSig {
                            name: "handle".to_string(),
                            ty: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                        },
                        DartFFIFunctionParamSig {
                            name: "callback_data".to_string(),
                            ty: DartFFIType::Int(DartFFIIntType::Uint64),
                        },
                        DartFFIFunctionParamSig {
                            name: "callback".to_string(),
                            ty: DartFFIType::NativeFunction {
                                sig: Box::new(DartFFIFunctionSig {
                                    args: vec![
                                        DartFFIFunctionParamSig {
                                            name: String::new(),
                                            ty: DartFFIType::Int(DartFFIIntType::Uint64),
                                        },
                                        DartFFIFunctionParamSig {
                                            name: String::new(),
                                            ty: DartFFIType::Int(DartFFIIntType::Int8),
                                        },
                                    ],
                                    ret: DartFFIType::Void,
                                }),
                            },
                        },
                    ],
                    ret: DartFFIType::Void,
                },
                is_leaf: false,
            },
            pop_batch_fn,
            wait_fn: DartFFIFunctionDef {
                symbol: abi_stream.wait.to_string(),
                sig: DartFFIFunctionSig {
                    args: vec![
                        DartFFIFunctionParamSig {
                            name: "handle".to_string(),
                            ty: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                        },
                        DartFFIFunctionParamSig {
                            name: "timeout_milliseconds".to_string(),
                            ty: DartFFIType::Int(DartFFIIntType::Uint32),
                        },
                    ],
                    ret: DartFFIType::Int(DartFFIIntType::Int32),
                },
                is_leaf: true,
            },
            unsubscribe_fn: DartFFIFunctionDef {
                symbol: abi_stream.unsubscribe.to_string(),
                sig: DartFFIFunctionSig {
                    args: vec![DartFFIFunctionParamSig {
                        name: "handle".to_string(),
                        ty: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                    }],
                    ret: DartFFIType::Void,
                },
                is_leaf: false,
            },
            free_fn: DartFFIFunctionDef {
                symbol: abi_stream.free.to_string(),
                sig: DartFFIFunctionSig {
                    args: vec![DartFFIFunctionParamSig {
                        name: "handle".to_string(),
                        ty: DartFFIType::Pointer(Box::new(DartFFIType::Void)),
                    }],
                    ret: DartFFIType::Void,
                },
                is_leaf: false,
            },
            mode: stream.mode,
            doc: stream.doc.clone(),
        }
    }

    fn lower_one_class(&self, class: &ClassDef) -> DartClass {
        let constructors = class
            .constructors
            .iter()
            .enumerate()
            .map(|(i, ctor)| {
                self.lower_constructor(
                    ctor,
                    CallId::Constructor {
                        class_id: class.id.clone(),
                        index: i,
                    },
                )
            })
            .collect();

        let methods = class
            .methods
            .iter()
            .map(|meth| {
                self.lower_method(
                    meth,
                    CallId::Method {
                        class_id: class.id.clone(),
                        method_id: meth.id.clone(),
                    },
                )
            })
            .collect();

        let streams = class
            .streams
            .iter()
            .map(|s| self.lower_one_stream(s, &class.id))
            .collect();

        DartClass {
            name: NamingConvention::class_name(class.id.as_str()),
            create_symbol: naming::class_ffi_new(class.id.as_str()).to_string(),
            free_symbol: naming::class_ffi_free(class.id.as_str()).to_string(),
            constructors,
            methods,
            streams,
            doc: class.doc.clone(),
        }
    }

    pub(super) fn lower_classes(&self) -> Vec<DartClass> {
        self.ffi
            .catalog
            .all_classes()
            .map(|c| self.lower_one_class(c))
            .collect()
    }
}
