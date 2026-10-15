//! `NoteIndex` against the real MEI Verovio 6.3.0 produces from the site's
//! score (site/content/piano/op57.mxl), loaded with the page's
//! `xmlIdSeed: 1` so the note ids match what the browser sees.
//!
//! Regenerate the fixture (e.g. after a Verovio upgrade) by loading the score
//! the same way and saving `toolkit.getMEI()`.

use hello_core::NoteIndex;

const OP57: &str = include_str!("fixtures/op57.mei");

fn describe(id: &str) -> String {
    NoteIndex::from_mei(OP57)
        .expect("fixture is valid MEI")
        .describe_note(id)
        .expect("fixture has this note")
}

#[test]
fn opening_c_in_the_right_hand() {
    assert_eq!(
        describe("u1bb0tuc"),
        "C4 — quarter note on the treble staff (staff 1), measure 1 (the opening upbeat), \
         at the start of the measure. Key signature: 4 flats (A♭ major or F minor). \
         Time signature: 12/8. MIDI note 60, 261.6 Hz."
    );
}

#[test]
fn flat_from_the_key_signature_is_part_of_the_pitch() {
    // Verovio records this flat only as accid.ges (it's in the key
    // signature, not printed), which its own getElementAttr() drops.
    assert_eq!(
        describe("xfggj11"),
        "A♭3 — sixteenth note on the treble staff (staff 1), measure 1 (the opening upbeat), \
         2½ eighths into the measure. Key signature: 4 flats (A♭ major or F minor). \
         Time signature: 12/8. MIDI note 56, 207.7 Hz."
    );
}

#[test]
fn left_hand_notes_are_on_the_bass_staff() {
    let note = describe("a3ie8ud");
    assert!(
        note.starts_with("D♭2 — quarter note on the bass staff (staff 2)"),
        "{note}"
    );
    assert!(note.contains("MIDI note 37"), "{note}");

    assert!(describe("d1eyoooy").contains("2 eighths into the measure"));
}
