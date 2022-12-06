use std::str::FromStr;

use wasm_bindgen::JsCast;
use web_sys::{Event, EventTarget, HtmlInputElement};
use yew::{Callback, UseStateHandle};

pub(crate) mod age_in_range;
pub(crate) mod doc_exp_no_earlier_than;
pub(crate) mod document_issuer_in;
pub(crate) mod header;
pub(crate) mod in_range;
pub(crate) mod loader;
pub(crate) mod member_of;
pub(crate) mod nationality_in;
pub(crate) mod residence_in;
pub(crate) mod reveal_attribute;
pub(crate) mod statement;
pub(crate) mod younger_than;

pub(crate) fn on_change_handler<A: FromStr + 'static>(
    s: &UseStateHandle<A>,
) -> Callback<Event, ()> {
    let s = s.clone();
    Callback::from(move |e: Event| {
        // When events are created the target is undefined, it's only
        // when dispatched does the target get added.
        let target: Option<EventTarget> = e.target();
        // Events can bubble so this listener might catch events from child
        // elements which are not of type HtmlInputElement
        let input = target.and_then(|t| t.dyn_into::<HtmlInputElement>().ok());

        if let Some(input) = input {
            if let Ok(v) = input.value().parse::<A>() {
                s.set(v)
            } else {
                // do nothing
            }
        }
    })
}

pub(crate) fn append_message(messages: &UseStateHandle<Vec<String>>, msg: impl Into<String>) {
    let mut msgs = (&**messages).clone();
    msgs.push(msg.into());
    messages.set(msgs);
}
