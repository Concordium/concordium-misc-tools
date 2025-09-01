use anyhow::Context;
use clap::Parser;
use colored::Colorize;

#[derive(clap::Parser, Debug)]
#[clap(arg_required_else_help(true))]
#[clap(version, author)]
struct App {
    #[clap(
        long = "wp-url",
        help = "Base URL of the wallet-proxy.",
        default_value = "http://wallet-proxy.stagenet.concordium.com",
        env = "WP_LOAD_SIMULATOR_URL"
    )]
    url: reqwest::Url,
    #[clap(
        long = "accounts",
        help = "List of accounts to query.",
        env = "WP_LOAD_SIMULATOR_ACCOUNTS"
    )]
    accounts: std::path::PathBuf,
    #[clap(
        long = "delay",
        help = "Delay between requests by each parallel workers, in milliseconds.",
        env = "WP_LOAD_SIMULATOR_DELAY"
    )]
    delay: u64,
    #[clap(
        long = "max-parallel",
        help = "Number of parallel queries to make at the same time.",
        env = "WP_LOAD_SIMULATOR_PARALLEL"
    )]
    num: usize,
    #[clap(
        long = "only-failures",
        help = "Output only responses that are not in 2xx range.",
        env = "WP_LOAD_SIMULATOR_ONLY_FAILURES"
    )]
    only_failures: bool,
    #[clap(
        long = "timeout",
        help = "Timeout to apply to requests, in milliseconds.",
        default_value = "10000",
        env = "WP_LOAD_SIMULATOR_REQUEST_TIMEOUT"
    )]
    timeout: u64,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();
    let accounts: Vec<String> = serde_json::from_reader(
        std::fs::File::open(app.accounts).context("Unable to open accounts list file.")?,
    )
    .context("Invalid account list")?;
    let mut handles = Vec::new();
    let delay = app.delay;

    let mut senders = Vec::new();
    let mut receivers = Vec::new();
    for _ in 0..app.num {
        let (sender, receiver) = tokio::sync::mpsc::channel(100);
        senders.push(sender);
        receivers.push(receiver);
    }

    let url = app.url.clone();
    let sender_task = tokio::spawn(async move {
        let mut i: usize = 0;
        for account in accounts.iter().cycle() {
            let mut url = url.clone();
            url.set_path(&format!("v0/accBalance/{}", account));
            senders[i]
                .send(url)
                .await
                .context(format!("Receiver {i} died."))?;
            i += 1;
            i %= senders.len();
        }
        Ok::<(), anyhow::Error>(())
    });

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

    for (i, mut receiver) in receivers.into_iter().enumerate() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(app.timeout))
            .build()?;
        let sender = sender.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(delay));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let url = receiver.recv().await.unwrap();
                let start = chrono::Utc::now();
                let response = client.get(url.clone()).send().await;
                let end = chrono::Utc::now();
                let diff = end.signed_duration_since(start).num_milliseconds();
                match response {
                    Ok(response) => {
                        let code = response.status().as_u16();
                        let _body = response.json::<serde_json::Value>().await;
                        sender.send((true, i, url, diff, code, None)).unwrap();
                    }
                    Err(e) => {
                        sender.send((false, i, url, diff, 0, Some(e))).unwrap();
                    }
                }
            }
        });
        handles.push(handle);
    }
    {
        let mut start = chrono::Utc::now();
        let mut count = 0;
        while let Some((success, i, url, diff, code, err)) = receiver.recv().await {
            if chrono::Utc::now()
                .signed_duration_since(start)
                .num_milliseconds()
                > 1000
            {
                count = 0;
                start = chrono::Utc::now();
            }
            count += 1;
            if app.only_failures && (200..300).contains(&code) {
                continue;
            }
            if (200..300).contains(&code) {
                println!(
                    "{}",
                    format!("{count:8}, {i}, {url}, {diff:8}ms, {code}, {success}",).green()
                );
            } else {
                let err_str = match err {
                    Some(e) => e.to_string(),
                    None => "".into(),
                };
                println!(
                    "{}",
                    format!("{count:8}, {i}, {url}, {diff:8}ms, {code}, {success}, {err_str}",)
                        .red()
                );
            }
        }
    }
    futures::future::join_all(handles).await.clear();
    sender_task.abort();
    Ok(())
}
