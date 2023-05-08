# Concordium Genesis Creator

This page describes how to create the `genesis.dat` file needed for starting up
a node.

## Building the tool

The tool for creating the genesis is a pure Rust project, but it does depend on the [protobuf compiler](https://github.com/protocolbuffers/protobuf#protocol-compiler-installation) version at least 3.15. The minimum supported
Rust version is 1.64. You need the Rust toolchain installed for your platform.
The easiest way to do that is to install [rustup](https://rustup.rs/).

To build it
- make sure to check out git submodules
  ```console
  git submodule update --init --recursive
  ```
- Run the build
  ```console
  cargo build --release
  ```

This will produce a single binary `./target/release/genesis-creator`.

## Using the tool

The tool has two modes: `generate` that can generate a new genesis, and `assemble` that can produce a genesis from existing files (for example to regenerate the Mainnet `genesis.dat`).

## The `generate` mode
To generate a new genesis, run the command
```console
genesis-creator generate --config genesis-config.toml
```
where `genesis-config.toml` is a TOML file specifying the genesis. The TOML configuration file should specify
- the protocol version that the chain should start in
- the cryptographic parameters
- the anonymity revokers,
- the identity providers
- the genesis accounts
- the chain updates keys
- the genesis parameters
- where to output
  * the `genesis.dat` file
  * the cryptographic parameters
  * the private genesis account keys
  * the private chain updates keys
  * the genesis baker keys
  * the private identity provider keys
  * the private anonymity revoker keys

### Specifying the protocol version
The line
```toml
protocolVersion = n
```
specifies that the chain should start up in protocol version `n`.

### Specifying output paths
The output paths are to be specified with lines of the form
```toml
[out]
updateKeys = "some/path/update-keys"
accountKeys = "some/path/accounts-out"
bakerKeys = "some/path/bakers-out"
identityProviders = "some/path/idps-out"
anonymityRevokers = "some/path/ars-out"
genesis = "some/path/genesis.dat"
genesisHash = "some/path/genesis_hash"
cryptographicParameters = "some/path/global"
```
If the line `deleteExisting = true` is added, then all the files and folders
will be deleted (if they exist) before creating them. This will happen even if
the tool happens to fail in some later stage.

### Specifying the cryptographic parameters
The cryptographic parameters can either be generated or contructed from a file with existing cryptographic parameters.

The lines
```toml
[cryptographicParameters]
kind = "generate"
genesisString = "Test genesis string."
```
are for generating the cryptographic parameters using the given genesis string.

The lines
```toml
[cryptographicParameters]
kind = "existing"
source = "some/path/cryptographic-parameters.json"
```
are for using the cryptographic parameters in the given file.

### Specifying the anonymity revokers
Each genesis anonymity revoker can either be generated freshly or given from a file with an existing anonymity revoker.

The lines
```toml
[[anonymityRevokers]]
kind = "fresh"
id = 1
```
specify that an anonymity revoker should be generated freshly and have id 1. If more than one anonymity revoker should be generated, adding the line `repeat = n` will generate `n` anonymity revokers. As example, the lines
```toml
[[anonymityRevokers]]
kind = "fresh"
id = 1
repeat = 3
```
specify that 3 anonymity revokers should be genereated, starting with ids starting from 1.

The lines
```toml
[[anonymityRevokers]]
kind = "existing"
source = "some/path/ar-info.json"
```
are for using the anonymity revoker from the given file.

It is possible to add several anonymity revokers using existing ones by having the lines above several times, also while some are being generated freshly. As an example, the lines
```toml
[[anonymityRevokers]]
kind = "fresh"
id = 3
repeat = 3

[[anonymityRevokers]]
kind = "existing"
source = "some/path/ar-info-1.json"

[[anonymityRevokers]]
kind = "existing"
source = "some/path/ar-info-2.json"
```
specify that two existing anonymity revokers should be used, while three new anonymity revokers should be generated, with ids starting from 3. It is not allowed to have several anonymity revokers with the same id, so existing anonymity revokers **must** have different ids, and their ids must be different from the generated ones.


### Specifying the identity providers
Each genesis identity provider can either be generated freshly or given from a file with an existing identity provider.

The lines
```toml
[[identityProviders]]
kind = "fresh"
id = 0
```
specify that an identity provider should be generated freshly and have id 0. If more than one identity provider should be generated, adding the line `repeat = n` will generate `n` identity providers. As example, the lines
```toml
[[identityProviders]]
kind = "fresh"
id = 0
repeat = 3
```
specify that 3 identity providers should be genereated, starting with ids starting from 0.

The lines
```toml
[[identityProviders]]
kind = "existing"
source = "some/path/ip-info.json"
```
are for using the identity provider from the given file.

It is possible to add several identity providers using existing ones by having the lines above several times, also while some are being generated freshly. As an example, the lines
```toml
[[identityProviders]]
kind = "fresh"
id = 2
repeat = 3

[[identityProviders]]
kind = "existing"
source = "some/path/ip-info-0.json"

[[identityProviders]]
kind = "existing"
source = "some/path/ip-info-1.json"
```
specify that two existing identity providers should be used, while three new identity providers should be generated, with ids starting from 2. It is not allowed to have several identity providers with the same id, so existing identity providers **must** have different ids, and their ids must be different from the generated ones.

### Specifying the genesis accounts
Each genesis account can either be generated freshly or given from a file with an existing account.
The lines
```toml
[[accounts]]
kind = "fresh"
balance = "1000000000"
template = "foundation"
identityProvider = 0
numKeys = 1
threshold = 1
foundation = true
```
specify that an account should be generated freshly with a balance of 1000000000, using the identity provider with id 0, having one account key with threshold 1. The `foundation = true` specifies that the account is the foundation account. The `template = "foundation"` specifies that the account keys output file will be prefixed with the string `"foundation"`. If more than one account should be generated, adding the line `repeat = n` will generate `n` accounts. As an example, the lines
```toml
[[accounts]]
kind = "fresh"
balance = "1000000000"
template = "foundation"
identityProvider = 0
numKeys = 1
threshold = 1
repeat = 10000
foundation = true
```
specify that 1000 accounts should be generated. In this case the first one will be the foundation account.

An existing account can be added using the lines

```toml
[[accounts]]
kind = "existing"
source = "some/path/account.json"
balance = "1000000000"
```

If an account should be a baker account, the line `stake = x` is needed, where `x` is how much the baker should start staking with. The line `restakeEarnings = true` is added, the baker will be configured to restake earnings. In case of using an existing account that should be a baker, existing baker credential keys can be specified using the line `bakerKeys = "..."`. Otherwise, new baker keys will be generated and output.

As an example, the lines
```toml
[[accounts]]
kind = "fresh"
balance = "1000000000"
template = "foundation"
identityProvider = 0
numKeys = 1
threshold = 1
repeat = 10
foundation = true


[[accounts]]
kind = "fresh"
balance = "1000000000"
stake = "500000000"
template = "baker"
identityProvider = 0
numKeys = 1
threshold = 1
repeat = 5

[[accounts]]
kind = "existing"
source = "account.json"
balance = "1000000000"

[[accounts]]
kind = "existing"
source = "baker.json"
balance = "1000000000"
stake = "500000000"
restakeEarnings = true
bakerKeys = "baker-credentials.json"

[[accounts]]
kind = "existing"
source = "baker2.json"
balance = "1000000000"
stake = "500000000"
```
specify that
- 10 (non-baker) accounts should be genereated with the first one being the foundation account.
- 5 baker accounts should be generated.
- the existing account given by the file `account.json` should be included in the genesis.
- the existing account given by the file `baker.json` should be included in the genesis,
  and this baker should restake earnings. This baker's keys are given by the file the file `baker-credentials.json`.
- the existing account given by the file `baker2.json` should be included in the genesis.
  No file with baker keys provided, so these will be generated freshly and output.

### Specifying the chain updates root and level 1 keys
Each root and level 1 keys can either be generated freshly or given from a file with an existing key.
The lines
```toml
[updates]
root = { threshold = ..., keys = [...]}
level1 = { threshold = ..., keys = [...]}
```
specify the thresholds and the root and level1 keys. The treshold is an integer, and each element of the keys list should be either be of the form
```toml
{kind = "fresh", repeat = n}
```
for generating `n` fresh keys, or
```toml
{kind = "existing", source = "some/path/key.json"}
```
for using an existing key. As an example,
```toml
[updates]
root = { threshold = 5, keys = [{kind = "fresh", repeat = 7}, {kind = "existing", source = "root-key.json"}]}
level1 = { threshold = 7, keys = [{kind = "fresh", repeat = 15}]}
```
specifies that
  - 7 freshly generated root keys and one existing root key should be used, with threshold 5, and
  - 15 freshly generated level 1 keys should be used, with threshold 7.


### Specifying the chain updates level 2 keys
Each level 2 key can either be generated freshly or given from a file with an existing key.
The keys are specified in the same format as for the root and level 1 keys. For example, the lines
```toml
[updates.level2]
keys = [{kind = "fresh", repeat = 15}]
```
specify that 15 level 2 keys should be genereated freshly and used in the genesis.
After this line it should be specified for each level 2 update which level 2 keys can change do it, and how many of that are needed to sign a chain update transaction. This is specified by lines of the form
```toml
someChainUpdate = {authorizedKeys = [...], threshold = ...}
```
where the value of `authorizedKeys` should be a list of indices specifying the level 2 keys that are allowed to do the update `someChainUpdate`, and `threshold` should be an integer. The concrete possible chain updates depends on the protocol version. In protocol version 1-3 the lines
```toml
[updates.level2]
keys = [{kind = "fresh", repeat = 15}]
emergency = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
protocol = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
electionDifficulty = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
euroPerEnergy = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
microCCDPerEuro = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
foundationAccount = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
mintDistribution = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
transactionFeeDistribution = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
gasRewards = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
poolParameters = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
addAnonymityRevoker = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
addIdentityProvider = {authorizedKeys = [0,1,2,3,4,5,6], threshold = 7}
```
specify that 15 level 2 keys should be generated, and that all of the level 2 chain updates can be done by the first 7 of the level 2 keys.

In protocol version 4, there are two more chain updates: one that updates the cooldown parameters, and one that updates the time parameters. In this case, adding the lines
```toml
cooldownParameters = {authorizedKeys = [...], threshold = ...}
timeParameters = {authorizedKeys = [...], threshold = ...}
```
would specify these two additional chain updates.

### Specifying the genesis parameters
The core genesis parameters, the initial leadership election nonce, the finalization parameters and the initial chain parameters shall be specified.

The core genesis parameters and the initial leadership election nonce is to be specified with lines of the form
```toml
[parameters]
genesisTime = "2021-06-09T06:00:00Z"
slotDuration = 250
leadershipElectionNonce = "60ab0feb036f5e3646f957085238f02fea83df5993db8e784e11500969af9420"
epochLength = 14400
maxBlockEnergy = 3_000_000
```
where the concrete values above are replaced with those desired.

The finalization parameteres is to be specified with lines of the form
```toml
[parameters.finalization]
minimumSkip = 0
committeeMaxSize = 1000
waitingTime = 100
skipShrinkFactor = 0.5
skipGrowFactor = 2
delayShrinkFactor = 0.5
delayGrowFactor = 2
allowZeroDelay = true
```
where the concrete values above are replaced with those desired.

The chain parameters depend on the chain parameters version. In chain parameters version 0, the chain parameters is to be specified with lines of the form
```toml
[parameters.chain]
version = "v0"
electionDifficulty = 0.025
euroPerEnergy = 0.00002
microCCDPerEuro = 500_000
accountCreationLimit = 10
bakerCooldownEpochs = 166
minimumThresholdForBaking = "2500000000"
[parameters.chain.rewardParameters]
mintDistribution = { mintPerSlot = 0.0000000007555665, bakingReward = 0.85, finalizationReward = 0.05 }
transactionFeeDistribution = { baker = 0.45, gasAccount = 0.45 }
gASRewards = { baker = 0.25, finalizationProof = 0.005, accountCreation = 0.02, chainUpdate = 0.005 }
```
where the concrete values above are replaced with those desired.

In chain parameters version 1, the chain parameters is to be specified with lines of the form
```toml
[parameters.chain]
version = "v1"
electionDifficulty = 0.025
euroPerEnergy = 0.00002
microCCDPerEuro = 50_000
accountCreationLimit = 10
[parameters.chain.timeParameters]
rewardPeriodLength = 24
mintPerPayday = 2.61157877e-4
[parameters.chain.poolParameters]
"passiveFinalizationCommission" = 1.0
"passiveBakingCommission" = 0.12
"passiveTransactionCommission" = 0.12
"finalizationCommissionRange" = {"max" = 1.0,"min" = 1.0}
"bakingCommissionRange" = {"max" = 0.1,"min" = 0.1}
"transactionCommissionRange" = {"max" = 0.1,"min" = 0.1}
"minimumEquityCapital" = "1000"
"capitalBound" = 0.1
"leverageBound" = {"denominator" = 1, "numerator" = 3}
[parameters.chain.cooldownParameters]
"poolOwnerCooldown" = 1814400
"delegatorCooldown" = 1209600
[parameters.chain.rewardParameters]
mintDistribution = { mintPerSlot = 0.0000000007555665, bakingReward = 0.85, finalizationReward = 0.05 }
transactionFeeDistribution = { baker = 0.45, gasAccount = 0.45 }
gASRewards = { baker = 0.25, finalizationProof = 0.005, accountCreation = 0.02, chainUpdate = 0.005 }
```

## The `assemble` mode
To generate a genesis from existing file, run
```console
genesis-creator assemble -- assemble-config.toml
```
where `assemble-config.toml` is a TOML file specifying the genesis. The TOML configuration file should specify
- the protocol version
- the foundation account
- a path to a file with the genesis accounts
- a path to a file with the identity providers
- a path to a file with the anonymity revokers
- a path to a file with the governance keys
- a path to a file with the cryptographic parameters
- where to output the `genesis.dat` file
- the genesis parameters

The protocol version, the foundation account and the paths are to be specified with the lines
```toml
protocolVersion = n
foundationAccount = "..."
accounts = "path//accounts.json"
idps = "path//identity-providers.json"
ars = "path/anonymity-revokers.json"
governanceKeys = "path/governance-keys.json"
global = "path/cryptographic-parameters.json"
genesisOut = "path/genesis.dat"
genesisHash = ".path/genesis_hash"
```

The genesis parameters are specified in the same format as when generating a new genesis, see the section above.
