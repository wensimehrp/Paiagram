use std::sync::LazyLock;

use ib_matcher::matcher::{IbMatcher, PinyinMatchConfig, PlainMatchConfig, RomajiMatchConfig};
use ib_matcher::pinyin::PinyinNotation;

static PINYIN_MATCH_DATA: LazyLock<PinyinMatchConfig> = LazyLock::new(|| {
    PinyinMatchConfig::builder(
        PinyinNotation::Ascii
            | PinyinNotation::AsciiFirstLetter
            | PinyinNotation::DiletterMicrosoft,
    )
    .build()
});

static ROMAJI_MATCH_DATA: LazyLock<RomajiMatchConfig> =
    LazyLock::new(|| RomajiMatchConfig::builder().build());

/// Builds a very permissive matcher
pub(crate) fn build_matcher(query: &str) -> IbMatcher<'_> {
    IbMatcher::builder(query)
        .plain(Some(
            PlainMatchConfig::builder().case_insensitive(true).build(),
        ))
        .pinyin(PINYIN_MATCH_DATA.shallow_clone())
        .romaji(ROMAJI_MATCH_DATA.shallow_clone())
        .maybe_mix_lang(Some(true))
        .analyze(true)
        .build()
}

pub(crate) fn search<'a>(
    query: &str,
    candidates: impl Iterator<Item = &'a str>,
    limit: usize,
) -> impl Iterator<Item = usize> {
    let matcher = build_matcher(query);
    candidates
        .enumerate()
        .filter_map(move |(idx, cand)| matcher.is_match(cand).then_some(idx))
        .take(limit)
}
