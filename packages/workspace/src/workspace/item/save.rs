impl Workspace {
    //
    pub(crate) fn save_all(&mut self, action: &SaveAll, window: &mut Window, cx: &mut Context<Self>) {
        self.save_all_internal(
            action.save_intent.unwrap_or(SaveIntent::SaveAll),
            true,
            window,
            cx,
        )
        .detach_and_log_err(cx);
    }
    // pub fn prompt_to_save_or_discard_dirty_items
    /// Prompts the user to save or discard each dirty item, returning
    /// `true` if they confirmed (saved/discarded everything) or `false`
    /// if they cancelled. Used before removing worktree roots during
    /// thread archival.
    pub fn prompt_to_save_or_discard_dirty_items(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<bool>> {
        self.save_all_internal(SaveIntent::Close, true, window, cx)
    }
    // fn save_all_internal
    fn save_all_internal(
        &mut self,
        mut save_intent: SaveIntent,
        allow_hot_exit_serialization: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<bool>> {
        if self.project.read(cx).is_disconnected(cx) {
            return Task::ready(Ok(true));
        }
        let dirty_items = self
            .panes
            .iter()
            .flat_map(|pane| {
                pane.read(cx).items().filter_map(|item| {
                    if item.is_dirty(cx) {
                        item.tab_content_text(0, cx);
                        Some((pane.clone(), item.boxed_clone()))
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<_>>();

        let project = self.project.clone();
        cx.spawn_in(window, async move |workspace, cx| {
            let dirty_items = if save_intent == SaveIntent::Close && !dirty_items.is_empty() {
                let mut serialize_tasks = Vec::new();
                let mut remaining_dirty_items = Vec::new();
                if allow_hot_exit_serialization {
                    workspace.update(cx, |workspace, cx| {
                        for (pane, item) in dirty_items {
                            if let Some(task) = item
                                .to_serializable_item_handle(cx)
                                .and_then(|handle| handle.serialize(workspace, true, cx))
                            {
                                serialize_tasks.push((pane, item, task));
                            } else {
                                remaining_dirty_items.push((pane, item));
                            }
                        }
                    })?;

                    for (pane, item, task) in serialize_tasks {
                        if task.await.log_err().is_none() {
                            remaining_dirty_items.push((pane, item));
                        }
                    }
                } else {
                    remaining_dirty_items = dirty_items;
                }

                if !remaining_dirty_items.is_empty() {
                    workspace.update(cx, |_, cx| cx.emit(Event::Activate))?;
                }

                if remaining_dirty_items.len() > 1 {
                    let answer = workspace.update_in(cx, |_, window, cx| {
                        cx.emit(Event::Activate);
                        let detail = Pane::file_names_for_prompt(
                            &mut remaining_dirty_items.iter().map(|(_, handle)| handle),
                            cx,
                        );
                        window.prompt(
                            PromptLevel::Warning,
                            "Do you want to save all changes in the following files?",
                            Some(&detail),
                            &["Save all", "Discard all", "Cancel"],
                            cx,
                        )
                    })?;
                    match answer.await.log_err() {
                        Some(0) => save_intent = SaveIntent::SaveAll,
                        Some(1) => save_intent = SaveIntent::Skip,
                        Some(2) => return Ok(false),
                        _ => {}
                    }
                }

                remaining_dirty_items
            } else {
                dirty_items
            };

            for (pane, item) in dirty_items {
                let (singleton, project_entry_ids) = cx.update(|_, cx| {
                    (
                        item.buffer_kind(cx) == ItemBufferKind::Singleton,
                        item.project_entry_ids(cx),
                    )
                })?;
                if (singleton || !project_entry_ids.is_empty())
                    && !Pane::save_item(project.clone(), pane, &*item, save_intent, cx).await?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        })
    }
    // pub fn save_active_item
    pub fn save_active_item(
        &mut self,
        save_intent: SaveIntent,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let project = self.project.clone();
        let pane = self.active_pane().clone();
        let item = pane.read(cx).active_item();

        window.spawn(cx, async move |cx| {
            if let Some(item) = item {
                Pane::save_item(project, pane, item.as_ref(), save_intent, cx)
                    .await
                    .map(|_| ())
            } else {
                Ok(())
            }
        })
    }
    // fn flush_deferred_saves
    fn flush_deferred_saves(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let deferred = std::mem::take(&mut self.deferred_save_items);
        for weak_item in deferred {
            let Some(item) = weak_item.upgrade() else {
                continue;
            };
            // Skip if focus returned to this item
            let focus_handle = item.item_focus_handle(cx);
            if focus_handle.contains_focused(window, cx) {
                continue;
            }
            Pane::autosave_item(item.as_ref(), self.project.clone(), window, cx)
                .detach_and_log_err(cx);
        }
    }
}
