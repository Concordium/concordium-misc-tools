use yew::prelude::*;

use super::statement::StatementProp;

#[derive(Properties, PartialEq, Clone, Debug)]
pub struct AgeProp {
    pub statement: UseStateHandle<StatementProp>,
    pub younger:   bool,
    pub errors:    UseStateHandle<Vec<String>>,
}

#[function_component(YoungerThan)]
pub fn statement(s: &AgeProp) -> Html {
    let age_state = use_state_eq(|| 18u64);

    let on_age_change = super::on_change_handler(&age_state);

    let on_click_add = {
        let age = age_state.clone();
        let statements = s.statement.clone();
        let younger = s.younger;
        let errors = s.errors.clone();
        move |_: MouseEvent| {
            let new = if younger {
                statements.statement.clone().younger_than(*age)
            } else {
                statements.statement.clone().older_than(*age)
            };
            if let Some(new) = new {
                statements.set(StatementProp { statement: new });
            } else {
                super::append_message(&errors, "Cannot construct younger than statement.");
            }
        }
    };

    let current_age = *age_state;

    html! {
        <form>
            <div class="form-group border rounded border-primary my-2">
            <label>{if s.younger {"Prove younger than."} else {"Prove older than."}} </label>
              <input class="my-1" onchange={on_age_change} value={current_age.to_string()}/>
            <button onclick={on_click_add} type="button" class="btn btn-primary">{"Add"}</button>
            </div>
            </form>
    }
}
