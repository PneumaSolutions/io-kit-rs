use std::ffi::CStr;

use std::ffi::c_void;
use std::mem;
use std::os::raw::c_char;

use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::runloop::CFRunLoopSource;
use core_foundation::string::CFString;
use io_kit_sys::types::{io_iterator_t, io_object_t, io_service_t};
use io_kit_sys::*;
use mach::kern_return::KERN_SUCCESS;

pub struct IOObject(io_object_t);

impl Drop for IOObject {
    fn drop(&mut self) {
        self.release().unwrap();
    }
}

impl TIOObject<io_object_t> for IOObject {
    #[inline]
    fn as_concrete_io_object_t(&self) -> io_object_t {
        self.0
    }

    #[inline]
    fn as_io_object_t(&self) -> io_object_t {
        self.as_concrete_io_object_t()
    }
}

pub struct IOIterator(io_iterator_t);

impl Drop for IOIterator {
    fn drop(&mut self) {
        self.release().unwrap();
    }
}

impl Iterator for IOIterator {
    type Item = IOObject;

    fn next(&mut self) -> Option<IOObject> {
        unsafe {
            let result = IOIteratorNext(self.as_io_object_t());

            if result != 0 {
                Some(IOObject(result))
            } else {
                None
            }
        }
    }
}

impl IOIterator {
    pub fn reset(&mut self) {
        unsafe { IOIteratorReset(self.as_io_object_t()) }
    }

    pub fn is_valid(&self) -> bool {
        unsafe { IOIteratorIsValid(self.as_io_object_t()) != 0 }
    }
}

impl TIOObject<io_iterator_t> for IOIterator {
    #[inline]
    fn as_concrete_io_object_t(&self) -> io_iterator_t {
        self.0
    }

    #[inline]
    fn as_io_object_t(&self) -> io_object_t {
        self.as_concrete_io_object_t()
    }
}

type IOServiceMatchingCallbackFn<'notif_life> = Box<dyn FnMut(Vec<IOService>) + 'notif_life>;

pub struct IOServiceMatchingNotification<'notif_life> {
    _iterator: IOIterator,
    _callback: IOServiceMatchingCallbackFn<'notif_life>,
}

fn make_services(iterator: &mut IOIterator) -> Vec<IOService> {
    let mut services = Vec::new();
    while let Some(obj) = iterator.next() {
        services.push(IOService(obj.as_io_object_t()));
        mem::forget(obj); // the reference is taken over by the service
    }
    services
}

unsafe extern "C" fn service_matching_callback_internal(
    refcon: *mut c_void,
    iterator: io_iterator_t,
) {
    let callback = refcon as *mut IOServiceMatchingCallbackFn;
    let mut iterator = IOIterator(iterator);
    let services = make_services(&mut iterator);
    mem::forget(iterator); // we're only borrowing the iterator
    (*callback)(services)
}

pub struct IOService(io_service_t);

impl Drop for IOService {
    fn drop(&mut self) {
        self.release().unwrap();
    }
}

impl IOService {
    pub fn get_matching_service(matching: CFDictionary) -> Option<IOService> {
        unsafe {
            let result =
                IOServiceGetMatchingService(kIOMasterPortDefault, matching.as_CFTypeRef() as _);

            if result != 0 {
                Some(IOService(result))
            } else {
                None
            }
        }
    }

    pub fn get_matching_services(matching: CFDictionary) -> Result<Vec<Self>, i32> {
        unsafe {
            let mut io_iterator_t: io_iterator_t = 0;

            let result = IOServiceGetMatchingServices(
                kIOMasterPortDefault,
                matching.as_CFTypeRef() as _,
                &mut io_iterator_t,
            );

            if result != KERN_SUCCESS {
                return Err(result);
            }

            let mut v: Vec<Self> = Vec::new();

            loop {
                let result = IOIteratorNext(io_iterator_t);

                if result == 0 {
                    break;
                }

                v.push(IOService(result))
            }

            Ok(v)
        }
    }

    pub fn add_matching_notification<'notif_life>(
        notify_port: &IONotificationPort,
        notification_type: *const c_char,
        matching: CFDictionary,
        callback: impl 'notif_life + FnMut(Vec<IOService>),
    ) -> Result<IOServiceMatchingNotification<'notif_life>, i32> {
        let mut callback = Box::new(Box::new(callback) as IOServiceMatchingCallbackFn);
        let cbr = callback.as_mut() as *mut IOServiceMatchingCallbackFn;
        let mut iterator: io_iterator_t = 0;
        let result = unsafe {
            IOServiceAddMatchingNotification(
                notify_port.0,
                notification_type,
                matching.as_concrete_TypeRef(),
                service_matching_callback_internal,
                cbr as *mut c_void,
                &mut iterator as *mut io_iterator_t,
            )
        };
        mem::forget(matching); // the function consumed the reference
        if result == KERN_SUCCESS {
            let mut iterator = IOIterator(iterator);
            let services = make_services(&mut iterator);
            (*callback)(services);
            Ok(IOServiceMatchingNotification {
                _iterator: iterator,
                _callback: callback,
            })
        } else {
            Err(result)
        }
    }
}

impl TIOObject<io_service_t> for IOService {
    #[inline]
    fn as_concrete_io_object_t(&self) -> io_service_t {
        self.0
    }

    #[inline]
    fn as_io_object_t(&self) -> io_object_t {
        self.as_concrete_io_object_t()
    }
}

pub trait TIOObject<concrete_io_object_t> {
    /// Returns the object as its concrete `io_object_t`.
    fn as_concrete_io_object_t(&self) -> concrete_io_object_t;

    /// Returns the object as a raw `io_object_t`.
    fn as_io_object_t(&self) -> io_object_t;

    fn release(&self) -> Result<(), i32> {
        unsafe {
            let result = IOObjectRelease(self.as_io_object_t());

            if result == KERN_SUCCESS {
                Ok(())
            } else {
                Err(result)
            }
        }
    }

    fn retain(&self) -> Result<(), i32> {
        unsafe {
            let result = IOObjectRetain(self.as_io_object_t());

            if result == KERN_SUCCESS {
                Ok(())
            } else {
                Err(result)
            }
        }
    }

    fn get_class(&self) -> Result<String, i32> {
        unsafe {
            let mut buf = Vec::<c_char>::with_capacity(128);

            let result = IOObjectGetClass(self.as_io_object_t(), buf.as_mut_ptr());

            if result == KERN_SUCCESS {
                Ok(String::from(
                    CStr::from_ptr(buf.as_ptr()).to_str().unwrap().to_string(),
                ))
            } else {
                Err(result)
            }
        }
    }

    fn copy_class(&self) -> Option<CFString> {
        unsafe {
            let result = IOObjectCopyClass(self.as_io_object_t());

            if result.is_null() {
                None
            } else {
                Some(TCFType::wrap_under_get_rule(result))
            }
        }
    }

    fn copy_superclass_for_class(&self, class_name: CFString) -> Option<CFString> {
        unsafe {
            let result = IOObjectCopySuperclassForClass(class_name.as_CFTypeRef() as _);

            if result.is_null() {
                None
            } else {
                Some(TCFType::wrap_under_get_rule(result))
            }
        }
    }

    fn copy_bundle_identifier_for_class(&self, class_name: CFString) -> Option<CFString> {
        unsafe {
            let result = IOObjectCopyBundleIdentifierForClass(class_name.as_CFTypeRef() as _);

            if result.is_null() {
                None
            } else {
                Some(TCFType::wrap_under_get_rule(result))
            }
        }
    }

    fn conforms_to(&self, class_name: *mut c_char) -> bool {
        unsafe { IOObjectConformsTo(self.as_io_object_t(), class_name) != 0 }
    }

    fn is_equal_to(&self, object: IOObject) -> bool {
        unsafe { IOObjectIsEqualTo(self.as_io_object_t(), object.as_io_object_t()) != 0 }
    }

    fn get_kernel_retain_count(&self) -> u32 {
        unsafe { IOObjectGetKernelRetainCount(self.as_io_object_t()) }
    }

    fn get_user_retain_count(&self) -> u32 {
        unsafe { IOObjectGetUserRetainCount(self.as_io_object_t()) }
    }

    fn get_retain_count(&self) -> u32 {
        unsafe { IOObjectGetRetainCount(self.as_io_object_t()) }
    }
}

pub fn io_service_matching(name: *const c_char) -> Option<CFDictionary> {
    unsafe {
        let result = IOServiceMatching(name);

        if result.is_null() {
            None
        } else {
            Some(TCFType::wrap_under_get_rule(result as *const _))
        }
    }
}

#[repr(transparent)]
pub struct IONotificationPort(IONotificationPortRef);

impl Drop for IONotificationPort {
    fn drop(&mut self) {
        unsafe { IONotificationPortDestroy(self.0) };
    }
}

impl IONotificationPort {
    pub fn new() -> Result<Self, ()> {
        let port = unsafe { IONotificationPortCreate(kIOMasterPortDefault) };
        if port.is_null() {
            Err(())
        } else {
            Ok(Self(port))
        }
    }

    pub fn get_run_loop_source(&self) -> CFRunLoopSource {
        let source = unsafe { IONotificationPortGetRunLoopSource(self.0) };
        assert!(!source.is_null());
        unsafe { TCFType::wrap_under_get_rule(source) }
    }
}
