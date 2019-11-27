#![feature(rustc_private)]

extern crate rustc;
extern crate rustc_driver;
extern crate rustc_interface;
extern crate syntax;

use rustc::hir::def_id::LOCAL_CRATE;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use syntax::visit;

mod visitor;

struct CallgraphCallbacks;

impl Callbacks for CallgraphCallbacks {
    fn config(&mut self, _config: &mut Config) {
        _config.opts.debugging_opts.save_analysis = true;
    }

    fn after_analysis(&mut self, compiler: &Compiler) -> Compilation {
        let expanded_crate = &compiler.expansion().unwrap().peek().0;
        compiler.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            if tcx.analysis(LOCAL_CRATE).is_err() {
                return;
            }
            let mut visitor = visitor::CallgraphVisitor::new(&tcx);
            // This actually does the walking.
            visit::walk_crate(&mut visitor, &expanded_crate);

            visitor.dump();
        });

        Compilation::Stop
    }
}

pub fn run(args: Vec<String>) {
    let mut calls = CallgraphCallbacks;

    rustc_driver::run_compiler(&args, &mut calls, None, None).unwrap();
}

fn main() {
    let args: Vec<_> = std::env::args().collect();

    run(args);
}
