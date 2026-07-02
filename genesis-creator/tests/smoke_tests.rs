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

/// Rewrites the `[out]` section of a generate-mode TOML so all output paths
/// point into `out_dir`.  Returns the path to the rewritten TOML file.
fn rewrite_generate_toml(toml_path: &Path, out_dir: &Path) -> PathBuf {
    let content = std::fs::read_to_string(toml_path).expect("Failed to read example TOML");
    let mut value: toml::Value = toml::from_str(&content).expect("Example TOML must be valid TOML");

    let abs = |name: &str| toml::Value::String(out_dir.join(name).to_string_lossy().into_owned());

    if let Some(out) = value.get_mut("out") {
        let t = out.as_table_mut().expect("[out] must be a table");
        t.insert("accountKeys".into(), abs("accounts"));
        t.insert("updateKeys".into(), abs("update-keys"));
        t.insert("bakerKeys".into(), abs("bakers"));
        t.insert("identityProviders".into(), abs("idps"));
        t.insert("anonymityRevokers".into(), abs("ars"));
        t.insert("genesis".into(), abs("genesis.dat"));
        t.insert("genesisHash".into(), abs("genesis_hash"));
        t.insert("cryptographicParameters".into(), abs("global"));
        t.insert("deleteExisting".into(), toml::Value::Boolean(false));
    }

    let rewritten = toml::to_string_pretty(&value).expect("Failed to serialise modified TOML");
    let dest = out_dir.join("config.toml");
    std::fs::write(&dest, rewritten).expect("Failed to write modified TOML");
    dest
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
