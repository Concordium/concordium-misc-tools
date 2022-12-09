use concordium_base::id::{
    constants::AttributeKind,
    types::{AttributeStringTag, AttributeTag},
};
use wasm_bindgen::JsCast;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use super::statement::StatementProp;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct RangeProp {
    pub statement: UseStateHandle<StatementProp>,
}

#[function_component(InRange)]
pub fn statement(s: &RangeProp) -> Html {
    let lower_state = use_state_eq(|| String::from("19900505"));
    let upper_state = use_state_eq(|| String::from("20000505"));
    let selected = use_state_eq(|| AttributeTag(0));

    let on_lower_change = super::on_change_handler(&lower_state);

    let on_upper_change = super::on_change_handler(&upper_state);

    let on_click_add = {
        let lower = lower_state.clone();
        let upper = upper_state.clone();
        let selected = selected.clone();
        let statements = s.statement.clone();
        move |_: MouseEvent| {
            let new = statements.statement.clone().in_range(
                *selected,
                AttributeKind(lower.to_string()),
                AttributeKind(upper.to_string()),
            );
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

    let current_lower = lower_state.to_string();
    let current_upper = upper_state.to_string();

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{"Prove attribute in range"}</label>
            <select class="rounded my-1" onchange={on_change}>
        {(0u8..=253).into_iter().map(|tag| {
                html!{
                    <option selected={tag==0} value={AttributeStringTag::from(AttributeTag(tag)).to_string()}>{AttributeStringTag::from(AttributeTag(tag))} </option>
                }
        }
        ).collect::<Html>()}
        </select><br />
              {"Lower: "}<input class="my-1" onchange={on_lower_change} value={current_lower.to_string()}/><br />
              {"Upper: "}<input class="my-1" onchange={on_upper_change} value={current_upper.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}