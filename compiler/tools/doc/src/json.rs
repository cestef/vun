//! The documentation as JSON, for the language server and indexes: every namespace with its
//! declarations, as the pages show them.

use vun_grammar::PATH_SEPARATOR;
use vun_json::{Object, Value};

use crate::Docs;
use crate::item::Item;

/// The file the JSON is written to, beside the pages.
pub(crate) const FILE: &str = "docs.json";

impl Docs {
    #[must_use]
    pub fn json(&self) -> Object<'static> {
        let namespaces = self.namespaces.iter().map(|(path, namespace)| {
            Object::new()
                .with("path", path.join(PATH_SEPARATOR))
                .with("file", namespace.file.clone())
                .with("doc", namespace.doc.clone())
                .with("items", namespace.items.iter().map(Item::json).collect::<Vec<_>>())
        });
        Object::new().with("namespaces", namespaces.collect::<Vec<_>>())
    }
}

impl Item {
    fn json(&self) -> Value<'static> {
        Object::new()
            .with("name", self.name.clone())
            .with("kind", self.kind.name())
            .with("signature", self.signature.clone())
            .with("attributes", self.attributes.clone())
            .with("doc", self.doc.clone())
            .with("line", self.line)
            .with("members", self.members.iter().map(Self::json).collect::<Vec<_>>())
            .into()
    }
}
