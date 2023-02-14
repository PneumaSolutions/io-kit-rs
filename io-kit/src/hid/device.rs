use std::os::raw::c_char;

use core_foundation::{
    base::{kCFAllocatorDefault, CFRelease, CFType, CFTypeID, TCFType},
    dictionary::CFDictionary,
    runloop::CFRunLoop,
    string::{CFString, CFStringRef},
};

pub use io_kit_sys::hid::base::IOHIDDeviceRef;
pub use io_kit_sys::hid::device::*;
use io_kit_sys::types::IOOptionBits;
use io_kit_sys::CFSTR;

use crate::{
    base::{IOService, TIOObject},
    ret::{kIOReturnSuccess, IOReturn},
};

pub struct IOHIDDevice(IOHIDDeviceRef);

impl Drop for IOHIDDevice {
    fn drop(&mut self) {
        unsafe { CFRelease(self.as_CFTypeRef()) }
    }
}

pub struct IOHIDDeviceOpenGuard {
    device: IOHIDDevice,
    options: IOOptionBits,
}

impl Drop for IOHIDDeviceOpenGuard {
    fn drop(&mut self) {
        unsafe { IOHIDDeviceClose(self.device.0, self.options) };
    }
}

pub struct IOHIDDeviceScheduleGuard {
    device: IOHIDDevice,
    run_loop: CFRunLoop,
    mode: CFString,
}

impl Drop for IOHIDDeviceScheduleGuard {
    fn drop(&mut self) {
        unsafe {
            IOHIDDeviceUnscheduleFromRunLoop(
                self.device.0,
                self.run_loop.as_concrete_TypeRef(),
                self.mode.as_concrete_TypeRef(),
            )
        };
    }
}

impl IOHIDDevice {
    pub fn get_type_id() -> CFTypeID {
        unsafe { IOHIDDeviceGetTypeID() }
    }

    pub fn create(service: IOService) -> Option<IOHIDDevice> {
        unsafe {
            let result = IOHIDDeviceCreate(kCFAllocatorDefault, service.as_io_object_t());

            if result.is_null() {
                None
            } else {
                Some(IOHIDDevice(result))
            }
        }
    }

    pub fn open(&mut self, options: IOOptionBits) -> Result<IOHIDDeviceOpenGuard, IOReturn> {
        unsafe {
            let result = IOHIDDeviceOpen(self.0, options);

            if result == kIOReturnSuccess {
                Ok(IOHIDDeviceOpenGuard {
                    device: self.clone(),
                    options,
                })
            } else {
                Err(result)
            }
        }
    }

    pub fn schedule_with_run_loop(
        &mut self,
        run_loop: &CFRunLoop,
        mode: CFStringRef,
    ) -> IOHIDDeviceScheduleGuard {
        unsafe { IOHIDDeviceScheduleWithRunLoop(self.0, run_loop.as_concrete_TypeRef(), mode) };
        IOHIDDeviceScheduleGuard {
            device: self.clone(),
            run_loop: run_loop.clone(),
            mode: unsafe { TCFType::wrap_under_get_rule(mode) },
        }
    }

    pub fn conforms_to(&self, usage_page: u32, usage: u32) -> bool {
        unsafe { IOHIDDeviceConformsTo(self.0, usage_page, usage) != 0 }
    }

    pub fn get_property(&self, key: *const c_char) -> Option<CFType> {
        unsafe {
            let result = IOHIDDeviceGetProperty(self.0, CFSTR(key));

            if result.is_null() {
                None
            } else {
                Some(TCFType::wrap_under_get_rule(result))
            }
        }
    }

    pub fn set_input_value_matching(&self, matching: Option<&CFDictionary>) {
        unsafe {
            IOHIDDeviceSetInputValueMatching(
                self.as_concrete_TypeRef(),
                matching.map_or_else(std::ptr::null, TCFType::as_concrete_TypeRef),
            )
        }
    }
}

impl_TCFType!(IOHIDDevice, IOHIDDeviceRef, IOHIDDeviceGetTypeID);
