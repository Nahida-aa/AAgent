impl Workspace {
    // │   ├── active_call
    // │   └── active_global_call
    pub fn active_call(&self) -> Option<&dyn AnyActiveCall> {
        self.active_call.as_ref().map(|(call, _)| &*call.0)
    }

    pub fn active_global_call(&self) -> Option<GlobalAnyActiveCall> {
        self.active_call.as_ref().map(|(call, _)| call.clone())
    }

}
