use anyhow::Context;
use concordium_rust_sdk::{
    protocol_level_tokens::{self, TokenState},
    types::hashes::BlockHash,
    v2::{self, IntoBlockIdentifier},
};
use futures::TryStreamExt;
use tokio::try_join;
use tracing::{debug, info, warn};

/// Compares the protocol level token identifiers of two nodes and returns the
/// common identifiers.
pub async fn compare_token_identifiers(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<Vec<protocol_level_tokens::TokenId>> {
    info!("Comparing PLT token identifiers.");

    debug!("Fetching PLT token lists.");
    let (res1, res2) = try_join!(
        client1.get_token_list(block1),
        client2.get_token_list(block2),
    )?;
    let tokens1 = res1.response.try_collect::<Vec<_>>().await?;
    let tokens2 = res2.response.try_collect::<Vec<_>>().await?;

    debug!(
        "Block 1 has {} tokens, Block 2 has {} tokens.",
        tokens1.len(),
        tokens2.len()
    );
    debug!(
        "tokens_block_1 = {:?}, tokens_block_2 = {:?}",
        tokens1, tokens2
    );

    Ok(compare_token_identifier_lists(tokens1, tokens2))
}

fn compare_token_identifier_lists(
    tokens1: Vec<protocol_level_tokens::TokenId>,
    tokens2: Vec<protocol_level_tokens::TokenId>,
) -> Vec<protocol_level_tokens::TokenId> {
    compare!(tokens1, tokens2, "PLT Token identifiers");

    let result: Vec<protocol_level_tokens::TokenId> = tokens1
        .into_iter()
        .filter(|id| tokens2.contains(id))
        .collect();

    debug!("Returning result with {} common tokens.", result.len());

    result
}

pub async fn compare_token_info_for_ids(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
    token_ids: &[protocol_level_tokens::TokenId],
) -> anyhow::Result<()> {
    debug!("compare_token_info_for_ids");
    for token_id in token_ids {
        debug!(
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

        // Compare at the token info level
        compare!(info1, info2, "Token Info for ID {:?}", token_id);

        // check the decoded token module states match
        let decoded_mod_state1 = TokenState::decode_module_state(&info1.token_state)
            .context("Error in getting decoded module state1")?;
        let decoded_mod_state2 = TokenState::decode_module_state(&info2.token_state)
            .context("Error in getting decoded module state2")?;

        compare!(
            decoded_mod_state1,
            decoded_mod_state2,
            "Decoded module state for token id: {:?}",
            token_id
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use concordium_rust_sdk::protocol_level_tokens::TokenId;
    use std::{collections::HashSet, str::FromStr};

    fn make_token(value: &str) -> TokenId {
        TokenId::from_str(value).unwrap()
    }

    #[test]
    fn test_compare_token_identities_matching() {
        let tokens1: Vec<TokenId> = vec![make_token("TokenA"), make_token("TokenB")];
        let tokens2: Vec<TokenId> = vec![make_token("TokenB"), make_token("TokenA")];

        let result: Vec<TokenId> = compare_token_identifier_lists(tokens1, tokens2);

        let expected: HashSet<_> = vec![make_token("TokenA"), make_token("TokenB")]
            .into_iter()
            .collect();
        let result_set: HashSet<_> = result.into_iter().collect();
        assert_eq!(result_set, expected);
    }

    #[test]
    fn test_compare_token_identifiers_different() {
        let tokens1: Vec<TokenId> = vec![make_token("TokenA")];
        let tokens2: Vec<TokenId> = vec![make_token("TokenB")];

        let result: Vec<TokenId> = compare_token_identifier_lists(tokens1, tokens2);
        assert!(result.is_empty());
    }
}
