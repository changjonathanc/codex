use std::path::Path;

/// Determines who installs the binary used by the daemon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UpdateSource {
    OfficialInstaller,
    ExternalPackage,
}

impl UpdateSource {
    pub(crate) fn for_version(version: &str) -> Self {
        let version_without_metadata = version
            .split_once('+')
            .map_or(version, |(version, _)| version);
        if version_without_metadata
            .split_once('-')
            .is_some_and(|(_, prerelease)| prerelease == "fork" || prerelease.starts_with("fork."))
        {
            Self::ExternalPackage
        } else {
            Self::OfficialInstaller
        }
    }

    pub(crate) fn missing_install_error(self, managed_codex_bin: &Path) -> anyhow::Error {
        let managed_codex_path = managed_codex_bin.display();
        match self {
            Self::OfficialInstaller => anyhow::anyhow!(
                "daemon executable not found at {managed_codex_path}; repair the existing \
                 installation, or run `codex app-server daemon start` to install a missing daemon"
            ),
            Self::ExternalPackage => anyhow::anyhow!(
                "daemon executable not found at {managed_codex_path}\n\n\
                 repair the existing installation with the fork installation workflow. It must \
                 create or repair the managed package link. Then rerun this command."
            ),
        }
    }
}
