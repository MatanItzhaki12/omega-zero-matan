/*
  The colours the components draw in, made from the backdrop colour the player picks in
  Settings > Theme > Colour, so the menus always sit in the same colour as the shelf behind
  them. Night, the default, gives plain greys.

  The ramp is Mega Man Zero's own menu colours, which are what Amber gives. Any other backdrop
  colour keeps how light each step is (in OKLCH) and turns the ramp to its own hue, taking as much
  of the ramp's colour as it has itself: none for Night, a touch for Slate, all of it for the rest.

    OmegaPalette.apply('teal');            // the whole page
    OmegaPalette.apply('plum', someDiv);   // only what is inside someDiv

  apply() sets the --oz-* properties (and the backdrop's --oz-bg) on the element, has backdrop.js
  paint the backdrop's patterns in the colour there, and tells the components under it to draw
  their sprites again. Night is applied to the page as soon as this file loads.

  Load it after pixel.js and backdrop.js, and before the components.
*/
window.OmegaPalette = (() => {
  // The backdrop colours omega-zero offers, as crates/omega-ui/src/backdrop.rs draws them.
  const BACKDROPS = {
    night: '#050508',
    slate: '#22252c',
    navy: '#101b36',
    teal: '#0b2b2e',
    green: '#102c16',
    plum: '#291431',
    crimson: '#300f16',
    amber: '#302208',
    trainer: '#171719',
    ruby: '#25171c',
    sapphire: '#181c28',
    emerald: '#17231f',
    firered: '#281c1a',
    leafgreen: '#1d251b',
  };

  // The colour a backdrop colour's menus take their hue from, where it is not its own: Trainer's
  // dark grey takes a capture ball's red, and each game's grey that game's colour. omega-zero's
  // custom colours, one every 10 degrees of hue, are not offered here.
  const TONES = {
    trainer: '#d83030',
    ruby: '#c0183c',
    sapphire: '#2058d0',
    emerald: '#18a860',
    firered: '#e85020',
    leafgreen: '#68c038',
  };
  // How light the menus of a colour with a tone are next to the ramp, but for their two text
  // colours: deeper, so Trainer's are a capture ball's red rather than the pink the ramp's light
  // steps make of it.
  const DEPTH = 0.84;
  const TEXT = ['--oz-label', '--oz-cream'];

  // Mega Man Zero's menu colours, darkest first: the ramp every backdrop colour turns.
  const RAMP = {
    '--oz-dark': '#2a0f04',     // the bar under the actions, outlines
    '--oz-edge': '#6a3d01',     // the shadow line inside a well
    '--oz-well': '#4f300d',     // a panel's body, a label's shadow
    '--oz-plate': '#765020',    // what is off on a row: the bars a signal does not reach
    '--oz-tab': '#845a1a',      // a panel's title tab
    '--oz-band': '#a57221',     // the top of an action's button
    '--oz-stripe': '#b0731b',   // the dark line between two frame lines
    '--oz-dim': '#be9732',      // the title of a panel that is not active
    '--oz-frame': '#e6ab47',    // frames
    '--oz-frame-hi': '#f8b83f', // the strip beside a title tab, the arrows
    '--oz-label': '#f2c87d',    // an action's big label
    '--oz-cream': '#f5e5bb',    // the active title, an action's hint
  };

  // A backdrop colour this close to grey (in OKLCH chroma) takes none of the ramp's colour.
  const GREY = 0.01;
  // How far a step of the ramp may sit from the ramp's own hue, in degrees: the game's darkest
  // browns lean further toward red than this, which reads oddly once turned to another hue.
  const LEAN = 15;

  const toLinear = c => ((c /= 255) <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const toSrgb = c => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
  const channels = hex => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));
  const toHex = rgb => '#' + rgb.map(c => Math.round(c).toString(16).padStart(2, '0')).join('');

  // OKLCH of a #rrggbb colour (Björn Ottosson's OKLab), the hue in degrees.
  function oklch(hex) {
    const [r, g, b] = channels(hex).map(toLinear);
    const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    const L = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
    const A = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    const B = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    return [L, Math.hypot(A, B), ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360];
  }

  // Linear sRGB of an OKLCH colour, which may fall outside 0..1.
  function linear(L, C, H) {
    const A = C * Math.cos((H * Math.PI) / 180);
    const B = C * Math.sin((H * Math.PI) / 180);
    const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3;
    const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3;
    const s = (L - 0.0894841775 * A - 1.2914855480 * B) ** 3;
    return [
      4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
      -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
      -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ];
  }

  // An OKLCH colour as #rrggbb, its chroma cut back as far as it takes to fit in sRGB.
  function fit(L, C, H) {
    const fits = c => linear(L, c, H).every(v => v >= -1e-6 && v <= 1 + 1e-6);
    let lo = 0;
    let hi = C;
    if (fits(C)) lo = C;
    else for (let i = 0; i < 30; i++) fits((lo + hi) / 2) ? (lo = (lo + hi) / 2) : (hi = (lo + hi) / 2);
    return toHex(linear(L, lo, H).map(v => toSrgb(Math.min(1, Math.max(0, v))) * 255));
  }

  // Every --oz-* property for the backdrop colour `name` (Night when it is not one).
  function palette(name) {
    const base = BACKDROPS[name] ?? BACKDROPS.night;
    const [, chroma, hue] = oklch(TONES[name] ?? base);
    const [, rampChroma, rampHue] = oklch(BACKDROPS.amber);
    const take = Math.min(1, Math.max(0, (chroma - GREY) / (rampChroma - GREY)));
    const out = { '--oz-bg': base };
    for (const [token, colour] of Object.entries(RAMP)) {
      const [l, C, H] = oklch(colour);
      const L = TEXT.includes(token) || !(name in TONES) ? l : l * DEPTH;
      const lean = Math.max(-LEAN, Math.min(LEAN, ((H - rampHue + 540) % 360) - 180));
      out[token] = fit(L, C * take, take ? hue + lean : 0);
    }
    return out;
  }

  function apply(name, root = document.documentElement) {
    const tokens = palette(name);
    for (const [token, value] of Object.entries(tokens)) root.style.setProperty(token, value);
    OmegaBackdrop.paint(root, tokens['--oz-bg']);
    root.dataset.colour = name in BACKDROPS ? name : 'night';
    document.dispatchEvent(new CustomEvent('omega-palette', { detail: { root } }));
  }

  apply('night');

  return { COLOURS: Object.keys(BACKDROPS), palette, apply };
})();
