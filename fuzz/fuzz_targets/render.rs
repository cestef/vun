//! Every diagnostic the front end reports on any text renders in every format: spans on any
//! bytes, tabs and line ends never break a renderer.

#![no_main]

use std::io;

use libfuzzer_sys::fuzz_target;
use vun_diag::{Color, DiagCtxt, DiagCx, DiagSink};
use vun_hir::lower::Source;
use vun_hir::tree::Hir;
use vun_origin::OriginTable;
use vun_render_annotate::AnnotateRenderer;
use vun_render_ariadne::AriadneRenderer;
use vun_render_json::JsonRenderer;
use vun_render_sarif::Sarif;
use vun_render_short::ShortRenderer;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

fuzz_target!(|text: &str| {
    let sources = SourceFiles::new();
    let file = sources.add("fuzz.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    Hir::lower(Source { cst: &cst, tokens: &tokens, text }, &origins).resolve(&mut cx);
    let sarif = Sarif::default();
    let mut sinks: [Box<dyn DiagSink>; 5] = [
        Box::new(AnnotateRenderer::new(io::sink(), Color::Always)),
        Box::new(AriadneRenderer::new(io::sink(), Color::Always)),
        Box::new(ShortRenderer::new(io::sink(), Color::Always)),
        Box::new(JsonRenderer::new(io::sink())),
        Box::new(sarif.clone()),
    ];
    for d in diag.sorted(&origins) {
        for sink in &mut sinks {
            sink.emit(d, &origins, &sources);
        }
    }
    sarif.write(&mut io::sink()).unwrap();
});
