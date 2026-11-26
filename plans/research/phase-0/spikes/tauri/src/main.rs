#[tauri::command]
fn probe() -> &'static str {
    "Offish WebView ready"
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![probe])
        .run(tauri::generate_context!())
        .expect("open the Phase 0 WebView");
}
