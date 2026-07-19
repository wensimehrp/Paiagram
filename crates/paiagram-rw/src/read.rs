use rfd::AsyncFileDialog;

#[derive(Clone)]
pub struct ReadFile {
    pub title: String,
    pub extensions: Vec<(String, Vec<String>)>,
}

impl ReadFile {
    fn make_dialog(self) -> AsyncFileDialog {
        let mut dialog = AsyncFileDialog::new().set_title(self.title);
        for (name, exts) in self.extensions {
            dialog = dialog.add_filter(name, &exts);
        }
        dialog
    }

    async fn pick_file(self) -> Option<Vec<u8>> {
        match self.make_dialog().pick_file().await {
            None => None,
            Some(content) => Some(content.read().await),
        }
    }
}
