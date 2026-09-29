//! The variants a document declares, as `#metadata(..) <variants>`: an array of
//! `(path: (..), inputs: (..))`, read with `typst eval`.

use std::collections::BTreeMap;
use std::path::Path;

use vun_data::FromData;
use vun_tool::{Arg, Executable, Invocation, Tool};

use super::ROOT;
use super::error::Unread;

#[derive(FromData)]
pub(super) struct Variant {
    /// Where its output goes in the document's directory: a directory per part but the last.
    pub(super) path: Vec<String>,
    /// The values it gives `sys.inputs`.
    pub(super) inputs: BTreeMap<String, String>,
}

const EVAL: &str = "eval";
const IN: &str = "--in";
const QUERY: &str = "query(<variants>).first().value";

impl Variant {
    /// The variants `source` declares, relative to `root`.
    pub(super) fn read(root: &Path, source: &str, typst: &Executable) -> Result<Vec<Self>, Unread> {
        let source = root.join(source);
        let args = vec![
            EVAL.into(),
            QUERY.into(),
            ROOT.into(),
            Arg::Path(root.to_owned()),
            IN.into(),
            Arg::Path(source.clone()),
        ];
        let inv = Invocation { args, inputs: vec![source], ..Invocation::default() };
        let out = typst.check(&inv).map_err(Unread::Typst)?;
        let text = std::str::from_utf8(&out.stdout).map_err(Unread::Encoding)?;
        let value = vun_data::json::parse(text).map_err(Unread::Json)?;
        Vec::from_data(&value).map_err(Unread::Shape)
    }
}
