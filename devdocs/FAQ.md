# Frequently asked question for developers

Let's collect some questions a lot of Nushell contributors have.
- How do I do....?
- Why do I need to do certain things a certain way?

Let's keep the answers concise and up to date (or general enough) to remain relevant

## How do I properly test my feature or bugfix?
TODO (Probably fork out into its own file)

## I want to report an error to the user

Approximate flow:

1. Are you reporting the error in the parser/static checking phase?
    - Use `nu_protocol::ParseError` variants
    - Follow the logic used in the context as we need to collect multiple errors for a good IDE experience
2. Pick the right `nu_protocol::ShellError` variant
    - Does a matching existing variant fit your need? (go to references of the `ShellError` variant for inspiration)
    - Check what context the [`miette`](https://docs.rs/miette) macros add during formatting! (go to definition of `ShellError`)
    - If it is a one-of specific error, consider using a generic variant
    - Else add a new class of errors
        - add the necessary `Span` information
        - general shared error text, to inform and point to a resolution
        - dynamic information gathered from the error site
        - Don't use a tuple enum variant, named structs going forward only!
3. Are you in a command?
    - `return Err(ShellError::...)` and you're done in a `Command::run`
4. Do you want to report a warning but not stop execution?
    - **NEVER** `println!`, we can write to stderr if necessary but...
    - good practice: `nu_protocol::report_error::report_error` or `report_error_new`
        - depending on whether you have access to a `StateWorkingSet`
    - if only relevant to in the field debugging: `log`-crate macros.

## Which upstream projects power some of Nu's built-in commands?

A number of Nu's file and system commands are built on
[uutils/coreutils](https://github.com/uutils/coreutils), a cross-platform Rust
reimplementation of the GNU coreutils. These currently include `cp`, `mv`,
`mkdir`, `mktemp`, `touch`, `whoami`, and `uname` (see the `uu_*` workspace
dependencies in the root `Cargo.toml`). Relying on uutils lets these commands
behave consistently across Windows, macOS, and Linux.

## How do I add a new `$env.config` option?

Config options live as plain Rust structs in `crates/nu-protocol/src/config/`, one module per section
(e.g. `history.rs` for `$env.config.history`). For a new field:

1. Add the field to the struct and its `Default` impl.
2. Add it to the struct's `IntoValue` impl, otherwise it won't show up in `$env.config`.
3. Handle the key in the `UpdateFromValue` impl. Reuse the existing `update` impls for simple types
    and report problems through `ConfigErrors` (`type_mismatch`, `invalid_value`, ...) instead of
    returning early, so users see every mistake at once. Validate as much as you can here (e.g.
    compile regexes), so consumers can trust the stored value.
4. Decide whether the option may change at runtime. Most can. Options that are only read when
    something is set up (like the history file location) have to reject later changes, see
    `history_locked_after_startup` for an example.
5. Read the option through `engine_state.get_config()` (or `stack.get_config(engine_state)` inside
    commands) at the point where it is used. Values that need to be compiled should be cached and only
    rebuilt when the config actually changed; the REPL re-reads the config before every prompt.
6. Document it in `crates/nu-config/default_files/doc_config.nu`, which is what `config nu --doc`
    shows.
7. Test the parsing next to the struct (unit tests), and the user-facing behavior in
    `tests/repl/test_config.rs`.

If the option also gets a command line flag (like `--table-mode` or `--history-ignore`), parse and
validate it in `src/command.rs` and apply it in `src/run.rs` after the config files were read.

## How do I check an environment variable?
TODO

## WTF is `PipelineMetadata`?
TODO
