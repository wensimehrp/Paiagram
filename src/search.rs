use crate::settings::ApplicationSettings;
use bevy::ecs::message::MessageId;
use bevy::prelude::*;
use ib_matcher::matcher::{IbMatcher, PinyinMatchConfig, RomajiMatchConfig};
use ib_matcher::pinyin::PinyinNotation;
use rayon::prelude::*;
use std::sync::Arc;

pub struct SearchPlugin;
impl Plugin for SearchPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SearchCommand>()
            .add_message::<SearchResponse>()
            .add_systems(Update, search.run_if(on_message::<SearchCommand>));
    }
}

#[derive(Message)]
pub enum SearchCommand {
    Table {
        data: Arc<Vec<String>>,
        query: String,
    },
}

#[derive(Message)]
pub enum SearchResponse {
    Table(MessageId<SearchCommand>, Vec<usize>),
}

pub fn search(
    mut msg_reader: MessageReader<SearchCommand>,
    mut msg_writer: MessageWriter<SearchResponse>,
    settings: Res<ApplicationSettings>,
) {
    let now = instant::Instant::now();
    for (msg, id) in msg_reader.read_with_id() {
        match msg {
            SearchCommand::Table { data, query } => {
                let result = search_table(data, &build_matcher(query, &settings));
                msg_writer.write(SearchResponse::Table(id, result));
            }
        }
    }
    debug!("Search completed in {:?}", now.elapsed());
}

fn build_matcher(query: &str, settings: &ApplicationSettings) -> IbMatcher<'static> {
    let now = instant::Instant::now();
    let matcher = IbMatcher::builder(query)
        .pinyin(PinyinMatchConfig::notations(
            PinyinNotation::Ascii | PinyinNotation::DiletterMicrosoft,
        ))
        .maybe_romaji(
            settings
                .enable_romaji_search
                .then(RomajiMatchConfig::default),
        )
        .build();
    debug!("Matcher built in {:?}", now.elapsed());
    matcher
}

fn search_table(data: &Arc<Vec<String>>, searcher: &IbMatcher<'static>) -> Vec<usize> {
    data.par_iter()
        .enumerate()
        .filter_map(|(index, row)| {
            if searcher.is_match(row.as_str()) {
                None
            } else {
                Some(index)
            }
        })
        .collect()
}
