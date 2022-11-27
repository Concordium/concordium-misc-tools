use concordium_base::id::{self, id_proof_types::AtomicStatement};
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone, Debug, Default)]
pub struct StatementProp {
    pub statement:
        id::id_proof_types::Statement<id::constants::ArCurve, id::constants::AttributeKind>,
}

impl StatementProp {
    pub fn push(
        &self,
        s: AtomicStatement<id::constants::ArCurve, id::constants::AttributeKind>,
    ) -> Self {
        let mut inner = self.statement.statements.clone();
        inner.push(s);
        Self {
            statement: id::id_proof_types::Statement { statements: inner },
        }
    }
}

#[function_component(Statement)]
pub fn statement(s: &StatementProp) -> Html {
    s.statement
        .statements
        .iter()
        .map(|atomic_s| match atomic_s {
            AtomicStatement::RevealAttribute { statement } => {
                html! {
                    <div class="m-3 p-4 border rounded d-flex align-items-center">
                      <img src="https://robohash.org/hicveldicta.png?size=50x50&set=set1" class="mr-2" alt="img" />
                      <div class="">
                          <p class="fw-bold mb-1">{"Reveal attribute"}</p>
                          <p class="fw-normal mb-1">{statement.attribute_tag}</p>
                      </div>
                    </div>
                }
            }
            AtomicStatement::AttributeInRange { statement } => todo!(),
            AtomicStatement::AttributeInSet { statement } => todo!(),
            AtomicStatement::AttributeNotInSet { statement } => todo!() })
        .collect::<Html>()
    // html! {
    // <div class="m-3 p-4 border rounded d-flex align-items-center">
    //     <img src="https://robohash.org/hicveldicta.png?size=50x50&set=set1" class="mr-2" alt="img" />
    //     <div class="">
    //         <p class="fw-bold mb-1">{s.statement.clone()}</p>
    //         <p class="fw-normal mb-1">{s.statement.clone()}</p>
    //         <p class="fw-normal mb-1">{s.statement.clone()}</p>
    //         <p class="fw-normal mb-1">{s.statement.clone()}</p>
    //     </div>
    // </div>
    // }
}
