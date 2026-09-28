/// The current Codex CLI version as embedded at compile time.
#[cfg(not(test))]
pub const CODEX_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

// Keep snapshot layout independent of the release being built.
#[cfg(test)]
pub const CODEX_CLI_VERSION: &str = "0.0.0";
