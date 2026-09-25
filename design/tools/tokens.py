"""Color ramps, themes and type, read from tokens/ and themes/."""

from dataclasses import dataclass, replace
from enum import Enum
from pathlib import Path

import tomllib


class TokenError(Exception):
    def __init__(self, source: Path, reason: str):
        super().__init__(f"{source}: {reason}")
        self.source = source
        self.reason = reason


@dataclass(frozen=True)
class Ref:
    """A `ramp.step` naming one color of the palette."""

    ramp: str
    step: str

    @classmethod
    def parse(cls, source: Path, text: str) -> "Ref":
        ramp, dot, step = text.partition(".")
        if not dot:
            raise TokenError(source, f"`{text}` is not a `ramp.step` reference")
        return cls(ramp, step)


@dataclass(frozen=True)
class Palette:
    ramps: dict[str, dict[str, str]]
    source: Path

    @classmethod
    def read(cls, path: Path) -> "Palette":
        return cls(tomllib.loads(path.read_text()), path)

    def extended(self, ramps: dict[str, dict[str, str]], source: Path) -> "Palette":
        return Palette(self.ramps | ramps, source)

    def color(self, ref: Ref, user: Path) -> str:
        steps = self.ramps.get(ref.ramp)
        if steps is None or ref.step not in steps:
            raise TokenError(user, f"no color `{ref.ramp}.{ref.step}` in {self.source}")
        return steps[ref.step]


class Section(Enum):
    """The role tables of a theme."""

    ART = "art"
    UI = "ui"
    DIAG = "diag"
    CODE = "code"


@dataclass(frozen=True)
class Theme:
    name: str
    roles: dict[Section, dict[str, Ref]]
    source: Path

    @classmethod
    def read(cls, path: Path) -> "Theme":
        return cls.parse(path, tomllib.loads(path.read_text()))

    @classmethod
    def parse(cls, path: Path, table: dict) -> "Theme":
        roles = {
            section: {role: Ref.parse(path, text) for role, text in table.get(section.value, {}).items()}
            for section in Section
        }
        return cls(path.stem, roles, path)

    def over(self, base: "Theme") -> "Theme":
        """This theme's roles laid over `base`'s."""
        roles = {section: base.roles[section] | self.roles[section] for section in Section}
        return replace(self, roles=roles)

    def resolve(self, palette: Palette, section: Section) -> dict[str, str]:
        return {role: palette.color(ref, self.source) for role, ref in self.roles[section].items()}

    def paint(self, palette: Palette, role: str, user: Path) -> str:
        ref = self.roles[Section.ART].get(role)
        if ref is None:
            raise TokenError(user, f"no art role `{role}` in {self.source}")
        return palette.color(ref, self.source)


@dataclass(frozen=True)
class Face:
    family: str
    fallback: list[str]
    weights: list[int]

    def stack(self) -> str:
        return ", ".join([f'"{self.family}"', *self.fallback])


@dataclass(frozen=True)
class Scale:
    base: float
    ratio: float
    steps: list[int]

    def size(self, step: int) -> float:
        return round(self.base * self.ratio**step, 2)


@dataclass(frozen=True)
class Type:
    faces: dict[str, Face]
    scale: Scale

    @classmethod
    def read(cls, path: Path) -> "Type":
        table = tomllib.loads(path.read_text())
        scale = Scale(**table.pop("scale"))
        return cls({role: Face(**face) for role, face in table.items()}, scale)


@dataclass(frozen=True)
class Tokens:
    palette: Palette
    themes: list[Theme]
    type: Type

    @classmethod
    def read(cls, root: Path) -> "Tokens":
        themes = [Theme.read(path) for path in sorted((root / "themes").glob("*.toml"))]
        return cls(
            Palette.read(root / "tokens" / "palette.toml"),
            themes,
            Type.read(root / "tokens" / "type.toml"),
        )

    def theme(self, name: str) -> Theme:
        return next(theme for theme in self.themes if theme.name == name)
