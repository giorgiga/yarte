use std::{collections::HashSet, iter};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, Expr, Ident};

use yarte_dom::dom::{Each, ExprId, VarId};

use super::{
    component::get_component,
    state::{InsertPath, Len, Parent, State, Step},
    utils::*,
    BlackBox, WASMCodeGen,
};

impl<'a> WASMCodeGen<'a> {
    #[inline]
    pub(super) fn gen_each(
        &mut self,
        id: ExprId,
        Each {
            args,
            body,
            expr,
            var,
        }: Each,
        fragment: bool,
        last: bool,
        insert_point: &[InsertPath],
    ) {
        // Get current state
        let current_bb = self.current_bb();

        // Get bases
        let (key, index) = var;
        let var_id = vec![key];
        let mut var_id_index = vec![key];
        let mut bases = HashSet::new();
        bases.insert(key);
        if let Some(index) = index {
            var_id_index.push(index);
            bases.insert(index);
        }

        // TODO: Expressions in path
        let parent_id = if fragment {
            self.parent_node()
        } else {
            self.stack.last().steps.len()
        };

        // Push
        self.stack.push(State {
            id: Parent::Expr(id),
            bases,
            parent_id,
            current_bb,
            ..Default::default()
        });

        // TODO: component build
        let component = get_component(id, body.iter(), self);
        self.cur_mut().component = Some(component);

        // Do steps
        self.step(body);

        let vdom = get_vdom_ident(id);
        let component_ty = get_component_ty_ident(id);
        let table = get_table_ident(id);
        // TODO: Path to Dom is registered, use old
        let table_dom = get_table_dom_ident(id);

        // Pop
        let mut curr = self.stack.pop();

        // Update state
        let old_on = self.stack.last().id;
        let (base, _) = self.bb_t_root(var_id.into_iter());
        curr.add_t_root(base);

        // TODO: Multiple root
        curr.black_box.push(BlackBox {
            doc: "root dom element".to_string(),
            name: get_field_root_ident(),
            ty: parse2(quote!(yarte_wasm_app::web::Element)).unwrap(),
        });

        // Write component
        self.helpers.extend(curr.get_black_box(&component_ty));
        self.helpers
            .extend(get_drop(&component_ty, iter::once(get_field_root_ident())));

        // TODO
        for (_, path) in curr
            .path_nodes
            .iter_mut()
            .chain(curr.path_events.iter_mut())
        {
            if path.starts_with(&[Step::FirstChild, Step::FirstChild]) {
                // Remove marker
                path.remove(0);
            } else {
                todo!("multi node expressions");
            }
        }

        let current_bb = &curr.current_bb;

        // TODO: remove self
        let build_args: TokenStream = quote!(#args)
            .to_string()
            .replace("self .", "")
            .parse()
            .unwrap();
        let build = Self::build_each(
            &curr,
            build_args,
            &expr,
            &component_ty,
            insert_point,
            &vdom,
            &table,
            &table_dom,
        );

        let parent = match old_on {
            Parent::Expr(id) => {
                let ident = get_vdom_ident(id);
                quote!(#ident)
            }
            Parent::Body | Parent::Head => quote!(#current_bb.#table_dom),
        };
        let (new, cached) = self.new_each(
            &curr,
            curr.component.as_ref().expect("some component"),
            &component_ty,
            last,
            insert_point,
            &vdom,
            quote!(#current_bb.#table_dom),
            Some(parent),
        );
        let render = self.render_each(
            &curr,
            new,
            cached,
            &args,
            &expr,
            fragment,
            &vdom,
            quote!(#current_bb.#table),
            quote!(#current_bb.#table_dom),
            key,
        );
        let (new, cached) = self.new_each(
            &curr,
            curr.component.as_ref().expect("some component"),
            &component_ty,
            last,
            insert_point,
            &vdom,
            quote!(#table_dom),
            None,
        );

        let mut vars = self.solver.expr_inner_var(&id).clone();

        for (i, _) in &curr.buff_render {
            for j in i {
                if !var_id_index.contains(&self.solver.var_base(j)) {
                    vars.insert(*j);
                }
            }
        }

        let last = self.stack.last_mut();
        last.buff_render.push((vars, render));
        last.buff_build.push(build);
        last.buff_new.push(if let Some(cached) = cached {
            quote! {
                let __cached__ = #cached;
                let mut #table: Vec<#component_ty> = vec![];
                for #expr in #args.skip(__dom_len__) {
                    #table.push({ #new });
                }
            }
        } else {
            quote! {
                let mut #table: Vec<#component_ty> = vec![];
                for #expr in #args.skip(__dom_len__) {
                        #table.push({ #new });
                }
            }
        });
        if !curr.path_events.is_empty() {
            let root = get_field_root_ident();
            let steps = get_steps(curr.path_events.iter(), quote!(#vdom.#root));
            let hydrate = curr.buff_hydrate;
            let hydrate = quote! {
                for (#vdom, #expr) in #current_bb.#table
                        .iter_mut()
                        .zip(#args)
                    {
                        #steps
                        #(#hydrate)*
                    }
            };
            last.buff_hydrate.push(hydrate);
        }
        last.path_nodes
            .push((table_dom.clone(), last.steps[..parent_id].to_vec()));
        last.black_box.push(BlackBox {
            doc: "Each Virtual DOM node".to_string(),
            name: table,
            ty: parse2(quote!(Vec<#component_ty>)).unwrap(),
        });
        last.black_box.push(BlackBox {
            doc: "Each DOM Element".to_string(),
            name: table_dom,
            ty: parse2(quote!(yarte_wasm_app::web::Element)).unwrap(),
        });
    }

    fn new_each(
        &self,
        curr: &State,
        component: &Ident,
        component_ty: &Ident,
        last: bool,
        insert_point: &[InsertPath],
        vdom: &Ident,
        table_dom: TokenStream,
        parent: Option<TokenStream>,
    ) -> (TokenStream, Option<TokenStream>) {
        let bb = self.global_bb_ident();
        let tmp = format_ident!("__tmp__");
        let froot = get_field_root_ident();
        let steps = get_steps(
            curr.path_nodes.iter().chain(curr.path_events.iter()),
            quote!(#tmp),
        );
        let fields = curr.get_black_box_fields(&tmp, false);

        let (insert_point, cached) = if last {
            (
                quote!(#table_dom.append_child(&#vdom.#froot).unwrap_throw();),
                None,
            )
        } else {
            let len: Len = insert_point.into();
            let base = len.base as u32 + 1;
            let mut tokens = quote!(#base);
            for i in &len.expr {
                let ident = get_table_ident(*i);
                if let Some(parent) = &parent {
                    tokens.extend(quote!(+ #parent.#ident.len() as u32))
                } else {
                    tokens.extend(quote!(+ #ident.len() as u32))
                }
            }

            (
                quote!(#table_dom.insert_before(&#vdom.#froot, __cached__.as_ref()).unwrap_throw();),
                Some(if parent.is_some() {
                    quote!(#table_dom.children().item(#tokens + __dom_len__ as u32).map(yarte_wasm_app::JsCast::unchecked_into::<yarte_wasm_app::web::Node>))
                } else {
                    quote!(#table_dom.children().item(#tokens).map(yarte_wasm_app::JsCast::unchecked_into::<yarte_wasm_app::web::Node>))
                }),
            )
        };

        let build = &curr.buff_new;
        (
            quote! {
                 let #tmp = yarte_wasm_app::JsCast::unchecked_into::<yarte_wasm_app::web::Element>(self.#bb.#component
                     .clone_node_with_deep(true)
                     .unwrap_throw());
                 #steps
                 #(#build)*
                 let #vdom = #component_ty { #fields };
                 #insert_point
                 #vdom
            },
            cached,
        )
    }

    #[inline]
    fn build_each(
        curr: &State,
        args: TokenStream,
        expr: &Expr,
        component_ty: &Ident,
        insert_point: &[InsertPath],
        vdom: &Ident,
        table: &Ident,
        table_dom: &Ident,
    ) -> TokenStream {
        let froot = get_field_root_ident();
        let steps = get_steps(curr.path_nodes.iter(), quote!(#vdom));
        let fields = curr.get_black_box_fields(vdom, true);
        let build = &curr.buff_build;

        let insert_point = {
            let len: Len = insert_point.into();
            let base = len.base as u32;
            let mut tokens = quote!(#base);
            for i in &len.expr {
                let ident = get_table_ident(*i);
                tokens.extend(quote!(+ #ident.len() as u32))
            }

            quote!(#table_dom.children().item(#tokens).unwrap_throw())
        };

        quote! {
            let mut #table: Vec<#component_ty> = vec![];
            for #expr in #args {
                let #vdom = #table.last().map(|__x__| __x__.#froot.next_element_sibling().unwrap_throw()).unwrap_or_else(|| #insert_point);
                #steps
                #(#build)*
                #table.push(#component_ty { #fields });
            }
        }
    }

    #[inline]
    fn render_each(
        &self,
        curr: &State,
        new: TokenStream,
        cached: Option<TokenStream>,
        args: &Expr,
        expr: &Expr,
        fragment: bool,
        vdom: &Ident,
        table: TokenStream,
        table_dom: TokenStream,
        each_base: VarId,
    ) -> TokenStream {
        let froot = get_field_root_ident();

        // TODO: remove for fragments
        // TODO: remove on drop
        // TODO: remove component method
        let new_block = if let Some(cached) = &cached {
            quote! {
                let __cached__ = #cached;
                for #expr in #args.skip(__dom_len__) {
                    #table.push({ #new });
                }
            }
        } else {
            quote! {
                for #expr in #args.skip(__dom_len__) {
                    #table.push({ #new });
                }
            }
        };
        let render = if curr.buff_render.is_empty() {
            quote!()
        } else {
            // TODO:
            let parents = curr.get_render_hash().into_iter().any(|(i, _)| {
                for j in i {
                    let base = self.solver.var_base(&j);
                    if base != each_base {
                        return true;
                    }
                }
                false
            });

            let render = self.render(curr);
            assert!(!render.is_empty());
            if parents {
                quote! {
                    for (#vdom, #expr) in #table
                        .iter_mut()
                        .zip(#args)
                    {
                        #render
                        #vdom.t_root = yarte_wasm_app::YNumber::zero();
                    }
                }
            } else {
                quote! {
                    for (#vdom, #expr) in #table
                        .iter_mut()
                        .zip(#args)
                        .filter(|(__d__, _)| yarte_wasm_app::YNumber::neq_zero(__d__.t_root))
                        {
                            #render
                            #vdom.t_root = yarte_wasm_app::YNumber::zero();
                        }
                }
            }
        };
        let body = quote! {
            #render
            if __dom_len__ < __data_len__ { #new_block } else {
                #table.drain(__data_len__..);
            }
        };

        // TODO: #[filter] or child is `if`
        let data_len = if true {
            quote!(let __data_len__ = #args.size_hint().0;)
        } else {
            quote!(let __data_len__ = #args.count();)
        };
        if fragment {
            quote! {
                let __dom_len__ = #table.len();
                #data_len
                #body
            }
        } else {
            quote! {
                let __dom_len__ = #table.len();
                #data_len;
                if __data_len__ == 0 {
                    #table_dom.set_text_content(None);
                    #table.clear()
                } else { #body }
            }
        }
    }
}
