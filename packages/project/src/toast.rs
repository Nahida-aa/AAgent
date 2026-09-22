use gpui::SharedString;

#[derive(PartialEq, Debug, Clone)]
pub struct ToastLink {
    pub label: &'static str,
    pub url: &'static str,
}
