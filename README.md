git-publish
===========

A git subcommand to push a remote while filtering out some paths.


Why?
----

This projects intends to make it easy to push a project hosted on a private
repository into a public repository while removing the parts that should not be
exposed (such as internal CI stuff).

While some tools exist to filter on a Git repository (such as
[git-filter-repo]), they are primarily built to erase some mistake from the
whole git history or perform some general cleanups.

Here the goal is just to push a specific branch to a remote and to make it as
simple as a `git publish`.


How to install
--------------

### From sources

This requires the Rust toolchain with version at least 1.63.

```shell
git clone https://github.com/remi-dupre/git-publish.git
cd git-publish
cargo build --release
cp target/release/git-publish ~/.local/bin  # ensure that $HOME/.local/bin is in your $PATH
```


Usage
-----

### Configuration

First you need to create a configuration file in *.git-publish/config.yml*
inside your repository.

Here is an example config:

```yaml
remotes:
  - # Remote URL you want to publish to
    url: git@github.com:Qwant/fafnir.git

    # The paths you want to keep in your remote
    include:
       - .github/
       - src/
       - LICENSE
       - README.md
```

### Command line

Now you can push your remote by simply running `git publish`:

```shell
$ git publish
Rewrote 306 objects and skipped 175
Filtered 4 paths:
  - .gitlab-ci.yml
  - .travis.yml
  - ci
Successfully pushed refs/heads/bar
```

See more parameters in the command help:

```shell
$ git publish --help
git-publish 0.1.0

USAGE:
    git-publish [OPTIONS] [ARGS]

ARGS:
    <SRC>    Specify the branch that must be pushed, defaults to current working branch
    <DST>    Specify the name of remote branch, by default it will be the same as the local
             branch that will be pushed

OPTIONS:
    -f, --force      Usually, the command refuses to update a remote ref that is not an ancestor of
                     the local ref used to overwrite it. This flag disables this check, and can
                     cause the remote repository to lose commits; use it with care
    -h, --help       Print help information
    -n, --dry-run    Perform a normal run but doesn't attempt to push
    -V, --version    Print version information
```

[git-filter-repo]: https://github.com/newren/git-filter-repo
