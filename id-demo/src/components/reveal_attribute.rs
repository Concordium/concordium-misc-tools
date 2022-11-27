use concordium_base::id::{
    self,
    id_proof_types::RevealAttributeStatement,
    types::{AttributeStringTag, AttributeTag},
};
use gloo_console::log;
use wasm_bindgen::JsCast;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use super::statement::StatementProp;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct RevealAttributeProp {
    pub statement: UseStateHandle<StatementProp>,
}

#[function_component(RevealAttribute)]
pub fn statement(s: &RevealAttributeProp) -> Html {
    let selected = use_state_eq(|| AttributeTag(0));
    let on_click_reveal = {
        let selected = selected.clone();
        || {
            let statements = s.statement.clone();
            move |_: MouseEvent| {
                let reveal = RevealAttributeStatement {
                    attribute_tag: *selected,
                };
                let new = statements.push(id::id_proof_types::AtomicStatement::RevealAttribute {
                    statement: reveal,
                });
                log!(serde_json::to_string_pretty(&new.statement).unwrap()); // TODO: Remove logging
                statements.set(new);
            }
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

    html! {
        <form>
            <div class="form-group">
            <label>{"Reveal attribute."} </label>
            <select onchange={on_change}>
        {(0u8..=253).into_iter().map(|tag| {
                html!{
                    <option selected={tag==0} value={AttributeStringTag::from(AttributeTag(tag)).to_string()}>{AttributeStringTag::from(AttributeTag(tag))} </option>
                }
        }
        ).collect::<Html>()}
        </select>
            </div>
            <button onclick={on_click_reveal()}type="button" class="btn btn-primary">{"Add"}</button>
            </form>
    }
}
