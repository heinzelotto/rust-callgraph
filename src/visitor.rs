use rustc::hir;
use rustc::hir::def_id::DefId;
use rustc::ty;
use rustc::ty::TyCtxt;
use std::collections::HashSet;
use syntax::{ast, visit};

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


#[derive(Hash, PartialEq, Eq, Debug)]
struct StaticCall {
    // the call expression
    call_expr: hir::HirId,
    // possible enclosing function
    caller: Option<DefId>,
    // call target
    callee: DefId,
}

pub struct CallgraphVisitor<'tcx> {
    // type context
    tcx: TyCtxt<'tcx>,

    // free functions
    functions: HashSet<DefId>,
    // trait method declarations without default implementation
    method_decls: HashSet<DefId>,
    //
    //method_impls: HashSet<DefId>,

    // static calls
    static_calls: HashSet<StaticCall>,

    // tracks the current function we're in during AST walk
    cur_fn: Option<DefId>,
}

impl<'tcx> CallgraphVisitor<'tcx> {
    pub fn new(tcx: &TyCtxt<'tcx>) -> CallgraphVisitor<'tcx> {
        CallgraphVisitor {
            tcx: *tcx,
            functions: HashSet::new(),
            method_decls: HashSet::new(),
            static_calls: HashSet::new(),
            cur_fn: None,
        }
    }

    pub fn dump(&self) {
        dbg!(&self.functions);
        dbg!(&self.method_decls);
        dbg!(&self.static_calls);
    }
}

impl<'v, 'tcx> visit::Visitor<'v> for CallgraphVisitor<'tcx> {
    fn visit_expr(&mut self, expr: &'v ast::Expr) {
        skip_generated_code!(expr.span);

        let hir_id = self.tcx.hir().node_to_hir_id(expr.id);
        let hir_node = self.tcx.hir().find(hir_id);
        if let Some(hir::Node::Expr(hir_expr)) = hir_node {
            match hir_expr.kind {
                hir::ExprKind::Path(ref qpath) => {
                    if let hir::QPath::Resolved(_, p) = qpath {
                        if let hir::def::Res::Def(_, def_id) = p.res {
                            self.static_calls.insert(StaticCall {
                                call_expr: hir_id,
                                caller: self.cur_fn,
                                callee: def_id,
                            });
                        }
                    }
                }
                hir::ExprKind::MethodCall(_, _, _) => {
                    let o_def_id = hir_id.owner_def_id();
                    let typeck_tables = self.tcx.typeck_tables_of(o_def_id);
                    let substs = typeck_tables.node_substs(hir_id);
                    let method_id = typeck_tables.type_dependent_def_id(hir_id).expect("fail");
                    let param_env = self.tcx.param_env(method_id);
                    if let Some(inst) =
                        ty::Instance::resolve(self.tcx, param_env, method_id, substs)
                    {
                        let res_def_id = inst.def_id();
                        self.static_calls.insert(StaticCall {
                            call_expr: hir_id,
                            caller: self.cur_fn,
                            callee: res_def_id,
                        });
                    }
                    // TODO also log dynamically dispatched calls
                }
                _ => {}
            }
        }

        // traverse
        visit::walk_expr(self, expr);
    }

    fn visit_item(&mut self, item: &'v ast::Item) {
        skip_generated_code!(item.span);

        let hir_id = self.tcx.hir().node_to_hir_id(item.id);
        // find returns None for macros that don't appear in HIR
        if let Some(hir::Node::Item(hir_item)) = self.tcx.hir().find(hir_id) {
            if let hir::ItemKind::Fn(_, _, _, _) = hir_item.kind {
                let def_id = self.tcx.hir().local_def_id(hir_id);
                self.functions.insert(def_id);

                push_walk_pop!(self, def_id, visit::walk_item(self, item));

                return;
            }
        }

        // traverse
        visit::walk_item(self, item)
    }

    fn visit_trait_item(&mut self, ti: &'v ast::TraitItem) {
        skip_generated_code!(ti.span); // TODO ?do we want this

        let hir_id = self.tcx.hir().node_to_hir_id(ti.id);
        let def_id = self.tcx.hir().local_def_id(hir_id);

        match ti.kind {
            ast::TraitItemKind::Method(_, None) => {
                self.method_decls.insert(def_id);
            }
            ast::TraitItemKind::Method(_, Some(_)) => {
                self.method_decls.insert(def_id);
                // TODO this is also a method impl

                push_walk_pop!(self, def_id, visit::walk_trait_item(self, ti));

                return;
            }
            _ => {}
        }

        // traverse
        visit::walk_trait_item(self, ti)
    }

    fn visit_impl_item(&mut self, ii: &'v ast::ImplItem) {
        skip_generated_code!(ii.span);

        let hir_id = self.tcx.hir().node_to_hir_id(ii.id);
        let def_id = self.tcx.hir().local_def_id(hir_id);

        if let ast::ImplItemKind::Method(..) = ii.kind {
            self.functions.insert(def_id);
            // TODO: store link to decl

            push_walk_pop!(self, def_id, visit::walk_impl_item(self, ii));

            return;
        }

        // traverse
        visit::walk_impl_item(self, ii)
    }
}
