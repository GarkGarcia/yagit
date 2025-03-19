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
$ yagit render REPO_PATH OUTPUT_PATH
```

The argument `REPO_PATH` should have the form `PARENT_PATH/REPO_NAME`, where
`PARENT_PATH` is the path to the parent directory of `REPO_PATH`. yagit will
generate the HTML pages for `REPO_PATH` at `OUTPUT_PATH/REPO_NAME`. yagit will
also generate an index of all git repositories in `PARENT_PATH` at
`OUTPUT_PATH/index.html`.

To render HTML pages for all repositories in a given directory in batch mode
run:

```console
$ yagit render-batch BATCH_PATH OUTPUT_PATH
```

yagit will generate the HTML pages for `BATCH_PATH/REPO_NAME` at
`OUTPUT_PATH/REPO_NAME`, as well as an index of all git repositories in
`BATCH_PATH` at `OUTPUT_PATH/index.html`.
