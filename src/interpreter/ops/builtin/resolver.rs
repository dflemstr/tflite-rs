use std::mem;
use std::ffi;

use crate::bindings;
use crate::interpreter::op_resolver::OpResolver;

cpp! {{
    #include "tensorflow/lite/kernels/register.h"

    using namespace tflite::ops::builtin;
}}

pub struct Resolver {
    handle: Box<bindings::tflite::OpResolver>,
}

impl Resolver {
    pub fn add_custom(&mut self, name: &str, registration: &'static bindings::TfLiteRegistration) {
        use std::ops::DerefMut;

        let handle = self.handle.deref_mut();
        let name = ffi::CString::new(name).unwrap();
        let name = name.as_ptr();
        unsafe {
            cpp!([handle as "BuiltinOpResolver*", name as "char*", registration as "TfLiteRegistration*"] {
                handle->AddCustom(name, registration);
            });
        }
    }
}

impl Drop for Resolver {
    #[allow(clippy::useless_transmute, clippy::forget_copy, deprecated)]
    fn drop(&mut self) {
        let handle = Box::into_raw(mem::take(&mut self.handle));
        unsafe {
            cpp!([handle as "BuiltinOpResolver*"] {
                delete handle;
            });
        }
    }
}

impl OpResolver for Resolver {
    fn get_resolver_handle(&self) -> &bindings::tflite::OpResolver {
        self.handle.as_ref()
    }
}

impl Default for Resolver {
    #[allow(clippy::forget_copy, deprecated)]
    fn default() -> Self {
        let handle = unsafe {
            cpp!([] -> *mut bindings::tflite::OpResolver as "OpResolver*" {
                return new BuiltinOpResolver();
            })
        };
        let handle = unsafe { Box::from_raw(handle) };
        Self { handle }
    }
}
