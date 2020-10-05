use crate::bindings;
use crate::error;
use crate::external_context::ExternalContext;
use std::ffi;
use std::os::raw;

cpp! {{
    #include "edgetpu.h"
    #include <iostream>

    using namespace edgetpu;
}}

pub struct Context {
    handle: Option<Box<ffi::c_void>>,
}

pub fn custom_op_name() -> &'static ffi::CStr {
    unsafe {
        let raw = cpp!([] -> *const raw::c_char as "const char *" {
            return kCustomOp;
        });
        ffi::CStr::from_ptr(raw)
    }
}

pub fn register_custom_op() -> &'static bindings::TfLiteRegistration {
    unsafe {
        let raw = cpp!([] -> *const bindings::TfLiteRegistration as "TfLiteRegistration*" {
            return RegisterCustomOp();
        });
        raw.as_ref().unwrap()
    }
}

impl Context {
    pub fn new() -> Result<Self, error::Error> {
        let handle = unsafe {
            cpp!([] -> *mut ffi::c_void as "void*" {
                auto device = EdgeTpuManager::GetSingleton()->OpenDevice();
                return static_cast<void*>(new std::shared_ptr<EdgeTpuContext>(device));
            })
        };
        let handle = Some(unsafe { Box::from_raw(handle) });
        Ok(Context { handle })
    }

    pub fn enable_debug_printing(verbosity: raw::c_int) {
        unsafe {
            cpp!([verbosity as "int"] {
                EdgeTpuManager::GetSingleton()->SetVerbosity(verbosity);
            });
        }
    }
}

impl ExternalContext for Context {
    fn get_external_context_handle(&self) -> &bindings::TfLiteExternalContext {
        let handle = &**self.handle.as_ref().unwrap();
        unsafe {
            let raw = cpp!([handle as "void*"] -> *const bindings::TfLiteExternalContext as "TfLiteExternalContext*"{
                return static_cast<std::shared_ptr<EdgeTpuContext>*>(handle)->get();
            });
            raw.as_ref().unwrap()
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let handle = Box::into_raw(self.handle.take().unwrap());
        unsafe {
            cpp!([handle as "void*"] {
                delete static_cast<std::shared_ptr<EdgeTpuContext>*>(handle);
            });
        }
    }
}
