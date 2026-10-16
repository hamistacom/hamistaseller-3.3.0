//! Secret storage. On Windows: DPAPI (CurrentUser) — only the same Windows account on the same PC can decrypt.
//! Elsewhere (development builds only) values are stored as-is, clearly tagged.

const DPAPI: &[u8] = b"D1:";
const PLAIN: &[u8] = b"P0:";

pub fn seal(plain: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        dpapi::run(plain, true)
            .map(|v| [DPAPI, &v].concat())
            .ok_or_else(|| "رمزنگاری با DPAPI ناموفق بود.".into())
    }
    #[cfg(not(windows))]
    {
        Ok([PLAIN, plain].concat())
    }
}

pub fn open(sealed: &[u8]) -> Result<Vec<u8>, String> {
    if let Some(v) = sealed.strip_prefix(PLAIN) {
        return Ok(v.to_vec());
    }
    let Some(v) = sealed.strip_prefix(DPAPI) else {
        return Err("داده رمزشده ناشناخته است.".into());
    };
    #[cfg(windows)]
    {
        dpapi::run(v, false).ok_or_else(|| "رمزها روی این ویندوز قابل بازگشایی نیستند؛ کلیدها و رمزها را دوباره وارد کنید.".into())
    }
    #[cfg(not(windows))]
    {
        let _ = v;
        Err("این داده روی ویندوز رمز شده است.".into())
    }
}

pub fn seal_str(s: &str) -> Result<Vec<u8>, String> {
    seal(s.as_bytes())
}

pub fn open_str(sealed: &[u8]) -> Result<String, String> {
    String::from_utf8(open(sealed)?).map_err(|_| "داده رمزشده خراب است.".into())
}

#[cfg(windows)]
mod dpapi {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    #[repr(C)]
    struct Blob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            data_in: *const Blob,
            description: *const u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const Blob,
            description: *mut *mut u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut Blob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(mem: *mut c_void) -> *mut c_void;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
    const ENTROPY: &[u8] = b"hamista-seller/secrets/v1";

    pub fn run(data: &[u8], protect: bool) -> Option<Vec<u8>> {
        let input = Blob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let entropy = Blob {
            cb_data: ENTROPY.len() as u32,
            pb_data: ENTROPY.as_ptr() as *mut u8,
        };
        let mut out = Blob {
            cb_data: 0,
            pb_data: null_mut(),
        };
        // SAFETY: all pointers are valid for the call; DPAPI only reads `input`/`entropy` and allocates `out` with LocalAlloc.
        let ok = unsafe {
            if protect {
                CryptProtectData(&input, null(), &entropy, null_mut(), null_mut(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            } else {
                CryptUnprotectData(&input, null_mut(), &entropy, null_mut(), null_mut(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            }
        };
        if ok == 0 || out.pb_data.is_null() {
            return None;
        }
        // SAFETY: DPAPI returned `cb_data` valid bytes at `pb_data`, which we copy and then free exactly once.
        let v = unsafe { std::slice::from_raw_parts(out.pb_data, out.cb_data as usize).to_vec() };
        unsafe { LocalFree(out.pb_data as *mut c_void) };
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = seal_str("کلید-API-۱۲۳").unwrap();
        assert_ne!(s, "کلید-API-۱۲۳".as_bytes());
        assert_eq!(open_str(&s).unwrap(), "کلید-API-۱۲۳");
        assert!(open(b"XX:abc").is_err());
    }
}
