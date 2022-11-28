use concordium_base::{
    common::base16_decode_string,
    contracts_common::AccountAddress,
    id::{id_proof_types::StatementWithContext, types::AttributeTag},
};
use gloo_console::{console, console_dbg, error, log};
use gloo_net::http::Request;
use wasm_bindgen::{
    prelude::{wasm_bindgen, Closure},
    JsCast, JsError, JsValue,
};
use web_sys::{EventTarget, HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

mod components;
mod models;

use components::header::Header;

use crate::components::{
    reveal_attribute::RevealAttribute,
    statement::{Statement, StatementProp}, younger_than::YoungerThan,
};
#[wasm_bindgen(module = "/detector.js")]
extern "C" {
    type WalletApi;
    #[wasm_bindgen(catch)]
    async fn detectConcordiumProvider() -> Result<JsValue, JsValue>;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(method, catch)]
    async fn connect(this: &WalletApi) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    // the closure argument is the account address representation.
    fn on(
        this: &WalletApi,
        event: &str,
        callback: &Closure<dyn FnMut(JsValue)>,
    ) -> Result<JsValue, JsValue>;
}

struct Wallet {
    inner: WalletApi,
}

struct WalletConnection {
    _closure: Closure<dyn FnMut(JsValue)>,
}

impl Wallet {
    pub async fn new() -> Result<Self, JsValue> {
        Ok(Self {
            inner: detectConcordiumProvider().await?.into(),
        })
    }

    pub async fn connect(&mut self) -> Result<(AccountAddress, WalletConnection), JsValue> {
        let addr_json = self.inner.connect().await?;
        let cl = Closure::new(
            |v| match serde_wasm_bindgen::from_value::<AccountAddress>(v) {
                Ok(v) => {
                    log!(format!("Account disconnected: {}", v));
                }
                Err(e) => {
                    log!("Disconnect event, but could not be parsed: {}", e);
                }
            },
        );
        match self.inner.on("accountDisconnected", &cl) {
            Ok(_) => (),
            Err(e) => {
                console_dbg!("Could not register disconnect handler: {}", e);
            }
        };
        match serde_wasm_bindgen::from_value(addr_json) {
            Ok(v) => Ok((v, WalletConnection { _closure: cl })),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    }
}

#[function_component(App)]
fn app() -> Html {
    let statements: UseStateHandle<StatementProp> = use_state(Default::default);
    let wallet_conn: UseStateHandle<Option<WalletConnection>> = use_state(Default::default);

    // {
    //     // create copies of states
    //     let users = statements.clone();
    //     let error = error.clone();

    //     use_effect_with_deps(
    //         move |_| {
    //             wasm_bindgen_futures::spawn_local(async move {
    //                 let fetched_users = Request::get("https://dummyjson.com/users").send().await;
    //                 match fetched_users {
    //                     Ok(response) => {
    //                         let json = response.json::<Users>().await;
    //                         match json {
    //                             Ok(json_resp) => {
    //                                 users.set(Some(json_resp));
    //                             }
    //                             Err(e) => error.set(Some(e)),
    //                         }
    //                     }
    //                     Err(e) => error.set(Some(e)),
    //                 }
    //             });
    //             || ()
    //         },
    //         (),
    //     );
    // }

    // let user_list_logic = match statements.as_ref() {
    //     Some(users) => users
    //         .users
    //         .iter()
    //         .map(|user| {
    //             html! {
    //               <Card user={user.clone() }/>
    //             }
    //         })
    //         .collect(),
    //     None => match error.as_ref() {
    //         Some(_) => {
    //             html! {
    //                 <Message text={"Error getting list of users"}
    // css_class={"text-danger"}/>             }
    //         }
    //         None => {
    //             html! {
    //               <Loader />
    //             }
    //         }
    //     },
    // };

    let connect_wallet = {
        let wallet_conn = wallet_conn.clone();
        move |_| {
            let wallet_conn = wallet_conn.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut wallet = Wallet::new().await.unwrap(); // TODO
                let (addr, conn) = wallet.connect().await.unwrap(); // TODO
                wallet_conn.set(Some(conn));
                web_sys::window()
                    .unwrap()
                    .alert_with_message(&addr.to_string())
                    .unwrap();
            });
        }
    };

    let inject_statement = {
        let inject_statements = statements.clone();
        move |_| {
            let inject_statements = inject_statements.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let r = Request::post("http://localhost:8100/inject").json(&
                    inject_statements.statement.clone(),
                ).unwrap(); // TODO
                let res = r.send().await.unwrap(); // TODO
                if res.ok() {
                    let data = res.json::<serde_json::Value>().await.unwrap(); // TODO: Handle error
                    log!(serde_json::to_string_pretty(&data).unwrap())
                } else {
                    error!(format!("Could not inject statement {:#?}", res));
                }
            })
        }
    };

    html! {
      <>
        <Header />
        <div class="container">
          <div class="row">
            <div class="col-sm">
              <div class="btn-group-vertical">
                <button onclick={connect_wallet} type="button" class="btn btn-primary btn-lg mb-1">{"Connect"}</button>
                <button onclick={inject_statement} type="button" class="btn btn-primary btn-lg mt-1">{"Inject statement"}</button>
              </div>
            </div>

            <div class="col-sm">
            {html!{
                  <RevealAttribute statement={statements.clone()} />
            }}
            {html!{
                  <YoungerThan statement={statements.clone()} younger=true />
            }}
            {html!{
                  <YoungerThan statement={statements.clone()} younger=false />
            }}
            </div>
            <div class="col-sm">
            {html!{
                  <Statement statement={statements.statement.clone()} />
            }}
            </div>
         </div>
        </div>
      </>
    }
}

fn main() { yew::Renderer::<App>::new().render(); }
