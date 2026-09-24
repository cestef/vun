use super::*;
use vun_diag::DiagCtxt;
use vun_origin::OriginTable;

struct Named(&'static str, &'static [&'static str]);

impl Pass<Vec<&'static str>> for Named {
    fn id(&self) -> &'static str {
        self.0
    }
    fn requires(&self) -> &'static [&'static str] {
        self.1
    }
    fn run(&self, ir: &mut Vec<&'static str>, _: &mut PassCx<'_>) -> PassResult {
        ir.push(self.0);
        PassResult::Changed
    }
}

#[test]
fn requirements_run_first() {
    let mut pm = PassManager::new();
    pm.register(Box::new(Named("borrows", &["places"])));
    pm.register(Box::new(Named("places", &[])));
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut log = Vec::new();
    let report = DiagCx { diag: &mut diag, origins: &origins };
    pm.run(&mut log, &mut PassCx { report }).unwrap();
    assert_eq!(log, ["places", "borrows"]);
}

#[test]
fn cycles_are_reported() {
    let mut pm: PassManager<Vec<&'static str>> = PassManager::new();
    pm.register(Box::new(Named("a", &["b"])));
    pm.register(Box::new(Named("b", &["a"])));
    assert!(matches!(pm.order(), Err(OrderError::Cycle(_))));
}
