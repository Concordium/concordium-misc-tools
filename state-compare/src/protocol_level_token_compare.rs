use anyhow::{anyhow, Context};
use concordium_rust_sdk::{
    protocol_level_tokens::{self, TokenState},
    types::hashes::BlockHash,
    v2::{self, IntoBlockIdentifier},
};
use futures::TryStreamExt;
use tokio::try_join;
use tracing::{info, trace, warn};

/// Compares the protocol level token identifiers of two nodes and returns the
/// common identifiers.
pub async fn compare_token_identifiers(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<Vec<protocol_level_tokens::TokenId>> {
    trace!("Comparing PLT token identifiers.");

    info!("Fetching PLT token lists.");
    let (res1, res2) = try_join!(
        client1.get_token_list(block1),
        client2.get_token_list(block2),
    )?;
    let tokens1 = res1.response.try_collect::<Vec<_>>().await?;
    let tokens2 = res2.response.try_collect::<Vec<_>>().await?;

    info!(
        "Block 1 has {} tokens, Block 2 has {} tokens.",
        tokens1.len(),
        tokens2.len()
    );
    info!(
        "tokens_block_1 = {:?}, tokens_block_2 = {:?}",
        tokens1, tokens2
    );

    Ok(compare_token_identifier_lists(tokens1, tokens2))
}

pub fn compare_token_identifier_lists(
    tokens1: Vec<protocol_level_tokens::TokenId>,
    tokens2: Vec<protocol_level_tokens::TokenId>,
) -> Vec<protocol_level_tokens::TokenId> {
    compare!(tokens1, tokens2, "PLT Token identifiers");

    let result: Vec<protocol_level_tokens::TokenId> = tokens1
        .iter()
        .filter(|id| tokens2.contains(id))
        .cloned()
        .collect();

    info!("Returning result with {} common tokens.", result.len());

    result
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

        let block1_identifier = &block1.into_block_identifier();
        let block2_identifier = &block2.into_block_identifier();
        let (res1, res2) = try_join!(
            client1.get_token_info(token_id.clone(), block1_identifier),
            client2.get_token_info(token_id.clone(), block2_identifier),
        )?;

        let info1 = res1.response;
        let info2 = res2.response;

        info!("Comparing token info {:?}", token_id);
        compare!(
            info1.token_state.decimals,
            info2.token_state.decimals,
            "Token Info for ID {:?}",
            token_id
        );

        // check token module state is matching for paused
        let decoded_mod_state1 = TokenState::decode_module_state(&info1.token_state)
            .context("Error in getting decoded module state1")?;
        let decoded_mod_state2 = TokenState::decode_module_state(&info2.token_state)
            .context("Error in getting decoded module state2")?;
        let paused1 = decoded_mod_state1.paused.ok_or_else(|| {
            anyhow!(
                "Missing paused state in token1 module state for token: {:?}",
                token_id
            )
        })?;
        let paused2 = decoded_mod_state2.paused.ok_or_else(|| {
            anyhow!(
                "Missing paused state in token2 module state for token: {:?}",
                token_id
            )
        })?;
        compare!(
            paused1,
            paused2,
            "Token module paused check differs for token: {:?}",
            token_id
        );
    }
    Ok(())
}
