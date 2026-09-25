//! Background spell checking with a zspell dictionary.
//!
//! The engine loads a Hunspell-compatible dictionary once, then runs checks on
//! a short-lived worker thread so keystrokes never block on dictionary work.
//! A generation counter drops stale results when the user keeps typing.
//! Ranges are character indices into the full projection, matching the rest
//! of the editor's offset space.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, OnceLock};

use crate::error::DocumentError;

/// Inclusive-exclusive character range of one misspelled word.
pub type Misspelling = (usize, usize);

/// Shared dictionary cache so repeated `App` construction does not reparse
/// the system Hunspell files.
static SYSTEM_DICT: OnceLock<Option<Arc<zspell::Dictionary>>> = OnceLock::new();

/// Spell-check engine: dictionary handle plus in-flight check state.
pub struct ProofEngine {
    dict: Arc<zspell::Dictionary>,
    generation: u64,
    ranges: Vec<Misspelling>,
    tx: Sender<(u64, Vec<Misspelling>)>,
    rx: Receiver<(u64, Vec<Misspelling>)>,
    in_flight: bool,
    pending: Option<String>,
    last_scheduled: Option<String>,
}

impl ProofEngine {
    /// Build an engine from Hunspell `.aff` and `.dic` text already in memory.
    ///
    /// ```
    /// use tty_office::ProofEngine;
    /// let engine = ProofEngine::from_parts("SET UTF-8\n", "1\nhello\n")
    ///     .expect("fixture dictionary");
    /// assert!(engine.check_now("hello").is_empty());
    /// assert_eq!(engine.check_now("helo").len(), 1);
    /// ```
    pub fn from_parts(aff: &str, dic: &str) -> Result<Self, DocumentError> {
        let dict = zspell::builder()
            .config_str(aff)
            .dict_str(dic)
            .build()
            .map_err(|err| DocumentError::Parse(err.to_string()))?;
        Ok(Self::from_shared(Arc::new(dict)))
    }

    /// Load the system dictionary (hunspell/myspell English paths).
    ///
    /// Results are cached process-wide. Returns a parse error when no
    /// candidate dictionary can be read, which callers surface as a soft
    /// "spellcheck unavailable" message rather than a hard failure.
    pub fn load() -> Result<Self, DocumentError> {
        let shared = SYSTEM_DICT
            .get_or_init(|| load_system_dictionary().ok())
            .clone()
            .ok_or_else(|| DocumentError::Parse("no system dictionary found".to_string()))?;
        Ok(Self::from_shared(shared))
    }

    fn from_shared(dict: Arc<zspell::Dictionary>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            dict,
            generation: 0,
            ranges: Vec::new(),
            tx,
            rx,
            in_flight: false,
            pending: None,
            last_scheduled: None,
        }
    }

    /// Check `text` synchronously and return misspelled character ranges.
    ///
    /// Use this for tests and one-shot checks; interactive callers should
    /// prefer [`schedule`](Self::schedule) plus [`poll`](Self::poll).
    pub fn check_now(&self, text: &str) -> Vec<Misspelling> {
        check_text(&self.dict, text)
    }

    /// Latest applied misspelling ranges (character indices).
    pub fn ranges(&self) -> &[Misspelling] {
        &self.ranges
    }

    /// Whether a worker check is currently running.
    pub fn is_checking(&self) -> bool {
        self.in_flight
    }

    /// Request a check of `text`. No-op when the same text is already the
    /// last completed or in-flight input.
    pub fn schedule(&mut self, text: &str) {
        if self.last_scheduled.as_deref() == Some(text) && self.pending.is_none() {
            return;
        }
        self.pending = Some(text.to_string());
        self.kick();
    }

    /// Drain finished worker results. Call once per event-loop tick.
    pub fn poll(&mut self) {
        while let Ok((generation, ranges)) = self.rx.try_recv() {
            if generation == self.generation {
                self.ranges = ranges;
            }
            self.in_flight = false;
            self.kick();
        }
    }

    /// Spawn a worker for `pending` when none is in flight.
    fn kick(&mut self) {
        if self.in_flight {
            return;
        }
        let Some(text) = self.pending.take() else {
            return;
        };
        if self.last_scheduled.as_deref() == Some(text.as_str()) {
            return;
        }
        self.last_scheduled = Some(text.clone());
        self.generation = self.generation.wrapping_add(1);
        self.in_flight = true;
        let generation = self.generation;
        let dict = Arc::clone(&self.dict);
        let tx = self.tx.clone();
        // One short-lived thread per completed check keeps the dictionary
        // out of the render path without retaining a long-lived worker that
        // would hold an Arc across idle periods.
        std::thread::spawn(move || {
            let ranges = check_text(&dict, &text);
            let _ = tx.send((generation, ranges));
        });
    }
}

/// Convert zspell byte offsets into character ranges for the editor.
fn check_text(dict: &zspell::Dictionary, text: &str) -> Vec<Misspelling> {
    dict.check_indices(text)
        .map(|(byte_start, word)| {
            let start = text[..byte_start].chars().count();
            let end = start + word.chars().count();
            (start, end)
        })
        .collect()
}

/// Locate and parse the first usable system dictionary.
fn load_system_dictionary() -> Result<Arc<zspell::Dictionary>, DocumentError> {
    for (aff_path, dic_path) in dictionary_candidates() {
        if !aff_path.is_file() || !dic_path.is_file() {
            continue;
        }
        let aff = std::fs::read_to_string(&aff_path).map_err(DocumentError::Io)?;
        let dic = std::fs::read_to_string(&dic_path).map_err(DocumentError::Io)?;
        let dict = zspell::builder()
            .config_str(&aff)
            .dict_str(&dic)
            .build()
            .map_err(|err| DocumentError::Parse(err.to_string()))?;
        return Ok(Arc::new(dict));
    }
    Err(DocumentError::Parse(
        "no system dictionary found".to_string(),
    ))
}

/// Ordered dictionary candidates: `TTY_OFFICE_DICT` first, then distro paths.
fn dictionary_candidates() -> Vec<(PathBuf, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(base) = std::env::var("TTY_OFFICE_DICT") {
        let path = PathBuf::from(base);
        let prefix = if path.extension().is_some_and(|e| e == "aff") {
            path.with_extension("")
        } else {
            path
        };
        out.push((prefix.with_extension("aff"), prefix.with_extension("dic")));
    }
    for dir in ["/usr/share/hunspell", "/usr/share/myspell/dicts"] {
        let dir = Path::new(dir);
        out.push((dir.join("en_US.aff"), dir.join("en_US.dic")));
    }
    out
}
