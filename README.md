# yagit

Yet another static site generator for Git

## Philosophy

yagit aims to provide a minimalist user interface and implement the following
simple feature set:

* Accessible and optimized HTML output implementing current best practices
* Simple static web-pages with no need for JavaScript
* Incremental builds: only render pages for commits and blobs if necessary

For a live example please see <https://git.pablopie.xyz>!

## Usage

yagit maintains a store of Git repositories at `REPOS_DIR/` and
renders HTML pages for such repositories at the location `OUTPUT_DIR/`.

By default, yagit renders HTML pages in incremental mode: pages for Git
commits and blobs are only renderer if the relevant commits are newer than the
page's last modification date. This option can be disabled with the
`--full-build` flag.

yagit also maintains a store of Git repositories at `PRIVATE_REPOS_DIR/`,
which can be switched on using the `--private` flag. The HTML pages for
repositories at `PRIVATE_REPOS_DIR/` are rendered at
`OUTPUT_PATH/PRIVATE_OUTPUT_ROOT/`.

To render the HTML pages for a single repository using yagit run:

```console
$ yagit render REPO_NAME
```

To render HTML pages for all repositories at `REPOS_DIR` run:

```console
$ yagit render-batch
```

To initiliaze an empty repository at repository store run

```console
$ yagit init REPO_NAME
```

For more information check the `yagit.1` man page.

## Limitations

* yagit is only supported on UNIX systems
* yagit is single threaded: this is because my personal VPS has a single core
* yagit _does not_ support customization of the HTML output (see the
  **Configuration** section bellow)

## Configuration

A number of configuration options is provided at compile-time. See
`config.toml`.

### Customizing the HTML Output

The user is expected to modify the source code to customize the HTML output,
_no templating system is provided_. The idea is that instead of relying in a
complex and inflexible HTML templating systems, users should fork the
application to adapt it for their own needs.

## Installation

yagit can be installed from source using the following commands:

```console
$ git clone git://git.pablopie.xyz/yagit
$ cargo build --release
# install -m 755 ./target/release/yagit /usr/bin/yagit
# install -m 644 ./yagit.1 /usr/share/man/man1/yagit.1
# mandb
```

### Build Dependencies

* [libgit2](https://libgit2.org)
* [libssl](https://www.openssl-library.org)
