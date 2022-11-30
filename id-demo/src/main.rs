use concordium_base::{
    base::CredentialRegistrationID,
    common::{base16_decode_string, Versioned},
    contracts_common::AccountAddress,
    id::{
        self,
        constants::{ArCurve, AttributeKind},
        id_proof_types::{Proof, StatementWithContext},
        types::AttributeTag,
    },
};
use gloo_console::{console, console_dbg, error, log};
use gloo_net::http::Request;
use serde::Serialize;
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
    statement::{Statement, StatementProp},
    younger_than::YoungerThan,
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

    #[wasm_bindgen(method, catch)]
    async fn requestIdProof(
        this: &WalletApi,
        accountAddress: JsValue,
        statement: JsValue,
        challenge: Vec<u8>,
    ) -> Result<JsValue, JsValue>;
}

struct Wallet {
    inner: WalletApi,
}

struct WalletConnection {
    _closure: Closure<dyn FnMut(JsValue)>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StatementWithChallenge {
    challenge: String, // TODO: Should be Vec<u8>
    statement: id::id_proof_types::Statement<ArCurve, AttributeKind>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ProofWithContext {
    credential: CredentialRegistrationID,
    proof:      Versioned<Proof<ArCurve, AttributeKind>>,
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

    pub async fn request_id_proof(
        &self,
        addr: &AccountAddress,
        statement: &id::id_proof_types::Statement<ArCurve, AttributeKind>,
        challenge: &[u8],
    ) -> Result<ProofWithContext, JsValue> {
        let statement = statement
            .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
            .unwrap();
        let response = self
            .inner
            .requestIdProof(
                serde_wasm_bindgen::to_value(addr).unwrap(),
                statement,
                challenge.to_vec(),
            )
            .await?;
        let r = serde_wasm_bindgen::from_value(response)?;
        Ok(r)
    }
}

#[function_component(App)]
fn app() -> Html {
    let statements: UseStateHandle<StatementProp> = use_state(Default::default);
    let wallet_conn: UseStateHandle<Option<WalletConnection>> = use_state(Default::default);

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
                let r = Request::post("http://localhost:8100/inject")
                    .json(&inject_statements.statement.clone())
                    .unwrap(); // TODO
                let res = r.send().await.unwrap(); // TODO
                if res.ok() {
                    let data = res.json::<StatementWithChallenge>().await.unwrap(); // TODO: Handle error
                    log!(serde_json::to_string_pretty(&data).unwrap())
                } else {
                    error!(format!("Could not inject statement {:#?}", res));
                }
            })
        }
    };

    let get_proof = {
        let inject_statements = statements.clone();
        move |_: MouseEvent| {
            let inject_statements = inject_statements.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let r = Request::post("http://localhost:8100/inject")
                    .json(&inject_statements.statement.clone())
                    .unwrap(); // TODO
                let res = r.send().await.unwrap(); // TODO
                if res.ok() {
                    log!("Got result");
                    let data = res.json::<StatementWithChallenge>().await.unwrap(); // TODO: Handle error
                    log!(serde_json::to_string_pretty(&data).unwrap());
                    let mut wallet = Wallet::new().await.unwrap(); // TODO
                    let (addr, _) = wallet.connect().await.unwrap(); // TODO
                    log!("Requesting proof.");
                    let proof = wallet
                        .request_id_proof(
                            &addr,
                            &data.statement,
                            &hex::decode(&data.challenge).unwrap(),
                        )
                        .await; // TODO: Don't unwrap.
                    match proof {
                        Ok(proof) => {
                            log!("Got proof.");
                            log!(serde_json::to_string_pretty(&proof).unwrap());
                            let verify_request = Request::post("http://localhost:8100/prove")
                                .json(&serde_json::json!({
                                    "challenge": data.challenge,
                                    "proof": proof,
                                }))
                                .unwrap();
                            let verify = verify_request.send().await;
                            match verify {
                                Ok(verify) => {
                                    let b = verify.json().await;
                                    match b {
                                        Ok(b) => {
                                            if b {
                                                log!("Proof OK.");
                                            } else {
                                                log!("Proof invalid.")
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "Unexpected response from server: ",
                                                e.to_string()
                                            )
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Could not verify: ", e.to_string());
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to get proof.", e);
                        }
                    }
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
                <button onclick={get_proof} type="button" class="btn btn-primary btn-lg mt-1">{"Get proof"}</button>
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
