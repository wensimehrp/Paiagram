use rayon;
use rfd::AsyncFileDialog;

fn read_file<F>(dialog: AsyncFileDialog, cb: impl FnOnce(Vec<u8>) + Send + Sync + 'static) {
    rayon::spawn(|| {});
}
