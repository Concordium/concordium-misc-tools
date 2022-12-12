use super::statement::StatementProp;
use concordium_base::id::constants::AttributeKind;
use std::{collections::BTreeSet, ops::Deref};
use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement};
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct DocumentInProp {
    pub statement: UseStateHandle<StatementProp>,
    pub in_set:    bool,
    pub errors:    UseStateHandle<Vec<String>>,
}

#[function_component(DocumentIssuerIn)]
pub fn statement(s: &DocumentInProp) -> Html {
    let set_state = use_state_eq(BTreeSet::<AttributeKind>::new);

    let on_cautious_change = {
        let s = set_state.clone();
        Callback::from(move |e: Event| {
            // When events are created the target is undefined, it's only
            // when dispatched does the target get added.
            let target: Option<EventTarget> = e.target();
            // Events can bubble so this listener might catch events from child
            // elements which are not of type HtmlInputElement
            let input = target.and_then(|t| t.dyn_into::<HtmlInputElement>().ok());

            if let Some(input) = input {
                if let Ok(v) = input.value().parse::<String>() {
                    let iter = v.split(',').map(|x| AttributeKind(String::from(x.trim())));
                    let bset: BTreeSet<AttributeKind> = BTreeSet::from_iter(iter);
                    s.set(bset)
                } else {
                    // do nothing
                }
            }
        })
    };

    let on_click_add = {
        let set = set_state.clone();
        let statements = s.statement.clone();
        let in_set = s.in_set;
        let errors = s.errors.clone();
        move |_: MouseEvent| {
            let new = if in_set {
                statements
                    .statement
                    .clone()
                    .document_issuer_in(set.deref().clone())
            } else {
                statements
                    .statement
                    .clone()
                    .document_issuer_not_in(set.deref().clone())
            };
            if let Some(new) = new {
                statements.set(StatementProp { statement: new });
            } else {
                super::append_message(&errors, "Cannot construct document issuer statement.");
            }
        }
    };

    let current_set = set_state
        .deref()
        .iter()
        .map(|x| x.0.clone())
        .collect::<Vec<String>>()
        .join(",");

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{"Prove document issuer"}{if !s.in_set {{" not"}} else {""}}{" in set"}</label>
              <input class="my-1" onchange={on_cautious_change} value={current_set.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}
