//! The explanation pages of the diagnostics (spec 10, section 1), checked as corpus files. A page
//! has an `Example` section, whose Overt blocks report the page's diagnostic exactly as their
//! `//~` annotations say, and a `Fix` section, whose version of the annotated file reports
//! nothing. A block may name the file it is (```` ```ov build.ov ````), so an example can be a
//! package; a section of command transcripts (`console`) is shown, not checked.

use std::fmt;
use std::path::{Path, PathBuf};

use overt_diag::{DiagId, errors};
use overt_fs::Scratch;
use overt_harness::{Block, Expected, Page};
use overt_span::EXTENSION;

use crate::corpus::{Corpus, Expect, Outcome, Verdict};

/// Where the library lives, relative to the repository root; page paths are relative to it.
const LIBRARY: &str = "lib";
const EXAMPLE: &str = "Example";
const FIX: &str = "Fix";
/// The file of a block that names none.
const UNNAMED: &str = "example";

/// Why a page does not explain its diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unexplained {
    Missing,
    /// No `Example` or no `Fix` section with a block.
    Sections,
    /// The example annotates no file with the page's diagnostic.
    NotShown,
}

impl fmt::Display for Unexplained {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("no page"),
            Self::Sections => write!(f, "a page needs `## {EXAMPLE}` and `## {FIX}` sections with examples"),
            Self::NotShown => f.write_str("no example file annotates the page's diagnostic"),
        }
    }
}

impl Corpus<'_> {
    /// Checks the page of every diagnostic.
    ///
    /// # Errors
    /// Returns an error if a page or a scratch file cannot be read or written.
    pub(crate) fn explanations(&self) -> std::io::Result<Vec<Outcome>> {
        let mut outcomes = Vec::new();
        for (id, page) in errors::EXPLANATIONS {
            let path = Path::new(LIBRARY).join(page);
            for (suffix, verdict) in self.page(id, &path)? {
                let mut shown = path.clone().into_os_string();
                shown.push(suffix);
                outcomes.push(Outcome { path: shown.into(), verdict });
            }
        }
        Ok(outcomes)
    }

    /// The verdicts of the page at `path`, each for a section: `#Example`, `#Fix`.
    fn page(&self, id: &DiagId, path: &Path) -> std::io::Result<Vec<(String, Verdict)>> {
        let Ok(text) = std::fs::read_to_string(self.root.join(path)) else {
            return Ok(vec![(String::new(), Verdict::Unexplained(Unexplained::Missing))]);
        };
        let page = Page::parse(&text);
        let (example, fix) = (page.section(EXAMPLE), page.section(FIX));
        if example.is_empty() || fix.is_empty() {
            return Ok(vec![(String::new(), Verdict::Unexplained(Unexplained::Sections))]);
        }
        let sources = |blocks: &[Block]| -> Vec<(String, String)> {
            let source = blocks.iter().filter(|b| b.lang == EXTENSION);
            source
                .map(|b| (b.file.clone().unwrap_or_else(|| format!("{UNNAMED}.{EXTENSION}")), b.text.clone()))
                .collect()
        };
        let (example, fix) = (sources(example), sources(fix));
        if example.is_empty() {
            return Ok(vec![(String::new(), Verdict::Shown)]);
        }
        let relative = id.to_string();
        let annotated = example.iter().find(|(_, text)| {
            Expected::all(text).is_ok_and(|expected| expected.iter().any(|e| e.at.id == relative))
        });
        let Some((checked, _)) = annotated.cloned() else {
            return Ok(vec![(String::new(), Verdict::Unexplained(Unexplained::NotShown))]);
        };
        let mut verdicts = Vec::new();
        for (section, files, expect) in [(EXAMPLE, &example, Expect::Annotated), (FIX, &fix, Expect::Clean)] {
            let scratch = Scratch::new(&format!("{relative}-{section}"));
            for (file, text) in files {
                scratch.write(file, text);
            }
            let target: PathBuf = scratch.join(&checked);
            let verdict = if target.exists() { Verdict::of(&target, expect)? } else { Verdict::Shown };
            verdicts.push((format!("#{section}"), verdict));
        }
        Ok(verdicts)
    }
}
