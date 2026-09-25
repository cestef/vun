// The tweaker's behavior. Data comes from #data (built by tweak.py); state lives in the URL hash,
// so a link reproduces exactly what is on screen.
const data = JSON.parse(document.getElementById("data").textContent);
const root = document.documentElement;
const SVG = "http://www.w3.org/2000/svg";
const NONE = "";
const MINI_SIZES = [16, 32, 64, 128];
const ADJUST = [
  { key: "hue", label: "Hue shift", min: -180, max: 180, unit: "deg", neutral: 0 },
  { key: "sat", label: "Saturation", min: 0, max: 200, unit: "%", neutral: 100 },
  { key: "light", label: "Lightness", min: -40, max: 40, unit: "", neutral: 0 },
  { key: "pastel", label: "Pastel (mix white)", min: 0, max: 80, unit: "%", neutral: 0 },
];

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (key.startsWith("on")) node.addEventListener(key.slice(2), value);
    else node.setAttribute(key, value);
  }
  node.append(...children.filter((child) => child !== null));
  return node;
}

// Color math: hex <-> rgb <-> hsl, all channels in 0..1.
const clamp = (x, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, x));
function rgb(hex) {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => c / 255);
}
function hex([r, g, b]) {
  return "#" + [r, g, b].map((c) => Math.round(clamp(c) * 255).toString(16).padStart(2, "0")).join("").toUpperCase();
}
function hsl([r, g, b]) {
  const max = Math.max(r, g, b), min = Math.min(r, g, b), l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min, s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h / 6, s, l];
}
function fromHsl([h, s, l]) {
  if (s === 0) return [l, l, l];
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s, p = 2 * l - q;
  const channel = (t) => {
    t = (t + 1) % 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  return [channel(h + 1 / 3), channel(h), channel(h - 1 / 3)];
}
function adjusted(color, adjust) {
  let [h, s, l] = hsl(rgb(color));
  h = (h + adjust.hue / 360 + 1) % 1;
  s = clamp(s * adjust.sat / 100);
  l = clamp(l + adjust.light / 100);
  const mixed = fromHsl([h, s, l]).map((c) => c + (1 - c) * adjust.pastel / 100);
  return hex(mixed);
}

function initial(preset) {
  preset ||= data.presets[0];
  return {
    preset: preset.name,
    colors: { ...preset.colors },
    backdrop: preset.backdrop,
    slots: { ...data.defaults, ...preset.slots },
    adjust: Object.fromEntries(ADJUST.map((a) => [a.key, a.neutral])),
    wordmark: data.wordmarks[0],
    font: data.fonts[0].family,
  };
}
function load() {
  try {
    return JSON.parse(decodeURIComponent(escape(atob(location.hash.slice(1)))));
  } catch {
    return null;
  }
}
// A saved state overlays its preset, so a link from an older build still opens.
function restore(saved) {
  const base = initial(data.presets.find((p) => p.name === saved?.preset));
  if (!saved) return base;
  const known = (table, keys) => Object.fromEntries(Object.entries(table || {}).filter(([key]) => keys.includes(key)));
  return {
    ...base,
    ...known(saved, ["backdrop", "wordmark", "font"]),
    colors: { ...base.colors, ...known(saved.colors, data.roles) },
    slots: { ...base.slots, ...known(saved.slots, Object.keys(data.slots)) },
    adjust: { ...base.adjust, ...known(saved.adjust, ADJUST.map((a) => a.key)) },
  };
}
let state = restore(load());

const final = (role) => adjusted(state.colors[role], state.adjust);

function apply() {
  for (const role of data.roles) root.style.setProperty(`--${role}`, final(role));
  document.getElementById("backdrop").style.background = state.backdrop;
  for (const layer of document.querySelectorAll("[data-slot]"))
    layer.style.display = state.slots[layer.dataset.slot] === layer.dataset.part ? "" : "none";
  for (const mark of document.querySelectorAll("[data-wordmark]"))
    mark.style.display = mark.dataset.wordmark === state.wordmark ? "" : "none";
  const face = data.fonts.find((f) => f.family === state.font);
  Object.assign(document.getElementById("typeset").style, { fontFamily: `"${face.family}"`, fontWeight: face.weight });
  for (const input of document.querySelectorAll("[data-role]")) {
    const color = final(input.dataset.role);
    if (input.type === "color") input.value = state.colors[input.dataset.role].toLowerCase();
    else input.textContent = color;
  }
  history.replaceState(null, "", "#" + btoa(unescape(encodeURIComponent(JSON.stringify(state)))));
  exports();
}

function select(label, options, value, onchange) {
  const input = el("select", { onchange: (e) => onchange(e.target.value) });
  for (const [group, items] of options) {
    const parent = group ? input.appendChild(el("optgroup", { label: group })) : input;
    for (const [key, text] of items) parent.append(el("option", { value: key }, text));
  }
  input.value = value;
  return el("label", { class: "row" }, el("span", {}, label), input);
}

function controls() {
  const panel = document.getElementById("controls");
  panel.replaceChildren();
  const groups = {};
  for (const preset of data.presets) (groups[preset.group] ||= []).push([preset.name, preset.name]);
  panel.append(
    el("h1", {}, "vun tweaker"),
    el("p", { class: "hint" }, "Tweak, then copy the TOML or the link and send it back."),
    select("Preset", Object.entries(groups), state.preset, (name) => {
      state = { ...initial(data.presets.find((p) => p.name === name)), wordmark: state.wordmark, font: state.font };
      controls();
      apply();
    }),
    el("h2", {}, "Parts"),
    ...Object.entries(data.slots).map(([slot, parts]) =>
      select(slot, [[null, [[NONE, "none"], ...parts.map((p) => [p, p.split("/").pop() + (p.startsWith("explore") ? " *" : "")])]]], state.slots[slot] ?? NONE, (part) => {
        state.slots[slot] = part;
        apply();
      })
    ),
    el("h2", {}, "Adjust all colors"),
    ...ADJUST.map((a) => {
      const out = el("output", {}, `${state.adjust[a.key]}${a.unit}`);
      const input = el("input", {
        type: "range", min: a.min, max: a.max, value: state.adjust[a.key],
        oninput: (e) => {
          state.adjust[a.key] = Number(e.target.value);
          out.textContent = `${e.target.value}${a.unit}`;
          apply();
        },
        ondblclick: (e) => {
          e.target.value = a.neutral;
          e.target.dispatchEvent(new Event("input"));
        },
      });
      return el("label", { class: "row" }, el("span", {}, a.label), input, out);
    }),
    el("h2", {}, "Roles"),
    el("datalist", { id: "ramps" }, ...Object.values(data.ramps).flatMap((steps) => Object.values(steps).map((c) => el("option", { value: c.toLowerCase() })))),
    ...data.roles.map((role) =>
      el(
        "label",
        { class: "row" },
        el("span", {}, role),
        el("input", {
          type: "color", list: "ramps", "data-role": role,
          oninput: (e) => {
            state.colors[role] = e.target.value.toUpperCase();
            apply();
          },
        }),
        el("code", { "data-role": role })
      )
    ),
    el(
      "label",
      { class: "row" },
      el("span", {}, "backdrop"),
      el("input", { type: "color", list: "ramps", value: state.backdrop.toLowerCase(), oninput: (e) => { state.backdrop = e.target.value.toUpperCase(); apply(); } })
    ),
    el("h2", {}, "Name"),
    select("Wordmark", [[null, data.wordmarks.map((w) => [w, w.split("/").pop()])]], state.wordmark, (w) => { state.wordmark = w; apply(); }),
    select("Typeface", [[null, data.fonts.map((f) => [f.family, f.family])]], state.font, (f) => { state.font = f; apply(); }),
    el("button", { onclick: () => { state = initial(data.presets.find((p) => p.name === state.preset)); controls(); apply(); } }, "Reset to preset")
  );
}

function minis() {
  const [x, y, w, h] = data.view;
  const strip = (background) =>
    el("div", { class: "strip", style: `background:${background}` }, ...MINI_SIZES.map((size) => {
      const mini = document.createElementNS(SVG, "svg");
      mini.setAttribute("viewBox", `${x} ${y} ${w} ${h}`);
      mini.setAttribute("width", size);
      mini.setAttribute("height", size);
      const use = document.createElementNS(SVG, "use");
      use.setAttribute("href", `#${data.stage}`);
      mini.append(use);
      return mini;
    }));
  const light = data.presets[0].backdrop;
  document.getElementById("minis").append(strip(light), strip(data.ramps.stone["950"]));
  document.getElementById("typeset").append(el("span", { class: "accent" }, "v"), "un");
}

function exports() {
  const art = data.roles.map((role) => `${role} = "tweak.${role}"`).join("\n");
  const ramp = data.roles.map((role) => `${role} = "${final(role)}"`).join("\n");
  const colorway = `note = "Tweaked from ${state.preset}."\nbackdrop = "tweak.backdrop"\n\n[ramps.tweak]\n${ramp}\nbackdrop = "${state.backdrop}"\n\n[art]\n${art}\n`;
  const layers = Object.values(state.slots).filter((part) => part !== NONE).map((part) => `  "${part}",`).join("\n");
  const pose = `view = [${data.view.join(", ")}]\nlayers = [\n${layers}\n]\n`;
  const box = (title, file, text) =>
    el("section", {}, el("h2", {}, title, " ", el("small", {}, file)),
      el("button", { onclick: () => navigator.clipboard.writeText(text) }, "Copy"), el("pre", {}, text));
  document.getElementById("export").replaceChildren(
    box("Colorway", "explore/mascot/colorways/<name>.toml", colorway),
    box("Pose", "mascot/poses/<name>.toml", pose),
    el("section", {}, el("h2", {}, "Link"), el("button", { onclick: () => navigator.clipboard.writeText(location.href) }, "Copy link"))
  );
}

controls();
minis();
apply();
