#[cfg(target_os = "linux")]
mod linux {
    use gtk::{gdk, glib::Propagation, prelude::*};
    use offish::agent::codex::auth::AccountStore;
    use offish::agent::{Conversation, Prepared, Request, Update};
    use offish::markdown::SourceEdits;
    use offish::markdown::table_cell::{self, TableCell, TableCellAddress};
    use offish::workspace::context::history::{HistoryStore, Message};
    use offish::workspace::review::{Acceptance, Proposal, ReviewHistory};
    use offish::workspace::search::{SearchIndex, SearchReport, SearchRoot};
    use offish::workspace::{Document, Entry, PreviewImage, Save, TrashEntry, Workspace};
    use offish::workspaces::{ActiveWorkspace, CatalogStore, SavedWorkspace, canonical_roots};
    use std::{
        io,
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

    type Current = Mutex<Runtime>;
    type Response<T> = Result<T, String>;

    struct Runtime {
        catalog: CatalogStore,
        recovery: PathBuf,
        chat: PathBuf,
        conversation: Conversation,
        accounts: AccountStore,
        active: Option<ActiveWorkspace>,
        warning: Option<String>,
        search: Arc<Mutex<SearchIndex>>,
        private_paths: Vec<PathBuf>,
    }

    impl Runtime {
        fn activate(&mut self, record: SavedWorkspace) -> String {
            let name = record.name();
            self.replace_workspace(Some(ActiveWorkspace::open(record, &self.recovery)));
            self.warning = None;
            name
        }

        fn replace_workspace(&mut self, workspace: Option<ActiveWorkspace>) {
            self.conversation.revoke();
            self.active = workspace;
            self.search = Arc::default();
        }

        fn remember(&mut self, paths: Vec<PathBuf>) -> io::Result<SavedWorkspace> {
            let roots = canonical_roots(paths)?;
            self.catalog.update(|catalog| catalog.remember(roots))
        }
    }

    fn with_conversation<T>(
        state: State<'_, Current>,
        call: impl FnOnce(
            &mut Conversation,
            &ActiveWorkspace,
            &[PathBuf],
            &HistoryStore,
        ) -> io::Result<T>,
    ) -> Response<T> {
        let mut current = state
            .lock()
            .map_err(|_| "Conversation state is unavailable.")?;
        let Runtime {
            active,
            chat,
            private_paths,
            conversation,
            ..
        } = &mut *current;
        let workspace = active.as_ref().ok_or("Open a workspace folder first.")?;
        let history = HistoryStore::new(chat, workspace).map_err(|error| error.to_string())?;
        call(conversation, workspace, private_paths, &history).map_err(|error| error.to_string())
    }

    #[tauri::command]
    fn conversation_history(state: State<'_, Current>) -> Response<Vec<Message>> {
        with_conversation(state, |_, _, _, history| history.load())
    }

    #[tauri::command]
    fn clear_conversation(state: State<'_, Current>) -> Response<()> {
        with_conversation(state, |conversation, _, _, history| {
            conversation.revoke();
            history.clear()
        })
    }

    #[tauri::command]
    async fn prepare_agent_turn(app: AppHandle, request: Request) -> Response<Prepared> {
        tauri::async_runtime::spawn_blocking(move || {
            with_conversation(app.state(), |conversation, workspace, private, _| {
                conversation.prepare(workspace, private, request)
            })
        })
        .await
        .map_err(|_| "Could not prepare context.")?
    }

    #[tauri::command]
    fn approve_agent_turn(state: State<'_, Current>, id: String) -> Response<Message> {
        let accounts = state
            .lock()
            .map_err(|_| "Conversation state is unavailable.")?
            .accounts
            .clone();
        with_conversation(state, |conversation, workspace, private, history| {
            conversation.approve_with_account(&id, workspace, private, history, Some(&accounts))
        })
    }

    async fn account_operation<T: Send + 'static>(
        state: State<'_, Current>,
        run: impl FnOnce(&AccountStore) -> io::Result<T> + Send + 'static,
    ) -> Response<T> {
        let accounts = state
            .lock()
            .map_err(|_| "ChatGPT account state is unavailable.")?
            .accounts
            .clone();
        tauri::async_runtime::spawn_blocking(move || {
            run(&accounts).map_err(|error| error.to_string())
        })
        .await
        .map_err(|_| "The ChatGPT account operation failed.")?
    }

    #[tauri::command]
    async fn chatgpt_connected(state: State<'_, Current>) -> Response<bool> {
        account_operation(state, AccountStore::connected).await
    }

    #[tauri::command]
    async fn connect_chatgpt(state: State<'_, Current>) -> Response<()> {
        account_operation(state, |accounts| {
            offish::agent::codex::check_runtime()?;
            accounts.connect()
        })
        .await
    }

    #[tauri::command]
    async fn disconnect_chatgpt(state: State<'_, Current>) -> Response<()> {
        account_operation(state, AccountStore::disconnect).await
    }

    #[tauri::command]
    fn poll_agent_turn(state: State<'_, Current>, id: String) -> Response<Update> {
        with_conversation(state, |conversation, workspace, private, history| {
            conversation.poll(&id, workspace, private, history)
        })
    }

    #[tauri::command]
    fn cancel_agent_turn(state: State<'_, Current>, id: String) -> Response<Option<Message>> {
        with_conversation(state, |conversation, _, _, history| {
            conversation.cancel(&id, history)
        })
    }

    fn with_workspace<T>(
        state: State<'_, Current>,
        call: impl FnOnce(&ActiveWorkspace) -> io::Result<T>,
    ) -> Response<T> {
        let current = state.lock().map_err(|error| error.to_string())?;
        call(
            current
                .active
                .as_ref()
                .ok_or("Open a workspace folder first.")?,
        )
        .map_err(|error| error.to_string())
    }

    fn with_root<T>(
        state: State<'_, Current>,
        path: &str,
        call: impl FnOnce(&Workspace, &str) -> io::Result<T>,
    ) -> Response<T> {
        with_workspace(state, |workspace| {
            let (_, root, relative) = workspace.route(path)?;
            call(root, relative)
        })
    }

    async fn pick_paths(
        app: &AppHandle,
        title: &'static str,
        folder: bool,
        multiple: bool,
    ) -> Response<Option<Vec<PathBuf>>> {
        let (send, receive) = tauri::async_runtime::channel(1);
        app.run_on_main_thread(move || {
            let picker = gtk::FileChooserNative::new(
                Some(title),
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
            picker.set_select_multiple(multiple);
            let retained = std::rc::Rc::new(std::cell::RefCell::new(Some(picker.clone())));
            picker.connect_response(move |picker, response| {
                let result = if response == gtk::ResponseType::Accept {
                    let paths = picker.filenames();
                    if paths.is_empty() {
                        Err("Choose a local file or folder.".to_string())
                    } else {
                        Ok(Some(paths))
                    }
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
        select_workspace(&app, "Open workspace folder", false).await
    }

    #[tauri::command]
    async fn create_workspace(app: AppHandle) -> Response<Option<String>> {
        select_workspace(&app, "Create workspace", true).await
    }

    async fn select_workspace(
        app: &AppHandle,
        title: &'static str,
        multiple: bool,
    ) -> Response<Option<String>> {
        let Some(paths) = pick_paths(app, title, true, multiple).await? else {
            return Ok(None);
        };
        let state = app.state::<Current>();
        let mut current = state.lock().map_err(|error| error.to_string())?;
        let record = current.remember(paths).map_err(|error| error.to_string())?;
        Ok(Some(current.activate(record)))
    }

    #[tauri::command]
    async fn add_workspace_folders(app: AppHandle) -> Response<Option<String>> {
        let Some(paths) = pick_paths(&app, "Add folders to workspace", true, true).await? else {
            return Ok(None);
        };
        let state = app.state::<Current>();
        let mut current = state.lock().map_err(|error| error.to_string())?;
        let roots = canonical_roots(paths).map_err(|error| error.to_string())?;
        let Some(active) = &current.active else {
            let record = current
                .catalog
                .update(|catalog| catalog.remember(roots))
                .map_err(|error| error.to_string())?;
            return Ok(Some(current.activate(record)));
        };
        let record = current
            .catalog
            .update(|catalog| catalog.add_roots(&active.record, roots))
            .map_err(|error| error.to_string())?;
        let name = record.name();
        let Runtime {
            active, recovery, ..
        } = &mut *current;
        if let Some(active) = active {
            active.extend(record, recovery);
        }
        Ok(Some(name))
    }

    #[tauri::command]
    fn open_saved_workspace(state: State<'_, Current>, id: u64) -> Response<String> {
        let mut current = state.lock().map_err(|error| error.to_string())?;
        let record = current
            .catalog
            .update(|catalog| catalog.reopen(id))
            .map_err(|error| error.to_string())?;
        Ok(current.activate(record))
    }

    #[tauri::command]
    fn rename_workspace(state: State<'_, Current>, name: String) -> Response<String> {
        let mut current = state.lock().map_err(|error| error.to_string())?;
        let Runtime {
            active, catalog, ..
        } = &mut *current;
        let active = active.as_mut().ok_or("Open a workspace first.")?;
        let name = catalog
            .update(|catalog| catalog.rename(active.record.id, &name))
            .map_err(|error| error.to_string())?;
        active.record.custom_name = Some(name.clone());
        Ok(name)
    }

    #[tauri::command]
    fn close_workspace(state: State<'_, Current>) -> Response<()> {
        let mut current = state.lock().map_err(|error| error.to_string())?;
        current
            .catalog
            .update(|catalog| {
                catalog.last = None;
                Ok(())
            })
            .map_err(|error| error.to_string())?;
        current.replace_workspace(None);
        Ok(())
    }

    #[derive(serde::Serialize)]
    struct PickedFile {
        workspace: String,
        path: String,
        switched: bool,
    }

    #[tauri::command]
    async fn choose_file(app: AppHandle, preserve_workspace: bool) -> Response<Option<PickedFile>> {
        let Some(path) = pick_paths(&app, "Open file", false, false)
            .await?
            .and_then(|mut paths| paths.pop())
        else {
            return Ok(None);
        };
        let state = app.state::<Current>();
        let mut current = state.lock().map_err(|error| error.to_string())?;
        if let Some(workspace) = current.active.as_ref() {
            if let Some(relative) = workspace
                .selected_file(&path)
                .map_err(|error| error.to_string())?
            {
                return Ok(Some(PickedFile {
                    workspace: workspace.record.name(),
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
        let roots = canonical_roots(vec![parent.into()]).map_err(|error| error.to_string())?;
        let mut workspace = ActiveWorkspace::open(
            SavedWorkspace {
                id: 0,
                roots,
                custom_name: None,
            },
            &current.recovery,
        );
        let relative = workspace
            .selected_file(&path)
            .map_err(|error| error.to_string())?
            .ok_or("The selected file is outside its workspace.")?;
        workspace.record = current
            .catalog
            .update(|catalog| catalog.remember(workspace.record.roots.clone()))
            .map_err(|error| error.to_string())?;
        let name = workspace.record.name();
        current.replace_workspace(Some(workspace));
        current.warning = None;
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

    #[derive(serde::Serialize)]
    struct WorkspaceState {
        name: Option<String>,
        saved: Vec<WorkspaceTile>,
        warning: Option<String>,
    }

    #[derive(serde::Serialize)]
    struct WorkspaceTile {
        id: u64,
        name: String,
        roots: Vec<PathBuf>,
    }

    #[tauri::command]
    fn workspace_state(state: State<'_, Current>) -> Response<WorkspaceState> {
        let current = state.lock().map_err(|error| error.to_string())?;
        let (saved, warning) = match current.catalog.load() {
            Ok(catalog) => (
                catalog
                    .saved
                    .into_iter()
                    .map(|entry| WorkspaceTile {
                        name: entry.name(),
                        id: entry.id,
                        roots: entry.roots,
                    })
                    .collect(),
                current.warning.clone(),
            ),
            Err(error) => (
                Vec::new(),
                Some(format!("Could not load saved workspaces: {error}")),
            ),
        };
        Ok(WorkspaceState {
            name: current.active.as_ref().map(|active| active.record.name()),
            saved,
            warning,
        })
    }

    #[tauri::command]
    fn list_directory(state: State<'_, Current>, path: String) -> Response<Vec<Entry>> {
        with_workspace(state, |workspace| {
            if path.is_empty() {
                return Ok(workspace.root_entries());
            }
            let (_, root, relative) = workspace.route(&path)?;
            root.list(relative)
        })
    }

    #[tauri::command]
    async fn search_workspace(
        state: State<'_, Current>,
        query: String,
        rebuild: bool,
    ) -> Response<SearchReport> {
        let (index, roots, private_paths) = {
            let current = state.lock().map_err(|error| error.to_string())?;
            let active = current.active.as_ref().ok_or("Open a workspace first.")?;
            let roots = active
                .available_roots()
                .map(|(index, root)| SearchRoot::new(index, root))
                .collect::<io::Result<Vec<_>>>()
                .map_err(|error| error.to_string())?;
            (
                Arc::clone(&current.search),
                roots,
                current.private_paths.clone(),
            )
        };
        tauri::async_runtime::spawn_blocking(move || {
            let mut index = index.try_lock().map_err(|_| "Workspace search is busy.")?;
            index
                .search(&roots, &private_paths, &query, rebuild)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    fn create_entry(
        state: State<'_, Current>,
        parent: String,
        name: String,
        folder: bool,
    ) -> Response<Option<String>> {
        with_root(state, &parent, |root, relative| {
            root.create_entry(relative, &name, folder)
        })
    }

    #[tauri::command]
    fn rename_entry(
        state: State<'_, Current>,
        path: String,
        name: String,
    ) -> Response<Option<String>> {
        with_root(state, &path, |root, relative| {
            root.rename_entry(relative, &name)
        })
    }

    #[tauri::command]
    fn move_entry(
        state: State<'_, Current>,
        path: String,
        folder: String,
    ) -> Response<Option<String>> {
        with_workspace(state, |workspace| {
            let (source, root, path) = workspace.route(&path)?;
            let (destination, _, folder) = workspace.route(&folder)?;
            if source != destination {
                return Err(io::Error::other(
                    "Choose a destination within the same root folder.",
                ));
            }
            root.move_entry(path, folder)
        })
    }

    #[tauri::command]
    fn duplicate_entry(
        state: State<'_, Current>,
        path: String,
        name: String,
    ) -> Response<Option<String>> {
        with_root(state, &path, |root, relative| {
            root.duplicate_entry(relative, &name)
        })
    }

    #[tauri::command]
    fn trash_entry(state: State<'_, Current>, path: String) -> Response<Option<String>> {
        with_root(state, &path, |root, relative| root.trash_entry(relative))
    }

    #[tauri::command]
    fn delete_entry(state: State<'_, Current>, path: String) -> Response<Option<String>> {
        with_root(state, &path, |root, relative| root.delete_entry(relative))
    }

    #[tauri::command]
    fn list_trash(state: State<'_, Current>) -> Response<Vec<TrashEntry>> {
        with_workspace(state, |workspace| {
            let mut entries = Vec::new();
            for (index, root) in workspace.available_roots() {
                entries.extend(root.list_trash()?.into_iter().map(|mut entry| {
                    entry.id = format!("root-{index}/{}", entry.id);
                    entry.path = format!("root-{index}/{}", entry.path);
                    entry
                }));
            }
            Ok(entries)
        })
    }

    #[tauri::command]
    fn restore_entry(state: State<'_, Current>, id: String) -> Response<Option<String>> {
        with_root(state, &id, Workspace::restore_entry)
    }

    #[tauri::command]
    fn read_document(state: State<'_, Current>, path: String) -> Response<Document> {
        with_root(state, &path, |root, relative| root.read(relative))
    }

    #[tauri::command]
    fn save_document(
        state: State<'_, Current>,
        path: String,
        base: String,
        text: String,
    ) -> Response<Save> {
        with_root(state, &path, |root, relative| {
            root.save(relative, &base, &text)
        })
    }

    #[tauri::command]
    fn save_source_edits(
        state: State<'_, Current>,
        path: String,
        base: String,
        batch: SourceEdits,
    ) -> Response<Save> {
        with_root(state, &path, |root, relative| {
            root.save_source_edits(relative, &base, &batch)
        })
    }

    #[tauri::command]
    fn pilot_proposal(state: State<'_, Current>, path: String, base: String) -> Response<Proposal> {
        with_root(state, &path, |root, relative| {
            root.pilot_proposal(relative, &base)
        })
    }

    #[tauri::command]
    fn accept_review_hunk(
        state: State<'_, Current>,
        path: String,
        proposal: Proposal,
        hunk_id: String,
    ) -> Response<Acceptance> {
        with_root(state, &path, |root, relative| {
            if proposal.path != relative {
                return Err(io::Error::other("The proposal targets another document."));
            }
            root.accept_proposal(&proposal, &[hunk_id])
        })
    }

    #[tauri::command]
    fn review_history(state: State<'_, Current>, path: String) -> Response<ReviewHistory> {
        with_root(state, &path, Workspace::review_history)
    }

    #[tauri::command]
    fn restore_review_checkpoint(
        state: State<'_, Current>,
        path: String,
        base: String,
        id: u64,
    ) -> Response<(Save, Option<String>)> {
        with_root(state, &path, |root, relative| {
            root.restore_checkpoint_with_text(relative, &base, id)
        })
    }

    #[tauri::command]
    fn delete_review_checkpoint(state: State<'_, Current>, path: String, id: u64) -> Response<()> {
        with_root(state, &path, |root, relative| {
            root.delete_checkpoint(relative, id)
        })
    }

    #[tauri::command]
    fn review_preimage(state: State<'_, Current>, path: String, id: u64) -> Response<String> {
        with_root(state, &path, |root, relative| {
            root.review_preimage(relative, id)
        })
    }

    #[tauri::command]
    fn keep_disk_after_review_conflict(
        state: State<'_, Current>,
        path: String,
        id: u64,
        base: String,
    ) -> Response<()> {
        with_root(state, &path, |root, relative| {
            root.keep_disk_after_review_conflict(relative, id, Some(&base))
        })
    }

    #[tauri::command]
    async fn inspect_table_cell(source: String, address: TableCellAddress) -> Response<TableCell> {
        tauri::async_runtime::spawn_blocking(move || table_cell::inspect(&source, &address))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    async fn edit_table_cell(
        source: String,
        address: TableCellAddress,
        replacement: String,
    ) -> Response<TableCell> {
        tauri::async_runtime::spawn_blocking(move || {
            table_cell::edit(&source, &address, &replacement)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
    }

    #[tauri::command]
    async fn read_preview_image(
        app: AppHandle,
        path: String,
        reference: String,
    ) -> Response<PreviewImage> {
        tauri::async_runtime::spawn_blocking(move || {
            with_root(app.state(), &path, |root, document| {
                root.read_preview_image(document, &reference)
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    fn recovery_draft(state: State<'_, Current>, path: String) -> Response<Option<String>> {
        with_root(state, &path, |root, relative| root.recovery_draft(relative))
    }

    #[tauri::command]
    fn discard_recovery(state: State<'_, Current>, path: String, draft: String) -> Response<()> {
        with_root(state, &path, |root, relative| {
            root.discard_recovery(relative, &draft)
        })
    }

    #[tauri::command]
    fn close_offish(window: WebviewWindow) -> Response<()> {
        window.destroy().map_err(|error| error.to_string())
    }

    fn tab_cycle_direction(key: gdk::keys::Key, modifiers: gdk::ModifierType) -> Option<bool> {
        use gdk::{
            ModifierType as Modifiers,
            keys::constants::{ISO_Left_Tab as ISO_LEFT_TAB, KP_Tab as KP_TAB, Tab as TAB},
        };

        let control = Modifiers::CONTROL_MASK;
        let shift = Modifiers::SHIFT_MASK;
        if modifiers != control && modifiers != control | shift {
            return None;
        }
        match key {
            ISO_LEFT_TAB => Some(true),
            TAB | KP_TAB => Some(modifiers.contains(shift)),
            _ => None,
        }
    }

    fn register_tab_shortcuts(window: WebviewWindow) -> tauri::Result<()> {
        let gtk_window = window.gtk_window()?;
        gtk_window.connect_key_press_event(move |_, event| {
            let modifiers = event.state() & gtk::accelerator_get_default_mod_mask();
            let Some(backward) = tab_cycle_direction(event.keyval(), modifiers) else {
                return Propagation::Proceed;
            };
            // Consume the shortcut before GTK can use it to move focus.
            let _ = window.emit_to(window.label(), "offish-cycle-tab", backward);
            Propagation::Stop
        });
        Ok(())
    }

    pub fn run() {
        tauri::Builder::default()
            .setup(|app| {
                let config = app.path().app_config_dir()?;
                let data = app.path().app_local_data_dir()?;
                let mut current = Runtime {
                    catalog: CatalogStore::new(config.clone()),
                    recovery: data.join("recovery"),
                    chat: data.join("chat"),
                    conversation: Conversation::default(),
                    accounts: AccountStore::new(data.join("siwc")),
                    active: None,
                    warning: None,
                    search: Arc::default(),
                    private_paths: vec![config, data],
                };
                if let Some(path) = std::env::args_os().nth(1) {
                    match current.remember(vec![PathBuf::from(path)]) {
                        Ok(record) => {
                            current.activate(record);
                        }
                        Err(error) => {
                            current.warning = Some(format!("Could not open folder: {error}"))
                        }
                    }
                } else if let Ok(catalog) = current.catalog.load() {
                    if let Some(record) = catalog.recent().cloned() {
                        current.activate(record);
                    }
                }
                app.manage(Mutex::new(current));
                if let Some(window) = app.get_webview_window("main") {
                    register_tab_shortcuts(window)?;
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
                workspace_state,
                create_workspace,
                add_workspace_folders,
                open_saved_workspace,
                rename_workspace,
                close_workspace,
                conversation_history,
                clear_conversation,
                prepare_agent_turn,
                approve_agent_turn,
                poll_agent_turn,
                cancel_agent_turn,
                chatgpt_connected,
                connect_chatgpt,
                disconnect_chatgpt,
                list_directory,
                search_workspace,
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
                save_source_edits,
                pilot_proposal,
                accept_review_hunk,
                review_history,
                restore_review_checkpoint,
                delete_review_checkpoint,
                review_preimage,
                keep_disk_after_review_conflict,
                inspect_table_cell,
                edit_table_cell,
                read_preview_image,
                recovery_draft,
                discard_recovery,
                close_offish
            ])
            .run(tauri::generate_context!())
            .expect("run Offish");
    }

    #[cfg(test)]
    mod tests {
        use super::tab_cycle_direction;
        use gtk::gdk::{
            ModifierType as Modifiers,
            keys::constants::{ISO_Left_Tab, Tab, w},
        };

        #[test]
        fn embedded_editor_matches_the_frontend_bundle() {
            let context: tauri::Context<tauri::Wry> = tauri::generate_context!();
            let editor = context
                .assets()
                .get(&"editor.js".into())
                .expect("the embedded editor must be readable");
            assert!(editor.as_ref() == include_bytes!("../web/editor.js"));
        }

        #[test]
        fn control_tab_recognizes_both_native_shifted_forms() {
            let control = Modifiers::CONTROL_MASK;
            let shift = Modifiers::SHIFT_MASK;
            assert_eq!(tab_cycle_direction(Tab, control), Some(false));
            assert_eq!(tab_cycle_direction(Tab, control | shift), Some(true));
            assert_eq!(
                tab_cycle_direction(ISO_Left_Tab, control | shift),
                Some(true)
            );
            assert_eq!(tab_cycle_direction(ISO_Left_Tab, control), Some(true));
        }

        #[test]
        fn ordinary_focus_and_other_shortcuts_pass_through() {
            let control = Modifiers::CONTROL_MASK;
            let shift = Modifiers::SHIFT_MASK;
            let alt = Modifiers::MOD1_MASK;
            assert_eq!(tab_cycle_direction(Tab, Modifiers::empty()), None);
            assert_eq!(tab_cycle_direction(ISO_Left_Tab, shift), None);
            assert_eq!(tab_cycle_direction(Tab, alt), None);
            assert_eq!(tab_cycle_direction(Tab, control | alt), None);
            assert_eq!(tab_cycle_direction(w, control), None);
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    linux::run();
    #[cfg(not(target_os = "linux"))]
    eprintln!("The Phase 1 desktop build currently supports Linux.");
}
