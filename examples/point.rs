#![allow(non_camel_case_types)]

#[repr(C)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[no_mangle]
pub extern "C" fn dist(p: *const Point) -> f64 {
    let p = unsafe { &*p };
    (p.x * p.x + p.y * p.y).sqrt()
}

#[no_mangle]
pub extern "C" fn fill(p: *mut Point) {
    let p = unsafe { &mut *p };
    p.x = 3.0;
    p.y = 4.0;
}
