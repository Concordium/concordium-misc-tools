use yew::prelude::*;

#[function_component(Loader)]
pub fn loader() -> Html {
    html! {
    <div class="spinner-border" role="status">
        <span class="visually-hidden">{"Loading..."}</span>
    </div>
    }
}
