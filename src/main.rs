#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::def_id::LOCAL_CRATE;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::Queries;
use rustc_interface::interface::{Compiler, Config};
use rustc_hir::intravisit;
use rustc_hir::intravisit::Visitor;
use rustc_span::def_id::LocalModDefId;

mod visitor;

struct CallgraphCallbacks;

impl Callbacks for CallgraphCallbacks {

    fn after_analysis<'tcx>(&mut self, compiler: &Compiler, queries: &'tcx Queries<'tcx>) -> Compilation {
        // let expanded_crate = &compiler.expansion().unwrap().peek().0;
        queries.global_ctxt().unwrap().enter(|tcx| {
            let mut visitor = visitor::CallgraphVisitor::new(&tcx);
            tcx.hir().visit_all_item_likes_in_crate(&mut visitor);

            visitor.dump();
        });

        Compilation::Stop
    }
}

pub fn run(args: Vec<String>) {
    let mut calls = CallgraphCallbacks;

    let run_compiler = rustc_driver::RunCompiler::new(&args, &mut calls);
    run_compiler.run();
}

fn main() {
    let args: Vec<_> = std::env::args().collect();

    run(args);
}