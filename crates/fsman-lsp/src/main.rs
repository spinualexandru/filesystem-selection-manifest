use std::{error::Error, ops::Range as ByteRange, process::ExitCode};

use lsp_server::{Connection, ErrorCode, Message, Notification, Response};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeResult, NumberOrString, Position, PositionEncodingKind,
    PublishDiagnosticsParams, Range, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
};

type ServerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

fn main() -> ExitCode {
    match run_stdio() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fsman-lsp failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_stdio() -> ServerResult<()> {
    let (connection, io_threads) = Connection::stdio();
    run(&connection)?;
    io_threads.join()?;
    Ok(())
}

fn run(connection: &Connection) -> ServerResult<()> {
    let (initialize_id, _) = connection.initialize_start()?;
    let initialize_result = InitializeResult {
        capabilities: server_capabilities(),
        server_info: Some(ServerInfo {
            name: env!("CARGO_PKG_NAME").to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
    };
    connection.initialize_finish(initialize_id, serde_json::to_value(initialize_result)?)?;

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }

                let response = Response::new_err(
                    request.id,
                    ErrorCode::MethodNotFound as i32,
                    format!("unsupported request: {}", request.method),
                );
                connection.sender.send(response.into())?;
            }
            Message::Notification(notification) => {
                handle_notification(connection, notification);
            }
            Message::Response(_) => {}
        }
    }

    Ok(())
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(PositionEncodingKind::UTF16),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..TextDocumentSyncOptions::default()
            },
        )),
        ..ServerCapabilities::default()
    }
}

fn handle_notification(connection: &Connection, notification: Notification) {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let params = serde_json::from_value::<DidOpenTextDocumentParams>(notification.params);
            match params {
                Ok(params) => publish_diagnostics(
                    connection,
                    params.text_document.uri,
                    params.text_document.version,
                    &params.text_document.text,
                ),
                Err(error) => malformed_notification("textDocument/didOpen", error),
            }
        }
        "textDocument/didChange" => {
            let params = serde_json::from_value::<DidChangeTextDocumentParams>(notification.params);
            match params {
                Ok(params) => {
                    let full_change = params
                        .content_changes
                        .into_iter()
                        .rev()
                        .find(|change| change.range.is_none());

                    if let Some(change) = full_change {
                        publish_diagnostics(
                            connection,
                            params.text_document.uri,
                            params.text_document.version,
                            &change.text,
                        );
                    } else {
                        eprintln!("ignoring textDocument/didChange without a full-document change");
                    }
                }
                Err(error) => malformed_notification("textDocument/didChange", error),
            }
        }
        "textDocument/didClose" => {
            let params = serde_json::from_value::<DidCloseTextDocumentParams>(notification.params);
            match params {
                Ok(params) => send_diagnostics(connection, params.text_document.uri, None, vec![]),
                Err(error) => malformed_notification("textDocument/didClose", error),
            }
        }
        _ => {}
    }
}

fn malformed_notification(method: &str, error: serde_json::Error) {
    eprintln!("ignoring malformed {method} notification: {error}");
}

fn publish_diagnostics(connection: &Connection, uri: Uri, version: i32, text: &str) {
    send_diagnostics(connection, uri, Some(version), diagnostics(text));
}

fn send_diagnostics(
    connection: &Connection,
    uri: Uri,
    version: Option<i32>,
    diagnostics: Vec<Diagnostic>,
) {
    let params = PublishDiagnosticsParams::new(uri, diagnostics, version);
    let notification = Notification::new("textDocument/publishDiagnostics".to_owned(), params);

    if let Err(error) = connection.sender.send(notification.into()) {
        eprintln!("could not publish diagnostics: {error}");
    }
}

fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let Err(error) = fsman::parse_manifest(text) else {
        return vec![];
    };

    vec![Diagnostic {
        range: lsp_range(text, visible_byte_range(text, error.byte_range())),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("syntax-error".to_owned())),
        source: Some("fsman".to_owned()),
        message: error.message(),
        ..Diagnostic::default()
    }]
}

fn visible_byte_range(text: &str, range: ByteRange<usize>) -> ByteRange<usize> {
    if range.start < range.end {
        return range;
    }

    let point = range.start;
    if point < text.len() {
        let remaining = &text[point..];
        let character = remaining
            .chars()
            .next()
            .expect("a non-empty string has a first character");
        let width = if character == '\r' && remaining.as_bytes().get(1) == Some(&b'\n') {
            2
        } else {
            character.len_utf8()
        };
        return point..point + width;
    }

    let content_end = text.trim_end_matches(['\r', '\n']).len();
    let end = if content_end == 0 {
        text.len()
    } else {
        content_end
    };
    let start = text[..end]
        .char_indices()
        .next_back()
        .map_or(end, |(position, _)| position);
    start..end
}

fn lsp_range(text: &str, range: ByteRange<usize>) -> Range {
    Range::new(position_at(text, range.start), position_at(text, range.end))
}

fn position_at(text: &str, byte_offset: usize) -> Position {
    let target = byte_offset.min(text.len());
    let bytes = text.as_bytes();
    let mut byte = 0;
    let mut line = 0_u32;
    let mut character = 0_u32;

    while byte < target {
        match bytes[byte] {
            b'\r' => {
                let is_crlf = bytes.get(byte + 1) == Some(&b'\n');
                if is_crlf && target == byte + 1 {
                    break;
                }
                byte += if is_crlf { 2 } else { 1 };
                line = line.saturating_add(1);
                character = 0;
            }
            b'\n' => {
                byte += 1;
                line = line.saturating_add(1);
                character = 0;
            }
            _ => {
                let next = text[byte..]
                    .chars()
                    .next()
                    .expect("the byte index is on a character boundary");
                let width = next.len_utf8();
                if byte + width > target {
                    break;
                }
                byte += width;
                character = character.saturating_add(next.len_utf16() as u32);
            }
        }
    }

    Position::new(line, character)
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, thread};

    use lsp_server::{Message, Notification, Request, RequestId, ResponseKind};
    use lsp_types::{
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        PublishDiagnosticsParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
        TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn converts_unicode_and_crlf_positions_to_utf16() {
        let diagnostic = diagnostics("valid\r\n😀 }\r\n").pop().unwrap();

        assert_eq!(
            diagnostic.range,
            Range::new(Position::new(1, 3), Position::new(1, 4))
        );
    }

    #[test]
    fn highlights_the_previous_character_for_an_eof_error() {
        let diagnostic = diagnostics("dir {\nfile\n").pop().unwrap();

        assert_eq!(
            diagnostic.range,
            Range::new(Position::new(1, 3), Position::new(1, 4))
        );
    }

    #[test]
    fn returns_no_diagnostics_for_valid_input() {
        assert!(diagnostics("dir {\n  file\n}\n").is_empty());
    }

    #[test]
    fn serves_diagnostics_over_an_lsp_connection() {
        let (server, client) = Connection::memory();
        let server_thread = thread::spawn(move || run(&server));

        client
            .sender
            .send(Request::new(RequestId::from(1), "initialize".to_owned(), json!({})).into())
            .unwrap();
        let initialize_response = client.receiver.recv().unwrap();
        let Message::Response(initialize_response) = initialize_response else {
            panic!("expected initialize response");
        };
        let ResponseKind::Ok { result } = initialize_response.response_kind else {
            panic!("expected successful initialize response");
        };
        assert_eq!(result["capabilities"]["positionEncoding"], "utf-16");
        assert_eq!(result["capabilities"]["textDocumentSync"]["change"], 1);

        client
            .sender
            .send(Notification::new("initialized".to_owned(), json!({})).into())
            .unwrap();

        let uri = Uri::from_str("file:///tmp/example.fsman").unwrap();
        let open = DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "fsman".to_owned(),
                1,
                "dir {\nfile\n".to_owned(),
            ),
        };
        client
            .sender
            .send(Notification::new("textDocument/didOpen".to_owned(), open).into())
            .unwrap();
        let opened = receive_diagnostics(&client);
        assert_eq!(opened.version, Some(1));
        assert_eq!(opened.diagnostics.len(), 1);

        let change = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(uri.clone(), 2),
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "dir {\nfile\n}\n".to_owned(),
            }],
        };
        client
            .sender
            .send(Notification::new("textDocument/didChange".to_owned(), change).into())
            .unwrap();
        let changed = receive_diagnostics(&client);
        assert_eq!(changed.version, Some(2));
        assert!(changed.diagnostics.is_empty());

        let close = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier::new(uri),
        };
        client
            .sender
            .send(Notification::new("textDocument/didClose".to_owned(), close).into())
            .unwrap();
        let closed = receive_diagnostics(&client);
        assert_eq!(closed.version, None);
        assert!(closed.diagnostics.is_empty());

        client
            .sender
            .send(Request::new(RequestId::from(2), "shutdown".to_owned(), ()).into())
            .unwrap();
        client
            .sender
            .send(Notification::new("exit".to_owned(), ()).into())
            .unwrap();
        let shutdown_response = client.receiver.recv().unwrap();
        assert!(matches!(shutdown_response, Message::Response(_)));

        server_thread.join().unwrap().unwrap();
    }

    fn receive_diagnostics(connection: &Connection) -> PublishDiagnosticsParams {
        let message = connection.receiver.recv().unwrap();
        let Message::Notification(notification) = message else {
            panic!("expected diagnostics notification");
        };
        assert_eq!(notification.method, "textDocument/publishDiagnostics");
        serde_json::from_value(notification.params).unwrap()
    }
}
