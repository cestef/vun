"""A look: a theme overlaid by a file that may bring its own ramps, as used by explorations."""

from dataclasses import dataclass
from pathlib import Path

import tomllib

from tokens import Face, Palette, Ref, Section, Theme, Tokens

BASE_THEME = "light"


@dataclass(frozen=True)
class Look:
    palette: Palette
    theme: Theme
    table: dict

    @classmethod
    def read(cls, tokens: Tokens, path: Path) -> "Look":
        table = tomllib.loads(path.read_text())
        palette = tokens.palette.extended(table.get("ramps", {}), path)
        theme = Theme.parse(path, table).over(tokens.theme(table.get("base", BASE_THEME)))
        return cls(palette, theme, table)

    @classmethod
    def of(cls, tokens: Tokens, theme: Theme) -> "Look":
        return cls(tokens.palette, theme, {})

    def colors(self, section: Section) -> dict[str, str]:
        return self.theme.resolve(self.palette, section)

    def backdrop(self) -> str:
        if "backdrop" in self.table:
            return self.palette.color(Ref.parse(self.theme.source, self.table["backdrop"]), self.theme.source)
        return self.colors(Section.UI)["bg"]

    def faces(self, fallback: dict[str, Face]) -> dict[str, Face]:
        return fallback | {role: Face(**face) for role, face in self.table.get("type", {}).items()}
