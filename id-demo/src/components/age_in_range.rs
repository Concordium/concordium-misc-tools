use yew::prelude::*;

use super::statement::StatementProp;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct AgeInRangeProp {
    pub statement: UseStateHandle<StatementProp>,
    pub errors:    UseStateHandle<Vec<String>>,
}

#[function_component(AgeInRange)]
pub fn statement(s: &AgeInRangeProp) -> Html {
    let lower_state = use_state_eq(|| 18u64);
    let upper_state = use_state_eq(|| 30u64);

    let on_lower_change = super::on_change_handler(&lower_state);

    let on_upper_change = super::on_change_handler(&upper_state);

    let on_click_add = {
        let lower = lower_state.clone();
        let upper = upper_state.clone();
        let statements = s.statement.clone();
        let errors = s.errors.clone();
        move |_: MouseEvent| {
            let new = statements.statement.clone().age_in_range(*lower, *upper);
            if let Some(new) = new {
                statements.set(StatementProp { statement: new });
            } else {
                super::append_message(&errors, "Cannot construct age statement.");
            }
        }
    };

    let current_lower = *lower_state;
    let current_upper = *upper_state;

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{"Prove age in range"} </label> <br />
              {"Lower age: "}<input class="my-1" onchange={on_lower_change} value={current_lower.to_string()}/><br />
              {"Upper age: "}<input class="my-1" onchange={on_upper_change} value={current_upper.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}
