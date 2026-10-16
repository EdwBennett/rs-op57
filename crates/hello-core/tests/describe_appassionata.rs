//! `NoteIndex` against the real MEI Verovio 6.3.0 produces from the site's
//! score (site/content/piano/appassionata.mxl: the opening upbeat and four
//! more measures), loaded with the page's `xmlIdSeed: 1` so the note ids
//! match what the browser sees.
//!
//! Regenerate the fixture (e.g. after a Verovio upgrade) by loading the score
//! the same way and saving `toolkit.getMEI()`.

use hello_core::NoteIndex;

const APPASSIONATA: &str = include_str!("fixtures/appassionata.mei");

fn describe(id: &str) -> String {
    NoteIndex::from_mei(APPASSIONATA)
        .expect("fixture is valid MEI")
        .describe_note(id)
        .expect("fixture has this note")
}

#[test]
fn upbeat_flat_from_the_key_signature() {
    assert_eq!(
        describe("c1kyp1ws"),
        "A♭3 — sixteenth note on the treble staff (staff 1), measure 1 (the opening upbeat), \
         2½ eighths into the measure. Key signature: 4 flats (A♭ major or F minor). \
         Time signature: 12/8. MIDI note 56, 207.7 Hz."
    );
}

#[test]
fn upbeat_left_hand_is_on_the_bass_staff() {
    let note = describe("f12ecyql");
    assert!(
        note.starts_with("C2 — quarter note on the bass staff (staff 2), measure 1"),
        "{note}"
    );
    assert!(note.contains("MIDI note 36"), "{note}");
}

#[test]
fn later_measures_are_numbered_and_timed() {
    assert_eq!(
        describe("z16wv72r"),
        "F3 — dotted quarter note on the treble staff (staff 1), measure 2, \
         6 eighths into the measure. Key signature: 4 flats (A♭ major or F minor). \
         Time signature: 12/8. MIDI note 53, 174.6 Hz."
    );
    assert!(describe("m13k1pa7").contains("measure 5, 11½ eighths into the measure"));
}

#[test]
fn chord_notes_keep_their_own_pitch() {
    // A printed natural cancels the key signature's E♭.
    let note = describe("csdt8x1");
    assert!(
        note.starts_with(
            "E3 — dotted half note in a 3-note chord on the bass staff (staff 2), measure 4"
        ),
        "{note}"
    );
    assert!(describe("lke0vy6").starts_with("B♭3 — dotted half note in a 3-note chord"));
}
