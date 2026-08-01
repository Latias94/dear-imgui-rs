use super::super::NodeEditor;

impl Drop for NodeEditor<'_> {
    fn drop(&mut self) {
        self.finish_native();
    }
}
