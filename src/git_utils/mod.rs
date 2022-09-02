pub mod rebuilder;

use anyhow::{Context, Result};
use git2::{Commit, PushOptions, RemoteCallbacks, Repository};
use git2_credentials::CredentialHandler;

const WORKING_BRANCH_NAME: &str = "git-publish_working_branch";

pub fn build_remote_callbacks() -> Result<RemoteCallbacks<'static>> {
    let git_config = git2::Config::open_default().unwrap();
    let mut cb = RemoteCallbacks::new();
    let mut ch = CredentialHandler::new(git_config);

    cb.credentials(move |url, username, allowed| ch.try_next_credential(url, username, allowed))
        .transfer_progress(|p| {
            eprintln!("{}/{}", p.indexed_objects(), p.total_objects());
            eprintln!("{}/{}", p.indexed_deltas(), p.total_deltas());
            true
        })
        .push_update_reference(|reference, status| {
            if let Some(msg) = status {
                println!(r"/!\ failed to push {reference}: {msg}");
            } else {
                println!("Successfully pushed {reference}");
            }

            Ok(())
        });

    Ok(cb)
}

pub fn push_reference(
    rep: &Repository,
    src_reference: &str,
    dst_reference: &str,
    remote_url: &str,
    force: bool,
) -> Result<()> {
    let refspec_prefix = {
        if force {
            "+"
        } else {
            ""
        }
    };

    let refspec = format!("{refspec_prefix}{src_reference}:{dst_reference}");
    let mut remote = rep.remote_anonymous(remote_url)?;
    let mut push_opt = PushOptions::new();
    push_opt.remote_callbacks(build_remote_callbacks()?);
    remote.push(&[refspec], Some(&mut push_opt))?;
    Ok(())
}

pub fn push_commit(
    rep: &Repository,
    commit: &Commit,
    dst_reference: &str,
    remote_url: &str,
    force: bool,
) -> Result<()> {
    let mut src_ref = rep
        .branch(WORKING_BRANCH_NAME, commit, true)
        .context("failed to create working branch")?
        .into_reference();

    let src_ref_name = src_ref
        .name()
        .context("failed to get source reference name")?;

    let push_res = push_reference(rep, src_ref_name, dst_reference, remote_url, force);
    src_ref.delete()?;
    push_res
}
