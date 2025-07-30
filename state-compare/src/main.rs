//! A tool to compare the state at two given blocks, potentially in two
//! different nodes.
//!
//! The program will print a collection of diffs between the various parts of
//! the states between the two blocks.

use std::fmt::Display;

use anyhow::Context;
use clap::Parser;
use concordium_rust_sdk::{
    cis2::TokenId, endpoints, id::types::AccountAddress, protocol_level_tokens, types::{
        hashes::BlockHash, smart_contracts::ModuleReference, ContractAddress, ProtocolVersion,
    }, v2::{self, IntoBlockIdentifier, Scheme}
};
use futures::{StreamExt, TryStreamExt};
use indicatif::ProgressBar;
use pretty_assertions::Comparison;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::EnvFilter;

/// Compares the given values and prints a pretty diff with the given message if
/// they are not equal.
macro_rules! compare {
    ($v1:expr, $v2:expr, $($arg:tt)*) => {
        if $v1 != $v2 {
            warn!("{} differs:\n{}", format!($($arg)*), Comparison::new(&$v1, &$v2))
        }
    };
}

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    /// GRPC V2 interface of the node.
    #[arg(
        long,
        default_value = "http://localhost:20000",
        env = "STATE_COMPARE_NODE1"
    )]
    node1: endpoints::Endpoint,

    /// Optionally, a GRPC V2 interface of a second node to compare state with.
    ///
    /// If not provided the first node is used.
    #[arg(long, env = "STATE_COMPARE_NODE2")]
    node2: Option<endpoints::Endpoint>,

    /// The first block to compare state against.
    ///
    /// If not given the default is the last finalized block `before` the last
    /// protocol update.
    #[arg(long, env = "STATE_COMPARE_BLOCK1")]
    block1: Option<BlockHash>,

    /// The second block where to compare state.
    ///
    /// If not given the default is the genesis block of the current protocol.
    #[arg(long, env = "STATE_COMPARE_BLOCK2")]
    block2: Option<BlockHash>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env()?,
        )
        .init();

    print!("Attempt to create client 1 here..."); 

    // create the client - if https is un the URI, we need to create and add TLS config, otherwise client can be created directly
    let mut client1 = if args.node1.uri().scheme() == Some(&Scheme::HTTPS) {
        info!("Scheme contained https - will attempt to construct client with TLS");
        v2::Client::new(
    args.node1
                .tls_config(tonic::transport::channel::ClientTlsConfig::new())
                .context("Unable to construct tls")?
        )
    } else {
        v2::Client::new(args.node1)
    }.await?;


    let mut client2 = match args.node2 {
        Some(ep) => v2::Client::new(ep).await?,
        None => client1.clone(),
    };

    let ci1 = client1.get_consensus_info().await?;
    let ci2 = client2.get_consensus_info().await?;

    let block1 = match args.block1 {
        Some(bh) => bh,
        None => {
            client1
                .get_block_info(ci1.current_era_genesis_block)
                .await?
                .response
                .block_parent
        }
    };

    let block2 = match args.block2 {
        Some(bh) => bh,
        None => ci1.current_era_genesis_block,
    };

    let (pv1, pv2) = get_protocol_versions(&mut client1, &mut client2, block1, block2).await?;

    info!(
        "Comparing states in blocks {block1} (protocol version {pv1}) and {block2} (protocol \
         version {pv2})."
    );


    compare!(ci1.genesis_block, ci2.genesis_block, "Genesis blocks");

    compare_accounts(&mut client1, &mut client2, block1, block2).await?;

    compare_modules(&mut client1, &mut client2, block1, block2).await?;

    compare_instances(&mut client1, &mut client2, block1, block2).await?;

    compare_passive_delegators(&mut client1, &mut client2, block1, block2).await?;

    compare_active_bakers(&mut client1, &mut client2, block1, block2).await?;

    compare_baker_pools(&mut client1, &mut client2, block1, block2).await?;

    compare_update_queues(&mut client1, &mut client2, block1, block2).await?;

    let (tokens_node_1, tokens_node_2) = fetch_token_lists(&mut client1, &mut client2, block1, block2).await?;
    let token_ids = compare_token_identities(&tokens_node_1, &tokens_node_2, block1, block2).await?;
    compare_token_info_for_ids(&mut client1, &mut client2, block1, block2, &token_ids).await?;

    info!("Done!");

    Ok(())
}



/// Get the protocol version for the two blocks.
/// This currently uses the rewards overview call since this is the cheapest
/// call that returns it.
async fn get_protocol_versions(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<(ProtocolVersion, ProtocolVersion)> {
    let t1 = client1.get_tokenomics_info(&block1).await?;
    let t2 = client2.get_tokenomics_info(&block2).await?;
    let p1 = match t1.response {
        concordium_rust_sdk::types::RewardsOverview::V0 { data } => data.protocol_version,
        concordium_rust_sdk::types::RewardsOverview::V1 { common, .. } => common.protocol_version,
    };
    let p2 = match t2.response {
        concordium_rust_sdk::types::RewardsOverview::V0 { data } => data.protocol_version,
        concordium_rust_sdk::types::RewardsOverview::V1 { common, .. } => common.protocol_version,
    };
    Ok((p1, p2))
}

async fn compare_update_queues(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<()> {
    info!("Checking update queues.");

    let s1 = client1
        .get_next_update_sequence_numbers(block1)
        .await?
        .response;

    let s2 = client2
        .get_next_update_sequence_numbers(block2)
        .await?
        .response;

    compare!(s1, s2, "Sequence numbers");

    let q1 = client1
        .get_block_pending_updates(block1)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;

    let q2 = client2
        .get_block_pending_updates(block2)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;

    // PendingUpdate unfortunately does not impl Eq so we'll settle for a comparison
    // of their debug representations Should be good enough to produce a nice
    // diff.
    compare!(format!("{q1:#?}"), format!("{q2:#?}"), "Pending updates");

    Ok(())
}

async fn compare_account_lists(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<Vec<AccountAddress>> {
    let mut accounts1 = client1
        .get_account_list(block1)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;
    accounts1.sort_unstable();
    let mut accounts2 = client2
        .get_account_list(block2)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;
    accounts2.sort_unstable();
    compare_iters(
        "Account",
        block1,
        block2,
        accounts1.iter(),
        accounts2.iter(),
    );
    Ok(accounts1)
}

async fn compare_instance_lists(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<Vec<ContractAddress>> {
    let mut cs1 = client1
        .get_instance_list(block1)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;
    cs1.sort_unstable();
    let mut cs2 = client2
        .get_instance_list(block2)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;
    cs2.sort_unstable();
    compare_iters("Instance", block1, block2, cs1.iter(), cs2.iter());
    Ok(cs1)
}

async fn compare_module_lists(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<Vec<ModuleReference>> {
    let mut ms1 = client1
        .get_module_list(block1)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;
    ms1.sort_unstable();
    let mut ms2 = client2
        .get_module_list(block2)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;
    ms2.sort_unstable();
    compare_iters("Module", block1, block2, ms1.iter(), ms2.iter());
    Ok(ms1)
}

/// Compare two iterators that are assumed to yield elements in increasing
/// order. Print any discrepancies.
fn compare_iters<A: Display + PartialOrd>(
    msg: &str,
    block1: BlockHash,
    block2: BlockHash,
    i1: impl Iterator<Item = A>,
    i2: impl Iterator<Item = A>,
) {
    let mut i1 = i1.peekable();
    let mut i2 = i2.peekable();
    while let Some(a1) = i1.peek() {
        if let Some(a2) = i2.peek() {
            if a1 < a2 {
                warn!("{msg} {a1} appears in {block1} but not in {block2}.",);
                i1.next();
            } else if a2 < a1 {
                warn!("{msg} {a2} appears in {block2} but not in {block1}.",);
                i2.next();
            } else {
                i1.next();
                i2.next();
            }
        } else {
            warn!("{msg} {a1} appears in {block1} but not in {block2}.",);
            i1.next();
        }
    }
}

async fn compare_accounts(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<()> {
    info!("Comparing account lists.");
    let accounts1 = compare_account_lists(client1, client2, block1, block2).await?;

    info!("Got {} accounts.", accounts1.len());

    let bar = ProgressBar::new(accounts1.len() as u64);

    info!("Querying and comparing all accounts.");
    for acc in accounts1 {
        let accid = acc.into();
        let (a1, a2) = futures::try_join!(
            client1.get_account_info(&accid, block1),
            client2.get_account_info(&accid, block2)
        )?;

        let mut a1 = a1.response;
        let mut a2 = a2.response;

        bar.inc(1);
        // We ignore the order of transactions in the release schedules since they are
        // not guaranteed to be in any specific order.
        for s in a1.account_release_schedule.schedule.iter_mut() {
            s.transactions.sort_unstable();
        }
        for s in a2.account_release_schedule.schedule.iter_mut() {
            s.transactions.sort_unstable();
        }

        // compare PLT tokens
        let tokens1 = &a1.tokens;
        let tokens2 = &a2.tokens;
        compare!(tokens1, tokens2, "PLT Tokens for account: {accid}");

        // compare account info
        compare!(a1, a2, "Account {accid}");
    }

    bar.finish_and_clear();

    Ok(())
}

async fn compare_modules(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<()> {
    info!("Comparing all modules.");
    let ms1 = compare_module_lists(client1, client2, block1, block2).await?;
    let bar = ProgressBar::new(ms1.len() as u64);

    for m in ms1 {
        bar.inc(1);
        let (m1, m2) = futures::try_join!(
            client1.get_module_source(&m, block1),
            client2.get_module_source(&m, block2)
        )?;

        compare!(m1.response, m2.response, "Module {m}");
    }

    bar.finish_and_clear();

    Ok(())
}

async fn compare_instances(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<()> {
    info!("Querying all contracts.");
    let cs1 = compare_instance_lists(client1, client2, block1, block2).await?;

    let bar = ProgressBar::new(cs1.len() as u64);

    for c in cs1 {
        bar.inc(1);
        let (ci1, ci2) = futures::try_join!(
            client1.get_instance_info(c, block1),
            client2.get_instance_info(c, block2)
        )?;

        compare!(ci1.response, ci2.response, "Contract instance {c}");

        let (state1, state2) = futures::try_join!(
            client1.get_instance_state(c, block1),
            client2.get_instance_state(c, block2)
        )?;

        let mut state1 = state1.response;
        let mut state2 = state2.response;

        while let Some(s1) = state1.next().await.transpose()? {
            if let Some(s2) = state2.next().await.transpose()? {
                compare!(s1, s2, "State for {c}");
            } else {
                warn!("State for {c} not present in block 2.");
            }
        }

        if state2.next().await.is_some() {
            warn!("State for {c} not present in block 1.");
        }
    }

    bar.finish_and_clear();

    Ok(())
}

async fn compare_passive_delegators(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<()> {
    info!("Checking passive delegators.");

    let mut passive1 = client1
        .get_passive_delegators(block1)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;

    let mut passive2 = client2
        .get_passive_delegators(block2)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;

    passive1.sort_unstable_by_key(|x| x.account);
    passive2.sort_unstable_by_key(|x| x.account);

    compare!(passive1, passive2, "Passive delegators");

    Ok(())
}

async fn compare_active_bakers(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<()> {
    info!("Checking active bakers.");

    let (ei1, ei2) = futures::try_join!(
        client1.get_election_info(block1),
        client2.get_election_info(block2)
    )?;

    compare!(ei1.response, ei2.response, "Election info");

    Ok(())
}

async fn compare_baker_pools(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
) -> anyhow::Result<()> {
    info!("Checking baker pools.");

    let mut pools1 = client1
        .get_baker_list(block1)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;

    let mut pools2 = client2
        .get_baker_list(block2)
        .await?
        .response
        .try_collect::<Vec<_>>()
        .await?;

    pools1.sort_unstable();
    pools2.sort_unstable();

    compare_iters("Pool", block1, block2, pools1.iter(), pools2.iter());

    for pool in pools1 {
        let (d1, d2) = futures::join!(
            client1.get_pool_delegators(block1, pool),
            client2.get_pool_delegators(block2, pool)
        );

        let (d1, d2) = match (d1, d2) {
            (Ok(d1), Ok(d2)) => (d1.response, d2.response),
            (Ok(_), Err(e)) => {
                warn!("Failed to get delegators for pool {pool} in block 2: {e}");
                continue;
            }
            // The pool should definitely appear in the first block, we got the list of pools from
            // that block.
            (Err(e), Ok(_)) => {
                return Err(e).with_context(|| format!("Failed to get delegators for pool {pool}"))
            }
            (Err(e1), Err(e2)) => {
                return Err(e2)
                    .context(e1)
                    .with_context(|| format!("Failed to get delegators for pool {pool}"))
            }
        };

        let mut ds1 = d1.try_collect::<Vec<_>>().await?;
        let mut ds2 = d2.try_collect::<Vec<_>>().await?;

        ds1.sort_unstable_by_key(|x| x.account);
        ds2.sort_unstable_by_key(|x| x.account);

        compare!(ds1, ds2, "Delegators for pool {pool}");

        let (p1, p2) = futures::join!(
            client1.get_pool_info(block1, pool),
            client2.get_pool_info(block2, pool)
        );

        let (p1, p2) = match (p1, p2) {
            (Ok(p1), Ok(p2)) => (p1.response, p2.response),
            (Ok(_), Err(e)) => {
                warn!("Failed to get pool {pool} in block 2: {e}");
                continue;
            }
            // The pool should definitely appear in the first block, we got the list of pools from
            // that block.
            (Err(e), Ok(_)) => return Err(e).with_context(|| format!("Failed to get pool {pool}")),
            (Err(e1), Err(e2)) => {
                return Err(e2)
                    .context(e1)
                    .with_context(|| format!("Failed to get pool {pool}"))
            }
        };

        compare!(p1, p2, "Pool {pool}");
    }

    Ok(())
}


async fn fetch_token_lists(  
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,) -> anyhow::Result<(Vec<protocol_level_tokens::TokenId>, Vec<protocol_level_tokens::TokenId>)> {
     
    info!("Fetching PLT token lists.");
    let tokens1 = client1.get_token_list(block1).await?.response
        .try_collect::<Vec<_>>()
        .await?;
    let tokens2 = client2.get_token_list(block2).await?.response
        .try_collect::<Vec<_>>()
        .await?;

    Ok((tokens1, tokens2))
}

async fn compare_token_identities(
    tokens_node_1: &[protocol_level_tokens::TokenId],
    tokens_node_2: &[protocol_level_tokens::TokenId],
    _block1: BlockHash,
    _block2: BlockHash,
) -> anyhow::Result<Vec<protocol_level_tokens::TokenId>> {

    info!("Comparing PLT token identities.");

    // We need to clone the tokens to sort them, otherwise the borrowing would not complain
    let mut tokens_node_1 = tokens_node_1.to_vec();
    let mut tokens_node_2 = tokens_node_2.to_vec();

    info!("Node 1 has {} tokens, Node 2 has {} tokens.", tokens_node_1.len(), tokens_node_2.len());
    info!(tokens_node_1 = ?tokens_node_1, tokens_node_2 = ?tokens_node_2, "Token identities");
    // Sort the tokens to ensure a consistent order for comparison.
    tokens_node_1.sort_unstable();
    tokens_node_2.sort_unstable();

    compare!(tokens_node_1, tokens_node_2, "PLT Token identities");

    info!("after comparison, Node 1 has {} tokens, Node 2 has {} tokens.", tokens_node_1.len(), tokens_node_2.len());
    let result: Vec<protocol_level_tokens::TokenId> = tokens_node_1
    .iter()
    .filter(|id| tokens_node_2.contains(id))
    .cloned()
    .collect();

    info!("Returning result with {} common tokens.", result.len());
    Ok(result)
}



async fn compare_token_info_for_ids(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
    token_ids: &[protocol_level_tokens::TokenId],
) -> anyhow::Result<()> {

    info!("compare_token_info_for_ids");
    for token_id in token_ids {
        info!("Getting token info for token ID {:?} from block {}", token_id, block1.into_block_identifier());
        let info1 = client1.get_token_info(token_id.clone(), &block1.into_block_identifier()).await;
        info!("Getting token info for token ID {:?} from block {}", token_id, block2.into_block_identifier());
        let info2 = client2.get_token_info(token_id.clone(), &block2.into_block_identifier()).await;

       match (info1, info2) {
            (Ok(info1), Ok(info2)) => {
                info!(?token_id, "Comparing token info");
                compare!(
                    info1.response.token_state.decimals,
                    info2.response.token_state.decimals,
                    "Token Info for ID {:?}",
                    token_id
                );
            }
            (Err(e1), Err(e2)) => {
                warn!(?token_id, "Token info not found on either node: {:?} / {:?}", e1, e2);
            }
            (Err(e), _) => {
                warn!(?token_id, "Token info missing on node 1: {:?}", e);
            }
            (_, Err(e)) => {
                warn!(?token_id, "Token info missing on node 2: {:?}", e);
            }
        }
    }
    Ok(())
}