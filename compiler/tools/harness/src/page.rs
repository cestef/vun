//! Markdown pages with code examples: sections by `##` heading, each holding fenced blocks tagged
//! with a language and optionally the file the block is: ```` ```ov build.ov ````.

/// Opens and closes a fenced block.
const FENCE: &str = "```";
/// Starts a section.
const HEADING: &str = "## ";

/// A fenced block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub lang: String,
    /// The file the block is, when its fence names one.
    pub file: Option<String>,
    pub text: String,
}

/// A page's sections, by heading, with their blocks in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub sections: Vec<(String, Vec<Block>)>,
}

impl Page {
    #[must_use]
    pub fn parse(markdown: &str) -> Self {
        let mut page = Self::default();
        let mut open: Option<(String, Option<String>, Vec<&str>)> = None;
        for line in markdown.lines() {
            let fence = line.trim_start().strip_prefix(FENCE);
            match (&mut open, fence) {
                (None, Some(info)) => {
                    let mut words = info.split_whitespace();
                    let lang = words.next().unwrap_or_default().to_owned();
                    open = Some((lang, words.next().map(str::to_owned), Vec::new()));
                }
                (Some((lang, file, lines)), Some(_)) => {
                    let text = lines.iter().fold(String::new(), |mut text, l| {
                        text.push_str(l);
                        text.push('\n');
                        text
                    });
                    let block = Block { lang: std::mem::take(lang), file: file.take(), text };
                    if let Some((_, blocks)) = page.sections.last_mut() {
                        blocks.push(block);
                    }
                    open = None;
                }
                (Some((_, _, lines)), None) => lines.push(line),
                (None, None) => {
                    if let Some(heading) = line.strip_prefix(HEADING) {
                        page.sections.push((heading.trim().to_owned(), Vec::new()));
                    }
                }
            }
        }
        page
    }

    /// The blocks of the section `heading`.
    #[must_use]
    pub fn section(&self, heading: &str) -> &[Block] {
        self.sections.iter().find(|(h, _)| h == heading).map_or(&[], |(_, blocks)| blocks)
    }
}
