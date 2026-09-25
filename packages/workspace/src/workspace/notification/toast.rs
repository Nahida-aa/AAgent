#[derive(Clone)]
pub struct Toast {
    id: NotificationId,
    message: Cow<'static, str>,
    autohide: bool,
    on_click: Option<(Cow<'static, str>, Arc<dyn Fn(&mut Window, &mut App)>)>,
}

impl Toast {
    pub fn new<I: Into<Cow<'static, str>>>(id: NotificationId, message: I) -> Self {
        Toast {
            id,
            message: message.into(),
            on_click: None,
            autohide: false,
        }
    }

    pub fn on_click<F, M>(mut self, message: M, on_click: F) -> Self
    where
        M: Into<Cow<'static, str>>,
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_click = Some((message.into(), Arc::new(on_click)));
        self
    }

    pub fn autohide(mut self) -> Self {
        self.autohide = true;
        self
    }
}

impl PartialEq for Toast {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.message == other.message
            && self.on_click.is_some() == other.on_click.is_some()
    }
}

impl Workspace {
    //
    pub fn toggle_status_toast<V: ToastView>(&mut self, entity: Entity<V>, cx: &mut App) {
        self.toast_layer
            .update(cx, |toast_layer, cx| toast_layer.toggle_toast(cx, entity))
    }
}
