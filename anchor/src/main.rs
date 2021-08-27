use std::io::Write;
use std::process;
use std::str::FromStr;
use std::{env, path::PathBuf};

use anyhow::anyhow;
use anyhow::Context as _;

use coins_bip32::path::DerivationPath;

use rad_anchor as anchor;
use radicle_tools::logger;

use anchor::{Address, Urn};

const USAGE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/USAGE"));

fn parse_options(help: &mut bool, verbose: &mut bool) -> anyhow::Result<anchor::Options> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_env();
    let mut org: Option<Address> = None;
    let mut project: Option<Urn> = None;
    let mut commit: Option<String> = None;
    let mut rpc_url: Option<String> = None;
    let mut keystore: Option<PathBuf> = None;
    let mut ledger_hdpath: Option<DerivationPath> = None;
    let mut dry_run = false;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("org") => {
                org = Some(
                    parser
                        .value()?
                        .parse()
                        .context("invalid value specified for '--org'")?,
                );
            }
            Long("project") => {
                project = Some(
                    parser
                        .value()?
                        .parse()
                        .context("invalid value specified for '--project'")?,
                );
            }
            Long("commit") => {
                commit = Some(parser.value()?.to_string_lossy().to_string());
            }
            Long("rpc-url") => {
                rpc_url = Some(parser.value()?.to_string_lossy().to_string());
            }
            Long("keystore") => {
                keystore = Some(parser.value()?.parse()?);
            }
            Long("ledger-hdpath") => {
                ledger_hdpath = Some(parser.value()?.parse()?);
            }
            Long("dry-run") => {
                dry_run = true;
            }
            Long("verbose") | Short('v') => {
                *verbose = true;
            }
            Long("help") => {
                *help = true;
            }
            _ => {
                return Err(anyhow!(arg.unexpected()));
            }
        }
    }

    let rpc_url = rpc_url
        .or_else(|| env::var("ETH_RPC_URL").ok())
        .and_then(|url| if url.is_empty() { None } else { Some(url) })
        .ok_or_else(|| {
            anyhow::anyhow!("An Ethereum JSON-RPC URL must be specified with '--rpc-url'")
        })?;

    let commit = if let Some(commit) = commit {
        commit
    } else {
        get_repository_head().map_err(|_| {
            anyhow::anyhow!(
                "repository head could not be retrieved, \
                please specify anchor hash with '--commit'"
            )
        })?
    };

    let ledger_hdpath = ledger_hdpath.or_else(|| {
        env::var("ETH_HDPATH")
            .ok()
            .and_then(|v| DerivationPath::from_str(v.as_str()).ok())
    });

    Ok(anchor::Options {
        org: org.ok_or(anyhow!("an org must be specified with '--org'"))?,
        project: project.ok_or(anyhow!("a project must be specified with '--project'"))?,
        commit,
        rpc_url,
        ledger_hdpath,
        keystore,
        dry_run,
    })
}

/// Get the `HEAD` commit hash of the current repository.
fn get_repository_head() -> anyhow::Result<String> {
    use std::process::Command;

    let output = Command::new("git").arg("rev-parse").arg("HEAD").output()?;
    let string = String::from_utf8(output.stdout)?;
    let hash = string.trim_end().to_owned();

    Ok(hash)
}

#[tokio::main]
async fn main() {
    logger::init(log::Level::Error, vec![env!("CARGO_CRATE_NAME")]).unwrap();

    match execute().await {
        Err(err) => {
            if let Some(&anchor::Error::NoWallet) = err.downcast_ref() {
                log::error!("Error: no wallet specified: either '--ledger-hdpath' or '--keystore' must be specified");
            } else if let Some(cause) = err.source() {
                log::error!("Error: {} ({})", err, cause);
            } else {
                log::error!("Error: {}", err);
            }
            process::exit(1);
        }
        Ok(()) => {}
    }
}

async fn execute() -> anyhow::Result<()> {
    let mut help = false;
    let mut verbose = false;
    let opts = parse_options(&mut help, &mut verbose)?;

    if help {
        std::io::stderr().write(USAGE)?;
        return Ok(());
    }

    if verbose {
        log::set_max_level(log::Level::Debug.to_level_filter());
    } else {
        log::set_max_level(log::Level::Info.to_level_filter());
    }
    anchor::run(opts).await?;

    Ok(())
}
