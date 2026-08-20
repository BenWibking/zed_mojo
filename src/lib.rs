mod managed_lsp;

use zed_extension_api as zed;

const MOJO_LSP_SERVER_ID: &str = "mojo-lsp-server";
const MOJO_LSP_EXECUTABLE: &str = "mojo-lsp-server";
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

        let mut env = worktree.shell_env();
        if let Some(settings_env) = binary_settings.and_then(|settings| settings.env) {
            for (name, value) in settings_env {
                env.retain(|(existing_name, _)| existing_name != &name);
                env.push((name, value));
            }
        }
        env.retain(|(name, _)| name != "MODULAR_TELEMETRY_ENABLED");
        env.push(("MODULAR_TELEMETRY_ENABLED".into(), "0".into()));

        let node = zed::node_binary_path()?;
        let proxy = std::env::current_dir()
            .map_err(|error| format!("unable to locate the installed extension: {error}"))?
            .join("bin/mojo-lsp-zed.js")
            .to_string_lossy()
            .into_owned();

        Ok(zed::Command {
            command: node,
            args: [vec![proxy, command], server_args].concat(),
            env,
        })
    }
}

zed::register_extension!(MojoExtension);
