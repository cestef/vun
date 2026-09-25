"""The live tweaker: every part, preset, wordmark and face in one page, recolored in the browser.

Parts keep their role classes; the page paints `f-<role>` and `s-<role>` from CSS variables that
the script sets, so a color change never rebuilds the SVG. Settings export back as TOML.
"""

import copy
import json
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path

from figure import Figure, Paint, Part, svg
from look import Look
from pages import fonts_link, node, page
from tokens import Face, Section, Tokens

HERE = Path(__file__).parent
POSE = "mascot/poses/default"
SLOTS = "mascot/slots.toml"
PART_DIRS = ("mascot/parts", "explore/mascot/parts")
WORDMARK_PARTS = ("identity/logo/parts/wordmark.svg", "explore/identity/wordmarks/parts/*.svg")
STAGE_ID = "mascot"


class Tweaker:
    def __init__(self, root: Path, tokens: Tokens):
        self.root = root
        self.tokens = tokens
        self.pose = Figure.read(root, POSE)
        self.base = Look.of(tokens, tokens.theme("light"))

    def name(self, path: Path) -> str:
        return str(path.relative_to(self.root).with_suffix(""))

    def slots(self) -> dict[str, list[Part]]:
        order = tomllib.loads((self.root / SLOTS).read_text())["order"]
        slots: dict[str, list[Part]] = {slot: [] for slot in order}
        for dir in PART_DIRS:
            for path in sorted((self.root / dir).glob("*/*.svg")):
                slots.setdefault(path.parent.name, []).append(Part.read(path))
        return slots

    def presets(self) -> list[dict]:
        colorways = sorted(
            (Look.read(self.tokens, path) for path in (self.root / "explore/mascot/colorways").glob("*.toml")),
            key=lambda look: (look.table.get("order", 1 << 16), look.theme.name),
        )
        directions = [
            Look.read(self.tokens, path) for path in sorted((self.root / "explore/identity/directions").glob("*.toml"))
        ]
        presets = [self.preset("Colorways", look.theme.name, look) for look in colorways]
        presets += [self.preset("Directions", look.table["title"], look) for look in directions]
        return presets

    def preset(self, group: str, name: str, look: Look) -> dict:
        return {
            "group": group,
            "name": name,
            "colors": look.colors(Section.ART),
            "backdrop": look.backdrop(),
            "slots": look.table.get("mascot", {}).get("swap", {}),
        }

    def stage(self, slots: dict[str, list[Part]]) -> ET.Element:
        """Every variant of every slot, stacked in pose order; the script shows one per slot."""
        x, y, width, height = self.pose.view
        root = ET.Element(svg("svg"), {"viewBox": f"{x} {y} {width} {height}", "class": "stage"})
        group = ET.SubElement(root, svg("g"), {"id": STAGE_ID})
        for slot, parts in slots.items():
            for part in parts:
                layer = ET.SubElement(group, svg("g"), {"data-slot": slot, "data-part": self.name(part.path)})
                layer.extend(copy.deepcopy(shape) for shape in part.shapes())
        return root

    def wordmarks(self) -> list[Part]:
        return [Part.read(path) for pattern in WORDMARK_PARTS for path in sorted(self.root.glob(pattern))]

    def wordmark(self, part: Part) -> ET.Element:
        x, y, width, height = part.view()
        root = ET.Element(svg("svg"), {"viewBox": f"{x} {y} {width} {height}", "data-wordmark": self.name(part.path)})
        root.extend(copy.deepcopy(shape) for shape in part.shapes())
        return root

    def type(self) -> dict:
        return tomllib.loads((self.root / "explore/identity/type.toml").read_text())

    def style(self, roles: list[str]) -> str:
        rules = [paint.rule(role, f"var(--{role})") for role in roles for paint in Paint]
        return (HERE / "tweak.css").read_text() + "\n".join(rules) + "\n"

    def render(self) -> bytes:
        slots = self.slots()
        wordmarks = self.wordmarks()
        type = self.type()
        fonts = [Face(entry["family"], ["sans-serif"], [entry["weight"], entry["text"]]) for entry in type["face"]]
        roles = list(self.base.colors(Section.ART))
        defaults = {layer.source.slot(): self.name(layer.source.path) for layer in self.pose.layers if isinstance(layer.source, Part)}
        data = {
            "roles": roles,
            "slots": {slot: [self.name(part.path) for part in parts] for slot, parts in slots.items()},
            "defaults": defaults,
            "presets": self.presets(),
            "ramps": self.tokens.palette.ramps,
            "wordmarks": [self.name(part.path) for part in wordmarks],
            "fonts": [{key: entry[key] for key in ("family", "weight", "text")} for entry in type["face"]],
            "specimen": {"headline": type["headline"], "body": type["body"]},
            "view": list(self.pose.view),
            "stage": STAGE_ID,
        }
        body = [
            node("aside", {"id": "controls"}),
            node(
                "main",
                None,
                node("div", {"id": "backdrop"}, self.stage(slots)),
                node("div", {"id": "minis"}),
                node("div", {"id": "wordmarks"}, *(self.wordmark(part) for part in wordmarks), node("div", {"id": "specimen"})),
                node("div", {"id": "export"}),
            ),
            node("script", {"type": "application/json", "id": "data"}, json.dumps(data)),
            node("script", None, (HERE / "tweak.js").read_text()),
        ]
        return page("Vun tweaker", self.style(roles), [fonts_link(fonts)], body)


def tweaker(root: Path, tokens: Tokens) -> bytes:
    return Tweaker(root, tokens).render()
