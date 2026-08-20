mod managed_lsp;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use zed_extension_api as zed;

const MOJO_LSP_SERVER_ID: &str = "mojo-lsp-server";
const MOJO_LSP_EXECUTABLE: &str = "mojo-lsp-server";
const MOJO_LSP_PROXY: &str = "bin/mojo-lsp-zed.js";
const MOJO_LSP_PROXY_SOURCE: &str = include_str!("../bin/mojo-lsp-zed.js");
const PIXI_MOJO_LSP_EXECUTABLES: [&str; 2] = [
    ".pixi/envs/default/bin/mojo-lsp-server",
    ".pixi/envs/default/Scripts/mojo-lsp-server.exe",
];

struct MojoExtension {
    managed_binary_path: Option<String>,
}

impl zed::Extension for MojoExtension {
    fn new() -> Self {
        Self {
            managed_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != MOJO_LSP_SERVER_ID {
            return Err(format!("unknown language server: {language_server_id}"));
        }

        let binary_settings =
            zed::settings::LspSettings::for_worktree(MOJO_LSP_SERVER_ID, worktree)
                .ok()
                .and_then(|settings| settings.binary);

        let discovered_command = binary_settings
            .as_ref()
            .and_then(|settings| settings.path.clone())
            .or_else(|| worktree.which(MOJO_LSP_EXECUTABLE))
            .or_else(|| {
                PIXI_MOJO_LSP_EXECUTABLES
                    .iter()
                    .find_map(|path| worktree.which(path))
            });

        let command = if let Some(command) = discovered_command {
            command
        } else if let Some(command) = self.managed_binary_path.clone() {
            command
        } else {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );
            match managed_lsp::managed_lsp_path() {
                Ok(command) => {
                    self.managed_binary_path = Some(command.clone());
                    zed::set_language_server_installation_status(
                        language_server_id,
                        &zed::LanguageServerInstallationStatus::None,
                    );
                    command
                }
                Err(error) => {
                    zed::set_language_server_installation_status(
                        language_server_id,
                        &zed::LanguageServerInstallationStatus::Failed(error.clone()),
                    );
                    return Err(format!(
                        "unable to find or install `{MOJO_LSP_EXECUTABLE}`: {error}"
                    ));
                }
            }
        };

        let server_args = binary_settings
            .as_ref()
            .and_then(|settings| settings.arguments.clone())
            .unwrap_or_else(|| vec!["--log=error".into()]);

        let configured_env = binary_settings
            .as_ref()
            .and_then(|settings| settings.env.as_ref());
        let env = server_environment(worktree.shell_env(), &command, configured_env);

        let node = zed::node_binary_path()?;
        let proxy =
            mojo_lsp_proxy_path(&std::env::current_dir().map_err(|error| {
                format!("unable to locate the extension work directory: {error}")
            })?)?
            .to_string_lossy()
            .into_owned();

        Ok(zed::Command {
            command: node,
            args: [vec![proxy, command], server_args].concat(),
            env,
        })
    }
}

fn mojo_lsp_proxy_path(work_dir: &Path) -> zed::Result<PathBuf> {
    let proxy = work_dir.join(MOJO_LSP_PROXY);
    if std::fs::read(&proxy).ok().as_deref() != Some(MOJO_LSP_PROXY_SOURCE.as_bytes()) {
        let parent = proxy
            .parent()
            .ok_or_else(|| "the Mojo LSP proxy path has no parent directory".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("unable to create the Mojo LSP proxy directory: {error}"))?;
        std::fs::write(&proxy, MOJO_LSP_PROXY_SOURCE)
            .map_err(|error| format!("unable to install the Mojo LSP proxy: {error}"))?;
    }
    Ok(proxy)
}

fn server_environment(
    mut env: Vec<(String, String)>,
    command: &str,
    configured_env: Option<&HashMap<String, String>>,
) -> Vec<(String, String)> {
    let has_configured_modular_home =
        configured_env.is_some_and(|configured_env| configured_env.contains_key("MODULAR_HOME"));
    if let Some(configured_env) = configured_env {
        for (name, value) in configured_env {
            env.retain(|(existing_name, _)| existing_name != name);
            env.push((name.clone(), value.clone()));
        }
    }
    if !has_configured_modular_home {
        if let Some(modular_home) = modular_home_for_command(command) {
            env.retain(|(name, _)| name != "MODULAR_HOME");
            env.push(("MODULAR_HOME".into(), modular_home));
        }
    }
    env.retain(|(name, _)| name != "MODULAR_TELEMETRY_ENABLED");
    env.push(("MODULAR_TELEMETRY_ENABLED".into(), "0".into()));
    env
}

fn modular_home_for_command(command: &str) -> Option<String> {
    let bin_dir = Path::new(command).parent()?;
    if bin_dir.file_name()?.to_str()? != "bin" {
        return None;
    }
    bin_dir
        .parent()?
        .join("share/max")
        .to_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materializes_the_proxy_in_an_empty_extension_work_directory() {
        let work_dir = tempfile::tempdir().unwrap();

        let proxy = mojo_lsp_proxy_path(work_dir.path()).unwrap();

        assert!(proxy.is_file());
        assert_eq!(
            std::fs::read_to_string(&proxy).unwrap(),
            MOJO_LSP_PROXY_SOURCE
        );

        std::fs::write(&proxy, "stale proxy").unwrap();
        let refreshed_proxy = mojo_lsp_proxy_path(work_dir.path()).unwrap();

        assert_eq!(refreshed_proxy, proxy);
        assert_eq!(
            std::fs::read_to_string(refreshed_proxy).unwrap(),
            MOJO_LSP_PROXY_SOURCE
        );
    }

    #[test]
    fn derives_modular_home_from_the_selected_conda_binary() {
        let env = server_environment(
            vec![("MODULAR_HOME".into(), "/stale/environment".into())],
            "/opt/mojo/bin/mojo-lsp-server",
            None,
        );

        assert!(env.contains(&("MODULAR_HOME".into(), "/opt/mojo/share/max".into())));
        assert!(!env.contains(&("MODULAR_HOME".into(), "/stale/environment".into())));
    }

    #[test]
    fn configured_modular_home_overrides_the_derived_prefix() {
        let configured_env = HashMap::from([(
            "MODULAR_HOME".to_string(),
            "/configured/modular".to_string(),
        )]);

        let env = server_environment(
            Vec::new(),
            "/opt/mojo/bin/mojo-lsp-server",
            Some(&configured_env),
        );

        assert!(env.contains(&("MODULAR_HOME".into(), "/configured/modular".into())));
        assert!(!env.contains(&("MODULAR_HOME".into(), "/opt/mojo/share/max".into())));
    }
}

zed::register_extension!(MojoExtension);
