use gloo_console::{error, log};
use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement};
use yew::prelude::*;

use super::statement::StatementProp;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct AgeProp {
    pub statement: UseStateHandle<StatementProp>,
    pub younger:   bool,
}

#[function_component(YoungerThan)]
pub fn statement(s: &AgeProp) -> Html {
    let age_state = use_state_eq(|| 18u64);

    let on_cautious_change = {
        let s = age_state.clone();
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
        let age = age_state.clone();
        let statements = s.statement.clone();
        let younger = s.younger;
        move |_: MouseEvent| {
            let new = if younger {
                statements.statement.clone().younger_than(*age)
            } else {
                statements.statement.clone().older_than(*age)
            };
            if let Some(new) = new {
                log!(serde_json::to_string_pretty(&new).unwrap()); // TODO: Remove logging
                statements.set(StatementProp { statement: new });
            } else {
                error!("Cannot construct younger than statement.")
            }
        }
    };

    let current_age = *age_state;

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{if s.younger {"Prove younger than."} else {"Prove older than."}} </label>
              <input class="my-1" onchange={on_cautious_change} value={current_age.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}
