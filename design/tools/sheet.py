"""The identity sheet: the adopted palette, themes, type and every figure in every theme."""

from pathlib import Path

from figure import Figure
from look import Look
from pages import Sample, card, fonts_link, node, page, section, swatch

from tokens import Section, Tokens

STYLE = Path(__file__).with_name("gallery.css")


def identity(root: Path, tokens: Tokens, figures: list[Figure]) -> bytes:
    ramps = [
        node("div", {"class": "ramp"}, *(swatch(color, f"{ramp}.{step}", color) for step, color in steps.items()))
        for ramp, steps in tokens.palette.ramps.items()
    ]
    sample = Sample.read(root / "identity" / "sample.toml")
    themes = []
    for theme in tokens.themes:
        look = Look.of(tokens, theme)
        ui = look.colors(Section.UI)
        chips = [
            swatch(color, f"{section.value}.{role}", color)
            for section in (Section.UI, Section.DIAG)
            for role, color in look.colors(section).items()
        ]
        themes.append(
            node(
                "article",
                {"class": "direction", "style": f"background:{ui['bg']};color:{ui['text']}"},
                node("h3", None, theme.name),
                sample.element(look.colors(Section.CODE), tokens.type.faces["mono"], ui["code-bg"]),
                node("div", {"class": "chips"}, *chips),
            )
        )
    faces = [
        node(
            "div",
            {"class": "specimen", "style": f"font-family:{face.stack()};font-weight:{max(face.weights)};font-size:48px;text-align:left"},
            f"{role}: {face.family}",
        )
        for role, face in tokens.type.faces.items()
    ]
    art = [
        card(
            figure.name,
            "",
            [(figure.element(theme, tokens.palette), Look.of(tokens, theme).backdrop()) for theme in tokens.themes],
        )
        for figure in figures
    ]
    body = [
        node("header", {"class": "top"}, node("h1", None, "Vun identity")),
        section("1", "Palette", "tokens/palette.toml", ramps, "stack"),
        section("2", "Themes", "themes/*.toml", themes, "stack"),
        section("3", "Type", "tokens/type.toml", faces, "stack"),
        section("4", "Figures", "every figure in every theme", art),
    ]
    return page("Vun identity", STYLE.read_text(), [fonts_link(list(tokens.type.faces.values()))], body)
