mod config;
mod git_utils;

use anyhow::{bail, Context, Result};

use clap::Parser;
use git2::{BranchType, Repository};

use config::read_config;
use git_utils::push_commit;
use git_utils::rebuilder::Rebuilder;

#[derive(Parser)]
#[clap(author, version, about)]
struct Args {
    /// Usually, the command refuses to update a remote ref that is not an ancestor of the local
    /// ref used to overwrite it. This flag disables this check, and can cause the remote
    /// repository to lose commits; use it with care.
    #[clap(long, short)]
    force: bool,

    /// Perform a normal run but doesn't attempt to push.
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// Specify the branch that must be pushed, defaults to current working branch.
    src: Option<String>,

    /// Specify the name of remote branch, by default it will be the same as the local branch that
    /// will be pushed.
    dst: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let rep = Repository::open_from_env()?;
    let config = read_config(&rep).context("could not load config")?;

    let src_branch_name = match args.src {
        Some(src) => src,
        None => {
            let head = rep.head()?;

            if !head.is_branch() {
                bail!("current HEAD must be a branch to publish");
            }

            head.shorthand().unwrap().to_string()
        }
    };

    let src_branch = rep
        .find_branch(&src_branch_name, BranchType::Local)
        .context(format!("could not find branch `{src_branch_name}`"))?;

    let dst_branch_name = args.dst.unwrap_or_else(|| src_branch_name.to_string());

    for remote_config in &config.remotes {
        let commit = src_branch.get().peel_to_commit()?;
        let mut rebuilder = Rebuilder::new(&rep, &remote_config.include);
        let new_commit = rebuilder.rebuild_commit(commit)?;

        rebuilder.debug_changes();
        rebuilder.debug_filtered();

        if !args.dry_run {
            push_commit(
                &rep,
                &new_commit,
                &format!("refs/heads/{dst_branch_name}"),
                &remote_config.url,
                args.force,
            )?;
        }
    }

    Ok(())
}
