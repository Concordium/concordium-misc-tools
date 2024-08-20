//! A tool to compare the state at two given blocks, potentially in two
//! different nodes.
//!
//! The program will print a collection of diffs between the various parts of
//! the states between the two blocks.

use std::fmt::Display;

use clap::Parser;
use concordium_rust_sdk::{
    endpoints,
    id::types::AccountAddress,
    types::{
        hashes::BlockHash, smart_contracts::ModuleReference, AccountInfo, ContractAddress,
        ProtocolVersion,
    },
    v2,
};
use futures::{StreamExt, TryStreamExt};
use indicatif::ProgressBar;
use pretty_assertions::Comparison;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::EnvFilter;

/// Compares the given values and prints a pretty diff with the given message if they are not equal.
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
    /// If not given the default is the last finalized block `before` the last protocol update.
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

    let mut client1 = v2::Client::new(args.node1).await?;
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

    info!("Comparing state in blocks {block1} (protocol version {pv1}) and {block2} (protocol version {pv2}).");

    compare!(ci1.genesis_block, ci2.genesis_block, "Genesis blocks");

    compare_accounts(&mut client1, &mut client2, block1, block2, pv1, pv2).await?;

    compare_modules(&mut client1, &mut client2, block1, block2).await?;

    compare_instances(&mut client1, &mut client2, block1, block2).await?;

    compare_passive_delegators(&mut client1, &mut client2, block1, block2, pv1, pv2).await?;

    compare_active_bakers(&mut client1, &mut client2, block1, block2, pv1, pv2).await?;

    compare_baker_pools(&mut client1, &mut client2, block1, block2, pv1, pv2).await?;

    compare_update_queues(&mut client1, &mut client2, block1, block2).await?;

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
) -> anyhow::Result<bool> {
    let s1 = client1
        .get_next_update_sequence_numbers(block1)
        .await?
        .response;
    let s2 = client2
        .get_next_update_sequence_numbers(block2)
        .await?
        .response;
    if s1 != s2 {
        warn!("    Sequence numbers differ: {s1:#?} {s2:#?}")
    }
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
    if q1.len() != q2.len() {
        return Ok(false);
    }
    Ok(s1 != s2)
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
                warn!("    {msg} {a1} appears in {block1} but not in {block2}.",);
                i1.next();
            } else if a2 < a1 {
                warn!("    {msg} {a2} appears in {block2} but not in {block1}.",);
                i2.next();
            } else {
                i1.next();
                i2.next();
            }
        } else {
            warn!("    {msg} {a1} appears in {block1} but not in {block2}.",);
            i1.next();
        }
    }
}

async fn compare_accounts(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
    pv1: ProtocolVersion,
    pv2: ProtocolVersion,
) -> anyhow::Result<()> {
    info!("Comparing account lists.");
    let accounts1 = compare_account_lists(client1, client2, block1, block2).await?;

    info!("Got {} accounts.", accounts1.len());

    let bar = ProgressBar::new(accounts1.len() as u64);

    info!("Querying and comparing all accounts.");
    for acc in accounts1 {
        let mut a_client = client1.clone();
        let mut a_client2 = client2.clone();
        let accid = acc.into();
        let (mut a1, mut a2) = futures::try_join!(
            a_client.get_account_info(&accid, block1),
            a_client2.get_account_info(&accid, block2)
        )?;

        bar.inc(1);
        // We ignore the order of transactions in the release schedules since they are
        // not guaranteed to be in any specific order.
        for s in a1.response.account_release_schedule.schedule.iter_mut() {
            s.transactions.sort_unstable();
        }
        for s in a2.response.account_release_schedule.schedule.iter_mut() {
            s.transactions.sort_unstable();
        }
        if a1.response != a2.response {
            match (&a1.response.account_stake, a2.response.account_stake) {
                (None, None) => {
                    warn!("Account {} differs. It does not have stake either in {block1} or {block2}.", a1.response.account_address);
                }
                (None, Some(_)) => {
                    warn!("Account {} differs. It does not have stake in {block1} but does in {block2}.", a1.response.account_address);
                }
                (Some(_), None) => {
                    warn!("Account {} differs. It does have stake in {block1} but does not in {block2}.", a1.response.account_address);
                }
                (Some(s1), Some(s2)) => {
                    // This is special case handling of P3->P4 upgrade.
                    if pv1 == ProtocolVersion::P3 && pv2 == ProtocolVersion::P4 {
                        match s1 {
                            concordium_rust_sdk::types::AccountStakingInfo::Baker {
                                pool_info: None,
                                ..
                            } => match s2 {
                                concordium_rust_sdk::types::AccountStakingInfo::Baker {
                                    staked_amount,
                                    restake_earnings,
                                    baker_info,
                                    pending_change,
                                    pool_info: Some(_),
                                } => {
                                    let s2_no_pool =
                                        concordium_rust_sdk::types::AccountStakingInfo::Baker {
                                            staked_amount,
                                            restake_earnings,
                                            baker_info,
                                            pending_change,
                                            pool_info: None,
                                        };
                                    let a2_no_pool = AccountInfo {
                                        account_stake: Some(s2_no_pool),
                                        ..a2.response
                                    };
                                    if a1.response != a2_no_pool {
                                        warn!("Account {} differs. It does have stake in both {block1} and {block2}.", a1.response.account_address);
                                    }
                                }
                                _ => {
                                    warn!("Account {} differs. It does have stake in both {block1} and {block2}.", a1.response.account_address);
                                }
                            },
                            _ => {
                                warn!("Account {} differs. It does have stake in both {block1} and {block2}.", a1.response.account_address);
                            }
                        }
                    } else {
                        warn!(
                            "Account {} differs. It does have stake in both {block1} and {block2}.",
                            a1.response.account_address
                        );
                    }
                }
            }
        }
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
    eprintln!("Comparing all modules.");
    let ms1 = compare_module_lists(client1, client2, block1, block2).await?;
    let bar = ProgressBar::new(ms1.len() as u64);

    for m in ms1 {
        bar.inc(1);
        let (m1, m2) = futures::try_join!(
            client1.get_module_source(&m, block1),
            client2.get_module_source(&m, block2)
        )?;
        if m1.response != m2.response {
            warn!("Module {m} differs.");
        }
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
    eprintln!("Querying all contracts.");
    let cs1 = compare_instance_lists(client1, client2, block1, block2).await?;

    let bar = ProgressBar::new(cs1.len() as u64);

    for c in cs1 {
        bar.inc(1);
        let (ci1, ci2) = futures::try_join!(
            client1.get_instance_info(c, block1),
            client2.get_instance_info(c, block2)
        )?;
        if ci1.response != ci2.response {
            warn!("Instance {c} differs.");
        }
        let (mut state1, mut state2) = futures::try_join!(
            client1.get_instance_state(c, block1),
            client2.get_instance_state(c, block2)
        )?;
        while let Some(s1) = state1.response.next().await.transpose()? {
            if let Some(s2) = state2.response.next().await.transpose()? {
                if s1 != s2 {
                    warn!("State differs for {c}.");
                    break;
                }
            } else {
                warn!("State differs for {c}.");
                break;
            }
        }
        if state2.response.next().await.is_some() {
            warn!("State differs for {c}.");
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
    pv1: ProtocolVersion,
    pv2: ProtocolVersion,
) -> anyhow::Result<bool> {
    eprintln!("Checking passive delegators.");
    let mut passive1 = if pv1 >= ProtocolVersion::P4 {
        client1
            .get_passive_delegators(block1)
            .await?
            .response
            .try_collect::<Vec<_>>()
            .await?
    } else {
        Vec::new()
    };
    let mut passive2 = if pv2 >= ProtocolVersion::P4 {
        client2
            .get_passive_delegators(block2)
            .await?
            .response
            .try_collect::<Vec<_>>()
            .await?
    } else {
        Vec::new()
    };
    passive1.sort_unstable_by_key(|x| x.account);
    passive2.sort_unstable_by_key(|x| x.account);
    if passive1 != passive2 {
        warn!("Passive delegators differ.");
        Ok(true)
    } else {
        Ok(false)
    }
}

async fn compare_active_bakers(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
    pv1: ProtocolVersion,
    pv2: ProtocolVersion,
) -> anyhow::Result<bool> {
    eprintln!("Checking active bakers.");
    let mut found_diff = false;
    let (ei1, ei2) = futures::try_join!(
        client1.get_election_info(block1),
        client2.get_election_info(block2)
    )?;
    if pv1 < ProtocolVersion::P6
        && pv2 < ProtocolVersion::P6
        && ei1.response.election_difficulty != ei2.response.election_difficulty
    {
        warn!("Election difficulty differs.");
        found_diff = true;
    }
    if ei1.response.bakers != ei2.response.bakers {
        warn!("Bakers differ.");
        found_diff = true;
    }
    Ok(found_diff)
}

async fn compare_baker_pools(
    client1: &mut v2::Client,
    client2: &mut v2::Client,
    block1: BlockHash,
    block2: BlockHash,
    pv1: ProtocolVersion,
    pv2: ProtocolVersion,
) -> anyhow::Result<()> {
    eprintln!("Checking baker pools.");
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

    if pv1 >= ProtocolVersion::P4 && pv2 >= ProtocolVersion::P4 {
        for pool in pools1 {
            let (d1, d2) = futures::try_join!(
                client1.get_pool_delegators(block1, pool),
                client2.get_pool_delegators(block2, pool)
            )?;
            let mut ds1 = d1.response.try_collect::<Vec<_>>().await?;
            let mut ds2 = d2.response.try_collect::<Vec<_>>().await?;
            ds1.sort_unstable_by_key(|x| x.account);
            ds2.sort_unstable_by_key(|x| x.account);
            if ds1 != ds2 {
                warn!("Delegators for pool {pool} differ.");
            }

            let (p1, p2) = futures::try_join!(
                client1.get_pool_info(block1, pool),
                client2.get_pool_info(block2, pool)
            )?;
            if p1.response != p2.response {
                warn!("Pool {pool} differs.");
            }
        }
    } else {
        warn!("Not comparing baker pools since one of the protocol versions is before P4.")
    }

    Ok(())
}
