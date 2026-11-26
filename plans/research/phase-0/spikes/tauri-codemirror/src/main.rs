//! Tauri host for the CodeMirror live-preview spike with workspace-scoped assets.
//!
//! Set OFFISH_WORKSPACE to an authorised directory (defaults to the Phase 0 pilot
//! fixtures when the spike lives under plans/research/phase-0/spikes/). Relative
//! image links resolve through the `workspace:` URI scheme. Path escapes outside
//! the root return 403.

use http::{header, StatusCode};
use serde::Serialize;
use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tauri::Manager;

#[derive(Clone)]
struct Workspace(Arc<PathBuf>);

#[derive(Serialize)]
struct Boot {
    title: String,
    source: String,
    asset_base: String,
}

fn default_workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pilot")
}

fn workspace_root() -> PathBuf {
    env::var_os("OFFISH_WORKSPACE")
        .map(PathBuf::from)
        .unwrap_or_else(default_workspace)
        .canonicalize()
        .unwrap_or_else(|error| panic!("OFFISH_WORKSPACE must exist: {error}"))
}

/// Resolve a relative workspace path. Reject absolute paths, parent segments, and
/// any target that does not stay under the authorised root after canonicalisation.
fn resolve_under(root: &Path, relative: &str) -> Result<PathBuf, StatusCode> {
    let trimmed = relative.trim_start_matches('/');
    if trimmed.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let requested = Path::new(trimmed);
    if requested.is_absolute()
        || requested
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let candidate = root.join(requested);
    let canonical = candidate.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;
    if !canonical.starts_with(root) {
        return Err(StatusCode::FORBIDDEN);
    }
    if !canonical.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(canonical)
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("md") => "text/markdown; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn response(status: StatusCode, content_type: &str, body: impl Into<Vec<u8>>) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body.into())
        .expect("workspace protocol response")
}

#[tauri::command]
fn boot(app: tauri::AppHandle) -> Result<Boot, String> {
    let workspace = app.state::<Workspace>().0.clone();
    let source = fs::read_to_string(workspace.join("brief.md"))
        .map_err(|error| format!("read brief.md under {}: {error}", workspace.display()))?;
    Ok(Boot {
        title: "Offish CodeMirror ready".into(),
        source,
        // Linux/macOS Origin for custom schemes is scheme://localhost/
        asset_base: "workspace://localhost/".into(),
    })
}

fn main() {
    let root = workspace_root();
    tauri::Builder::default()
        .manage(Workspace(Arc::new(root.clone())))
        .invoke_handler(tauri::generate_handler![boot])
        .register_uri_scheme_protocol("workspace", move |_ctx, request| {
            let path = request.uri().path();
            match resolve_under(&root, path) {
                Ok(file) => {
                    let body = fs::read(&file).unwrap_or_default();
                    response(StatusCode::OK, mime_for(&file), body)
                }
                Err(StatusCode::FORBIDDEN) => {
                    response(StatusCode::FORBIDDEN, "text/plain", "path outside workspace")
                }
                Err(status) => response(status, "text/plain", status.as_str()),
            }
        })
        .run(tauri::generate_context!())
        .expect("open the Phase 0 CodeMirror WebView");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_parent_escape_and_missing() {
        let directory = scratch();
        fs::write(directory.join("assets/timeline.svg"), b"<svg/>").unwrap();
        assert_eq!(
            resolve_under(&directory, "../secret").unwrap_err(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            resolve_under(&directory, "missing.svg").unwrap_err(),
            StatusCode::NOT_FOUND
        );
        let allowed = resolve_under(&directory, "/assets/timeline.svg").unwrap();
        assert!(allowed.ends_with("timeline.svg"));
    }

    #[test]
    fn rejects_symlink_escape() {
        let directory = scratch();
        let outside = directory.parent().unwrap().join(format!("offish-escape-{}", std::process::id()));
        fs::write(&outside, b"no").unwrap();
        std::os::unix::fs::symlink(&outside, directory.join("escape.svg")).unwrap();
        assert_eq!(
            resolve_under(&directory, "escape.svg").unwrap_err(),
            StatusCode::FORBIDDEN
        );
        let _ = fs::remove_file(outside);
    }

    fn scratch() -> PathBuf {
        let root = env::temp_dir().join(format!("offish-workspace-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("assets")).unwrap();
        root.canonicalize().unwrap()
    }
}
