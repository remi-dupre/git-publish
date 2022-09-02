pub mod rebuilder;

use anyhow::Result;
use git2::{Cred, RemoteCallbacks};

pub fn build_remote_callbacks() -> Result<RemoteCallbacks<'static>> {
    let mut cb = RemoteCallbacks::new();

    cb.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key(
            username_from_url.unwrap(),
            None,
            std::path::Path::new(&format!(
                "{}/.ssh/id_ed25519",
                std::env::var("HOME").unwrap()
            )),
            None,
        )
    })
    .transfer_progress(|p| {
        println!("{}/{}", p.indexed_objects(), p.total_objects());
        println!("{}/{}", p.indexed_deltas(), p.total_deltas());
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
