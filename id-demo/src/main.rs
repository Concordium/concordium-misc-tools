use concordium_base::{
    base::CredentialRegistrationID,
    common::Versioned,
    contracts_common::AccountAddress,
    id::{
        self,
        constants::{ArCurve, AttributeKind},
        id_proof_types::Proof,
    },
};
use gloo_console::{console_dbg, log};
use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen::{
    prelude::{wasm_bindgen, Closure},
    JsValue,
};

use yew::prelude::*;

mod components;
mod models;

use components::header::Header;

use crate::components::{
    age_in_range::AgeInRange,
    append_message,
    doc_exp_no_earlier_than::DocExpNoEarlierThan,
    document_issuer_in::DocumentIssuerIn,
    in_range::InRange,
    member_of::MemberOf,
    nationality_in::NationalityIn,
    residence_in::ResidenceIn,
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
        challenge: String,
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
                hex::encode(challenge),
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

    let errors: UseStateHandle<Vec<String>> = use_state(Default::default);
    let messages: UseStateHandle<Vec<String>> = use_state(Default::default);

    let connect_wallet = {
        let wallet_conn = wallet_conn;
        let errors = errors.clone();
        move |_| {
            let wallet_conn = wallet_conn.clone();
            let errors = errors.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut wallet = match Wallet::new().await {
                    Ok(w) => w,
                    Err(e) => {
                        let mut errs = (&*errors).clone();
                        errs.push(format!("Error getting the wallet: {:?}", e));
                        errors.set(errs);
                        return;
                    }
                };
                let (addr, conn) = match wallet.connect().await {
                    Ok(x) => x,
                    Err(e) => {
                        let mut errs = (&*errors).clone();
                        errs.push(format!("Error connecting to the wallet: {:?}", e));
                        errors.set(errs);
                        return;
                    }
                };
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
        let errors = errors.clone();
        move |_| {
            let errors = errors.clone();
            let inject_statements = inject_statements.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let r = match Request::post("http://localhost:8100/inject")
                    .json(&inject_statements.statement.clone())
                {
                    Ok(r) => r,
                    Err(e) => {
                        let mut errs = (&*errors).clone();
                        errs.push(format!(
                            "Error constructing the statement to inject: {:?}",
                            e
                        ));
                        errors.set(errs);
                        return;
                    }
                };
                let res = match r.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        let mut errs = (&*errors).clone();
                        errs.push(format!("Error injecting the statement: {:?}", e));
                        errors.set(errs);
                        return;
                    }
                };
                if res.ok() {
                    let data = res.json::<StatementWithChallenge>().await.unwrap(); // TODO: Handle error
                    log!(serde_json::to_string_pretty(&data).unwrap())
                } else {
                    let mut errs = (&*errors).clone();
                    errs.push(format!("Could not inject the statement: {:?}", res));
                    errors.set(errs);
                }
            })
        }
    };

    let get_proof = {
        let inject_statements = statements.clone();
        let errors = errors.clone();
        let messages = messages.clone();
        move |_: MouseEvent| {
            let errors = errors.clone();
            let messages = messages.clone();
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
                    let mut wallet = match Wallet::new().await {
                        Ok(w) => w,
                        Err(e) => {
                            let mut errs = (&*errors).clone();
                            errs.push(format!("Error getting the wallet: {:?}", e));
                            errors.set(errs);
                            return;
                        }
                    };
                    let (addr, _) = match wallet.connect().await {
                        Ok(x) => x,
                        Err(e) => {
                            let mut errs = (&*errors).clone();
                            errs.push(format!("Error connecting to the wallet: {:?}", e));
                            errors.set(errs);
                            return;
                        }
                    };
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
                            append_message(&messages, "Got proof from the wallet.");
                            log!(serde_json::to_string_pretty(&proof).unwrap());
                            let verify_request = match Request::post("http://localhost:8100/prove")
                                .json(&serde_json::json!({
                                    "challenge": data.challenge,
                                    "proof": proof,
                                })) {
                                Ok(vr) => vr,
                                Err(e) => {
                                    append_message(
                                        &errors,
                                        format!(
                                            "Failed to construct request to verify proof: {:?}",
                                            e
                                        ),
                                    );
                                    return;
                                }
                            };
                            let verify = verify_request.send().await;
                            match verify {
                                Ok(verify) => {
                                    if verify.ok() {
                                        append_message(&messages, "Proof OK");
                                    } else {
                                        let r = verify.json::<serde_json::Value>().await;
                                        match r {
                                            Ok(err) => {
                                                let mut errs = (&*errors).clone();
                                                errs.push(format!(
                                                    "Proof invalid: {}",
                                                    serde_json::to_string_pretty(&err).unwrap()
                                                ));
                                                errors.set(errs);
                                            }
                                            Err(e) => {
                                                let mut errs = (&*errors).clone();
                                                errs.push(format!(
                                                    "Unexpected response from server: {:#?}",
                                                    e
                                                ));
                                                errors.set(errs);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    let mut errs = (&*errors).clone();
                                    errs.push(format!("The proof could not be verified: {:#?}", e));
                                    errors.set(errs);
                                }
                            }
                        }
                        Err(e) => {
                            let mut errs = (&*errors).clone();
                            errs.push(format!("Did not get proof from the wallet: {:#?}", e));
                            errors.set(errs);
                        }
                    }
                } else {
                    let mut errs = (&*errors).clone();
                    errs.push(format!("Could not inject statement: {:#?}", res));
                    errors.set(errs);
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
              <ul class="item-list">
                {errors.iter().map(|s|
                  html! {<div class="alert alert-warning" role="alert">
                  {s}
                  </div>}).collect::<Html>()}
                </ul>
              <ul class="item-list">
                {messages.iter().map(|s|
                  html! {<div class="alert alert-success" role="alert">
                  {s}
                  </div>}).collect::<Html>()}
                </ul>
            </div>
            <div class="col-sm">
            {html!{
                  <RevealAttribute statement={statements.clone()} />
            }}
            {html!{
                <YoungerThan statement={statements.clone()} errors={errors.clone()} younger=true />
            }}
            {html!{
                <YoungerThan statement={statements.clone()} errors={errors.clone()} younger=false />
            }}
            {html!{
                  <AgeInRange statement={statements.clone()} errors={errors.clone()} />
            }}
            {html!{
                  <DocExpNoEarlierThan statement={statements.clone()} errors={errors.clone()} />
            }}
            {html!{
                  <InRange statement={statements.clone()} />
            }}
            {html!{
                  <MemberOf statement={statements.clone()} in_set=true />
            }}
            {html!{
                  <MemberOf statement={statements.clone()} in_set=false />
            }}
            {html!{
                  <NationalityIn statement={statements.clone()} in_set=true errors={errors.clone()}/>
            }}
            {html!{
                  <NationalityIn statement={statements.clone()} in_set=false errors={errors.clone()}/>
            }}
            {html!{
                  <ResidenceIn statement={statements.clone()} in_set=true errors={errors.clone()}/>
            }}
            {html!{
                  <ResidenceIn statement={statements.clone()} in_set=false errors={errors.clone()}/>
            }}
            {html!{
                  <DocumentIssuerIn statement={statements.clone()} in_set=true errors={errors.clone()} />
            }}
            {html!{
                  <DocumentIssuerIn statement={statements.clone()} in_set=false errors={errors.clone()} />
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
