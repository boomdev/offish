#[cfg(target_os = "linux")]
mod linux {
    use gtk::prelude::*;
    use offish::workspace::{Document, Entry, Save, Workspace};
    use std::{path::PathBuf, sync::Mutex};
    use tauri::{AppHandle, Manager, State, WebviewWindow};

    type Current = Mutex<Option<Workspace>>;
    type Response<T> = Result<T, String>;

    fn open_workspace(app: &AppHandle, path: &std::path::Path) -> Response<Workspace> {
        let recovery = app
            .path()
            .app_local_data_dir()
            .map_err(|error| error.to_string())?;
        Workspace::open(path)
            .map(|workspace| workspace.with_recovery(recovery.join("recovery")))
            .map_err(|error| error.to_string())
    }

    fn with_workspace<T>(
        state: State<'_, Current>,
        call: impl FnOnce(&Workspace) -> std::io::Result<T>,
    ) -> Response<T> {
        let workspace = state.lock().map_err(|error| error.to_string())?;
        call(workspace.as_ref().ok_or("Open a workspace folder first.")?)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    async fn choose_folder(app: AppHandle) -> Response<Option<String>> {
        let (send, receive) = tauri::async_runtime::channel(1);
        app.run_on_main_thread(move || {
            let picker = gtk::FileChooserNative::new(
                Some("Open workspace folder"),
                None::<&gtk::Window>,
                gtk::FileChooserAction::SelectFolder,
                Some("Open"),
                Some("Cancel"),
            );
            picker.set_modal(true);
            let retained = std::rc::Rc::new(std::cell::RefCell::new(Some(picker.clone())));
            picker.connect_response(move |picker, response| {
                let result = if response == gtk::ResponseType::Accept {
                    picker
                        .filename()
                        .ok_or_else(|| "Choose a local folder.".to_string())
                        .map(Some)
                } else {
                    Ok(None)
                };
                let _ = send.try_send(result);
                picker.hide();
                retained.borrow_mut().take();
            });
            picker.show();
        })
        .map_err(|error| error.to_string())?;
        let mut receive = receive;
        let Some(path) = receive.recv().await.ok_or("Folder picker closed.")?? else {
            return Ok(None);
        };
        let workspace = open_workspace(&app, &path)?;
        let name = workspace.name.clone();
        *app.state::<Current>()
            .lock()
            .map_err(|error| error.to_string())? = Some(workspace);
        Ok(Some(name))
    }

    #[tauri::command]
    fn workspace_name(state: State<'_, Current>) -> Option<String> {
        state
            .lock()
            .ok()?
            .as_ref()
            .map(|workspace| workspace.name.clone())
    }

    #[tauri::command]
    fn list_directory(state: State<'_, Current>, path: String) -> Response<Vec<Entry>> {
        with_workspace(state, |workspace| workspace.list(&path))
    }

    #[tauri::command]
    fn read_document(state: State<'_, Current>, path: String) -> Response<Document> {
        with_workspace(state, |workspace| workspace.read(&path))
    }

    #[tauri::command]
    fn save_document(
        state: State<'_, Current>,
        path: String,
        base: String,
        text: String,
    ) -> Response<Save> {
        with_workspace(state, |workspace| workspace.save(&path, &base, &text))
    }

    #[tauri::command]
    fn recovery_draft(state: State<'_, Current>, path: String) -> Response<Option<String>> {
        with_workspace(state, |workspace| workspace.recovery_draft(&path))
    }

    #[tauri::command]
    fn discard_recovery(state: State<'_, Current>, path: String, draft: String) -> Response<()> {
        with_workspace(state, |workspace| workspace.discard_recovery(&path, &draft))
    }

    #[tauri::command]
    fn close_offish(window: WebviewWindow) -> Response<()> {
        window.destroy().map_err(|error| error.to_string())
    }

    pub fn run() {
        use tauri::Emitter;
        tauri::Builder::default()
            .manage(Current::default())
            .setup(|app| {
                if let Some(path) = std::env::args_os().nth(1) {
                    let workspace = open_workspace(app.handle(), &PathBuf::from(path))
                        .map_err(std::io::Error::other)?;
                    *app.state::<Current>().lock().expect("workspace state") = Some(workspace);
                }
                Ok(())
            })
            .on_window_event(|window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.emit("offish-close-request", ());
                }
            })
            .invoke_handler(tauri::generate_handler![
                choose_folder,
                workspace_name,
                list_directory,
                read_document,
                save_document,
                recovery_draft,
                discard_recovery,
                close_offish
            ])
            .run(tauri::generate_context!())
            .expect("run Offish");
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    linux::run();
    #[cfg(not(target_os = "linux"))]
    eprintln!("The Phase 1 desktop build currently supports Linux.");
}
