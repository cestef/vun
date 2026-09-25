"""The files a build produces, written or checked in one place."""

from dataclasses import dataclass, field
from pathlib import Path

# Suffixes the build owns under out/: anything else there is left alone.
OWNED = {".svg", ".css", ".typ", ".json", ".html"}


@dataclass
class Outputs:
    root: Path
    out: Path
    files: dict[Path, bytes] = field(default_factory=dict)

    def add(self, path: Path, content: bytes | str) -> None:
        self.files[path] = content.encode() if isinstance(content, str) else content

    def stale(self) -> list[Path]:
        return sorted(
            path
            for path in self.out.rglob("*")
            if path.suffix in OWNED and path not in self.files
        )

    def changed(self) -> list[Path]:
        return sorted(
            path
            for path, content in self.files.items()
            if not path.exists() or path.read_bytes() != content
        )

    def write(self) -> list[Path]:
        changed = self.changed()
        for path in changed:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(self.files[path])
        for path in self.stale():
            path.unlink()
        return changed

    def relative(self, paths: list[Path]) -> list[str]:
        return [str(path.relative_to(self.root)) for path in paths]
