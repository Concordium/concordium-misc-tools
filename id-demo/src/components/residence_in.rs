use std::collections::BTreeSet;

use gloo_console::{error, log};
use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use concordium_base::id::{types::{AttributeStringTag, AttributeTag}, constants::AttributeKind};

use super::statement::StatementProp;
use std::ops::Deref;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct ResidenceProp {
    pub statement: UseStateHandle<StatementProp>,
    pub in_set: bool,
}

#[function_component(ResidenceIn)]
pub fn statement(s: &ResidenceProp) -> Html {
    let set_state = use_state_eq(|| BTreeSet::<AttributeKind>::new());

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
                match input.value().parse::<String>() {
                    Ok(v) => {
                        let iter = v.split(',').map(|x| AttributeKind(String::from(x)));
                        let bset: BTreeSet<AttributeKind> = BTreeSet::from_iter(iter);
                        s.set(bset)
                    },
                    Err(_) => (), // do nothing
                }
            }
        })
    };

    let on_click_add = {
        let set = set_state.clone();
        // || {
            let statements = s.statement.clone();
            let in_set = s.in_set.clone();
            move |_: MouseEvent| {
                let new = if in_set {
                        statements.statement.clone().residence_in(set.deref().clone())
                    } else {
                        statements.statement.clone().residence_not_in(set.deref().clone())
                    };
                if let Some(new) = new {
                    log!(serde_json::to_string_pretty(&new).unwrap()); // TODO: Remove logging
                    statements.set(StatementProp { statement: new });
                } else {
                    error!("Cannot construct younger than statement.")
                }
            }
        // }
    };

    let current_set = set_state.clone().deref().iter().map(|x|x.0.clone()).collect::<Vec<String>>().join(",");

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{"Prove residence"}{if !s.in_set {{" not"}} else {""}}{" in set"}</label>
              <input class="my-1" onchange={on_cautious_change} value={current_set.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}
