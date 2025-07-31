#[cfg(test)]
mod tests {
    use concordium_rust_sdk::{protocol_level_tokens::TokenId, types::hashes::BlockHash};
    use concordium_state_compare::token_compare::compare_token_identities;
    use std::{collections::HashSet, str::FromStr, sync::Once};

    static INIT: Once = Once::new();

    fn init_logger() {
        INIT.call_once(|| {
            env_logger::builder()
                .is_test(true)
                .filter_level(log::LevelFilter::Debug)
                .try_init()
                .ok();
        });
    }

    fn make_token(value: &str) -> TokenId { TokenId::from_str(value).unwrap() }

    #[tokio::test]
    async fn test_compare_token_identities_matching() {
        init_logger();

        let tokens1: Vec<TokenId> = vec![make_token("TokenA"), make_token("TokenB")];
        let tokens2: Vec<TokenId> = vec![make_token("TokenB"), make_token("TokenA")];
        let block1: BlockHash = [0u8; 32].into(); // Dummy BlockHash
        let block2: BlockHash = [1u8; 32].into();

        let result: Vec<TokenId> = compare_token_identities(&tokens1, &tokens2, block1, block2)
            .await
            .unwrap();

        let expected: HashSet<_> = vec![make_token("TokenA"), make_token("TokenB")]
            .into_iter()
            .collect();
        let result_set: HashSet<_> = result.into_iter().collect();
        assert_eq!(result_set, expected);
    }

    #[tokio::test]
    async fn test_compare_token_identities_different() {
        init_logger();

        let tokens1: Vec<TokenId> = vec![make_token("TokenA")];
        let tokens2: Vec<TokenId> = vec![make_token("TokenB")];
        let block1: BlockHash = [0u8; 32].into();
        let block2: BlockHash = [1u8; 32].into();

        let result: Vec<TokenId> = compare_token_identities(&tokens1, &tokens2, block1, block2)
            .await
            .unwrap();
        assert!(result.is_empty());
    }
}
