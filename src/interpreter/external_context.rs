use crate::bindings::TfLiteExternalContext as SysExternalContext;

pub trait ExternalContext: Send + Sync {
    fn get_external_context_handle(&self) -> &SysExternalContext;
}
