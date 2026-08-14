//! Smoke tests that exercise every example TOML configuration file through the
//! command-line entry points, asserting that a valid genesis block is produced.
//!
//! These run under `cargo test -p genesis-creator` (or simply `cargo test`).
//! Each test creates an isolated output directory under `target/smoke_tests/`
//! so no example files are ever modified.

use genesis_creator::{handle_assemble, handle_generate};
use std::path::{Path, PathBuf};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the absolute path to the `genesis-creator/examples/` directory.
fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

/// Creates (and clears) a dedicated output directory for a single test.
fn smoke_out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("smoke_tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("Failed to create smoke test output directory");
    dir
}

/// Writes a generate-mode TOML value with all output paths targeting `out_dir`.
fn write_generate_toml(mut value: toml::Value, out_dir: &Path) -> PathBuf {
    let abs = |name: &str| toml::Value::String(out_dir.join(name).to_string_lossy().into_owned());
    let out = value
        .get_mut("out")
        .and_then(toml::Value::as_table_mut)
        .expect("[out] must be a table");
    out.insert("accountKeys".into(), abs("accounts"));
    out.insert("updateKeys".into(), abs("update-keys"));
    out.insert("bakerKeys".into(), abs("bakers"));
    out.insert("identityProviders".into(), abs("idps"));
    out.insert("anonymityRevokers".into(), abs("ars"));
    out.insert("genesis".into(), abs("genesis.dat"));
    out.insert("genesisHash".into(), abs("genesis_hash"));
    out.insert("cryptographicParameters".into(), abs("global"));
    out.insert("deleteExisting".into(), toml::Value::Boolean(false));

    let rewritten = toml::to_string_pretty(&value).expect("Failed to serialise modified TOML");
    let dest = out_dir.join("config.toml");
    std::fs::write(&dest, rewritten).expect("Failed to write modified TOML");
    dest
}

/// Rewrites the `[out]` section of a generate-mode TOML so all output paths
/// point into `out_dir`. Returns the path to the rewritten TOML file.
fn rewrite_generate_toml(toml_path: &Path, out_dir: &Path) -> PathBuf {
    let content = std::fs::read_to_string(toml_path).expect("Failed to read example TOML");
    let value: toml::Value = toml::from_str(&content).expect("Example TOML must be valid TOML");
    write_generate_toml(value, out_dir)
}

/// Loads an example, applies a mutation, and rewrites its output paths.
fn rewrite_mutated_generate_toml(
    example: &str,
    out_dir: &Path,
    mutate: impl FnOnce(&mut toml::Value),
) -> PathBuf {
    let content =
        std::fs::read_to_string(examples_dir().join(example)).expect("Failed to read example TOML");
    let mut value: toml::Value = toml::from_str(&content).expect("Example TOML must be valid TOML");
    mutate(&mut value);
    write_generate_toml(value, out_dir)
}

/// Asserts that `genesis.dat` and `genesis_hash` exist in `out_dir` and are
/// non-empty / well-formed.
fn assert_genesis_output(out_dir: &Path) {
    let genesis_dat = out_dir.join("genesis.dat");
    assert!(
        genesis_dat.exists(),
        "genesis.dat not found in {}",
        out_dir.display()
    );
    let genesis_bytes = std::fs::read(&genesis_dat).expect("Failed to read genesis.dat");
    assert!(!genesis_bytes.is_empty(), "genesis.dat is empty");

    let hash_file = out_dir.join("genesis_hash");
    assert!(
        hash_file.exists(),
        "genesis_hash not found in {}",
        out_dir.display()
    );
    let hash_content = std::fs::read_to_string(&hash_file).expect("Failed to read genesis_hash");
    // genesis_hash is a JSON array containing one hex hash string
    let hashes: Vec<String> =
        serde_json::from_str(&hash_content).expect("genesis_hash must be a JSON array");
    assert_eq!(
        hashes.len(),
        1,
        "genesis_hash must contain exactly one hash"
    );
    assert_eq!(
        hashes[0].len(),
        64,
        "genesis hash must be a 64-character hex string"
    );
}

// ── Generate smoke tests ──────────────────────────────────────────────────────

/// P1 — genesis1.toml
#[test]
fn smoke_generate_p1() {
    let out = smoke_out_dir("generate_p1");
    let toml = rewrite_generate_toml(&examples_dir().join("genesis1.toml"), &out);
    handle_generate(&toml, false).expect("handle_generate must succeed for genesis1.toml");
    assert_genesis_output(&out);
}

/// P4 — genesis4.toml (CPV1 chain params)
#[test]
fn smoke_generate_p4() {
    let out = smoke_out_dir("generate_p4");
    let toml = rewrite_generate_toml(&examples_dir().join("genesis4.toml"), &out);
    handle_generate(&toml, false).expect("handle_generate must succeed for genesis4.toml");
    assert_genesis_output(&out);
}

/// P5 — genesis5.toml
#[test]
fn smoke_generate_p5() {
    let out = smoke_out_dir("generate_p5");
    let toml = rewrite_generate_toml(&examples_dir().join("genesis5.toml"), &out);
    handle_generate(&toml, false).expect("handle_generate must succeed for genesis5.toml");
    assert_genesis_output(&out);
}

/// P6 — genesis6.toml
#[test]
fn smoke_generate_p6() {
    let out = smoke_out_dir("generate_p6");
    let toml = rewrite_generate_toml(&examples_dir().join("genesis6.toml"), &out);
    handle_generate(&toml, false).expect("handle_generate must succeed for genesis6.toml");
    assert_genesis_output(&out);
}

/// P8 — genesis8.toml
#[test]
fn smoke_generate_p8() {
    let out = smoke_out_dir("generate_p8");
    let toml = rewrite_generate_toml(&examples_dir().join("genesis8.toml"), &out);
    handle_generate(&toml, false).expect("handle_generate must succeed for genesis8.toml");
    assert_genesis_output(&out);
}

/// P9 — genesis9.toml
#[test]
fn smoke_generate_p9() {
    let out = smoke_out_dir("generate_p9");
    let toml = rewrite_generate_toml(&examples_dir().join("genesis9.toml"), &out);
    handle_generate(&toml, false).expect("handle_generate must succeed for genesis9.toml");
    assert_genesis_output(&out);
}

/// P11 — genesis11.toml with tokenParameters and maxLockDuration.
#[test]
fn smoke_generate_p11() {
    let out = smoke_out_dir("generate_p11");
    let toml = rewrite_generate_toml(&examples_dir().join("genesis11.toml"), &out);
    handle_generate(&toml, false).expect("handle_generate must succeed for genesis11.toml");
    assert_genesis_output(&out);
}

#[test]
fn p11_requires_token_parameters() {
    let out = smoke_out_dir("p11_missing_token_parameters");
    let toml = rewrite_mutated_generate_toml("genesis11.toml", &out, |value| {
        value["updates"]["level2"]
            .as_table_mut()
            .expect("updates.level2 must be a table")
            .remove("tokenParameters");
    });
    let error = handle_generate(&toml, false).expect_err("missing tokenParameters must fail");
    assert!(error.to_string().contains("tokenParameters"));
}

#[test]
fn p11_requires_max_lock_duration() {
    let out = smoke_out_dir("p11_missing_max_lock_duration");
    let toml = rewrite_mutated_generate_toml("genesis11.toml", &out, |value| {
        value["parameters"]["chain"]
            .as_table_mut()
            .expect("parameters.chain must be a table")
            .remove("maxLockDuration");
    });
    let error = handle_generate(&toml, false).expect_err("missing maxLockDuration must fail");
    assert!(error.to_string().contains("maxLockDuration"));
}

#[test]
fn p11_rejects_invalid_token_parameters_access() {
    for (name, authorized_keys, threshold, expected) in [
        (
            "unknown_key",
            vec![toml::Value::Integer(99)],
            1,
            "does not specify a known update key",
        ),
        (
            "threshold",
            (0..7).map(toml::Value::Integer).collect(),
            8,
            "exceeds the number of authorized keys",
        ),
    ] {
        let out = smoke_out_dir(&format!("p11_invalid_token_parameters_{name}"));
        let toml = rewrite_mutated_generate_toml("genesis11.toml", &out, |value| {
            let token = value["updates"]["level2"]["tokenParameters"]
                .as_table_mut()
                .expect("tokenParameters must be a table");
            token.insert("authorizedKeys".into(), toml::Value::Array(authorized_keys));
            token.insert("threshold".into(), toml::Value::Integer(threshold));
        });
        let error = handle_generate(&toml, false).expect_err("invalid tokenParameters must fail");
        assert!(error.to_string().contains(expected), "{error:#}");
    }
}

#[test]
fn p8_through_p10_reject_p11_only_fields() {
    for (name, example, protocol) in [
        ("p8", "genesis8.toml", "8"),
        ("p9", "genesis9.toml", "9"),
        ("p10", "genesis9.toml", "10"),
    ] {
        let out = smoke_out_dir(&format!("{name}_p11_only_fields"));
        let toml = rewrite_mutated_generate_toml(example, &out, |value| {
            value["protocolVersion"] = toml::Value::String(protocol.into());
            let level2 = value["updates"]["level2"]
                .as_table_mut()
                .expect("updates.level2 must be a table");
            let token_parameters = level2
                .get("emergency")
                .expect("emergency authorization must exist")
                .clone();
            level2.insert("tokenParameters".into(), token_parameters);
            value["parameters"]["chain"]
                .as_table_mut()
                .expect("parameters.chain must be a table")
                .insert("maxLockDuration".into(), toml::Value::String("1d".into()));
        });
        let error = handle_generate(&toml, false).expect_err("pre-P11 fields must fail");
        assert!(error.to_string().contains("tokenParameters"), "{error:#}");
    }
}

/// P8 single-validator — single-validator-example-p8.toml
/// This example has 102 accounts and is somewhat slow (~15–30 s).
#[test]
fn smoke_generate_p8_single_validator() {
    let out = smoke_out_dir("generate_p8_single_validator");
    let toml = rewrite_generate_toml(
        &examples_dir().join("single-validator-example-p8.toml"),
        &out,
    );
    handle_generate(&toml, false)
        .expect("handle_generate must succeed for single-validator-example-p8.toml");
    assert_genesis_output(&out);
}

// ── Assemble smoke test ───────────────────────────────────────────────────────

/// P1 assemble — mainnet-assemble.toml
///
/// All paths (inputs and outputs) are rewritten to absolute paths so the test
/// is self-contained and never writes files into `examples/`.
#[test]
fn smoke_assemble_p1_mainnet() {
    let out = smoke_out_dir("assemble_p1_mainnet");
    let original_toml = examples_dir().join("mainnet-assemble.toml");
    let assemble_files = examples_dir().join("assemble-files");

    let content = std::fs::read_to_string(&original_toml).unwrap();
    let mut value: toml::Value = toml::from_str(&content).unwrap();

    let abs_in =
        |name: &str| toml::Value::String(assemble_files.join(name).to_string_lossy().into_owned());
    let abs_out = |name: &str| toml::Value::String(out.join(name).to_string_lossy().into_owned());

    // Rewrite input paths to absolute so we can place the TOML anywhere.
    *value.get_mut("accounts").unwrap() = abs_in("accounts.json");
    *value.get_mut("ars").unwrap() = abs_in("anonymity-revokers.json");
    *value.get_mut("idps").unwrap() = abs_in("identity-providers.json");
    *value.get_mut("global").unwrap() = abs_in("cryptographic-parameters.json");
    *value.get_mut("governanceKeys").unwrap() = abs_in("governance-keys.json");
    // Rewrite output paths.
    *value.get_mut("genesisOut").unwrap() = abs_out("genesis.dat");
    *value.get_mut("genesisHashOut").unwrap() = abs_out("genesis_hash");

    let rewritten = toml::to_string_pretty(&value).unwrap();
    // Place the TOML in the isolated out_dir — never touches examples/.
    let smoke_toml = out.join("mainnet-assemble.toml");
    std::fs::write(&smoke_toml, &rewritten).unwrap();

    handle_assemble(&smoke_toml, false)
        .expect("handle_assemble must succeed for mainnet-assemble.toml");
    assert_genesis_output(&out);
}

/// P11 assembly from artifacts generated by genesis11.toml.
#[test]
fn smoke_assemble_p11() {
    let generated = smoke_out_dir("assemble_p11_generated");
    let generate_toml = rewrite_generate_toml(&examples_dir().join("genesis11.toml"), &generated);
    handle_generate(&generate_toml, false).expect("P11 artifact generation must succeed");

    let generated_accounts_path = generated.join("accounts/accounts.json");
    let mut accounts: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&generated_accounts_path).expect("generated accounts must be readable"),
    )
    .expect("generated accounts must be valid JSON");
    let foundation_account = accounts
        .as_array()
        .and_then(|accounts| accounts.last())
        .and_then(|account| account.get("address"))
        .and_then(serde_json::Value::as_str)
        .expect("last generated account must have an address")
        .to_owned();

    // Assembly account JSON uses the node-compatible versioned credential form.
    for account in accounts
        .as_array_mut()
        .expect("generated accounts must be an array")
    {
        let credentials = account["credentials"].take();
        account["credentials"] = serde_json::json!({"v": 0, "value": credentials});
    }
    let accounts_path = generated.join("accounts/assembly-accounts.json");
    std::fs::write(
        &accounts_path,
        serde_json::to_vec_pretty(&accounts).expect("accounts must serialize"),
    )
    .expect("assembly accounts must be writable");

    let content = std::fs::read_to_string(examples_dir().join("genesis11.toml")).unwrap();
    let mut value: toml::Value = toml::from_str(&content).unwrap();
    let table = value.as_table_mut().expect("configuration must be a table");
    for key in [
        "out",
        "updates",
        "cryptographicParameters",
        "anonymityRevokers",
        "identityProviders",
        "accounts",
    ] {
        table.remove(key);
    }

    let assembled = smoke_out_dir("assemble_p11");
    let path_value = |path: PathBuf| toml::Value::String(path.to_string_lossy().into_owned());
    table.insert(
        "foundationAccount".into(),
        toml::Value::String(foundation_account),
    );
    table.insert("accounts".into(), path_value(accounts_path));
    table.insert(
        "ars".into(),
        path_value(generated.join("ars/anonymity-revokers.json")),
    );
    table.insert(
        "idps".into(),
        path_value(generated.join("idps/identity-providers.json")),
    );
    table.insert(
        "global".into(),
        path_value(generated.join("global/cryptographic-parameters.json")),
    );
    table.insert(
        "governanceKeys".into(),
        path_value(generated.join("update-keys/governance-keys.json")),
    );
    table.insert(
        "genesisOut".into(),
        path_value(assembled.join("genesis.dat")),
    );
    table.insert(
        "genesisHashOut".into(),
        path_value(assembled.join("genesis_hash")),
    );

    let assemble_toml = assembled.join("config.toml");
    std::fs::write(&assemble_toml, toml::to_string_pretty(&value).unwrap()).unwrap();
    handle_assemble(&assemble_toml, false).expect("P11 assembly must succeed");
    assert_genesis_output(&assembled);
    assert_eq!(
        std::fs::read(generated.join("genesis.dat")).unwrap(),
        std::fs::read(assembled.join("genesis.dat")).unwrap(),
        "generation and assembly must serialize identical P11 genesis data"
    );
}

/// P5 single-baker — single-baker-example-p5.toml
#[test]
fn smoke_generate_p5_single_baker() {
    let out = smoke_out_dir("generate_p5_single_baker");
    let toml = rewrite_generate_toml(&examples_dir().join("single-baker-example-p5.toml"), &out);
    handle_generate(&toml, false)
        .expect("handle_generate must succeed for single-baker-example-p5.toml");
    assert_genesis_output(&out);
}

/// P6 single-baker — single-baker-example-p6.toml
#[test]
fn smoke_generate_p6_single_baker() {
    let out = smoke_out_dir("generate_p6_single_baker");
    let toml = rewrite_generate_toml(&examples_dir().join("single-baker-example-p6.toml"), &out);
    handle_generate(&toml, false)
        .expect("handle_generate must succeed for single-baker-example-p6.toml");
    assert_genesis_output(&out);
}
