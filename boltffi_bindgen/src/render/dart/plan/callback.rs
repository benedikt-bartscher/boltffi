use boltffi_ffi_rules::callable::ExecutionKind;

#[derive(Debug, Clone)]
pub struct DartCallbackMethod {
    pub name: String,
    pub sig: super::DartFunctionSig,
    pub ffi_sig: super::DartFFIFunctionSig,
    pub params: Vec<super::DartFunctionParam>,
    pub kind: ExecutionKind,
    pub returns: super::DartFunctionReturns,
    pub doc: Option<String>,
}

impl DartCallbackMethod {
    pub fn is_async(&self) -> bool {
        matches!(self.kind, ExecutionKind::Async)
    }

    pub fn get_proxy_ffi_params(&self) -> Vec<String> {
        self.params.iter().flat_map(|p| p.get_ffi_param()).collect()
    }
}

#[derive(Debug, Clone)]
pub struct DartCallback {
    pub class_name: String,
    pub impl_class_name: String,
    pub vtable_struct_name: String,
    pub methods: Vec<DartCallbackMethod>,
    pub doc: Option<String>,
}
