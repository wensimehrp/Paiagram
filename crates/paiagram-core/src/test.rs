// SPDX-License-Identifier: MPL-2.0

use std::io::Cursor;

use paiagram_rw::ExportObject;

use crate::import::{ImportType, make_world};
use crate::{SaveFile, Source, World};

const SAMPLE_OUD2: &[u8] = include_bytes!("../../paiagram-oudia/test/sample.oud2");

fn sample_world() -> World {
    make_world(SAMPLE_OUD2, ImportType::OuDiaSecond).expect("the sample should import")
}

#[test]
fn save_file_round_trips_a_world() {
    let world = sample_world();
    assert!(!world.trips.is_empty());

    // Write the save file into memory, exactly as `write_to_file` does before the file dialog.
    let mut cursor = Cursor::new(Vec::new());
    SaveFile::from(world.clone())
        .write_content(&mut cursor)
        .expect("writing the save file should succeed");
    let bytes = cursor.into_inner();

    assert!(bytes.starts_with(b"PAIA"));

    let save_file =
        SaveFile::from_reader(bytes.as_slice()).expect("reading the save file should succeed");
    let source = Source::try_from(save_file).expect("a save file should become a source");
    assert_eq!(source.world, world);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn save_file_writes_and_reads_a_real_file() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let world = sample_world();
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!(
        "paiagram-save-test-{}-{unique}.paia",
        std::process::id()
    ));

    {
        let mut file = std::fs::File::create(&path).expect("the test file should be created");
        SaveFile::from(world.clone())
            .write_content(&mut file)
            .expect("writing the save file should succeed");
    }

    let file = std::fs::File::open(&path).expect("the test file should be readable");
    let save_file = SaveFile::from_reader(file).expect("reading the save file should succeed");
    let source = Source::try_from(save_file).expect("a save file should become a source");
    assert_eq!(source.world, world);

    std::fs::remove_file(path).expect("the test file should be removed");
}

#[test]
fn save_file_rejects_a_file_without_magic_bytes() {
    assert!(SaveFile::from_reader(Cursor::new(b"NOPE".as_slice())).is_err());
    assert!(SaveFile::from_reader(Cursor::new(b"".as_slice())).is_err());
}
