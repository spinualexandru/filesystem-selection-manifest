import * as vscode from "vscode";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

function serverOptions(): ServerOptions {
    const configuration = vscode.workspace.getConfiguration("fsman.server");
    const command = configuration.get<string>("path", "fsman-lsp").trim();
    const args = configuration.get<string[]>("arguments", []);
    const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

    if (command.length === 0) {
        throw new Error("fsman.server.path must not be empty");
    }

    return {
        command,
        args,
        options: cwd === undefined ? undefined : { cwd },
    };
}

async function startClient(): Promise<void> {
    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: "file", language: "fsman" },
            { scheme: "untitled", language: "fsman" },
        ],
    };

    client = new LanguageClient(
        "fsman",
        "fsman Language Server",
        serverOptions(),
        clientOptions,
    );

    try {
        await client.start();
    } catch (error) {
        client = undefined;
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(
            `Could not start fsman-lsp: ${message}. Set fsman.server.path to the server executable.`,
        );
    }
}

async function stopClient(): Promise<void> {
    const runningClient = client;
    client = undefined;
    await runningClient?.dispose();
}

async function restartClient(): Promise<void> {
    await stopClient();
    await startClient();
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    context.subscriptions.push(
        vscode.commands.registerCommand("fsman.restartServer", restartClient),
        vscode.workspace.onDidChangeConfiguration((event) => {
            if (event.affectsConfiguration("fsman.server")) {
                void restartClient();
            }
        }),
    );

    await startClient();
}

export async function deactivate(): Promise<void> {
    await stopClient();
}
