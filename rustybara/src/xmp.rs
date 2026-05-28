//! XMP metadata utilities for rustybara.
//!
//! Provides SHA-256 hashing, UUID generation, XMP block rendering, and
//! inject-or-create helpers used by [`crate::PdfPipeline::embed_metadata`].

use sha2::{Digest, Sha256};
use std::path::Path;

/// Namespace URI for the `rbara:` XMP namespace.
pub const RBARA_NS: &str = "https://rustybara.io/ns/1.0/";

/// Data carried in the `rbara` XMP description block.
#[derive(Clone)]
pub struct RbaraXmpBlock {
    pub uuid: String,
    pub version: String,
    pub timestamp: String,
    pub source_hash: String,
    pub parent_id: String,
    pub ops: Vec<String>,
}

/// Compute `"sha256:<hex>"` of raw bytes.
///
/// Use this in environments without a filesystem (e.g., WebAssembly) where the
/// PDF data is already in memory.
pub fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// Compute `"sha256:<hex>"` of a file's raw bytes.
///
/// Call this on the **source** path before opening a [`crate::PdfPipeline`] so
/// the hash reflects the unmodified file.
pub fn hash_file(path: &Path) -> crate::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hash_bytes(&bytes))
}

/// Generate a random v4 UUID string.
pub fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Scan raw XMP bytes for an existing `<rbara:uuid>` value.
///
/// Returns the UUID string if found, or an empty string if the file was not
/// previously processed by rustybara.
pub fn read_parent_id(xmp_bytes: &[u8]) -> String {
    let text = std::str::from_utf8(xmp_bytes).unwrap_or("");
    let open = "<rbara:uuid>";
    let close = "</rbara:uuid>";
    if let Some(s) = text.find(open) {
        let after = &text[s + open.len()..];
        if let Some(e) = after.find(close) {
            return after[..e].trim().to_string();
        }
    }
    String::new()
}

/// Parse a `rbara:` block from raw XMP bytes.
///
/// Returns `None` if the file has no `<!-- rbara:start -->` sentinel — i.e., it
/// was not processed by rustybara. All fields fall back to empty strings when a
/// tag is present but unparseable.
pub fn parse_rbara_block(xmp_bytes: &[u8]) -> Option<RbaraXmpBlock> {
    let text = std::str::from_utf8(xmp_bytes).ok()?;

    const START: &str = "<!-- rbara:start -->";
    const END: &str = "<!-- rbara:end -->";
    let start_pos = text.find(START)?;
    let end_pos = text.find(END)?;
    let block = &text[start_pos..end_pos + END.len()];

    fn extract(block: &str, tag: &str) -> String {
        let open = format!("<rbara:{tag}>");
        let close = format!("</rbara:{tag}>");
        block
            .find(open.as_str())
            .and_then(|s| {
                let after = &block[s + open.len()..];
                after
                    .find(close.as_str())
                    .map(|e| after[..e].trim().to_string())
            })
            .unwrap_or_default()
    }

    let ops = {
        let mut ops = Vec::new();
        let li_open = "<rdf:li>";
        let li_close = "</rdf:li>";
        let mut rest = block;
        while let Some(s) = rest.find(li_open) {
            let after = &rest[s + li_open.len()..];
            match after.find(li_close) {
                Some(e) => {
                    let op = after[..e].trim().to_string();
                    if !op.is_empty() {
                        ops.push(op);
                    }
                    rest = &after[e + li_close.len()..];
                }
                None => break,
            }
        }
        ops
    };

    Some(RbaraXmpBlock {
        uuid: extract(block, "uuid"),
        version: extract(block, "version"),
        timestamp: extract(block, "timestamp"),
        source_hash: extract(block, "sourceHash"),
        parent_id: extract(block, "parentId"),
        ops,
    })
}

/// Render the `rbara:` RDF Description block.
///
/// Wrapped in `<!-- rbara:start -->` / `<!-- rbara:end -->` sentinels so
/// subsequent processing runs can locate and replace it without XML parsing.
pub fn render_block(b: &RbaraXmpBlock) -> String {
    let ops_xml = if b.ops.is_empty() {
        "   <rbara:ops/>\n".to_string()
    } else {
        let items = b
            .ops
            .iter()
            .map(|op| format!("     <rdf:li>{op}</rdf:li>"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "      <rbara:ops>\n        <rdf:Seq>\n{items}\n        </rdf:Seq>\n      </rbara:ops>\n"
        )
    };

    format!(
        "<!-- rbara:start -->\n    \
<rdf:Description rdf:about=\"\"\n        \
    xmlns:rbara=\"{ns}\">\n      \
  <rbara:uuid>{uuid}</rbara:uuid>\n      \
  <rbara:version>{ver}</rbara:version>\n      \
  <rbara:timestamp>{ts}</rbara:timestamp>\n      \
  <rbara:sourceHash>{hash}</rbara:sourceHash>\n      \
  <rbara:parentId>{pid}</rbara:parentId>\n\
{ops_xml}\
    </rdf:Description>\n\
<!-- rbara:end -->\n",
        ns = RBARA_NS,
        uuid = b.uuid,
        ver = b.version,
        ts = b.timestamp,
        hash = b.source_hash,
        pid = b.parent_id,
    )
}

/// Create a minimal XMP document containing only the `rbara:` block.
pub fn create_xmp(b: &RbaraXmpBlock) -> String {
    format!(
        "<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"rustybara {ver}\">\n  \
  <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
{block}\
  </rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>",
        ver = b.version,
        block = render_block(b),
    )
}

/// Inject the `rbara:` block into an existing XMP document string.
///
/// Any previous `rbara:` block (identified by sentinel comments) is removed
/// first so there is never more than one. The new block is inserted immediately
/// before `</rdf:RDF>`. Falls back to appending if `</rdf:RDF>` is absent.
pub fn inject_into_xmp(existing: &str, b: &RbaraXmpBlock) -> String {
    const START: &str = "<!-- rbara:start -->";
    const END: &str = "<!-- rbara:end -->";

    let cleaned: std::borrow::Cow<str> = match (existing.find(START), existing.find(END)) {
        (Some(s), Some(e)) => {
            let end_pos = e + END.len();
            format!("{}{}", &existing[..s], &existing[end_pos..]).into()
        }
        _ => existing.into(),
    };

    let block = render_block(b);
    const CLOSE: &str = "</rdf:RDF>";
    match cleaned.find(CLOSE) {
        Some(pos) => format!("{}{}{}", &cleaned[..pos], block, &cleaned[pos..]),
        None => format!("{}\n{}", cleaned, block),
    }
}
