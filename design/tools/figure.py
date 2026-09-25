"""Parts (SVG layers on a shared canvas) and figures (stacks of parts), painted by a theme."""

import xml.etree.ElementTree as ET
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

import tomllib

from tokens import Palette, Theme, TokenError

SVG = "http://www.w3.org/2000/svg"
ET.register_namespace("", SVG)

STYLE_ID = "theme"
DEFAULT_VIEW = (0, 0, 512, 512)


def svg(tag: str) -> str:
    return f"{{{SVG}}}{tag}"


class Paint(Enum):
    """A role class `<prefix>-<role>` paints one attribute."""

    FILL = ("f", "fill")
    STROKE = ("s", "stroke")

    def __init__(self, prefix: str, attribute: str):
        self.prefix = prefix
        self.attribute = attribute

    @classmethod
    def split(cls, name: str) -> tuple["Paint", str] | None:
        prefix, dash, role = name.partition("-")
        paint = next((paint for paint in cls if paint.prefix == prefix), None)
        return (paint, role) if dash and paint else None

    def rule(self, role: str, color: str) -> str:
        return f".{self.prefix}-{role} {{ {self.attribute}: {color} }}"


@dataclass(frozen=True)
class Part:
    path: Path
    root: ET.Element

    @classmethod
    def read(cls, path: Path) -> "Part":
        parser = ET.XMLParser(target=ET.TreeBuilder(insert_comments=True))
        return cls(path, ET.parse(path, parser).getroot())

    def view(self) -> tuple[float, ...]:
        return tuple(float(n) for n in self.root.get("viewBox", "").split()) or DEFAULT_VIEW

    def slot(self) -> str:
        return self.path.parent.name

    def shapes(self) -> list[ET.Element]:
        return [
            child
            for child in self.root
            if child.tag not in (svg("style"), ET.Comment)
        ]

    def paint(self, element: ET.Element, theme: Theme, palette: Palette) -> ET.Element:
        """A copy of `element` with role classes replaced by the theme's colors."""
        copy = ET.Element(element.tag, dict(element.attrib))
        kept = []
        for name in copy.attrib.pop("class", "").split():
            split = Paint.split(name)
            if split is None:
                kept.append(name)
                continue
            paint, role = split
            copy.set(paint.attribute, theme.paint(palette, role, self.path))
        if kept:
            copy.set("class", " ".join(kept))
        copy.extend(
            self.paint(child, theme, palette) for child in element if child.tag is not ET.Comment
        )
        return copy

    def synced(self, style: str) -> bytes:
        """The part's source with its preview style set to `style`."""
        element = self.root.find(f"{svg('style')}[@id='{STYLE_ID}']")
        if element is None:
            element = ET.Element(svg("style"), {"id": STYLE_ID})
            self.root.insert(0, element)
        element.text = style
        return serialize(self.root)


@dataclass(frozen=True)
class Layer:
    source: "Part | Figure"
    transform: str | None


@dataclass(frozen=True)
class Figure:
    name: str
    view: tuple[float, ...]
    layers: list[Layer]
    path: Path

    @classmethod
    def read(cls, root: Path, name: str) -> "Figure":
        path = root / f"{name}.toml"
        table = tomllib.loads(path.read_text())
        layers = [cls.layer(root, path, entry) for entry in table["layers"]]
        return cls(name, tuple(table.get("view", DEFAULT_VIEW)), layers, path)

    @classmethod
    def layer(cls, root: Path, path: Path, entry: str | dict) -> Layer:
        entry = {"part": entry} if isinstance(entry, str) else entry
        if "part" in entry:
            source = Part.read(root / f"{entry['part']}.svg")
        elif "figure" in entry:
            source = cls.read(root, entry["figure"])
        else:
            raise TokenError(path, f"layer {entry} names neither a `part` nor a `figure`")
        return Layer(source, entry.get("transform"))

    def swapped(self, root: Path, swaps: dict[str, str], order: list[str]) -> "Figure":
        """This figure with the part in each named slot replaced; an empty path drops the slot.

        A slot the figure lacks is inserted where `order` places it among the figure's slots.
        """
        rank = {slot: index for index, slot in enumerate(order)}
        for slot in swaps.keys() - rank.keys():
            raise TokenError(self.path, f"`{slot}` is not a slot of {order}")
        by_slot = {layer.source.slot(): layer for layer in self.layers if isinstance(layer.source, Part)}
        for slot, part in swaps.items():
            if part:
                replaced = by_slot.get(slot)
                by_slot[slot] = Layer(Part.read(root / f"{part}.svg"), replaced.transform if replaced else None)
            else:
                by_slot.pop(slot, None)
        others = [layer for layer in self.layers if not isinstance(layer.source, Part)]
        layers = [by_slot[slot] for slot in sorted(by_slot, key=lambda slot: rank.get(slot, len(order)))]
        return Figure(self.name, self.view, layers + others, self.path)

    def group(self, theme: Theme, palette: Palette) -> list[ET.Element]:
        groups = []
        for layer in self.layers:
            group = ET.Element(svg("g"))
            if isinstance(layer.source, Part):
                group.set("id", f"{layer.source.slot()}-{layer.source.path.stem}")
                group.extend(layer.source.paint(shape, theme, palette) for shape in layer.source.shapes())
            else:
                group.extend(layer.source.group(theme, palette))
            if layer.transform:
                group.set("transform", layer.transform)
            groups.append(group)
        return groups

    def element(self, theme: Theme, palette: Palette) -> ET.Element:
        x, y, width, height = self.view
        root = ET.Element(
            svg("svg"),
            {"viewBox": f"{x} {y} {width} {height}", "width": f"{width}", "height": f"{height}"},
        )
        root.extend(self.group(theme, palette))
        return root

    def render(self, theme: Theme, palette: Palette) -> bytes:
        return serialize(self.element(theme, palette))


def serialize(root: ET.Element) -> bytes:
    ET.indent(root)
    return ET.tostring(root, encoding="utf-8", xml_declaration=False) + b"\n"
