pub const FEED_SYMS: &[&str] = &[
    "_ZN8Starfish4FeedEPKvix",
    "_ZN8Starfish4FeedEPKcix",
    "Feed",
];

#[cfg(all(target_arch = "arm", target_os = "linux"))]
mod native {
    use std::os::raw::c_char;

    extern "C" {
        pub fn sf_create() -> i32;
        pub fn sf_load(payload: *const c_char) -> i32;
        pub fn sf_play() -> i32;
        pub fn sf_pause() -> i32;
        pub fn sf_feed(bytes: *const u8, len: i32, pts_ns: i64, es: i32) -> i32;
        pub fn sf_flush() -> i32;
        pub fn sf_unload() -> i32;
        pub fn sf_destroy();
        pub fn acb_create() -> i32;
        pub fn acb_bind(media_id: *const c_char) -> i32;
        pub fn acb_send_video_data(info: *const u8, len: i32) -> i32;
        pub fn acb_start(x: i32, y: i32, w: i32, h: i32) -> i32;
        pub fn acb_unload() -> i32;
    }
}

#[cfg(all(target_arch = "arm", target_os = "linux"))]
pub use native::*;

#[cfg(not(all(target_arch = "arm", target_os = "linux")))]
mod stub {
    use std::os::raw::c_char;

    pub unsafe fn sf_create() -> i32 {
        -1
    }
    pub unsafe fn sf_load(_payload: *const c_char) -> i32 {
        -1
    }
    pub unsafe fn sf_play() -> i32 {
        -1
    }
    pub unsafe fn sf_pause() -> i32 {
        -1
    }
    pub unsafe fn sf_feed(_bytes: *const u8, _len: i32, _pts_ns: i64, _es: i32) -> i32 {
        -1
    }
    pub unsafe fn sf_flush() -> i32 {
        -1
    }
    pub unsafe fn sf_unload() -> i32 {
        -1
    }
    pub unsafe fn sf_destroy() {}
    pub unsafe fn acb_create() -> i32 {
        -1
    }
    pub unsafe fn acb_bind(_media_id: *const c_char) -> i32 {
        -1
    }
    pub unsafe fn acb_send_video_data(_info: *const u8, _len: i32) -> i32 {
        -1
    }
    pub unsafe fn acb_start(_x: i32, _y: i32, _w: i32, _h: i32) -> i32 {
        -1
    }
    pub unsafe fn acb_unload() -> i32 {
        -1
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "linux")))]
pub use stub::*;

#[no_mangle]
pub extern "C" fn sf_on_event(_type: i32, _json: *const std::os::raw::c_char) {}

#[no_mangle]
pub extern "C" fn acb_on_event(_type: i32, _json: *const std::os::raw::c_char) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_lookup_is_cxx11_then_precxx_then_c() {
        assert_eq!(FEED_SYMS.len(), 3);
        assert!(FEED_SYMS[0].contains("Starfish"));
        assert!(FEED_SYMS[1].contains("Starfish"));
        assert_ne!(FEED_SYMS[0], FEED_SYMS[1]);
        assert_eq!(FEED_SYMS[2], "Feed");
    }

    #[test]
    fn host_sf_create_is_unavailable() {
        assert_eq!(unsafe { sf_create() }, -1);
    }
}
