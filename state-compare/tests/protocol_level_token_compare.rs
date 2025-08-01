#[cfg(test)]
mod tests {
    use concordium_rust_sdk::protocol_level_tokens::TokenId;
    use concordium_state_compare::protocol_level_token_compare::compare_token_identifier_lists;
    use std::{collections::HashSet, str::FromStr};

    fn make_token(value: &str) -> TokenId { TokenId::from_str(value).unwrap() }

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
    fn test_compare_token_identities_different() {
        let tokens1: Vec<TokenId> = vec![make_token("TokenA")];
        let tokens2: Vec<TokenId> = vec![make_token("TokenB")];

        let result: Vec<TokenId> = compare_token_identifier_lists(tokens1, tokens2);
        assert!(result.is_empty());
    }
}
