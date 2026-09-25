"""Build design/out/ from tokens, themes, parts and figures.

    python3 design/tools/build.py          write out/ and refresh part previews
    python3 design/tools/build.py --check  fail if anything is out of date
    python3 design/tools/build.py --png    also rasterize every figure (needs ImageMagick)
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

import emit
import sheet
import tweak
import tomllib
from figure import Figure, Paint, Part
from outputs import Outputs

import explore
from tokens import Section, TokenError, Tokens

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "out"
EXPLORE = ROOT / "explore"
PREVIEW_THEME = "light"
PNG_WIDTH = 1024


def figures(root: Path) -> list[Figure]:
    """Every `*.toml` with `layers` outside out/, explore/ and the token directories."""
    skip = {OUT, EXPLORE, root / "tokens", root / "themes"}
    return [
        Figure.read(root, str(path.relative_to(root).with_suffix("")))
        for path in sorted(root.rglob("*.toml"))
        if not any(path.is_relative_to(dir) for dir in skip) and "layers" in tomllib.loads(path.read_text())
    ]


def parts(root: Path) -> list[Part]:
    return [Part.read(path) for path in sorted(root.rglob("parts/**/*.svg")) if not path.is_relative_to(OUT)]


def preview_style(tokens: Tokens) -> str:
    theme = tokens.theme(PREVIEW_THEME)
    colors = theme.resolve(tokens.palette, Section.ART)
    rules = [paint.rule(role, color) for role, color in colors.items() for paint in Paint]
    return "\n" + "\n".join(f"    {rule}" for rule in rules) + "\n  "


def collect(tokens: Tokens, outputs: Outputs) -> list[Figure]:
    outputs.add(OUT / "tokens" / "tokens.css", emit.css(tokens))
    outputs.add(OUT / "tokens" / "tokens.typ", emit.typst(tokens))
    outputs.add(OUT / "tokens" / "tokens.json", emit.data(tokens))
    style = preview_style(tokens)
    for part in parts(ROOT):
        outputs.add(part.path, part.synced(style))
    found = figures(ROOT)
    for figure in found:
        for theme in tokens.themes:
            outputs.add(OUT / figure.name / f"{theme.name}.svg", figure.render(theme, tokens.palette))
    outputs.add(OUT / "identity.html", sheet.identity(ROOT, tokens, found))
    outputs.add(OUT / "explore.html", explore.gallery(ROOT, tokens))
    outputs.add(OUT / "tweak.html", tweak.tweaker(ROOT, tokens))
    return found


def rasterize(paths: list[Path]) -> None:
    magick = shutil.which("magick")
    if magick is None:
        sys.exit("--png needs ImageMagick (`magick`) on PATH")
    for path in paths:
        subprocess.run(
            [magick, "-background", "none", "-density", "300", str(path), "-resize", f"{PNG_WIDTH}x", str(path.with_suffix(".png"))],
            check=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--png", action="store_true")
    args = parser.parse_args()
    outputs = Outputs(ROOT, OUT)
    try:
        collect(Tokens.read(ROOT), outputs)
    except TokenError as error:
        sys.exit(f"error: {error}")
    if args.check:
        stale = outputs.changed() + outputs.stale()
        if stale:
            sys.exit("out of date: " + ", ".join(outputs.relative(stale)))
        return
    for path in outputs.relative(outputs.write()):
        print(f"wrote {path}")
    if args.png:
        rasterize(sorted(path for path in outputs.files if path.suffix == ".svg" and path.is_relative_to(OUT)))


if __name__ == "__main__":
    main()
