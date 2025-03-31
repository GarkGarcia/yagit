# yagit

Yet another static site generator for Git

## Philosophy

yagit aims to provide a minimalist user interface and implement the following
simple feature set:

* Accessible and optimized HTML output implementing current best practices
* Simple static web-pages with no need for JavaScript
* Incremental builds: only render pages for commits and blobs if necessary

For a live example please see <https://git.pablopie.xyz>!

### Customizing the HTML Output

The user is expected to modify the source code to customize the HTML output,
_no templating system is provided_. The idea is that instead of relying in a
complex and inflexible HTML templating systems, users should fork the
application to adapt it for their own needs.

## Usage

To render the HTML pages for a single repository using yagit run:

```console
$ yagit render REPO_NAME
```

yagit will generate the HTML pages for `REPOS_DIR/REPO_NAME` at
`OUTPUT_PATH/REPO_NAME`. yagit will also generate an index of all git
repositories in `REPOS_DIR` at `OUTPUT_PATH/index.html`.

To render HTML pages for all repositories at `REPOS_DIR` run:

```console
$ yagit render-batch
```

## Installation

yagit can be installed via Cargo by cloning this repository, as in:

```console
$ git clone git://git.pablopie.xyz/yagit
$ cargo install --path ./yagit
```

### Build Dependencies

* [libgit2](https://libgit2.org)
* [libssl](https://www.openssl-library.org)
