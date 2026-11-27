#[cfg(target_os = "linux")]
mod linux {
    use gtk::prelude::*;
    use offish::workspace::{Document, Entry, Save, TrashEntry, Workspace};
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

    async fn pick_path(app: &AppHandle, folder: bool) -> Response<Option<PathBuf>> {
        let (send, receive) = tauri::async_runtime::channel(1);
        app.run_on_main_thread(move || {
            let picker = gtk::FileChooserNative::new(
                Some(if folder {
                    "Open workspace folder"
                } else {
                    "Open file"
                }),
                None::<&gtk::Window>,
                if folder {
                    gtk::FileChooserAction::SelectFolder
                } else {
                    gtk::FileChooserAction::Open
                },
                Some("Open"),
                Some("Cancel"),
            );
            picker.set_modal(true);
            let retained = std::rc::Rc::new(std::cell::RefCell::new(Some(picker.clone())));
            picker.connect_response(move |picker, response| {
                let result = if response == gtk::ResponseType::Accept {
                    picker
                        .filename()
                        .ok_or_else(|| "Choose a local file or folder.".to_string())
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
        receive
            .recv()
            .await
            .ok_or_else(|| "File chooser closed.".to_string())?
    }

    #[tauri::command]
    async fn choose_folder(app: AppHandle) -> Response<Option<String>> {
        let Some(path) = pick_path(&app, true).await? else {
            return Ok(None);
        };
        let workspace = open_workspace(&app, &path)?;
        let name = workspace.name.clone();
        *app.state::<Current>()
            .lock()
            .map_err(|error| error.to_string())? = Some(workspace);
        Ok(Some(name))
    }

    #[derive(serde::Serialize)]
    struct PickedFile {
        workspace: String,
        path: String,
        switched: bool,
    }

    #[tauri::command]
    async fn choose_file(app: AppHandle, preserve_workspace: bool) -> Response<Option<PickedFile>> {
        let Some(path) = pick_path(&app, false).await? else {
            return Ok(None);
        };
        let state = app.state::<Current>();
        let mut current = state.lock().map_err(|error| error.to_string())?;
        if let Some(workspace) = current.as_ref() {
            if let Some(relative) = workspace
                .selected_file(&path)
                .map_err(|error| error.to_string())?
            {
                return Ok(Some(PickedFile {
                    workspace: workspace.name.clone(),
                    path: relative,
                    switched: false,
                }));
            }
            if preserve_workspace {
                return Err(
                    "Save or close your unsaved tabs before opening a file outside this workspace."
                        .into(),
                );
            }
        }
        let parent = path
            .parent()
            .ok_or("The selected file has no containing folder.")?;
        let workspace = open_workspace(&app, parent)?;
        let relative = workspace
            .selected_file(&path)
            .map_err(|error| error.to_string())?
            .ok_or("The selected file is outside its workspace.")?;
        let name = workspace.name.clone();
        *current = Some(workspace);
        Ok(Some(PickedFile {
            workspace: name,
            path: relative,
            switched: true,
        }))
    }

    #[tauri::command]
    async fn copy_text(app: AppHandle, text: String) -> Response<()> {
        let (send, mut receive) = tauri::async_runtime::channel(1);
        app.run_on_main_thread(move || {
            gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_text(&text);
            let _ = send.try_send(());
        })
        .map_err(|error| error.to_string())?;
        receive
            .recv()
            .await
            .ok_or_else(|| "Clipboard is unavailable.".into())
    }

    #[tauri::command]
    async fn clipboard_text(app: AppHandle) -> Response<Option<String>> {
        let (send, mut receive) = tauri::async_runtime::channel(1);
        app.run_on_main_thread(move || {
            gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).request_text(move |_, text| {
                let _ = send.try_send(text.map(str::to_owned));
            });
        })
        .map_err(|error| error.to_string())?;
        receive
            .recv()
            .await
            .ok_or_else(|| "Clipboard is unavailable.".into())
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
    fn create_entry(
        state: State<'_, Current>,
        parent: String,
        name: String,
        folder: bool,
    ) -> Response<Option<String>> {
        with_workspace(state, |workspace| {
            workspace.create_entry(&parent, &name, folder)
        })
    }

    #[tauri::command]
    fn rename_entry(
        state: State<'_, Current>,
        path: String,
        name: String,
    ) -> Response<Option<String>> {
        with_workspace(state, |workspace| workspace.rename_entry(&path, &name))
    }

    #[tauri::command]
    fn move_entry(
        state: State<'_, Current>,
        path: String,
        folder: String,
    ) -> Response<Option<String>> {
        with_workspace(state, |workspace| workspace.move_entry(&path, &folder))
    }

    #[tauri::command]
    fn duplicate_entry(
        state: State<'_, Current>,
        path: String,
        name: String,
    ) -> Response<Option<String>> {
        with_workspace(state, |workspace| workspace.duplicate_entry(&path, &name))
    }

    #[tauri::command]
    fn trash_entry(state: State<'_, Current>, path: String) -> Response<Option<String>> {
        with_workspace(state, |workspace| workspace.trash_entry(&path))
    }

    #[tauri::command]
    fn delete_entry(state: State<'_, Current>, path: String) -> Response<Option<String>> {
        with_workspace(state, |workspace| workspace.delete_entry(&path))
    }

    #[tauri::command]
    fn list_trash(state: State<'_, Current>) -> Response<Vec<TrashEntry>> {
        with_workspace(state, Workspace::list_trash)
    }

    #[tauri::command]
    fn restore_entry(state: State<'_, Current>, id: String) -> Response<Option<String>> {
        with_workspace(state, |workspace| workspace.restore_entry(&id))
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
                choose_file,
                copy_text,
                clipboard_text,
                workspace_name,
                list_directory,
                create_entry,
                rename_entry,
                move_entry,
                duplicate_entry,
                trash_entry,
                delete_entry,
                list_trash,
                restore_entry,
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
