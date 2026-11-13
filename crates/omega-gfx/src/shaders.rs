use crate::surface::GfxError;

const VERT_PREAMBLE: &str = "#version 330 core\n#define attribute in\n#define varying out\n";

const FRAG_PREAMBLE: &str = "#version 330 core\n#define varying in\n\
                             #define texture2D texture\nout vec4 FRAG_COLOR;\n";

const VERT_PREAMBLE_ES: &str = "";
const FRAG_PREAMBLE_ES: &str = "#define FRAG_COLOR gl_FragColor\n";

pub fn program(vert: &str, frag: &str) -> Result<gl::types::GLuint, GfxError> {
    let (vp, fp) = match crate::gl::es() {
        true => (VERT_PREAMBLE_ES, FRAG_PREAMBLE_ES),
        false => (VERT_PREAMBLE, FRAG_PREAMBLE),
    };
    crate::gl::program(&format!("{vp}{vert}"), &format!("{fp}{frag}"))
}

/// A program whose attributes sit at the given locations, with the preambles `program` adds.
pub fn program_with(
    vert: &str,
    frag: &str,
    attributes: &[(&str, gl::types::GLuint)],
) -> Result<gl::types::GLuint, GfxError> {
    let (vp, fp) = match crate::gl::es() {
        true => (VERT_PREAMBLE_ES, FRAG_PREAMBLE_ES),
        false => (VERT_PREAMBLE, FRAG_PREAMBLE),
    };
    crate::gl::program_with(&format!("{vp}{vert}"), &format!("{fp}{frag}"), attributes)
}

pub const RECT_VERT: &str = r"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    vec2 p = (u_rect.xy + a_pos * u_rect.zw) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
";

pub const SPRITE_VERT: &str = r"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
uniform vec2 u_turn;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    vec2 mid = u_rect.zw * 0.5;
    vec2 local = a_pos * u_rect.zw - mid;
    vec2 turned = vec2(u_turn.x * local.x - u_turn.y * local.y,
                       u_turn.y * local.x + u_turn.x * local.y);
    vec2 p = (u_rect.xy + a_pos * u_rect.zw + (turned - local)) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
";

/// A sprite turned like `SPRITE_VERT`, then spun about its upright middle by `u_spin` (sin, cos,
/// depth) and seen from `u_focal` pixels away. Its own program, so the sprites that never turn
/// pay nothing for it. The spin is divided through on the vertex, so the clipper only ever sees
/// w = 1: some drivers drop a triangle that crosses the edge of the screen with any other w. For
/// the texture to keep true perspective, `v_uvq` carries uv/w and 1/w for `SPUN_FRAG` to divide.
pub const SPUN_VERT: &str = r"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
uniform vec2 u_turn;
uniform vec3 u_spin;
uniform float u_focal;
varying vec3 v_uvq;
void main() {
    vec2 mid = u_rect.zw * 0.5;
    vec2 local = a_pos * u_rect.zw - mid;
    vec2 turned = vec2(u_turn.x * local.x - u_turn.y * local.y,
                       u_turn.y * local.x + u_turn.x * local.y);
    float across = turned.x * u_spin.y - u_spin.z * u_spin.x;
    float w = (u_focal + turned.x * u_spin.x) / u_focal;
    v_uvq = vec3(a_pos, 1.0) / w;
    vec2 spun = vec2(across, turned.y) / w;
    vec2 p = (u_rect.xy + a_pos * u_rect.zw + (turned - local) + (spun - turned)) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
";

/// A model, such as a cart with its sides, drawn the way a PlayStation drew 3D. `a_xyz` is a
/// point of it across and down `u_rect` as shares of its size, and back from its face in shares
/// of its width. It is turned in the canvas by `u_turn` and spun about its upright middle by
/// `u_spin` (sin, cos), seen from `u_focal` pixels in front of that middle, and every vertex lands
/// on the grid of `u_snap` canvas pixels, so the model shimmers a little as it turns. A polygon
/// turned away from the eye is folded to nothing. The rest are lit per vertex from a point light at
/// `u_light.xyz` (canvas pixels from the middle, towards the eye negative), `u_light.w` of the
/// light falling everywhere alike, a face square to the eye at full brightness. Textures are
/// mapped without perspective, as the PlayStation mapped them.
pub const MODEL_VERT: &str = r"
attribute vec3 a_xyz;
attribute vec3 a_normal;
attribute vec3 a_uvs;
uniform vec4 u_rect;
uniform vec2 u_target;
uniform vec2 u_turn;
uniform vec3 u_spin;
uniform float u_focal;
uniform float u_snap;
uniform vec4 u_light;
varying vec2 v_uv;
varying float v_light;
void main() {
    vec2 mid = u_rect.zw * 0.5;
    vec2 local = a_xyz.xy * u_rect.zw - mid;
    float depth = a_xyz.z * u_rect.z;
    vec2 turned = vec2(u_turn.x * local.x - u_turn.y * local.y,
                       u_turn.y * local.x + u_turn.x * local.y);
    float across = turned.x * u_spin.y - depth * u_spin.x;
    float back = turned.x * u_spin.x + depth * u_spin.y;
    float w = (u_focal + back) / u_focal;
    vec2 p = u_rect.xy + mid + vec2(across, turned.y) / w;
    p = floor(p / u_snap + 0.5) * u_snap;
    vec2 nt = vec2(u_turn.x * a_normal.x - u_turn.y * a_normal.y,
                   u_turn.y * a_normal.x + u_turn.x * a_normal.y);
    vec3 n = vec3(nt.x * u_spin.y - a_normal.z * u_spin.x, nt.y,
                  nt.x * u_spin.x + a_normal.z * u_spin.y);
    float away = step(0.0, dot(n, vec3(across, turned.y, back + u_focal)));
    v_uv = a_uvs.xy;
    vec3 to_light = normalize(u_light.xyz - vec3(across, turned.y, back));
    float square = max(-normalize(u_light.xyz).z, 0.001);
    v_light = (u_light.w + (1.0 - u_light.w) * max(dot(n, to_light), 0.0) / square) * a_uvs.z;
    vec2 c = p / u_target;
    gl_Position = mix(vec4(c.x * 2.0 - 1.0, 1.0 - c.y * 2.0, 0.0, 1.0),
                      vec4(4.0, 4.0, 0.0, 1.0), away);
}
";

/// The model's pixels: its texture, lit, then cut to 15-bit colour through the PlayStation's
/// 4x4 ordered dither, a cell of it `u_dither` canvas pixels square. Its uniforms are its own,
/// none shared with the vertex stage: GLSL ES will not link a uniform declared at two precisions.
pub const MODEL_FRAG: &str = r"
precision mediump float;
uniform sampler2D u_tex;
uniform vec4 u_colour;
uniform float u_dither;
varying vec2 v_uv;
varying float v_light;
void main() {
    vec4 c = texture2D(u_tex, v_uv) * u_colour;
    c.rgb *= v_light;
    vec2 p = mod(floor(gl_FragCoord.xy / u_dither), 4.0);
    vec2 q = mod(p, 2.0);
    vec2 r = floor(p * 0.5);
    float b = 4.0 * mod(2.0 * q.x + 3.0 * q.y, 4.0) + mod(2.0 * r.x + 3.0 * r.y, 4.0);
    c.rgb = floor(clamp(c.rgb, 0.0, 1.0) * 31.0 + (b + 0.5) / 16.0) / 31.0;
    FRAG_COLOR = c;
}
";

pub const GAME_FRAG: &str = r"
precision mediump float;
uniform sampler2D u_game;
uniform sampler2D u_mask;
uniform vec2 u_src;
uniform vec4 u_uv;
uniform float u_bright;
uniform float u_glow;
varying vec2 v_uv;
void main() {
    vec2 uv = u_uv.xy + v_uv * u_uv.zw;
    vec3 rgb = texture2D(u_game, uv).rgb * texture2D(u_mask, v_uv * u_src).rgb;
    FRAG_COLOR = vec4(mix(rgb * u_bright, vec3(1.0), u_glow), 1.0);
}
";

/// The game at a scale that is not a whole number: each game pixel stays a sharp square, and
/// only the one panel pixel where two meet is blended, so nothing shimmers as it scrolls.
pub const GAME_SHARP_FRAG: &str = r"
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_game;
uniform vec2 u_src;
uniform vec4 u_uv;
uniform vec2 u_scale;
uniform float u_bright;
uniform float u_glow;
varying vec2 v_uv;
void main() {
    vec2 texel = (u_uv.xy + v_uv * u_uv.zw) * u_src;
    vec2 from_centre = fract(texel) - 0.5;
    vec2 region = 0.5 - 0.5 / u_scale;
    vec2 f = (from_centre - clamp(from_centre, -region, region)) * u_scale + 0.5;
    vec3 rgb = texture2D(u_game, (floor(texel) + f) / u_src).rgb;
    FRAG_COLOR = vec4(mix(rgb * u_bright, vec3(1.0), u_glow), 1.0);
}
";

pub const SPRITE_FRAG: &str = r"
precision mediump float;
uniform sampler2D u_tex;
uniform vec4 u_colour;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = texture2D(u_tex, v_uv) * u_colour;
}
";

pub const SPUN_FRAG: &str = r"
precision mediump float;
uniform sampler2D u_tex;
uniform vec4 u_colour;
varying vec3 v_uvq;
void main() {
    FRAG_COLOR = texture2D(u_tex, v_uvq.xy / v_uvq.z) * u_colour;
}
";

pub const BLIT_VERT: &str = r"
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0);
}
";

/// The blit for a panel mounted on its side: `u_turn` takes each viewport point to the point
/// of the upright picture that belongs there.
pub const BLIT_VERT_TURNED: &str = r"
attribute vec2 a_pos;
uniform mat2 u_turn;
varying vec2 v_uv;
void main() {
    v_uv = u_turn * (a_pos - 0.5) + 0.5;
    gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0);
}
";

pub const BLIT_FRAG: &str = r"
precision mediump float;
uniform sampler2D u_tex;
uniform vec3 u_gain;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = vec4(texture2D(u_tex, v_uv).rgb * u_gain, 1.0);
}
";
