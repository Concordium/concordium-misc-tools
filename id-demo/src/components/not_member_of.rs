use super::statement::StatementProp;
use concordium_base::id::{
    constants::AttributeKind,
    types::{AttributeStringTag, AttributeTag},
};
use std::{collections::BTreeSet, ops::Deref};
use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct NotSetProp {
    pub statement: UseStateHandle<StatementProp>,
}

#[function_component(NotMemberOf)]
pub fn statement(s: &NotSetProp) -> Html {
    let set_state = use_state_eq(|| BTreeSet::<AttributeKind>::new());
    let selected = use_state_eq(|| AttributeTag(0));

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
                        let iter = v.split(',').map(|x| AttributeKind(String::from(x.trim())));
                        let bset: BTreeSet<AttributeKind> = BTreeSet::from_iter(iter);
                        s.set(bset)
                    }
                    Err(_) => (), // do nothing
                }
            }
        })
    };

    let on_click_add = {
        let set = set_state.clone();
        let selected = selected.clone();
        let statements = s.statement.clone();
        move |_: MouseEvent| {
            let new = statements
                .statement
                .clone()
                .not_member_of(*selected, set.deref().clone());
            statements.set(StatementProp { statement: new });
        }
    };

    let on_change = {
        let reveal_state = selected;
        move |e: Event| {
            let target = e.target();
            let elem = target.and_then(|t| t.dyn_into::<HtmlSelectElement>().ok());
            match elem {
                None => (),
                Some(elem) => {
                    let tag = elem.value().parse();
                    match tag {
                        Ok(v) => reveal_state.set(v),
                        Err(e) => web_sys::window()
                            .unwrap()
                            .alert_with_message(&e.to_string())
                            .unwrap(),
                    }
                }
            }
        }
    };

    let current_set = set_state
        .clone()
        .deref()
        .iter()
        .map(|x| x.0.clone())
        .collect::<Vec<String>>()
        .join(",");

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{"Prove attribute not in set"}</label>
            <select class="rounded my-1" onchange={on_change}>
        {(0u8..=253).into_iter().map(|tag| {
                html!{
                    <option selected={tag==0} value={AttributeStringTag::from(AttributeTag(tag)).to_string()}>{AttributeStringTag::from(AttributeTag(tag))} </option>
                }
        }
        ).collect::<Html>()}
        </select><br />
              {"Set: "}<input class="my-1" onchange={on_cautious_change} value={current_set.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}