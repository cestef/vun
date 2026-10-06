//! The documenter of Vun packages as a component's service: the site of HTML pages and JSON.

use vun_component::Cause;
use vun_documenter::{Documenter, Site, Source, Types};

use crate::Docs;

/// The documenter that writes a page per namespace, an index, a stylesheet and the JSON.
pub struct Html;

impl Documenter for Html {
    fn name(&self) -> &'static str {
        "html"
    }

    fn document(&self, sources: &[Source], types: &dyn Types) -> Result<Site, Cause> {
        let mut docs = Docs::default();
        for Source { path, file, text } in sources {
            docs.add(path.clone(), file.clone(), text);
        }
        for (path, namespace) in &mut docs.namespaces {
            namespace.retype(|name| types.is_type(path, name));
        }
        Ok(Site { namespaces: docs.namespaces.len(), files: docs.site() })
    }
}
