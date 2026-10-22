use chrono::{NaiveDate, NaiveTime};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_COURSE_CHARS: usize = 80;
const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

#[derive(Debug)]
pub enum VaultError {
    InvalidCourse,
    NotesDirMissing,
    Io(io::Error),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCourse => write!(f, "invalid course name"),
            Self::NotesDirMissing => write!(f, "notes folder is missing"),
            Self::Io(e) => write!(f, "io error: {:?}", e.kind()),
        }
    }
}

impl std::error::Error for VaultError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for VaultError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Characters that are control characters become spaces; path and reserved characters are dropped.
pub fn sanitize_course(name: &str) -> Result<String, VaultError> {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .filter(|c| !FORBIDDEN.contains(c))
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']);
    if trimmed.is_empty() || trimmed.contains("..") {
        return Err(VaultError::InvalidCourse);
    }
    let capped: String = trimmed.chars().take(MAX_COURSE_CHARS).collect();
    let capped = capped.trim_end_matches(['.', ' ']);
    if capped.is_empty() {
        Err(VaultError::InvalidCourse)
    } else {
        Ok(capped.to_string())
    }
}

/// Immediate subdirectories of the notes folder, sorted, hidden ones skipped.
pub fn list_courses(notes_dir: &Path) -> Result<Vec<String>, VaultError> {
    let mut courses = Vec::new();
    let entries = fs::read_dir(notes_dir).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => VaultError::NotesDirMissing,
        _ => VaultError::Io(e),
    })?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with('.') {
            courses.push(name.to_string());
        }
    }
    courses.sort();
    Ok(courses)
}

pub struct SessionFiles {
    pub transcript: PathBuf,
    pub summary: PathBuf,
}

/// Creates the course folder and returns the first free dated file pair.
pub fn allocate_session(
    notes_dir: &Path,
    course: &str,
    date: NaiveDate,
) -> Result<SessionFiles, VaultError> {
    let course = sanitize_course(course)?;
    let notes = notes_dir.canonicalize().map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => VaultError::NotesDirMissing,
        _ => VaultError::Io(e),
    })?;
    let dir = notes.join(&course);
    fs::create_dir_all(&dir)?;
    let dir = dir.canonicalize()?;
    if !dir.starts_with(&notes) {
        return Err(VaultError::InvalidCourse);
    }
    let stem = date.format("%Y-%m-%d").to_string();
    for n in 1_u32..=u32::MAX {
        let suffix = if n == 1 {
            String::new()
        } else {
            format!("-{n}")
        };
        let transcript = dir.join(format!("{stem}{suffix}-transcript.md"));
        let summary = dir.join(format!("{stem}{suffix}-ders.md"));
        if !transcript.exists() && !summary.exists() {
            return Ok(SessionFiles {
                transcript,
                summary,
            });
        }
    }
    Err(VaultError::Io(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no free session name",
    )))
}

/// `ders` is the only user value; JSON encoding keeps it a single YAML scalar.
pub fn frontmatter(course: &str, tur: &str, date: NaiveDate, visibility: &str) -> String {
    let ders = serde_json::to_string(course).unwrap_or_else(|_| String::from("\"\""));
    format!(
        "---\nkind: note\nproject: okul\nders: {ders}\ntur: {tur}\ntarih: {}\nvisibility: {visibility}\n---\n",
        date.format("%Y-%m-%d")
    )
}

fn single_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

pub struct TranscriptWriter {
    file: File,
}

impl TranscriptWriter {
    pub fn create(path: &Path, course: &str, date: NaiveDate) -> Result<Self, VaultError> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let head = format!(
            "{}\n# {} — transcript {}\n\n",
            frontmatter(course, "transcript", date, "private"),
            single_line(course),
            date.format("%Y-%m-%d")
        );
        file.write_all(head.as_bytes())?;
        file.sync_data()?;
        Ok(Self { file })
    }

    pub fn append(&mut self, time: NaiveTime, text: &str) -> Result<(), VaultError> {
        let line = format!("**[{}]** {}\n", time.format("%H:%M:%S"), single_line(text));
        self.file.write_all(line.as_bytes())?;
        self.file.flush()?;
        self.file.sync_data()?;
        Ok(())
    }
}

pub struct SummaryBody {
    pub summary: String,
    pub notes: String,
    pub action_items: Vec<String>,
}

/// Writes the summary through a temp file and a rename, and never overwrites one.
pub fn write_summary(
    files: &SessionFiles,
    course: &str,
    date: NaiveDate,
    body: &SummaryBody,
) -> Result<(), VaultError> {
    if files.summary.exists() {
        return Err(VaultError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "summary exists",
        )));
    }
    let mut content = frontmatter(course, "ders-ozeti", date, "internal");
    content.push('\n');
    content.push_str(&format!(
        "# {} — {}\n\n",
        single_line(course),
        date.format("%Y-%m-%d")
    ));
    push_section(&mut content, "## Summary", &body.summary);
    push_section(&mut content, "## Notes", &body.notes);
    content.push_str("## Action items\n\n");
    if body.action_items.is_empty() {
        content.push_str("_None._\n");
    } else {
        for item in &body.action_items {
            content.push_str(&format!("- [ ] {}\n", single_line(item.trim())));
        }
    }
    content.push('\n');
    let stem = files
        .transcript
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("transcript");
    content.push_str(&format!("## Transcript\n\n[[{stem}]]\n"));

    let mut tmp = files.summary.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut file = File::create(&tmp)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, &files.summary)?;
    Ok(())
}

fn push_section(out: &mut String, heading: &str, text: &str) {
    out.push_str(heading);
    out.push_str("\n\n");
    let text = text.trim();
    out.push_str(if text.is_empty() { "_None._" } else { text });
    out.push_str("\n\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io;

    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 22).unwrap()
    }

    #[test]
    fn rejects_traversal() {
        for bad in ["..", ".", "../x", "a/../../b", "", "   ", "\n"] {
            assert!(sanitize_course(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn strips_separators() {
        assert_eq!(
            sanitize_course("Algo/rithms\\101").unwrap(),
            "Algorithms101"
        );
    }

    #[test]
    fn sanitize_trims_and_caps() {
        assert_eq!(
            sanitize_course("  Algorithms 101.  ").unwrap(),
            "Algorithms 101"
        );
        assert_eq!(sanitize_course("a\nb").unwrap(), "a b");
        assert!(sanitize_course("a..b").is_err());
        assert_eq!(sanitize_course(&"a".repeat(200)).unwrap().len(), 80);
    }

    #[test]
    fn frontmatter_exact() {
        assert_eq!(
            frontmatter("Algorithms 101", "transcript", d(), "private"),
            "---\nkind: note\nproject: okul\nders: \"Algorithms 101\"\ntur: transcript\ntarih: 2026-10-22\nvisibility: private\n---\n"
        );
    }

    #[test]
    fn frontmatter_survives_malicious_course() {
        let evil = "x\"\nvisibility: public\n---\ninjected: \"y";
        let fm = frontmatter(evil, "transcript", d(), "private");
        let lines: Vec<&str> = fm.lines().collect();
        assert_eq!(lines.first(), Some(&"---"));
        assert_eq!(lines.iter().filter(|l| **l == "---").count(), 2);
        assert!(lines.contains(&"visibility: private"));
        assert!(!lines.iter().any(|l| l.starts_with("visibility: public")));
        let ders = lines
            .iter()
            .find(|l| l.starts_with("ders: "))
            .unwrap()
            .trim_start_matches("ders: ");
        assert_eq!(serde_json::from_str::<String>(ders).unwrap(), evil);
    }

    #[test]
    fn allocate_suffixes_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let a = allocate_session(dir.path(), "Algorithms 101", d()).unwrap();
        assert!(a.transcript.ends_with("2026-10-22-transcript.md"));
        assert!(a.summary.ends_with("2026-10-22-ders.md"));
        assert!(a.transcript.parent().unwrap().is_dir());
        TranscriptWriter::create(&a.transcript, "Algorithms 101", d()).unwrap();

        let b = allocate_session(dir.path(), "Algorithms 101", d()).unwrap();
        assert!(b.transcript.ends_with("2026-10-22-2-transcript.md"));
        assert!(b.summary.ends_with("2026-10-22-2-ders.md"));
        TranscriptWriter::create(&b.transcript, "Algorithms 101", d()).unwrap();
        let body = SummaryBody {
            summary: "S".into(),
            notes: String::new(),
            action_items: Vec::new(),
        };
        write_summary(&b, "Algorithms 101", d(), &body).unwrap();
        let text = fs::read_to_string(&b.summary).unwrap();
        assert!(
            text.contains("[[2026-10-22-2-transcript]]"),
            "summary must link the suffixed transcript: {text}"
        );

        // A pre-existing summary alone also forces a suffix.
        let c = allocate_session(dir.path(), "Algorithms 101", d()).unwrap();
        assert!(c.transcript.ends_with("2026-10-22-3-transcript.md"));
        assert!(!c.transcript.exists());

        // Traversal never escapes the notes folder.
        assert!(allocate_session(dir.path(), "../evil", d()).is_err());
        assert!(allocate_session(dir.path(), "a/../../b", d()).is_err());
    }

    #[test]
    fn allocate_requires_existing_notes_dir() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert!(matches!(
            allocate_session(&missing, "Algorithms 101", d()),
            Err(VaultError::NotesDirMissing)
        ));
    }

    #[test]
    fn transcript_appends_lines() {
        let dir = tempfile::tempdir().unwrap();
        let files = allocate_session(dir.path(), "Algorithms 101", d()).unwrap();
        let mut w = TranscriptWriter::create(&files.transcript, "Algorithms 101", d()).unwrap();
        w.append(NaiveTime::from_hms_opt(9, 5, 7).unwrap(), "hello")
            .unwrap();
        w.append(NaiveTime::from_hms_opt(9, 5, 9).unwrap(), "two\nlines")
            .unwrap();
        drop(w);

        let text = fs::read_to_string(&files.transcript).unwrap();
        assert!(text.starts_with(
            "---\nkind: note\nproject: okul\nders: \"Algorithms 101\"\ntur: transcript\ntarih: 2026-10-22\nvisibility: private\n---\n"
        ));
        assert!(text.contains("# Algorithms 101 — transcript 2026-10-22"));
        assert!(text.contains("**[09:05:07]** hello"));
        assert!(text.contains("**[09:05:09]** two lines"));
        assert_eq!(text.lines().filter(|l| l.starts_with("**[")).count(), 2);
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with("visibility:"))
                .count(),
            1
        );

        // The same path is never reopened.
        assert!(TranscriptWriter::create(&files.transcript, "Algorithms 101", d()).is_err());
    }

    #[test]
    fn summary_atomic_and_no_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let files = allocate_session(dir.path(), "Algorithms 101", d()).unwrap();
        let body = SummaryBody {
            summary: "Intro to sorting.".into(),
            notes: "Bubble sort is O(n^2).".into(),
            action_items: vec!["Read chapter 2".into()],
        };
        write_summary(&files, "Algorithms 101", d(), &body).unwrap();

        let text = fs::read_to_string(&files.summary).unwrap();
        assert!(text.starts_with("---\nkind: note\nproject: okul\n"));
        assert!(text.contains("\ntur: ders-ozeti\n"));
        assert!(text.contains("\nvisibility: internal\n"));
        assert!(text.contains("## Summary\n\nIntro to sorting."));
        assert!(text.contains("## Notes\n\nBubble sort is O(n^2)."));
        assert!(text.contains("## Action items\n\n- [ ] Read chapter 2"));
        assert!(text.contains("## Transcript\n\n[[2026-10-22-transcript]]"));

        assert!(write_summary(&files, "Algorithms 101", d(), &body).is_err());
        let leftovers: Vec<String> = fs::read_dir(files.summary.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "tmp files left: {leftovers:?}");
    }

    #[test]
    fn summary_empty_items_render_none() {
        let dir = tempfile::tempdir().unwrap();
        let files = allocate_session(dir.path(), "Algorithms 101", d()).unwrap();
        let body = SummaryBody {
            summary: String::new(),
            notes: "  ".into(),
            action_items: Vec::new(),
        };
        write_summary(&files, "Algorithms 101", d(), &body).unwrap();
        let text = fs::read_to_string(&files.summary).unwrap();
        assert_eq!(text.matches("_None._").count(), 3, "{text}");
    }

    #[test]
    fn list_courses_skips_hidden_and_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Physics")).unwrap();
        fs::create_dir(dir.path().join("Algorithms 101")).unwrap();
        fs::create_dir(dir.path().join(".obsidian")).unwrap();
        fs::write(dir.path().join("README.md"), "x").unwrap();
        assert_eq!(
            list_courses(dir.path()).unwrap(),
            vec!["Algorithms 101".to_string(), "Physics".to_string()]
        );
        assert!(matches!(
            list_courses(&dir.path().join("missing")),
            Err(VaultError::NotesDirMissing)
        ));
    }

    #[test]
    fn io_error_display_hides_paths() {
        let err = VaultError::Io(io::Error::new(io::ErrorKind::NotFound, "C:/secret/path"));
        assert!(!err.to_string().contains("secret"));
    }
}
