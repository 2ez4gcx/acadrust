//! Regression coverage for the optional Rayon integration described in #156.
//!
//! The workload is generated in memory so the tests exercise the writer and
//! reader together without depending on a checked-in drawing fixture.

use std::io::Cursor;
use std::sync::{Arc, Mutex};
#[cfg(not(all(feature = "parallel", not(target_arch = "wasm32"))))]
use std::thread::ThreadId;

use acadrust::entities::{EntityType, Line};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, Vector3};

const LINE_COUNT: usize = 2048;

fn large_line_scene() -> CadDocument {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    for index in 0..LINE_COUNT {
        let index = index as f64;
        document
            .add_entity(EntityType::Line(Line::from_points(
                Vector3::new(index * 0.5, (index % 31.0) * 2.0, index % 7.0),
                Vector3::new(
                    index * 0.5 + 3.25,
                    (index % 31.0) * 2.0 + 1.5,
                    index % 7.0 + 0.75,
                ),
            )))
            .expect("add line");
    }
    document
}

fn encoded_scene() -> Vec<u8> {
    DwgWriter::write_to_vec(&large_line_scene()).expect("write generated DWG")
}

fn assert_roundtrip(bytes: &[u8]) {
    let loaded = DwgReader::from_stream(Cursor::new(bytes.to_vec()))
        .read()
        .expect("read generated DWG");
    let lines: Vec<_> = loaded
        .entities()
        .filter_map(|entity| match entity {
            EntityType::Line(line) => Some(line),
            _ => None,
        })
        .collect();

    assert_eq!(lines.len(), LINE_COUNT);
    for &index in &[0usize, LINE_COUNT / 2, LINE_COUNT - 1] {
        let index_as_f64 = index as f64;
        let line = lines[index];
        assert_eq!(
            line.start,
            Vector3::new(
                index_as_f64 * 0.5,
                (index_as_f64 % 31.0) * 2.0,
                index_as_f64 % 7.0
            )
        );
        assert_eq!(
            line.end,
            Vector3::new(
                index_as_f64 * 0.5 + 3.25,
                (index_as_f64 % 31.0) * 2.0 + 1.5,
                index_as_f64 % 7.0 + 0.75,
            )
        );
    }
}

#[test]
fn large_generated_scene_roundtrips_with_ordered_lines() {
    let bytes = encoded_scene();
    assert!(!bytes.is_empty());
    assert_roundtrip(&bytes);
}

#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
#[test]
fn application_owned_pool_bounds_concurrent_reads() {
    use rayon::prelude::*;

    let bytes = Arc::new(encoded_scene());
    let callback_threads = Arc::new(Mutex::new(Vec::<(Option<usize>, usize)>::new()));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("build bounded application pool");

    let results = pool.install(|| {
        (0..4)
            .into_par_iter()
            .map(|_| {
                let callback_threads = Arc::clone(&callback_threads);
                let bytes = Arc::clone(&bytes);
                let mut reader = DwgReader::from_stream(Cursor::new((*bytes).clone()));
                reader.set_progress_callback(Arc::new(move |_| {
                    callback_threads
                        .lock()
                        .expect("callback mutex")
                        .push((rayon::current_thread_index(), rayon::current_num_threads()));
                }));
                reader.read().map(|document| document.entity_count())
            })
            .collect::<Vec<_>>()
    });

    assert_eq!(results.len(), 4);
    assert!(results
        .iter()
        .all(|result| matches!(result, Ok(count) if *count == LINE_COUNT)));
    let callback_threads = callback_threads.lock().expect("callback mutex");
    assert!(
        !callback_threads.is_empty(),
        "reader should report progress"
    );
    assert!(callback_threads
        .iter()
        .all(|(thread_index, thread_count)| thread_index.is_some() && *thread_count == 2));
}

#[cfg(not(all(feature = "parallel", not(target_arch = "wasm32"))))]
#[test]
fn sequential_reader_reports_progress_on_calling_thread() {
    let expected_thread = std::thread::current().id();
    let callback_threads = Arc::new(Mutex::new(Vec::<ThreadId>::new()));
    let mut reader = DwgReader::from_stream(Cursor::new(encoded_scene()));
    let callback_threads_for_reader = Arc::clone(&callback_threads);
    reader.set_progress_callback(Arc::new(move |_| {
        callback_threads_for_reader
            .lock()
            .expect("callback mutex")
            .push(std::thread::current().id());
    }));

    let loaded = reader.read().expect("read generated DWG");
    assert_eq!(loaded.entity_count(), LINE_COUNT);
    let callback_threads = callback_threads.lock().expect("callback mutex");
    assert!(
        !callback_threads.is_empty(),
        "reader should report progress"
    );
    assert!(callback_threads
        .iter()
        .all(|thread| *thread == expected_thread));
}
