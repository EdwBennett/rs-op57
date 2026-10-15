//! Plain-English descriptions of the notes in an MEI score — the encoding
//! Verovio converts MusicXML into (`toolkit.getMEI()`) — for the Zola site's
//! piano page (via WASM).
//!
//! Each MEI `<note>`'s `xml:id` is also the `id` of its `<g class="note">` in
//! Verovio's SVG, so a clicked note in the rendered score can be described
//! from the musical data rather than from its drawing.

use std::collections::HashMap;
use std::fmt;

use roxmltree::{Document, Node, NodeId};

const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// Every note in an MEI document, looked up by its `xml:id`.
#[derive(Debug, Clone)]
pub struct NoteIndex {
    notes: HashMap<String, NoteFacts>,
}

/// The MEI text couldn't be parsed as XML.
#[derive(Debug)]
pub struct MeiError(roxmltree::Error);

impl fmt::Display for MeiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid MEI: {}", self.0)
    }
}

impl std::error::Error for MeiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl NoteIndex {
    /// Reads every `<note>` in `mei`, along with the context it's in: its
    /// staff's clef, key and time signature, its measure, and where in the
    /// measure it starts.
    ///
    /// Clef, key and time signature changes are followed in document order,
    /// so a change part-way through a measure in one layer is taken to apply
    /// to later layers of that staff from the start of the measure.
    pub fn from_mei(mei: &str) -> Result<Self, MeiError> {
        let doc = Document::parse(mei).map_err(MeiError)?;
        let mut ctx = Context::default();
        let mut notes = HashMap::new();

        for node in doc.descendants().filter(Node::is_element) {
            match node.tag_name().name() {
                "scoreDef" => ctx.score_def(node),
                "staffDef" => ctx.staff_def(node),
                "keySig" => ctx.key_sig(node),
                "meterSig" => ctx.meter_sig(node),
                "clef" => ctx.clef(node),
                "measure" => ctx.measure(node),
                "layer" => ctx.offset = Some(0),
                "chord" => {
                    let onset = ctx.advance(node);
                    ctx.chord_onset = Some((node.id(), onset));
                }
                "rest" | "space" | "mRest" | "mSpace" => {
                    ctx.advance(node);
                }
                "note" => {
                    if let Some(id) = node.attribute((XML_NS, "id")) {
                        notes.insert(id.to_owned(), ctx.note(node));
                    }
                }
                _ => {}
            }
        }

        Ok(Self { notes })
    }

    /// Describes the note with this `xml:id`, or `None` if there isn't one.
    pub fn describe_note(&self, id: &str) -> Option<String> {
        self.notes.get(id).map(NoteFacts::describe)
    }
}

/// What's known about one note, gathered while walking the MEI.
#[derive(Debug, Clone)]
struct NoteFacts {
    pitch: Option<Pitch>,
    duration: Option<Duration>,
    grace: bool,
    chord_size: Option<usize>,
    staff: Option<u32>,
    clef: Option<Clef>,
    measure: Option<MeasureFacts>,
    /// Where the note starts, in pulses (`dur.ppq` units) from the start of
    /// its measure, and how many pulses make a quarter note.
    onset: Option<(u32, u32)>,
    key: Option<KeySig>,
    meter: Option<Meter>,
}

impl NoteFacts {
    fn describe(&self) -> String {
        let mut text = match &self.pitch {
            Some(pitch) => pitch.to_string(),
            None => "Unpitched note".to_owned(),
        };

        let mut kind = match &self.duration {
            Some(duration) => format!("{duration} note"),
            None => "note".to_owned(),
        };
        if self.grace {
            kind = format!("grace {kind}");
        }
        text += &format!(" — {kind}");
        if let Some(size) = self.chord_size {
            text += &format!(" in a {size}-note chord");
        }

        if let Some(staff) = self.staff {
            match self.clef.as_ref().and_then(Clef::name) {
                Some(clef) => text += &format!(" on the {clef} staff (staff {staff})"),
                None => text += &format!(" on staff {staff}"),
            }
        }

        if let Some(measure) = &self.measure {
            text += &format!(", {measure}");
        }

        if let Some((pulses, ppq)) = self.onset {
            let unit = self.meter.as_ref().map_or(4, |meter| meter.unit);
            text += &format!(", {}", position(pulses, ppq, unit));
        }
        text.push('.');

        if let Some(key) = &self.key {
            text += &format!(" Key signature: {key}.");
        }
        if let Some(meter) = &self.meter {
            text += &format!(" Time signature: {meter}.");
        }
        if let Some(midi) = self.pitch.as_ref().and_then(Pitch::midi) {
            let hz = 440.0 * 2f64.powf(f64::from(midi - 69) / 12.0);
            text += &format!(" MIDI note {midi}, {hz:.1} Hz.");
        }

        text
    }
}

/// The running state of a walk through an MEI document.
#[derive(Debug, Default)]
struct Context {
    score_key: Option<KeySig>,
    score_meter: Option<Meter>,
    staff_keys: HashMap<u32, KeySig>,
    staff_meters: HashMap<u32, Meter>,
    clefs: HashMap<u32, Clef>,
    ppq: HashMap<u32, u32>,
    measures_seen: usize,
    measure: Option<(NodeId, MeasureFacts)>,
    /// Pulses since the start of the current layer's measure, or `None`
    /// once an event without a `dur.ppq` has made it unknowable.
    offset: Option<u32>,
    chord_onset: Option<(NodeId, Option<u32>)>,
}

impl Context {
    fn score_def(&mut self, node: Node) {
        if let Some(key) = KeySig::from_attrs(node, "key.sig", "key.mode") {
            self.set_score_key(key);
        }
        if let Some(meter) = Meter::from_attrs(node, "meter.count", "meter.unit", "meter.sym") {
            self.set_score_meter(meter);
        }
    }

    fn staff_def(&mut self, node: Node) {
        let Some(n) = attr_parse::<u32>(node, "n") else {
            return;
        };
        if let Some(ppq) = attr_parse(node, "ppq") {
            self.ppq.insert(n, ppq);
        }
        if let Some(key) = KeySig::from_attrs(node, "key.sig", "key.mode") {
            self.staff_keys.insert(n, key);
        }
        if let Some(meter) = Meter::from_attrs(node, "meter.count", "meter.unit", "meter.sym") {
            self.staff_meters.insert(n, meter);
        }
        if let Some(clef) = Clef::from_attrs(node, "clef.shape", "clef.line") {
            self.clefs.insert(n, clef);
        }
    }

    fn key_sig(&mut self, node: Node) {
        let Some(key) = KeySig::from_attrs(node, "sig", "mode") else {
            return;
        };
        match parent_def(node) {
            Some(Def::Score) => self.set_score_key(key),
            Some(Def::Staff(n)) => {
                self.staff_keys.insert(n, key);
            }
            None => {}
        }
    }

    fn meter_sig(&mut self, node: Node) {
        let Some(meter) = Meter::from_attrs(node, "count", "unit", "sym") else {
            return;
        };
        match parent_def(node) {
            Some(Def::Score) => self.set_score_meter(meter),
            Some(Def::Staff(n)) => {
                self.staff_meters.insert(n, meter);
            }
            None => {}
        }
    }

    /// A clef in a staff definition, or a clef change within a staff's music.
    fn clef(&mut self, node: Node) {
        let Some(clef) = Clef::from_attrs(node, "shape", "line") else {
            return;
        };
        let staff = match parent_def(node) {
            Some(Def::Staff(n)) => Some(n),
            Some(Def::Score) => None,
            None => staff_number(node),
        };
        if let Some(n) = staff {
            self.clefs.insert(n, clef);
        }
    }

    fn measure(&mut self, node: Node) {
        self.measures_seen += 1;
        let facts = MeasureFacts {
            n: node.attribute("n").map(str::to_owned),
            incomplete: node.attribute("metcon") == Some("false"),
            first: self.measures_seen == 1,
        };
        self.measure = Some((node.id(), facts));
    }

    /// Moves the layer's position past a timed event, returning where it
    /// started. Grace notes take no time, so don't move it.
    fn advance(&mut self, node: Node) -> Option<u32> {
        let onset = self.offset;
        if node.attribute("grace").is_none() {
            self.offset = onset
                .zip(attr_parse::<u32>(node, "dur.ppq"))
                .map(|(o, d)| o + d);
        }
        onset
    }

    fn note(&mut self, node: Node) -> NoteFacts {
        let chord = node.ancestors().find(|a| a.tag_name().name() == "chord");
        let onset = match chord {
            Some(chord) => self
                .chord_onset
                .filter(|(id, _)| *id == chord.id())
                .and_then(|(_, onset)| onset),
            None => self.advance(node),
        };

        // A note in a chord may leave its duration to the chord.
        let duration = Duration::from_node(node).or_else(|| chord.and_then(Duration::from_node));
        let chord_size = chord.map(|c| {
            c.children()
                .filter(|n| n.tag_name().name() == "note")
                .count()
        });

        let staff = staff_number(node);
        let measure = node
            .ancestors()
            .find(|a| a.tag_name().name() == "measure")
            .and_then(|m| {
                self.measure
                    .as_ref()
                    .filter(|(id, _)| *id == m.id())
                    .map(|(_, facts)| facts.clone())
            });
        let ppq = staff.and_then(|n| self.ppq.get(&n).copied());

        NoteFacts {
            pitch: Pitch::from_node(node),
            duration,
            grace: node.attribute("grace").is_some(),
            chord_size,
            staff,
            clef: staff.and_then(|n| self.clefs.get(&n).cloned()),
            measure,
            onset: onset.zip(ppq),
            key: staff
                .and_then(|n| self.staff_keys.get(&n))
                .or(self.score_key.as_ref())
                .cloned(),
            meter: staff
                .and_then(|n| self.staff_meters.get(&n))
                .or(self.score_meter.as_ref())
                .cloned(),
        }
    }

    /// A key signature for the whole score replaces any per-staff ones.
    fn set_score_key(&mut self, key: KeySig) {
        self.score_key = Some(key);
        self.staff_keys.clear();
    }

    fn set_score_meter(&mut self, meter: Meter) {
        self.score_meter = Some(meter);
        self.staff_meters.clear();
    }
}

enum Def {
    Score,
    Staff(u32),
}

/// Whether `node` sits directly in a `<scoreDef>` or (numbered) `<staffDef>`.
fn parent_def(node: Node) -> Option<Def> {
    let parent = node.parent_element()?;
    match parent.tag_name().name() {
        "scoreDef" => Some(Def::Score),
        "staffDef" => attr_parse(parent, "n").map(Def::Staff),
        _ => None,
    }
}

/// The `n` of the `<staff>` that `node` is in.
fn staff_number(node: Node) -> Option<u32> {
    node.ancestors()
        .find(|a| a.tag_name().name() == "staff")
        .and_then(|s| attr_parse(s, "n"))
}

fn attr_parse<T: std::str::FromStr>(node: Node, name: &str) -> Option<T> {
    node.attribute(name)?.trim().parse().ok()
}

#[derive(Debug, Clone, PartialEq)]
struct Pitch {
    /// Note letter, upper-case.
    step: char,
    accidental: Option<Accidental>,
    octave: i32,
}

impl Pitch {
    /// The sounding pitch: a written accidental (`accid`) if there is one,
    /// otherwise one implied by the key signature or earlier in the measure
    /// (`accid.ges`) — on the note itself or a child `<accid>`.
    fn from_node(node: Node) -> Option<Self> {
        let step = node
            .attribute("pname")?
            .chars()
            .next()?
            .to_ascii_uppercase();
        if !"CDEFGAB".contains(step) {
            return None;
        }
        let octave = attr_parse(node, "oct")?;
        let accid_child = node.children().find(|c| c.tag_name().name() == "accid");
        let accidental = ["accid", "accid.ges"].iter().find_map(|name| {
            node.attribute(*name)
                .or_else(|| accid_child.and_then(|a| a.attribute(*name)))
                .map(Accidental::from_mei)
        });
        Some(Self {
            step,
            accidental,
            octave,
        })
    }

    /// Standard MIDI note number (middle C, C4, is 60), if the accidental is
    /// one with a whole number of semitones.
    fn midi(&self) -> Option<i32> {
        let step = match self.step {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            _ => 11,
        };
        let alter = match &self.accidental {
            Some(Accidental::Semitones(n)) => *n,
            Some(Accidental::Other(_)) => return None,
            None => 0,
        };
        Some((self.octave + 1) * 12 + step + alter)
    }
}

impl fmt::Display for Pitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.accidental {
            Some(Accidental::Other(raw)) => {
                write!(f, "{}{} (accidental “{raw}”)", self.step, self.octave)
            }
            Some(Accidental::Semitones(n)) => {
                let symbol = match n {
                    -3 => "♭𝄫",
                    -2 => "𝄫",
                    -1 => "♭",
                    1 => "♯",
                    2 => "𝄪",
                    3 => "♯𝄪",
                    _ => "",
                };
                write!(f, "{}{symbol}{}", self.step, self.octave)
            }
            None => write!(f, "{}{}", self.step, self.octave),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Accidental {
    /// Sharps positive, flats negative; a natural is 0.
    Semitones(i32),
    /// Microtonal and other accidentals, kept as MEI wrote them.
    Other(String),
}

impl Accidental {
    fn from_mei(value: &str) -> Self {
        match value {
            "n" => Self::Semitones(0),
            "s" => Self::Semitones(1),
            "f" => Self::Semitones(-1),
            "ss" | "x" => Self::Semitones(2),
            "ff" => Self::Semitones(-2),
            "xs" | "sx" | "ts" => Self::Semitones(3),
            "tf" => Self::Semitones(-3),
            other => Self::Other(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Duration {
    /// MEI `dur`: "1" whole, "2" half, "4" quarter, …, or "breve" etc.
    dur: String,
    dots: u8,
}

impl Duration {
    fn from_node(node: Node) -> Option<Self> {
        Some(Self {
            dur: node.attribute("dur")?.to_owned(),
            dots: attr_parse(node, "dots").unwrap_or(0),
        })
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.dots {
            0 => {}
            1 => f.write_str("dotted ")?,
            2 => f.write_str("double-dotted ")?,
            3 => f.write_str("triple-dotted ")?,
            n => write!(f, "{n}-dotted ")?,
        }
        match self.dur.as_str() {
            "maxima" | "long" | "breve" => f.write_str(&self.dur),
            dur => match dur.parse::<u32>() {
                Ok(unit) => f.write_str(&unit_name(unit, false)),
                Err(_) => write!(f, "“{dur}”"),
            },
        }
    }
}

/// The name of the note value 1/`unit`: "quarter", "eighth", …
fn unit_name(unit: u32, plural: bool) -> String {
    let (one, many) = match unit {
        1 => ("whole", "wholes"),
        2 => ("half", "halves"),
        4 => ("quarter", "quarters"),
        8 => ("eighth", "eighths"),
        16 => ("sixteenth", "sixteenths"),
        32 => ("thirty-second", "thirty-seconds"),
        64 => ("sixty-fourth", "sixty-fourths"),
        _ => return format!("1/{unit}{}", if plural { " notes" } else { " note" }),
    };
    (if plural { many } else { one }).to_owned()
}

/// "at the start of the measure", "2½ eighths into the measure", …
fn position(pulses: u32, ppq: u32, unit: u32) -> String {
    if pulses == 0 {
        return "at the start of the measure".to_owned();
    }
    // pulses / ppq quarters, in units of 1/unit: pulses * unit / (4 * ppq).
    let (num, den) = reduce(pulses * unit, 4 * ppq);
    let (whole, rem) = (num / den, num % den);
    let fraction = match (rem, den) {
        (0, _) => String::new(),
        (1, 2) => "½".to_owned(),
        (1, 3) => "⅓".to_owned(),
        (2, 3) => "⅔".to_owned(),
        (1, 4) => "¼".to_owned(),
        (3, 4) => "¾".to_owned(),
        _ if whole == 0 => format!("{rem}/{den}"),
        _ => format!(" {rem}/{den}"),
    };
    let amount = match whole {
        0 => fraction,
        _ => format!("{whole}{fraction}"),
    };
    let plural = num > den;
    format!("{amount} {} into the measure", unit_name(unit, plural))
}

fn reduce(num: u32, den: u32) -> (u32, u32) {
    let (mut a, mut b) = (num, den);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    (num / a, den / a)
}

#[derive(Debug, Clone, PartialEq)]
struct Clef {
    shape: String,
    line: Option<u32>,
}

impl Clef {
    fn from_attrs(node: Node, shape: &str, line: &str) -> Option<Self> {
        Some(Self {
            shape: node.attribute(shape)?.to_owned(),
            line: attr_parse(node, line),
        })
    }

    /// "treble", "bass", … for the common clefs.
    fn name(&self) -> Option<&'static str> {
        match (self.shape.as_str(), self.line) {
            ("G", Some(2)) => Some("treble"),
            ("F", Some(4)) => Some("bass"),
            ("C", Some(3)) => Some("alto"),
            ("C", Some(4)) => Some("tenor"),
            ("perc", _) => Some("percussion"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct KeySig {
    /// Sharps positive, flats negative.
    fifths: i32,
    /// "major", "minor", … if the MEI says.
    mode: Option<String>,
}

impl KeySig {
    /// From MEI's "0", "3s", "4f" form (not "mixed", which has no single
    /// count of sharps or flats).
    fn from_attrs(node: Node, sig: &str, mode: &str) -> Option<Self> {
        let sig = node.attribute(sig)?;
        let fifths = if sig == "0" {
            0
        } else {
            let (count, kind) = sig.split_at(sig.len().checked_sub(1)?);
            let count: i32 = count.parse().ok()?;
            match kind {
                "s" => count,
                "f" => -count,
                _ => return None,
            }
        };
        Some(Self {
            fifths,
            mode: node.attribute(mode).map(str::to_owned),
        })
    }
}

impl fmt::Display for KeySig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const MAJOR: [&str; 15] = [
            "C♭", "G♭", "D♭", "A♭", "E♭", "B♭", "F", "C", "G", "D", "A", "E", "B", "F♯", "C♯",
        ];
        const MINOR: [&str; 15] = [
            "A♭", "E♭", "B♭", "F", "C", "G", "D", "A", "E", "B", "F♯", "C♯", "G♯", "D♯", "A♯",
        ];

        let count = self.fifths.unsigned_abs();
        match self.fifths {
            0 => f.write_str("no sharps or flats")?,
            1 => f.write_str("1 sharp")?,
            -1 => f.write_str("1 flat")?,
            n if n > 0 => write!(f, "{count} sharps")?,
            _ => write!(f, "{count} flats")?,
        }

        let Some(i) = usize::try_from(self.fifths + 7).ok().filter(|i| *i < 15) else {
            return Ok(());
        };
        match self.mode.as_deref() {
            Some("major") => write!(f, " ({} major)", MAJOR[i]),
            Some("minor") => write!(f, " ({} minor)", MINOR[i]),
            _ => write!(f, " ({} major or {} minor)", MAJOR[i], MINOR[i]),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Meter {
    /// May be additive, e.g. "3+2".
    count: String,
    unit: u32,
    /// "common" or "cut", if drawn as a symbol.
    sym: Option<String>,
}

impl Meter {
    fn from_attrs(node: Node, count: &str, unit: &str, sym: &str) -> Option<Self> {
        Some(Self {
            count: node.attribute(count)?.to_owned(),
            unit: attr_parse(node, unit)?,
            sym: node.attribute(sym).map(str::to_owned),
        })
    }
}

impl fmt::Display for Meter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.count, self.unit)?;
        match self.sym.as_deref() {
            Some("common") => f.write_str(" (common time)"),
            Some("cut") => f.write_str(" (cut time)"),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct MeasureFacts {
    n: Option<String>,
    /// MEI `metcon="false"`: fewer beats than the time signature says.
    incomplete: bool,
    first: bool,
}

impl fmt::Display for MeasureFacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.n {
            Some(n) => write!(f, "measure {n}")?,
            None => f.write_str("unnumbered measure")?,
        }
        match (self.incomplete, self.first) {
            (true, true) => f.write_str(" (the opening upbeat)"),
            (true, false) => f.write_str(" (an incomplete measure)"),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small hand-written score covering what op57's opening doesn't:
    /// written accidentals, dots, chords, grace notes, a clef change and a
    /// key-signature change.
    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<mei xmlns="http://www.music-encoding.org/ns/mei">
  <music><body><mdiv><score>
    <scoreDef>
      <keySig sig="2s" mode="major"/>
      <meterSig count="4" unit="4" sym="common"/>
      <staffGrp>
        <staffDef n="1" lines="5" ppq="8"><clef shape="G" line="2"/></staffDef>
      </staffGrp>
    </scoreDef>
    <section>
      <measure n="1">
        <staff n="1"><layer n="1">
          <note xml:id="dotted" pname="f" oct="5" dur="4" dots="1" dur.ppq="12" accid.ges="s"/>
          <note xml:id="grace" pname="b" oct="4" dur="8" grace="unacc"/>
          <note xml:id="natural" pname="c" oct="5" dur="8" dur.ppq="4"><accid accid="n"/></note>
          <chord xml:id="ch" dur="2" dur.ppq="16">
            <note xml:id="ch1" pname="d" oct="4"/>
            <note xml:id="ch2" pname="f" oct="4" accid.ges="s"/>
          </chord>
        </layer></staff>
      </measure>
      <scoreDef><keySig sig="3f"/></scoreDef>
      <measure n="2">
        <staff n="1"><layer n="1">
          <rest dur="8" dur.ppq="4"/>
          <clef shape="F" line="4"/>
          <note xml:id="after" pname="e" oct="3" dur="8" dur.ppq="4" accid="ff"/>
          <note xml:id="micro" pname="e" oct="3" dur="4" dur.ppq="8" accid="1qf"/>
        </layer></staff>
      </measure>
    </section>
  </score></mdiv></body></music>
</mei>"#;

    fn describe(id: &str) -> String {
        NoteIndex::from_mei(SAMPLE)
            .unwrap()
            .describe_note(id)
            .unwrap()
    }

    #[test]
    fn describes_dotted_note_with_key_signature_sharp() {
        assert_eq!(
            describe("dotted"),
            "F♯5 — dotted quarter note on the treble staff (staff 1), measure 1, \
             at the start of the measure. Key signature: 2 sharps (D major). \
             Time signature: 4/4 (common time). MIDI note 78, 740.0 Hz."
        );
    }

    #[test]
    fn grace_notes_take_no_time() {
        let grace = describe("grace");
        assert!(grace.starts_with("B4 — grace eighth note"), "{grace}");
        assert!(grace.contains("1½ quarters into the measure"), "{grace}");
        assert!(describe("natural").contains("1½ quarters into the measure"));
    }

    #[test]
    fn written_natural_overrides_key_signature() {
        assert!(describe("natural").starts_with("C5 — eighth note"));
    }

    #[test]
    fn chord_notes_share_the_chords_duration_and_onset() {
        let note = describe("ch2");
        assert!(
            note.starts_with("F♯4 — half note in a 2-note chord"),
            "{note}"
        );
        assert!(note.contains("2 quarters into the measure"), "{note}");
        assert!(describe("ch1").contains("2 quarters into the measure"));
    }

    #[test]
    fn follows_clef_and_key_changes() {
        let note = describe("after");
        assert!(
            note.starts_with("E𝄫3 — eighth note on the bass staff (staff 1), measure 2, ½ quarter into the measure."),
            "{note}"
        );
        assert!(note.contains("Key signature: 3 flats (E♭ major or C minor)."));
        assert!(note.contains("MIDI note 50"), "{note}");
    }

    #[test]
    fn microtonal_accidental_has_no_midi_number() {
        let note = describe("micro");
        assert!(note.starts_with("E3 (accidental “1qf”)"), "{note}");
        assert!(!note.contains("MIDI"), "{note}");
    }

    #[test]
    fn unknown_id_has_no_description() {
        let index = NoteIndex::from_mei(SAMPLE).unwrap();
        assert_eq!(index.describe_note("nope"), None);
    }

    #[test]
    fn rejects_malformed_xml() {
        assert!(NoteIndex::from_mei("<mei><note>").is_err());
    }

    #[test]
    fn positions_reduce_fractions() {
        assert_eq!(position(5, 4, 8), "2½ eighths into the measure");
        assert_eq!(position(2, 4, 8), "1 eighth into the measure");
        assert_eq!(position(1, 6, 4), "1/6 quarter into the measure");
        assert_eq!(position(7, 6, 4), "1 1/6 quarters into the measure");
    }
}
