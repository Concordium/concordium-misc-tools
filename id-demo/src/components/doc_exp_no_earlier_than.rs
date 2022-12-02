use gloo_console::{error, log};
use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use concordium_base::id::{types::{AttributeStringTag, AttributeTag}, constants::AttributeKind};

use super::statement::StatementProp;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct DocExpProp {
    pub statement: UseStateHandle<StatementProp>,
}

#[function_component(DocExpNoEarlierThan)]
pub fn statement(s: &DocExpProp) -> Html {
    let state = use_state_eq(|| String::from("20250505"));

    let on_cautious_change = {
        let s = state.clone();
        Callback::from(move |e: Event| {
            // When events are created the target is undefined, it's only
            // when dispatched does the target get added.
            let target: Option<EventTarget> = e.target();
            // Events can bubble so this listener might catch events from child
            // elements which are not of type HtmlInputElement
            let input = target.and_then(|t| t.dyn_into::<HtmlInputElement>().ok());

            if let Some(input) = input {
                match input.value().parse::<String>() {
                    Ok(v) => s.set(v),
                    Err(_) => (), // do nothing
                }
            }
        })
    };

    let on_click_add = {
        let state = state.clone();
        // || {
            let statements = s.statement.clone();
            move |_: MouseEvent| {
                let new = statements.statement.clone().doc_expiry_no_earlier_than(AttributeKind(state.to_string()));
                if let Some(new) = new {
                    log!(serde_json::to_string_pretty(&new).unwrap()); // TODO: Remove logging
                    statements.set(StatementProp { statement: new });
                } else {
                    error!("Cannot construct document expiry statement.")
                }
            }
        // }
    };

    let current_lower = state.to_string();

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{"Prove doc expiry no earlier than"}</label><input class="my-1" onchange={on_cautious_change} value={current_lower.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}
