# nu-cli

Log and error messages added under `crates/nu-cli/src/` must not include text the user typed or configured verbatim (command lines, patterns, history entries): they can hold secrets and logs are shared in bug reports. Name the setting and its index instead.
