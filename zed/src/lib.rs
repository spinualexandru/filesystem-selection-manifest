use zed_extension_api::{self as zed, settings::LspSettings};

const SERVER_NAME: &str = "fsman-lsp";

struct FsmanExtension;

impl zed::Extension for FsmanExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let settings = LspSettings::for_worktree(SERVER_NAME, worktree)?;
        let mut env = worktree.shell_env();

        let (configured_path, args) = match settings.binary {
            Some(binary) => {
                env.extend(binary.env.unwrap_or_default());
                (binary.path, binary.arguments.unwrap_or_default())
            }
            None => (None, Vec::new()),
        };

        let command = configured_path
            .or_else(|| worktree.which(SERVER_NAME))
            .ok_or_else(|| {
                format!(
                    "Could not find {SERVER_NAME} on PATH. Install it or set lsp.{SERVER_NAME}.binary.path in Zed settings."
                )
            })?;

        Ok(zed::Command { command, args, env })
    }
}

zed::register_extension!(FsmanExtension);
