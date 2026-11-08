use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::{Mutex, OnceLock};

use crate::value::Value;

static LIBS: OnceLock<Mutex<HashMap<u64, libloading::Library>>> = OnceLock::new();

fn libs() -> &'static Mutex<HashMap<u64, libloading::Library>> {
    LIBS.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT_ID: OnceLock<Mutex<u64>> = OnceLock::new();

fn next_id() -> u64 {
    let m = NEXT_ID.get_or_init(|| Mutex::new(1));
    let mut n = m.lock().unwrap();
    let id = *n;
    *n += 1;
    id
}

pub fn ffi_load_native(args: &[Value]) -> Result<Value, String> {
    let path = match args.first() {
        Some(Value::Str(p)) => p.clone(),
        _ => return Err("ffi_load expects a library path string".to_string()),
    };
    let lib = unsafe { libloading::Library::new(&path) }.map_err(|e| format!("cannot load library: {e}"))?;
    let id = next_id();
    libs().lock().unwrap().insert(id, lib);
    Ok(Value::Number(id as f64))
}

fn as_i64(v: &Value) -> Result<i64, String> {
    match v {
        Value::Number(n) => Ok(*n as i64),
        Value::Str(s) => s.trim().parse::<i64>().map_err(|_| format!("not an integer: {s}")),
        _ => Err("expected a number or integer string".to_string()),
    }
}

fn as_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Str(s) => s.trim().parse::<f64>().map_err(|_| format!("not a number: {s}")),
        _ => Err("expected a number".to_string()),
    }
}

fn as_cstr(v: &Value) -> Result<CString, String> {
    match v {
        Value::Str(s) => CString::new(s.as_str()).map_err(|_| "string contains NUL".to_string()),
        _ => Err("expected a string".to_string()),
    }
}

fn ret_i64(v: i64) -> Value {
    Value::Number(v as f64)
}

fn ret_f64(v: f64) -> Value {
    Value::Number(v)
}

fn ret_str(v: *const c_char) -> Result<Value, String> {
    if v.is_null() {
        return Ok(Value::Null);
    }
    let c = unsafe { std::ffi::CStr::from_ptr(v) };
    Ok(Value::Str(c.to_string_lossy().into_owned()))
}

fn symbol<'a, T>(lib: &'a libloading::Library, name: &[u8]) -> Result<libloading::Symbol<'a, T>, String> {
    unsafe { lib.get(name) }.map_err(|e| e.to_string())
}

unsafe fn call_0_v(lib: &libloading::Library, name: &[u8]) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn()>(lib, name)?;
    let f = *s;
    unsafe { f() };
    Ok(Value::Null)
}

unsafe fn call_0_i(lib: &libloading::Library, name: &[u8]) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn() -> i64>(lib, name)?;
    let f = *s;
    Ok(ret_i64(unsafe { f() }))
}

unsafe fn call_0_f(lib: &libloading::Library, name: &[u8]) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn() -> f64>(lib, name)?;
    let f = *s;
    Ok(ret_f64(unsafe { f() }))
}

unsafe fn call_1_i_v(lib: &libloading::Library, name: &[u8], a: i64) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(i64)>(lib, name)?;
    let f = *s;
    unsafe { f(a) };
    Ok(Value::Null)
}

unsafe fn call_1_i_i(lib: &libloading::Library, name: &[u8], a: i64) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(i64) -> i64>(lib, name)?;
    let f = *s;
    Ok(ret_i64(unsafe { f(a) }))
}

unsafe fn call_1_i_f(lib: &libloading::Library, name: &[u8], a: i64) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(i64) -> f64>(lib, name)?;
    let f = *s;
    Ok(ret_f64(unsafe { f(a) }))
}

unsafe fn call_1_f_v(lib: &libloading::Library, name: &[u8], a: f64) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(f64)>(lib, name)?;
    let f = *s;
    unsafe { f(a) };
    Ok(Value::Null)
}

unsafe fn call_1_f_i(lib: &libloading::Library, name: &[u8], a: f64) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(f64) -> i64>(lib, name)?;
    let f = *s;
    Ok(ret_i64(unsafe { f(a) }))
}

unsafe fn call_1_f_f(lib: &libloading::Library, name: &[u8], a: f64) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(f64) -> f64>(lib, name)?;
    let f = *s;
    Ok(ret_f64(unsafe { f(a) }))
}

unsafe fn call_1_s_i(lib: &libloading::Library, name: &[u8], a: CString) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(*const c_char) -> i64>(lib, name)?;
    let f = *s;
    Ok(ret_i64(unsafe { f(a.as_ptr()) }))
}

unsafe fn call_1_s_v(lib: &libloading::Library, name: &[u8], a: CString) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(*const c_char)>(lib, name)?;
    let f = *s;
    unsafe { f(a.as_ptr()) };
    Ok(Value::Null)
}

unsafe fn call_1_s_s(lib: &libloading::Library, name: &[u8], a: CString) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(*const c_char) -> *const c_char>(lib, name)?;
    let f = *s;
    ret_str(unsafe { f(a.as_ptr()) })
}

unsafe fn call_2_ff_f(lib: &libloading::Library, name: &[u8], a: f64, b: f64) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(f64, f64) -> f64>(lib, name)?;
    let f = *s;
    Ok(ret_f64(unsafe { f(a, b) }))
}

unsafe fn call_2_ii_i(lib: &libloading::Library, name: &[u8], a: i64, b: i64) -> Result<Value, String> {
    let s = symbol::<unsafe extern "C" fn(i64, i64) -> i64>(lib, name)?;
    let f = *s;
    Ok(ret_i64(unsafe { f(a, b) }))
}

pub fn ffi_alloc_native(args: &[Value]) -> Result<Value, String> {
    let len = as_i64(args.first().ok_or("ffi_alloc expects a size")?)? as usize;
    let mut buf = vec![0u8; len];
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    allocs().lock().unwrap().insert(ptr as usize, len);
    Ok(Value::Number(ptr as i64 as f64))
}

static ALLOCS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

fn allocs() -> &'static Mutex<HashMap<usize, usize>> {
    ALLOCS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn ffi_free_native(args: &[Value]) -> Result<Value, String> {
    let p = as_i64(args.first().ok_or("ffi_free expects a pointer")?)? as usize;
    if let Some(len) = allocs().lock().unwrap().remove(&p) {
        unsafe {
            drop(Vec::from_raw_parts(p as *mut u8, len, len));
        }
    }
    Ok(Value::Null)
}

fn arg_ptr(args: &[Value], i: usize) -> Result<*mut u8, String> {
    Ok(as_i64(args.get(i).ok_or("missing argument")?)? as usize as *mut u8)
}

pub fn ffi_write_i64_native(args: &[Value]) -> Result<Value, String> {
    let p = arg_ptr(args, 0)?;
    let off = as_i64(args.get(1).ok_or("missing offset")?)? as usize;
    let v = as_i64(args.get(2).ok_or("missing value")?)?;
    unsafe { *(p.add(off) as *mut i64) = v; }
    Ok(Value::Null)
}

pub fn ffi_write_f64_native(args: &[Value]) -> Result<Value, String> {
    let p = arg_ptr(args, 0)?;
    let off = as_i64(args.get(1).ok_or("missing offset")?)? as usize;
    let v = as_f64(args.get(2).ok_or("missing value")?)?;
    unsafe { *(p.add(off) as *mut f64) = v; }
    Ok(Value::Null)
}

pub fn ffi_write_u8_native(args: &[Value]) -> Result<Value, String> {
    let p = arg_ptr(args, 0)?;
    let off = as_i64(args.get(1).ok_or("missing offset")?)? as usize;
    let v = as_i64(args.get(2).ok_or("missing value")?)? as u8;
    unsafe { *(p.add(off) as *mut u8) = v; }
    Ok(Value::Null)
}

pub fn ffi_write_str_native(args: &[Value]) -> Result<Value, String> {
    let p = arg_ptr(args, 0)?;
    let off = as_i64(args.get(1).ok_or("missing offset")?)? as usize;
    let s = as_cstr(args.get(2).ok_or("missing value")?)?;
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), p.add(off) as *mut i8, s.as_bytes().len() + 1);
    }
    Ok(Value::Null)
}

pub fn ffi_read_i64_native(args: &[Value]) -> Result<Value, String> {
    let p = arg_ptr(args, 0)?;
    let off = as_i64(args.get(1).ok_or("missing offset")?)? as usize;
    Ok(ret_i64(unsafe { *(p.add(off) as *mut i64) }))
}

pub fn ffi_read_f64_native(args: &[Value]) -> Result<Value, String> {
    let p = arg_ptr(args, 0)?;
    let off = as_i64(args.get(1).ok_or("missing offset")?)? as usize;
    Ok(ret_f64(unsafe { *(p.add(off) as *mut f64) }))
}

pub fn ffi_read_u8_native(args: &[Value]) -> Result<Value, String> {
    let p = arg_ptr(args, 0)?;
    let off = as_i64(args.get(1).ok_or("missing offset")?)? as usize;
    Ok(Value::Number(unsafe { *(p.add(off) as *mut u8) } as f64))
}

pub fn ffi_read_str_native(args: &[Value]) -> Result<Value, String> {
    let p = arg_ptr(args, 0)?;
    ret_str(p as *const c_char)
}

pub fn ffi_call_native(args: &[Value]) -> Result<Value, String> {
    if args.len() != 5 {
        return Err("ffi_call expects (lib, name, return_type, param_types, args)".to_string());
    }
    let id = as_i64(&args[0])?;
    let name = match &args[1] {
        Value::Str(s) => s.clone(),
        _ => return Err("ffi_call expects a symbol name string".to_string()),
    };
    let ret = match &args[2] {
        Value::Str(s) => s.clone(),
        _ => return Err("ffi_call expects a return type string".to_string()),
    };
    let ptypes = match &args[3] {
        Value::List(l) => l.borrow().clone(),
        _ => return Err("ffi_call expects a param-type list".to_string()),
    };
    let vals = match &args[4] {
        Value::List(l) => l.borrow().clone(),
        _ => return Err("ffi_call expects an argument list".to_string()),
    };
    if ptypes.len() != vals.len() {
        return Err("param_types and args length mismatch".to_string());
    }
    let libs_guard = libs().lock().unwrap();
    let lib = libs_guard.get(&(id as u64)).ok_or("unknown library id")?;
    let name_bytes = name.as_bytes();
    let key = {
        let mut s = String::new();
        for t in &ptypes {
            match t {
                Value::Str(x) => s.push_str(x.as_str()),
                _ => return Err("param types must be strings".to_string()),
            }
        }
        s.push(':');
        s.push_str(&ret);
        s
    };
    match key.as_str() {
        "V" => unsafe { call_0_v(&lib, name_bytes) },
        "I" => unsafe { call_0_i(&lib, name_bytes) },
        "F" => unsafe { call_0_f(&lib, name_bytes) },
        "I:V" => unsafe { call_1_i_v(&lib, name_bytes, as_i64(&vals[0])?) },
        "I:I" => unsafe { call_1_i_i(&lib, name_bytes, as_i64(&vals[0])?) },
        "I:F" => unsafe { call_1_i_f(&lib, name_bytes, as_i64(&vals[0])?) },
        "F:V" => unsafe { call_1_f_v(&lib, name_bytes, as_f64(&vals[0])?) },
        "F:I" => unsafe { call_1_f_i(&lib, name_bytes, as_f64(&vals[0])?) },
        "F:F" => unsafe { call_1_f_f(&lib, name_bytes, as_f64(&vals[0])?) },
        "S:I" => unsafe { call_1_s_i(&lib, name_bytes, as_cstr(&vals[0])?) },
        "S:V" => unsafe { call_1_s_v(&lib, name_bytes, as_cstr(&vals[0])?) },
        "S:S" => unsafe { call_1_s_s(&lib, name_bytes, as_cstr(&vals[0])?) },
        "FF:F" => unsafe { call_2_ff_f(&lib, name_bytes, as_f64(&vals[0])?, as_f64(&vals[1])?) },
        "II:I" => unsafe { call_2_ii_i(&lib, name_bytes, as_i64(&vals[0])?, as_i64(&vals[1])?) },
        _ => Err(format!("unsupported FFI signature: {key}")),
    }
}
