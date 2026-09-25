"""The exploration gallery: every variant under explore/, labelled with an id for feedback."""

import xml.etree.ElementTree as ET
from pathlib import Path

import tomllib
from figure import Figure, Layer, Part
from look import Look
from pages import Sample, card, fluid, fonts_link, node, page, section, sized, swatch

from tokens import Face, Section, Tokens

STYLE = Path(__file__).with_name("gallery.css")
DEFAULT_POSE = "mascot/poses/default"
SLOTS = "mascot/slots.toml"
WORDMARK = "identity/logo/wordmark"
DEFAULT_LOOK = "quartz"
SPECIMEN = "vun"
ORDER_LAST = 1 << 16
ICON_SIZES = (16, 24, 32, 48, 64)


class Gallery:
    def __init__(self, root: Path, tokens: Tokens):
        self.root = root
        self.explore = root / "explore"
        self.tokens = tokens
        self.faces: list[Face] = list(tokens.type.faces.values())
        self.order = tomllib.loads((root / SLOTS).read_text())["order"]

    def look(self, name: str) -> Look:
        return Look.read(self.tokens, self.explore / "mascot" / "colorways" / f"{name}.toml")

    def mascot(self, spec: dict, look: Look) -> ET.Element:
        figure = Figure.read(self.root, spec.get("base", DEFAULT_POSE))
        figure = figure.swapped(self.root, spec.get("swap", {}), self.order)
        return figure.element(look.theme, look.palette)

    def mascots(self) -> list[ET.Element]:
        table = tomllib.loads((self.explore / "mascot" / "variants.toml").read_text())
        sections = []
        for group in table["group"]:
            cards = []
            for index, item in enumerate(group["item"], 1):
                look = self.look(item.get("look", DEFAULT_LOOK))
                cards.append(card(f"{group['id']}{index}", item["label"], [(self.mascot(item, look), look.backdrop())], item.get("note")))
            sections.append(section(group["id"], group["title"], group["note"], cards))
        return sections

    def colorways(self) -> ET.Element:
        paths = (self.explore / "mascot" / "colorways").glob("*.toml")
        looks = sorted((Look.read(self.tokens, path) for path in paths), key=lambda look: (look.table.get("order", ORDER_LAST), look.theme.name))
        cards = []
        for index, look in enumerate(looks, 1):
            path = look.theme.source
            cards.append(card(f"C{index}", path.stem, [(self.mascot({}, look), look.backdrop())], look.table.get("note")))
        return section("C", "Colorways", "The default pose in every colorway under explore/mascot/colorways/.", cards)


    def themed(self, figure: Figure) -> list[tuple[ET.Element, str]]:
        """`figure` once per adopted theme, each on that theme's background."""
        looks = [Look.of(self.tokens, theme) for theme in self.tokens.themes]
        return [(figure.element(look.theme, look.palette), look.backdrop()) for look in looks]

    def wordmarks(self) -> ET.Element:
        paths = sorted((self.explore / "identity" / "wordmarks" / "parts").glob("*.svg"))
        cards = []
        for index, path in enumerate(paths, 1):
            part = Part.read(path)
            figure = Figure(path.stem, part.view(), [Layer(part, None)], path)
            cards.append(card(f"W{index}", title(path), self.themed(figure)))
        return section("W", "Drawn wordmarks", "Stroke-built letters; the v carries the accent. Light and dark.", cards, "grid wide")

    def marks(self) -> list[ET.Element]:
        paths = sorted((self.explore / "identity" / "marks").glob("*.toml"))
        figures = [Figure.read(self.root, str(path.relative_to(self.root).with_suffix(""))) for path in paths]
        titles = [tomllib.loads(path.read_text())["title"] for path in paths]
        cards = [card(f"M{index}", label, self.themed(figure)) for index, (figure, label) in enumerate(zip(figures, titles), 1)]
        square = [card for card, figure in zip(cards, figures) if figure.view[2] == figure.view[3]]
        wide = [card for card, figure in zip(cards, figures) if figure.view[2] != figure.view[3]]
        strips = []
        for index, figure in enumerate(figures, 1):
            if figure.view[2] != figure.view[3]:
                continue
            panels = []
            for look in (Look.of(self.tokens, theme) for theme in self.tokens.themes):
                arts = [sized(figure.element(look.theme, look.palette), size) for size in ICON_SIZES]
                panels.append((node("div", {"class": "sizes"}, *arts), look.backdrop()))
            strips.append(card(f"Z{index}", ", ".join(f"{size}px" for size in ICON_SIZES), panels))
        return [
            section("M", "Marks", "Logo marks built from the mascot parts.", square),
            node("section", None, node("div", {"class": "grid wide"}, *wide)),
            section("Z", "Small sizes", "Square marks at favicon and avatar sizes: what survives at 16px?", strips, "grid wide"),
        ]

    def type(self) -> ET.Element:
        table = tomllib.loads((self.explore / "identity" / "type.toml").read_text())
        wordmark = Figure.read(self.root, WORDMARK)
        light = Look.of(self.tokens, self.tokens.theme("light"))
        cards = []
        for index, entry in enumerate(table["face"], 1):
            face = Face(entry["family"], ["sans-serif"], [entry["weight"], entry["text"]])
            self.faces.append(face)
            cards.append(
                node(
                    "figure",
                    {"class": "card type", "id": f"T{index}"},
                    node(
                        "div",
                        {"class": "specimen", "style": f"font-family:{face.stack()}"},
                        node("div", {"class": "lockup"}, fluid(wordmark.element(light.theme, light.palette))),
                        node("h3", {"style": f"font-weight:{entry['weight']}"}, table["headline"]),
                        node("p", {"style": f"font-weight:{entry['text']}"}, table["body"]),
                    ),
                    node("figcaption", None, node("span", {"class": "id"}, f"T{index}"), f" {face.family}", node("small", None, entry["note"])),
                )
            )
        return section("T", "Typefaces", "Candidate faces next to the ligature wordmark: headline and body text.", cards, "grid wide")

    def directions(self) -> ET.Element:
        sample = Sample.read(self.root / "identity" / "sample.toml")
        paths = sorted((self.explore / "identity" / "directions").glob("*.toml"))
        cards = [self.direction(f"D{index}", Look.read(self.tokens, path), sample) for index, path in enumerate(paths, 1)]
        return section("D", "Identity directions", "Palette, type and mascot together, as a site header would use them.", cards, "stack")

    def direction(self, id: str, look: Look, sample: Sample) -> ET.Element:
        faces = look.faces(self.tokens.type.faces)
        self.faces += faces.values()
        ui = look.colors(Section.UI)
        mascot = fluid(self.mascot(look.table.get("mascot", {}), look))
        display = faces["display"]
        name = node(
            "div",
            {"class": "name", "style": f"font-family:{display.stack()};font-weight:{max(display.weights)};color:{ui['text']}"},
            node("span", {"style": f"color:{ui['accent']}"}, SPECIMEN[0]),
            SPECIMEN[1:],
        )
        chips = [swatch(color, role, color) for role, color in ui.items()]
        chips += [swatch(color, role, color) for role, color in look.colors(Section.DIAG).items()]
        return node(
            "article",
            {"class": "direction", "id": id, "style": f"background:{ui['bg']};color:{ui['text']};font-family:{faces['text'].stack()}"},
            node("header", None, node("div", {"class": "mark"}, mascot), name),
            node("h3", None, node("span", {"class": "id"}, id), " ", look.table["title"]),
            node("p", {"style": f"color:{ui['muted']}"}, look.table["note"]),
            sample.element(look.colors(Section.CODE), faces["mono"], ui["code-bg"]),
            node("div", {"class": "chips"}, *chips),
        )

    def render(self) -> bytes:
        body = [
            node("header", {"class": "top"}, node("h1", None, "Vun design exploration"), node("p", None, "Reply with ids: \"S3 shell, E2 eyes, C4, D1 palette with T5 type\".")),
            *self.mascots(),
            self.colorways(),
            self.directions(),
            *self.marks(),
            self.wordmarks(),
            self.type(),
        ]
        return page("Vun exploration", STYLE.read_text(), [fonts_link(self.faces)], body)


def title(path: Path) -> str:
    """A file's name without its ordering prefix: `7-claw.svg` is `claw`."""
    order, dash, name = path.stem.partition("-")
    return name if dash and order.isdigit() else path.stem


def gallery(root: Path, tokens: Tokens) -> bytes:
    return Gallery(root, tokens).render()
