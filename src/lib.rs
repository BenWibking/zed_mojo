use zed_extension_api as zed;

const MOJO_LSP_SERVER_ID: &str = "mojo-lsp-server";
const MOJO_LSP_EXECUTABLE: &str = "mojo-lsp-server";

struct MojoExtension;

impl zed::Extension for MojoExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != MOJO_LSP_SERVER_ID {
            return Err(format!("unknown language server: {language_server_id}"));
        }

        let command = worktree.which(MOJO_LSP_EXECUTABLE).ok_or_else(|| {
            format!(
                "unable to find `{MOJO_LSP_EXECUTABLE}` in PATH. Install the Mojo package and open Zed from that environment, such as a Pixi shell."
            )
        })?;

        let node = zed::node_binary_path()?;
        let proxy = std::env::current_dir()
            .map_err(|error| format!("unable to locate the installed extension: {error}"))?
            .join("bin/mojo-lsp-zed.js")
            .to_string_lossy()
            .into_owned();

        Ok(zed::Command {
            command: node,
            args: vec![proxy, command, "--log=error".into()],
            env: vec![("MODULAR_TELEMETRY_ENABLED".into(), "0".into())],
        })
    }
}

zed::register_extension!(MojoExtension);
