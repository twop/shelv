use accessibility_sys::*;
use core_foundation::{
    array::{
        CFArray, CFArrayCreate, CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex,
        CFArrayRef,
    },
    base::{CFCopyTypeIDDescription, CFGetTypeID, CFIndex, CFRange, CFRelease, CFTypeRef, FromVoid, TCFType, TCFTypeRef},
    number::{kCFNumberSInt64Type, CFNumberGetTypeID, CFNumberGetValue, CFNumberRef},
    string::{CFString, CFStringGetTypeID},
    url::{CFURLGetTypeID, CFURL},
};
use itertools::Itertools;
use std::{ops::Deref, ptr};

#[derive(Debug)]
pub enum AXAttributeValueRef {
    AXUIElementRef(AXUIElementRef),
    CFStringRef(String),
    CFURLRef(String),
    CFIndex(isize),
    CFRange(CFRange),
    CFArrayRef(Vec<AXAttributeValueRef>),
}

impl TryInto<String> for AXAttributeValueRef {
    type Error = A11YError;

    fn try_into(self) -> Result<String, Self::Error> {
        match self {
            AXAttributeValueRef::CFStringRef(s) => Ok(s),
            AXAttributeValueRef::CFURLRef(s) => Ok(s),
            _ => Err(A11YError::UnexpectedType("Can't convert to string".into())),
        }
    }
}

impl TryInto<AXUIElementRef> for AXAttributeValueRef {
    type Error = A11YError;

    fn try_into(self) -> Result<AXUIElementRef, Self::Error> {
        match self {
            AXAttributeValueRef::AXUIElementRef(e) => Ok(e),
            _ => Err(A11YError::UnexpectedType("Can't convert to element".into())),
        }
    }
}

impl TryInto<Vec<AXUIElementRef>> for AXAttributeValueRef {
    type Error = A11YError;

    fn try_into(self) -> Result<Vec<AXUIElementRef>, Self::Error> {
        let results: Result<Vec<AXUIElementRef>, A11YError> = match self {
            AXAttributeValueRef::CFArrayRef(arr) => arr
                .into_iter()
                .map(|e| match e {
                    AXAttributeValueRef::AXUIElementRef(e_ref) => Ok(e_ref),
                    _ => Err(A11YError::UnexpectedType("Unexpected child".into())),
                })
                .collect(),
            _ => Err(A11YError::UnexpectedType("Can't convert to array".into())),
        };
        results
    }
}

#[derive(Debug)]
pub enum A11YError {
    AttributeError(String, AXError),
    AXAPIError {
        attribute_name: String,
        error: AXError,
    },
    UnexpectedType(String),
}

// impl From<AXError> for A11YError {
//     fn from(value: AXError) -> Self {
//         A11YError::AXAPIError(value)
//     }
// }

pub struct Retained<T: TCFType>(T);

impl<T: TCFType> Drop for Retained<T> {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0.as_CFTypeRef());
        }
    }
}

// fn ok_axcall(result: AXError) -> Result<(), AXError> {

// }
//

fn map_axerror() {}

// pub fn get_attribute_names(element: AXUIElementRef) -> Result<Vec<String>, AXError> {
//     let init_arr = CFArray::from_CFTypes(
//         &["test"]
//             .repeat(100)
//             .into_iter()
//             .map(|e| CFString::from_static_string(e)),
//     );
//     let mut raw_value: CFTypeRef = init_arr.as_CFTypeRef();
//     unsafe {
//         let result = AXUIElementCopyAttributeNames(element, raw_value as *mut CFArrayRef);
//         if result != 0 {
//             return Err(result);
//         }
//     }
//     let mut cfindex: CFIndex = 0;

//     let raw_array: CFArrayRef = raw_value as CFArrayRef;
//     let children_count = unsafe { CFArrayGetCount(raw_array) };

//     let mut children_array: Vec<String> = vec![];
//     for i in 0..children_count {
//         let child_ref = unsafe { CFArrayGetValueAtIndex(raw_array, i as isize) };
//         let string_ref = unsafe { CFString::from_void(child_ref) }.to_string();

//         children_array.push(string_ref);
//     }
//     Ok(children_array)
// }

pub fn get_attribute(
    element: *mut __AXUIElement,
    attribute: &str,
) -> Result<Option<AXAttributeValueRef>, A11YError> {
    unsafe {
        let mut raw_value: CFTypeRef = ptr::null();
        let cf_string = CFString::new(attribute);
        let result =
            AXUIElementCopyAttributeValue(element, cf_string.as_concrete_TypeRef(), &mut raw_value);

        match result {
            kAXErrorSuccess => {}
            kAXErrorNoValue | kAXErrorAttributeUnsupported => {
                return Ok(None);
            }
            _ => {
                return Err(A11YError::AXAPIError {
                    attribute_name: attribute.to_owned(),
                    error: result,
                });
            }
        }

        if raw_value.is_null() {
            return Ok(None);
        }

        let type_id = CFGetTypeID(raw_value);

        let value = if type_id == CFArrayGetTypeID() {
            let mut cfindex: CFIndex = 0;

            AXUIElementGetAttributeValueCount(
                element,
                cf_string.as_concrete_TypeRef(),
                &mut cfindex,
            );

            let children_count = cfindex as usize;

            let children_raw_array = CFArrayRef::from_void_ptr(raw_value);
            let mut children_array: Vec<AXAttributeValueRef> = vec![];
            for i in 0..children_count {
                let child_ref = unsafe { CFArrayGetValueAtIndex(children_raw_array, i as isize) };
                let value_ref = get_attribute_from_cftype(child_ref)?;
                if let Some(value) = value_ref {
                    children_array.push(value)
                }
            }
            Some(AXAttributeValueRef::CFArrayRef(children_array))
            // let array = CFArrayRef::
            // let count = array.len();
            // let mut values = Vec::new();
            // for i in 0..count {
            //     let item = array.get(i as isize).unwrap();
            //     if let Some(value) = get_attribute_from_cftype(item.deref()) {
            //         values.push(value);
            //     }
            // }
            // AXAttributeValueRef::CFArrayRef(values)
        } else {
            get_attribute_from_cftype(raw_value)?
        };

        // let value = if type_id == AXUIElementGetTypeID() {
        //     AXAttributeValueRef::AXUIElementRef(raw_value as *mut __AXUIElement)
        // } else if type_id == CFStringGetTypeID() {
        //     let string = CFString::from_void(raw_value).to_string();
        //     AXAttributeValueRef::CFStringRef(string)
        // } else if type_id == CFURLGetTypeID() {
        //     let url = CFURL::from_void(raw_value).get_string().to_string();
        //     AXAttributeValueRef::CFURLRef(url)
        // } else if type_id == CFNumberGetTypeID() {
        //     let mut value: CFIndex = 0;
        //     CFNumberGetValue(
        //         raw_value as CFNumberRef,
        //         kCFNumberSInt64Type,
        //         &mut value as *mut _ as *mut _,
        //     );
        //     AXAttributeValueRef::CFIndex(value)
        // } else if type_id == AXValueGetTypeID() {
        //     let mut range = CFRange {
        //         location: 0,
        //         length: 0,
        //     };
        //     AXValueGetValue(
        //         raw_value as AXValueRef,
        //         kAXValueTypeCFRange,
        //         &mut range as *mut _ as *mut _,
        //     );
        //     AXAttributeValueRef::CFRange(range)
        // } else if type_id == CFArrayGetTypeID() {
        //     let mut cfindex: CFIndex = 0;

        //     AXUIElementGetAttributeValueCount(
        //         element,
        //         cf_string.as_concrete_TypeRef(),
        //         &mut cfindex,
        //     );

        //     let children_count = cfindex as usize;

        //     let children_raw_array = CFArrayRef::from_void_ptr(raw_value);
        //     let mut children_array: Vec<AXAttributeValueRef> = vec![];
        //     for i in 0..children_count {
        //         println!("Getting child {}", i);
        //         let child_ref = unsafe { CFArrayGetValueAtIndex(children_raw_array, i as isize) };
        //         let value_ref = get_attribute_from_cftype(child_ref);
        //         if let Some(value) = value_ref {
        //             children_array.push(value)
        //         }
        //     }
        //     AXAttributeValueRef::CFArrayRef(children_array)
        //     // let array = CFArrayRef::
        //     // let count = array.len();
        //     // let mut values = Vec::new();
        //     // for i in 0..count {
        //     //     let item = array.get(i as isize).unwrap();
        //     //     if let Some(value) = get_attribute_from_cftype(item.deref()) {
        //     //         values.push(value);
        //     //     }
        //     // }
        //     // AXAttributeValueRef::CFArrayRef(values)
        // } else {
        //     CFRelease(raw_value);
        //     return None;
        // };

        // CFRelease(raw_value);

        Ok(value)
    }
}

// fn get_attribute_values(raw_value: CFTypeRef) -> Vec<AXAttributeValueRef> {
//     let children_raw_array = CFArrayRef::from_void_ptr(raw_value);
//     let mut children_array: Vec<AXUIElementRef> = vec![];
//     for i in 0..children_count {
//         println!("Getting child {}", i);
//         let child_ref = unsafe { CFArrayGetValueAtIndex(children_raw_array, i as isize) };

//         println!(
//             "Found {:?} child",
//             CFString::from_void(CFCopyTypeIDDescription(CFGetTypeID(child_ref)).as_void_ptr())
//         );

//         if result != 0 || raw_value.is_null() || CFGetTypeID(child_ref) != AXUIElementGetTypeID() {
//             println!("Error getting range attribute");
//             return None;
//         }

//         children_array.push(child_ref as AXUIElementRef);
//     }
// }

fn get_attribute_from_cftype(cf_type: CFTypeRef) -> Result<Option<AXAttributeValueRef>, A11YError> {
    unsafe {
        let type_id = CFGetTypeID(cf_type);

        let result = if type_id == AXUIElementGetTypeID() {
            Some(AXAttributeValueRef::AXUIElementRef(
                cf_type as *mut __AXUIElement,
            ))
        } else if type_id == CFStringGetTypeID() {
            let string = CFString::from_void(cf_type).to_string();
            Some(AXAttributeValueRef::CFStringRef(string))
        } else if type_id == CFURLGetTypeID() {
            let url = CFURL::from_void(cf_type).get_string().to_string();
            Some(AXAttributeValueRef::CFURLRef(url))
        } else if type_id == CFNumberGetTypeID() {
            let mut value: CFIndex = 0;
            CFNumberGetValue(
                cf_type as CFNumberRef,
                kCFNumberSInt64Type,
                &mut value as *mut _ as *mut _,
            );
            Some(AXAttributeValueRef::CFIndex(value))
        } else if type_id == AXValueGetTypeID() {
            let value_type = AXValueGetType(cf_type as AXValueRef);
            match value_type {
                kAXValueTypeCFRange => {
                    let mut range = CFRange {
                        location: 0,
                        length: 0,
                    };
                    AXValueGetValue(
                        cf_type as AXValueRef,
                        kAXValueTypeCFRange,
                        &mut range as *mut _ as *mut _,
                    );
                    Some(AXAttributeValueRef::CFRange(range))
                }
                _ => {
                    return Err(A11YError::UnexpectedType(format!(
                        "Was not expecting value {}",
                        value_type
                    )))
                }
            }
        } else {
            
            let raw_desc = CFCopyTypeIDDescription(type_id).as_void_ptr();
            return Err(A11YError::UnexpectedType(format!("{}", CFString::from_void(raw_desc).to_string())));
        };

        return Ok(result);
    }
}

#[derive(Debug)]
pub struct CoppiedTextContext {
    pub selected_text: Option<String>,
    pub url: Option<String>,
    pub document: Option<String>,
    pub window_title: Option<String>,
    pub application_title: Option<String>,
}

pub fn get_focused_element_text() -> Result<Option<CoppiedTextContext>, A11YError> {
    unsafe {
        let system_wide_element = AXUIElementCreateSystemWide();
        let Some(focused_element) =
            get_attribute(system_wide_element, kAXFocusedUIElementAttribute)?
        else {
            return Ok(None);
        };

        let AXAttributeValueRef::AXUIElementRef(focused_element) = focused_element else {
            return Err(A11YError::UnexpectedType("Focussed element wrong".into()));
        };

        let selected_text =
            get_attribute(focused_element, kAXSelectedTextAttribute)?.and_then(|v| {
                if let AXAttributeValueRef::CFStringRef(s) = v {
                    Some(s)
                } else {
                    None
                }
            });

        // let Some(AXAttributeValueRef::AXUIElementRef(parent_element)) =
        //     get_attribute(focused_element, kAXParentAttribute)?
        // else {
        //     return Err(A11YError::UnexpectedType("Parent unexpected".into()));
        // };

        // let Some(AXAttributeValueRef::AXUIElementRef(window_element)) =
        //     get_attribute(focused_element, kAXWindowAttribute)?
        // else {
        //     return Err(A11YError::UnexpectedType("Window unexpected".into()));
        // };

        // let Some(AXAttributeValueRef::AXUIElementRef(application_element)) =
        //     get_attribute(window_element, kAXParentAttribute)?
        // else {
        //     return Err(A11YError::UnexpectedType("Application unexpected".into()));
        // };

        // let window_title: Option<String> =
        //     get_attribute(window_element, kAXTitleAttribute)?.map(|w| w.try_into().unwrap());
        // let application_title: Option<String> =
        //     get_attribute(application_element, kAXTitleAttribute)?.map(|w| w.try_into().unwrap());
        // let url: Option<String> =
        //     get_attribute(focused_element, kAXURLAttribute)?.map(|w| w.try_into().unwrap());
        // let document: Option<String> =
        //     get_attribute(focused_element, kAXDocumentAttribute)?.map(|w| w.try_into().unwrap());

        // let role: Option<String> =
        //     get_attribute(focused_element, kAXRoleAttribute)?.map(|w| w.try_into().unwrap());
        // // let something = get_attribute(focused_element, "AXSelectedTextMarkerRange")?;
        // // println!("DID SOMETHING WORK?? {:?}", something);
        // let selected_range = get_attribute(focused_element, kAXSelectedTextRangeAttribute)?;
        // let selected_ranges = get_attribute(focused_element, kAXSelectedTextRangesAttribute)?;
        // let selected_children = get_attribute(focused_element, kAXSelectedChildrenAttribute)?;
        // let marker = get_attribute(focused_element, kAXMarkerUIElementsAttribute)?;
        // println!("{:?} selected_range: {:?}", role, selected_range);
        // println!("{:?} selected_ranges: {:?}", role, selected_ranges);
        // println!("{:?} selected_children: {:?}", role, selected_children);
        // println!("{:?} marker: {:?}", role, marker);

        // //let attr_names = get_attribute_names(focused_element);
        // // println!("attr {:?}", attr_names);

        // let role: Option<String> =
        //     get_attribute(parent_element, kAXRoleAttribute)?.map(|w| w.try_into().unwrap());
        // let selected_range = get_attribute(parent_element, kAXSelectedTextRangeAttribute)?;
        // let selected_ranges = get_attribute(parent_element, kAXSelectedTextRangesAttribute)?;
        // println!(
        //     "parent_element {:?} selected_range: {:?}",
        //     role, selected_range
        // );
        // println!(
        //     "parent_element {:?} selected_ranges: {:?}",
        //     role, selected_ranges
        // );

        // let role: Option<String> =
        //     get_attribute(window_element, kAXRoleAttribute)?.map(|w| w.try_into().unwrap());
        // let selected_range = get_attribute(window_element, kAXSelectedTextRangeAttribute)?;
        // let selected_ranges = get_attribute(window_element, kAXSelectedTextRangesAttribute)?;
        // println!(
        //     "window_element {:?} selected_range: {:?}",
        //     role, selected_range
        // );
        // println!(
        //     "window_element {:?} selected_ranges: {:?}",
        //     role, selected_ranges
        // );

        // let role: Option<String> =
        //     get_attribute(application_element, kAXRoleAttribute)?.map(|w| w.try_into().unwrap());
        // let selected_range = get_attribute(application_element, kAXSelectedTextRangeAttribute)?;
        // let selected_ranges = get_attribute(application_element, kAXSelectedTextRangesAttribute)?;
        // println!(
        //     "application_element {:?} selected_range: {:?}",
        //     role, selected_range
        // );
        // println!(
        //     "application_element {:?} selected_ranges: {:?}",
        //     role, selected_ranges
        // );

        // let children: Option<Vec<AXUIElementRef>> =
        //     get_attribute(focused_element, kAXChildrenAttribute)?.map(|w| w.try_into().unwrap());

        // if let Some(children) = children {
        //     for (child_index, child) in children.into_iter().enumerate() {
        //         let child_children: Option<Vec<AXUIElementRef>> =
        //             get_attribute(focused_element, kAXChildrenAttribute)?
        //                 .map(|w| w.try_into().unwrap());

        //         let role: Option<String> =
        //             get_attribute(child, kAXRoleAttribute)?.map(|w| w.try_into().unwrap());
        //         let selected_range = get_attribute(child, kAXSelectedTextRangeAttribute)?;
        //         let selected_ranges = get_attribute(child, kAXSelectedTextRangesAttribute)?;
        //         let child_count = child_children.map(|arr| arr.len()).unwrap_or(0);
        //         println!(
        //             "child {} {:?} selected_range: {:?} {:?} (children = {})",
        //             child_index, role, selected_range, selected_ranges, child_count
        //         );
        //     }
        // }

        Ok(Some(CoppiedTextContext {
            selected_text,
            url: None,
            document: None,
            window_title: None,
            application_title: None,
        }))
    }
}

// fn scan_hierarchy_for_attribute(element: *mut __AXUIElement, attribute: &str) -> Option<String> {
//     let mut current_element = element;

//     loop {
//         if let Some(AXAttributeValueRef::CFStringRef(value)) =
//             get_attribute(current_element, attribute)
//         {
//             return Some(value);
//         }

//         if let Some(AXAttributeValueRef::AXUIElementRef(parent)) =
//             get_attribute(current_element, kAXParentAttribute)
//         {
//             current_element = parent;
//         } else {
//             return None;
//         }
//     }
// }
