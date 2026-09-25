"""HTML pages built as element trees: the shapes shared by the identity sheet and the gallery."""

import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

import tomllib

from tokens import Face

FONTS = "https://fonts.googleapis.com/css2"
Child = ET.Element | str | None


def node(tag: str, attrs: dict[str, str] | None = None, *children: Child) -> ET.Element:
    element = ET.Element(tag, attrs or {})
    for child in children:
        append(element, child)
    return element


def append(element: ET.Element, child: Child) -> None:
    if child is None:
        return
    if isinstance(child, ET.Element):
        element.append(child)
    elif len(element):
        element[-1].tail = (element[-1].tail or "") + child
    else:
        element.text = (element.text or "") + child


def fonts_link(faces: list[Face]) -> ET.Element:
    families = {}
    for face in faces:
        families.setdefault(face.family, set()).update(face.weights)
    query = "&".join(
        "family=" + family.replace(" ", "+") + ":wght@" + ";".join(str(w) for w in sorted(weights))
        for family, weights in sorted(families.items())
    )
    return node("link", {"rel": "stylesheet", "href": f"{FONTS}?{query}&display=swap"})


def page(title: str, style: str, head: list[ET.Element], body: list[ET.Element]) -> bytes:
    root = node(
        "html",
        {"lang": "en"},
        node(
            "head",
            None,
            node("meta", {"charset": "utf-8"}),
            node("meta", {"name": "viewport", "content": "width=device-width, initial-scale=1"}),
            node("title", None, title),
            *head,
            node("style", None, style),
        ),
        node("body", None, *body),
    )
    # Not indented: whitespace inside <pre> is content.
    return b"<!doctype html>\n" + ET.tostring(root, encoding="utf-8", method="html") + b"\n"


def fluid(art: ET.Element) -> ET.Element:
    """`art` sized by its container instead of its own width and height."""
    if art.tag.endswith("svg"):
        art.attrib.pop("width", None)
        art.attrib.pop("height", None)
    return art


def sized(art: ET.Element, width: int) -> ET.Element:
    art.set("width", str(width))
    art.set("height", str(width))
    return art


def card(id: str, label: str, panels: list[tuple[ET.Element, str]], note: str | None = None) -> ET.Element:
    """A captioned card of one or more (art, backdrop color) panels."""
    return node(
        "figure",
        {"class": "card", "id": id},
        *(node("div", {"class": "art", "style": f"background:{backdrop}"}, fluid(art)) for art, backdrop in panels),
        node("figcaption", None, node("span", {"class": "id"}, id), " ", label, node("small", None, note) if note else None),
    )


def section(id: str, title: str, note: str, cards: list[ET.Element], kind: str = "grid") -> ET.Element:
    return node(
        "section",
        {"id": f"group-{id}"},
        node("h2", None, node("span", {"class": "id"}, id), " ", title),
        node("p", None, note),
        node("div", {"class": kind}, *cards),
    )


def swatch(color: str, label: str, detail: str) -> ET.Element:
    return node(
        "div",
        {"class": "swatch"},
        node("div", {"class": "chip", "style": f"background:{color}"}),
        node("b", None, label),
        node("code", None, detail),
    )


@dataclass(frozen=True)
class Sample:
    """A code specimen as (text, code role) tokens."""

    tokens: list[tuple[str, str]]

    @classmethod
    def read(cls, path: Path) -> "Sample":
        return cls([tuple(token) for token in tomllib.loads(path.read_text())["tokens"]])

    def element(self, colors: dict[str, str], face: Face, background: str) -> ET.Element:
        pre = node("pre", {"class": "sample", "style": f"font-family:{face.stack()};background:{background}"})
        for text, role in self.tokens:
            append(pre, node("span", {"style": f"color:{colors[role]}"}, text))
        return pre
