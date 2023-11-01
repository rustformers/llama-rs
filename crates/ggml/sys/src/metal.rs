/* automatically generated by rust-bindgen 0.65.1 */

use super::ggml_tensor;
use super::ggml_log_callback;
use super::ggml_cgraph;

pub const GGML_METAL_MAX_BUFFERS: u32 = 16;
pub const GGML_METAL_MAX_COMMAND_BUFFERS: u32 = 32;
extern "C" {
    pub fn ggml_metal_log_set_callback(
        log_callback: ggml_log_callback,
        user_data: *mut ::std::os::raw::c_void,
    );
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ggml_metal_context {
    _unused: [u8; 0],
}
extern "C" {
    pub fn ggml_metal_init(n_cb: ::std::os::raw::c_int) -> *mut ggml_metal_context;
}
extern "C" {
    pub fn ggml_metal_free(ctx: *mut ggml_metal_context);
}
extern "C" {
    pub fn ggml_metal_host_malloc(n: usize) -> *mut ::std::os::raw::c_void;
}
extern "C" {
    pub fn ggml_metal_host_free(data: *mut ::std::os::raw::c_void);
}
extern "C" {
    pub fn ggml_metal_set_n_cb(ctx: *mut ggml_metal_context, n_cb: ::std::os::raw::c_int);
}
extern "C" {
    pub fn ggml_metal_add_buffer(
        ctx: *mut ggml_metal_context,
        name: *const ::std::os::raw::c_char,
        data: *mut ::std::os::raw::c_void,
        size: usize,
        max_size: usize,
    ) -> bool;
}
extern "C" {
    pub fn ggml_metal_set_tensor(ctx: *mut ggml_metal_context, t: *mut ggml_tensor);
}
extern "C" {
    pub fn ggml_metal_get_tensor(ctx: *mut ggml_metal_context, t: *mut ggml_tensor);
}
extern "C" {
    pub fn ggml_metal_graph_find_concurrency(
        ctx: *mut ggml_metal_context,
        gf: *mut ggml_cgraph,
        check_mem: bool,
    );
}
extern "C" {
    pub fn ggml_metal_if_optimized(ctx: *mut ggml_metal_context) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn ggml_metal_get_concur_list(ctx: *mut ggml_metal_context) -> *mut ::std::os::raw::c_int;
}
extern "C" {
    pub fn ggml_metal_graph_compute(ctx: *mut ggml_metal_context, gf: *mut ggml_cgraph);
}
