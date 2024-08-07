use rustc_hir::{HirId, Node};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_middle::hir::map::Map;
use rustc_middle::ty::TypeckResults;
use rustc_middle::ty::ParamEnvAnd;
use std::collections::{HashMap, HashSet};
use rustc_hir::intravisit;
use rustc_middle::hir::nested_filter;
use rustc_span::Span;

macro_rules! skip_generated_code {
    ($span: expr) => {
        if $span.from_expansion() || $span.is_dummy() {
            return;
        }
    };
}

// Backup self.cur_fn, set cur_fn to id, continue to walk the AST by executing
// $walk, then restore self.cur_fn.
macro_rules! push_walk_pop {
    ($this: expr, $id: expr, $walk: expr) => {{
        let prev_fn = $this.cur_fn;
        $this.cur_fn = Some($id);
        $walk;
        $this.cur_fn = prev_fn;
    }};
}

pub fn print_span<'tcx>(tcx: TyCtxt<'tcx>, span: &Span) {
    let source_map = tcx.sess.source_map();
    println!(
        "```rust\n{}\n```\n",
        // source_map.span_to_diagnostic_string(span.clone()),
        source_map.span_to_snippet(span.clone()).unwrap()
    );
}

#[derive(Hash, PartialEq, Eq, Debug)]
struct Call {
    // the call expression
    call_expr: HirId,
    call_expr_span: Span,
    // possible enclosing function
    caller: Option<DefId>,
    caller_span: Option<Span>,
    // call target
    callee: DefId,
    callee_span: Span,
}

pub struct CallgraphVisitor<'tcx> {
    // type context
    tcx: TyCtxt<'tcx>,

    // free functions
    functions: HashSet<(DefId, Span)>,
    // trait method declarations without default implementation
    method_decls: HashSet<DefId>,
    // map decls to impls
    method_impls: HashMap<DefId, Vec<DefId>>,

    // static calls
    static_calls: HashSet<Call>,
    // dynamic calls
    dynamic_calls: HashSet<Call>,

    // tracks the current function we're in during AST walk
    cur_fn: Option<DefId>,
}

impl<'tcx> CallgraphVisitor<'tcx> {
    pub fn new(tcx: &TyCtxt<'tcx>) -> CallgraphVisitor<'tcx> {
        CallgraphVisitor {
            tcx: *tcx,
            functions: HashSet::new(),
            method_decls: HashSet::new(),
            method_impls: HashMap::new(),
            static_calls: HashSet::new(),
            dynamic_calls: HashSet::new(),
            cur_fn: None,
        }
    }

    pub fn dump(&self) {
        dbg!(&self.functions);
        dbg!(&self.method_decls);
        dbg!(&self.method_impls);
        dbg!(&self.static_calls);
        dbg!(&self.dynamic_calls);
    }
}

impl<'tcx> intravisit::Visitor<'tcx> for CallgraphVisitor<'tcx> {

    type NestedFilter = nested_filter::OnlyBodies;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr) {
        skip_generated_code!(expr.span);

        let hir_id = expr.hir_id;
        match expr.kind {
            rustc_hir::ExprKind::Path(ref qpath) => {
                if let rustc_hir::QPath::Resolved(_, p) = qpath {
                    if let rustc_hir::def::Res::Def(_, def_id) = p.res {
                        self.static_calls.insert(Call {
                            call_expr: hir_id,
                            call_expr_span: expr.span,
                            caller: self.cur_fn,
                            caller_span: None,
                            callee: def_id,
                            callee_span: p.span,
                        });
                    }
                }
            }
            rustc_hir::ExprKind::MethodCall(_, _, _, _) => {
                let o_def_id = hir_id.owner;
                let typeck_tables = self.tcx.typeck(o_def_id);
                let substs = typeck_tables.node_args(hir_id);
                let method_id = typeck_tables.type_dependent_def_id(hir_id).expect("fail");
                let param_env = self.tcx.param_env(method_id);
                if let Ok(Some(inst)) =
                    self.tcx.resolve_instance_raw(ParamEnvAnd{param_env, value: (method_id, substs)})
                {
                    let res_def_id = inst.def_id();
                    match self.tcx.hir().get_if_local(res_def_id) {
                        Some(rustc_hir::Node::TraitItem(rustc_hir::TraitItem{span, ..})) => {
                            // dynamic calls resolve only to the trait method decl
                            self.dynamic_calls.insert(Call {
                                call_expr: hir_id,
                                call_expr_span: expr.span,
                                caller: self.cur_fn,
                                caller_span: None,
                                callee: res_def_id,
                                callee_span: *span,
                            });
                        }
                        Some(rustc_hir::Node::ImplItem(rustc_hir::ImplItem{span, ..})) |
                                Some(rustc_hir::Node::Item(rustc_hir::Item{span, ..})) |
                                Some(rustc_hir::Node::ForeignItem(rustc_hir::ForeignItem{span, ..})) => {
                            // calls for which the receiver's type can be resolved
                            self.static_calls.insert(Call {
                                call_expr: hir_id,
                                call_expr_span: expr.span,
                                caller: self.cur_fn,
                                caller_span: None,
                                callee: res_def_id,
                                callee_span: *span,
                            });
                        },
                        None => (),
                        _ => todo!()
                    };
                }
            }
            _ => {}
        }
        // traverse
        intravisit::walk_expr(self, expr);
    }

    fn visit_item(&mut self, item: &'tcx rustc_hir::Item) {
        skip_generated_code!(item.span);

        let hir_id = item.hir_id();
        if let rustc_hir::ItemKind::Fn(_, _, _) = item.kind {
            let def_id = hir_id.owner.to_def_id();
            self.functions.insert((def_id, item.span));

            push_walk_pop!(self, def_id, intravisit::walk_item(self, item));

            return;
        }
        // traverse
        intravisit::walk_item(self, item)
    }

    fn visit_trait_item(&mut self, ti: &'tcx rustc_hir::TraitItem) {
        skip_generated_code!(ti.span); // TODO ?do we want this

        let hir_id = ti.hir_id();
        let def_id = hir_id.owner.to_def_id();

        match ti.kind {
            rustc_hir::TraitItemKind::Fn(_, rustc_hir::TraitFn::Required(_)) => {
                // a method declaration
                self.method_decls.insert(def_id);
                self.method_impls.insert(def_id, vec![]);
            }
            rustc_hir::TraitItemKind::Fn(_, rustc_hir::TraitFn::Provided(_)) => {
                // a method decl and def
                self.method_decls.insert(def_id);
                self.functions.insert((def_id, ti.span));
                self.method_impls.entry(def_id).or_default().push(def_id);

                push_walk_pop!(self, def_id, intravisit::walk_trait_item(self, ti));

                return;
            }
            _ => {}
        }

        // traverse
        intravisit::walk_trait_item(self, ti)
    }

    // self.tcx.hir().hir_to_pretty_string(ty.hir_id)

    fn visit_impl_item(&mut self, ii: &'tcx rustc_hir::ImplItem) {
        skip_generated_code!(ii.span);

        let hir_id = ii.hir_id();
        let def_id = hir_id.owner.to_def_id();

        if let rustc_hir::ImplItemKind::Fn(..) = ii.kind {
            self.functions.insert((def_id, ii.span));

            // store link to decl
            let mut decl_id = None;
            if let Some(impl_id) = self.tcx.impl_of_method(def_id) {
                if let Some(rustc_hir::Node::Item(item)) = self.tcx.hir().get_if_local(impl_id) {
                    if let rustc_hir::ItemKind::Impl(..) = item.kind {
                        // the next one filters methods that are just associated
                        // and do not belong to a struct
                        if let Some(trait_def_id) = self.tcx.trait_id_of_impl(impl_id) {
                            let item = self.tcx
                                .associated_items(trait_def_id)
                                .filter_by_name_unhygienic(ii.ident.name)
                                .next(); // There should ideally be only one item matching the name
                            if let Some(item) = item {
                                decl_id = Some(item.def_id);
                            };
                        }
                    }
                }
            }

            if let Some(decl_def_id) = decl_id {
                self.method_impls
                    .entry(decl_def_id)
                    .or_default()
                    .push(def_id);
            }

            push_walk_pop!(self, def_id, intravisit::walk_impl_item(self, ii));

            return;
        }

        // traverse
        intravisit::walk_impl_item(self, ii)
    }
}
