use bevy::tasks::futures_lite::future;
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};
use rfd::AsyncFileDialog;

pub struct ReadPlugin;
impl Plugin for ReadPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(dialog).add_systems(Update, poll);
    }
}

type CallbackFn = fn(&mut Commands, Vec<u8>);

#[derive(Component)]
struct SelectedFile(Task<Option<Vec<u8>>>, CallbackFn);

#[derive(Event)]
pub struct ReadFile {
    pub title: String,
    pub extensions: Vec<(String, Vec<String>)>,
    pub callback: CallbackFn,
}

fn dialog(trigger: On<ReadFile>, mut commands: Commands) {
    let thread_pool = AsyncComputeTaskPool::get();
    let callback = trigger.callback;
    let title = trigger.title.clone();
    let extensions = trigger.extensions.clone();
    let task = thread_pool.spawn(async move {
        let mut a = AsyncFileDialog::new().set_title(title);
        for (name, exts) in extensions.iter() {
            let ext_refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
            a = a.add_filter(name, &ext_refs);
        }
        match a.pick_file().await {
            None => None,
            Some(it) => Some(it.read().await),
        }
    });
    commands.spawn(SelectedFile(task, callback));
}

fn poll(mut commands: Commands, mut tasks: Query<(Entity, &mut SelectedFile)>) {
    for (entity, mut selected_file) in tasks.iter_mut() {
        if let Some(result) = future::block_on(future::poll_once(&mut selected_file.0)) {
            if let Some(res) = result {
                selected_file.1(&mut commands, res);
            }
            commands.entity(entity).despawn();
        }
    }
}
