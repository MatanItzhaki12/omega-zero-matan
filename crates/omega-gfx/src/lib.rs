//! The screen: an OpenGL ES 2 renderer the frontend draws its frames through, on the H700's
//! GPU by way of the framebuffer or EGL, in a window on a computer, or headless for the render
//! tests. It holds the textures and meshes, the low-poly pipeline the shelf is drawn in, the
//! game's picture with its LCD and grid filters, and the colour grading the theme takes.

mod canvas;
mod draw;
mod fbdev;
mod fbo;
mod gl;
mod gpu;
mod grade;
#[cfg(target_os = "macos")]
mod headless;
#[cfg(all(target_os = "linux", not(feature = "device")))]
#[path = "headless_egl.rs"]
mod headless;
#[cfg(feature = "host")]
mod host;
mod lcd3x;
mod pipeline;
mod power;
mod quad;
mod shaders;
mod surface;

pub use canvas::{canvas, out_h, out_w, set_canvas, Canvas, CANVAS_ENV};
pub use draw::{spun_x, Draw, MeshId, MeshVertex, TexId, FOCAL};
pub use fbdev::{egl_error, panel_mode, panel_size, FbdevSurface};
pub use fbo::{Bezel, Compositor, BACKDROP};
pub use gpu::{delete_fence, wait_and_free, GpuFrame, GpuFrames, GpuThread};
pub use grade::{blue_light_gain, grade_gain, low_light, BLUE_LIGHT_MAX};
#[cfg(any(target_os = "macos", all(target_os = "linux", not(feature = "device"))))]
pub use headless::HeadlessSurface;
#[cfg(feature = "host")]
pub use host::{host_rotation, HostSurface};
pub use lcd3x::{lcd3x_mask, mask_texture_rgba8, Mask};
pub use pipeline::{Filter, GameView, SCALE, SRC_H, SRC_W, WHOLE_TEXTURE};
pub use power::{screen_brightness, screen_glow, screen_rect_in, screen_scale, screen_width};
pub use surface::{
    blit_rect, blit_rect_fit, fit_rect, fit_scale, panel_rect, GfxError, Rotation, Surface,
};
