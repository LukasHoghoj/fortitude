use std::sync::atomic::{AtomicBool, Ordering};

use lsp_server::ErrorCode;
use lsp_types::{self as types, request as req};

use crate::server::Result;
use crate::server::api::LSPResult;
use crate::session::{Client, DocumentSnapshot};

pub(crate) struct Format;

impl super::RequestHandler for Format {
    type RequestType = req::Formatting;
}

impl super::BackgroundDocumentRequestHandler for Format {
    super::define_document_url!(params: &types::DocumentFormattingParams);
    fn run_with_snapshot(
        snapshot: DocumentSnapshot,
        client: &Client,
        _params: types::DocumentFormattingParams,
    ) -> Result<Option<Vec<types::TextEdit>>> {
        // The indentation comes from the Fortitude settings, so the editor's
        // formatting options (`tabSize`, `insertSpaces`, ...) are ignored
        let query = snapshot.query();
        if query.settings().format.preview.is_disabled() {
            warn_preview_disabled(client);
            return Ok(None);
        }
        crate::format::format(query, snapshot.encoding())
            .with_failure_code(ErrorCode::InternalError)
    }
}

/// Tell the user once per session why formatting does nothing
fn warn_preview_disabled(client: &Client) {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        client.show_warning_message(
            "The Fortitude formatter is experimental and only runs in preview mode. Enable it \
             with `preview = true` in the `[format]` section of your configuration file, or \
             with the `format.preview` editor setting.",
        );
    }
    tracing::info!("Not formatting: the formatter only runs in preview mode");
}
