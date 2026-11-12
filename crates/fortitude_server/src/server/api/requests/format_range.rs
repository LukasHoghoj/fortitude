use lsp_server::ErrorCode;
use lsp_types::{self as types, request as req};

use crate::server::Result;
use crate::server::api::LSPResult;
use crate::session::{Client, DocumentSnapshot};

use super::format::warn_preview_disabled;

pub(crate) struct FormatRange;

impl super::RequestHandler for FormatRange {
    type RequestType = req::RangeFormatting;
}

impl super::BackgroundDocumentRequestHandler for FormatRange {
    super::define_document_url!(params: &types::DocumentRangeFormattingParams);
    fn run_with_snapshot(
        snapshot: DocumentSnapshot,
        client: &Client,
        params: types::DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<types::TextEdit>>> {
        let query = snapshot.query();
        if query.settings().format.preview.is_disabled() {
            warn_preview_disabled(client);
            return Ok(None);
        }
        crate::format::format_range(query, params.range, snapshot.encoding())
            .with_failure_code(ErrorCode::InternalError)
    }
}
