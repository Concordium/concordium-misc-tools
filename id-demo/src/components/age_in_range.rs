use gloo_console::{error, log};
use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement};
use yew::prelude::*;

use super::statement::StatementProp;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct AgeInRangeProp {
    pub statement: UseStateHandle<StatementProp>,
}

#[function_component(AgeInRange)]
pub fn statement(s: &AgeInRangeProp) -> Html {
    let lower_state = use_state_eq(|| 18u64);
    let upper_state = use_state_eq(|| 30u64);

    let on_cautious_change = {
        let s = lower_state.clone();
        Callback::from(move |e: Event| {
            // When events are created the target is undefined, it's only
            // when dispatched does the target get added.
            let target: Option<EventTarget> = e.target();
            // Events can bubble so this listener might catch events from child
            // elements which are not of type HtmlInputElement
            let input = target.and_then(|t| t.dyn_into::<HtmlInputElement>().ok());

            if let Some(input) = input {
                match input.value().parse::<u64>() {
                    Ok(v) => s.set(v),
                    Err(_) => (), // do nothing
                }
            }
        })
    };

    let on_cautious_change2 = {
        let s = upper_state.clone();
        Callback::from(move |e: Event| {
            // When events are created the target is undefined, it's only
            // when dispatched does the target get added.
            let target: Option<EventTarget> = e.target();
            // Events can bubble so this listener might catch events from child
            // elements which are not of type HtmlInputElement
            let input = target.and_then(|t| t.dyn_into::<HtmlInputElement>().ok());

            if let Some(input) = input {
                match input.value().parse::<u64>() {
                    Ok(v) => s.set(v),
                    Err(_) => (), // do nothing
                }
            }
        })
    };

    let on_click_add = {
        let lower = lower_state.clone();
        let upper = upper_state.clone();
        let statements = s.statement.clone();
        move |_: MouseEvent| {
            let new = statements.statement.clone().age_in_range(*lower, *upper);
            if let Some(new) = new {
                log!(serde_json::to_string_pretty(&new).unwrap()); // TODO: Remove logging
                statements.set(StatementProp { statement: new });
            } else {
                error!("Cannot construct younger than statement.")
            }
        }
    };

    let current_lower = *lower_state;
    let current_upper = *upper_state;

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{"Prove age in range"} </label> <br />
              {"Lower age: "}<input class="my-1" onchange={on_cautious_change} value={current_lower.to_string()}/><br />
              {"Upper age: "}<input class="my-1" onchange={on_cautious_change2} value={current_upper.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}
