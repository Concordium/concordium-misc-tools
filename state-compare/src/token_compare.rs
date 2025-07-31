use concordium_rust_sdk::{
    protocol_level_tokens::{self, TokenState},
    types::hashes::BlockHash,
    v2::{self, IntoBlockIdentifier},
};
use futures::TryStreamExt;
use log::{info, warn};

pub async fn fetch_token_lists(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<(
    Vec<protocol_level_tokens::TokenId>,
    Vec<protocol_level_tokens::TokenId>,
)> {
    info!("Fetching PLT token lists.");
    let tokens1 = client1
        .get_token_list(block1)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;
    let tokens2 = client2
        .get_token_list(block2)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;

    Ok((tokens1, tokens2))
}

pub async fn compare_token_identities(
    tokens_node_1: &[protocol_level_tokens::TokenId],
    tokens_node_2: &[protocol_level_tokens::TokenId],
    _block1: BlockHash,
    _block2: BlockHash,
) -> anyhow::Result<Vec<protocol_level_tokens::TokenId>> {
    info!("Comparing PLT token identities.");

    // We need to clone the tokens to sort them, otherwise the borrowing would
    // complain
    let mut tokens_node_1 = tokens_node_1.to_vec();
    let mut tokens_node_2 = tokens_node_2.to_vec();

    info!(
        "Node 1 has {} tokens, Node 2 has {} tokens.",
        tokens_node_1.len(),
        tokens_node_2.len()
    );
    info!(
        "tokens_node_1 = {:?}, tokens_node_2 = {:?}",
        tokens_node_1, tokens_node_2
    );
    // Sort the tokens to ensure a consistent order for comparison.
    tokens_node_1.sort_unstable();
    tokens_node_2.sort_unstable();

    compare!(tokens_node_1, tokens_node_2, "PLT Token identities");

    info!(
        "after comparison, Node 1 has {} tokens, Node 2 has {} tokens.",
        tokens_node_1.len(),
        tokens_node_2.len()
    );
    let result: Vec<protocol_level_tokens::TokenId> = tokens_node_1
        .iter()
        .filter(|id| tokens_node_2.contains(id))
        .cloned()
        .collect();

    info!("Returning result with {} common tokens.", result.len());
    Ok(result)
}

pub async fn compare_token_info_for_ids(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
    token_ids: &[protocol_level_tokens::TokenId],
) -> anyhow::Result<()> {
    info!("compare_token_info_for_ids");
    for token_id in token_ids {
        info!(
            "Getting token info for token ID {:?} from block {}",
            token_id,
            block1.into_block_identifier()
        );
        let info1 = client1
            .get_token_info(token_id.clone(), &block1.into_block_identifier())
            .await;
        info!(
            "Getting token info for token ID {:?} from block {}",
            token_id,
            block2.into_block_identifier()
        );
        let info2 = client2
            .get_token_info(token_id.clone(), &block2.into_block_identifier())
            .await;

        match (info1, info2) {
            (Ok(info1), Ok(info2)) => {
                info!("Comparing token info {:?}", token_id);
                compare!(
                    info1.response.token_state.decimals,
                    info2.response.token_state.decimals,
                    "Token Info for ID {:?}",
                    token_id
                );

                // check token module state is matching for paused
                let decoded_mod_state1 =
                    TokenState::decode_module_state(&info1.response.token_state);
                let decoded_mod_state2 =
                    TokenState::decode_module_state(&info2.response.token_state);
                compare!(
                    decoded_mod_state1.as_ref().unwrap().paused,
                    decoded_mod_state2.as_ref().unwrap().paused,
                    "Token module paused check differs for token: {:?}",
                    token_id
                );
            }
            (Err(e1), Err(e2)) => {
                warn!(
                    "Token info {:?} not found on either node: {:?} / {:?}",
                    token_id, e1, e2
                );
            }
            (Err(e), _) => {
                warn!("Token info {:?} missing on node 1: {:?}", token_id, e);
            }
            (_, Err(e)) => {
                warn!("Token info {:?} missing on node 2: {:?}", token_id, e);
            }
        }
    }
    Ok(())
}
