use concordium_base::id::constants::AttributeKind;
use yew::prelude::*;

use super::statement::StatementProp;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct DocExpProp {
    pub statement: UseStateHandle<StatementProp>,
    pub errors:    UseStateHandle<Vec<String>>,
}

#[function_component(DocExpNoEarlierThan)]
pub fn statement(s: &DocExpProp) -> Html {
    let state = use_state_eq(|| String::from("20250505"));

    let on_cautious_change = super::on_change_handler(&state);

    let on_click_add = {
        let state = state.clone();
        let statements = s.statement.clone();
        let errors = s.errors.clone();
        move |_: MouseEvent| {
            let new = statements
                .statement
                .clone()
                .doc_expiry_no_earlier_than(AttributeKind(state.to_string()));
            if let Some(new) = new {
                statements.set(StatementProp { statement: new });
            } else {
                super::append_message(&errors, "Cannot construct document expiry statement.");
            }
        }
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
