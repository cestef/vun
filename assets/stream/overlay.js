// What every overlay shares, read from its address: `scheme` names a scheme of `palette.toml`
// (dark unless given), whose colors become the page's variables; an element with `data-param`
// shows the parameter it names, or its own text when none is given; an image with
// `data-figure` shows that figure of `generated/figures`, painted in the scheme.

const params = new URLSearchParams(location.search);
const name = params.get('scheme') ?? 'dark';
const scheme = SCHEMES[name];
if (scheme === undefined) {
  document.body.replaceChildren(`no scheme ${name}; there are ${Object.keys(SCHEMES).join(', ')}`);
} else {
  for (const [role, color] of Object.entries(scheme)) {
    document.documentElement.style.setProperty(`--${role}`, color);
  }
  for (const element of document.querySelectorAll('[data-param]')) {
    const value = params.get(element.dataset.param);
    if (value !== null) {
      element.textContent = value;
    }
  }
  for (const image of document.querySelectorAll('img[data-figure]')) {
    image.src = `generated/figures/${name}/${image.dataset.figure}.svg`;
  }
}
