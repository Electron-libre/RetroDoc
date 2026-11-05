//! The docs a long-running server answers from, kept up to date with `.retrodoc/edits.json`
//! (`specs/section-edit.md` SED-27 to SED-29): the file is read at each request and the docs rebuilt
//! only when the edits it holds differ from those shown.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use super::{edits_path, Edits};
use crate::read_model::saved_edits;

/// The edits docs show, `None` for docs with no section to edit.
type Shows<T> = Box<dyn Fn(&T) -> Option<&Edits> + Send + Sync>;
/// What builds the docs that show other edits.
type Rebuild<T> = Box<dyn Fn(&T, Edits) -> T + Send + Sync>;

/// The docs of a server, `T`, rebuilt by `rebuild` when the edits saved under their repository
/// change.
pub struct Live<T> {
    current: RwLock<Arc<T>>,
    /// `None` for docs that were not loaded from a repository: nothing to watch.
    watch: Option<Mutex<Watch>>,
    shows: Shows<T>,
    rebuild: Rebuild<T>,
}

impl<T> Live<T> {
    /// `docs` loaded from `repo_root`, rebuilt by `rebuild` with the edits saved there when they
    /// differ from those `shows` tells the docs show.
    pub fn new(
        docs: T,
        repo_root: Option<&Path>,
        shows: impl Fn(&T) -> Option<&Edits> + Send + Sync + 'static,
        rebuild: impl Fn(&T, Edits) -> T + Send + Sync + 'static,
    ) -> Self {
        Self {
            current: RwLock::new(Arc::new(docs)),
            watch: repo_root.map(|root| Mutex::new(Watch::new(root))),
            shows: Box::new(shows),
            rebuild: Box::new(rebuild),
        }
    }

    /// The docs with the edits saved now, rebuilt first when the file changed since the last call.
    pub fn current(&self) -> Arc<T> {
        if let Some(watch) = &self.watch {
            // Held while rebuilding: two requests that see the change rebuild once.
            let mut watch = watch.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(edits) = watch.changed() {
                let shown = Arc::clone(&self.read());
                if (self.shows)(&shown).is_some_and(|shows| shows != &edits) {
                    let rebuilt = (self.rebuild)(&shown, edits);
                    *self.current.write().unwrap_or_else(PoisonError::into_inner) =
                        Arc::new(rebuilt);
                }
            }
        }
        Arc::clone(&self.read())
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Arc<T>> {
        self.current.read().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The content of the file of edits, compared at each call with the one seen before.
struct Watch {
    repo_root: PathBuf,
    seen: Seen,
}

/// What the watch saw of the file of edits.
#[derive(PartialEq)]
enum Seen {
    /// Before the first call: the docs were loaded with whatever the file held then.
    Nothing,
    /// The content of the file, `None` when there was none.
    Content(Option<Vec<u8>>),
}

impl Watch {
    fn new(repo_root: &Path) -> Self {
        Self {
            repo_root: repo_root.to_path_buf(),
            seen: Seen::Nothing,
        }
    }

    /// The edits saved now when the file changed since the last call (at the first call, always),
    /// `None` otherwise; none, with a warning, when the file can't be read.
    fn changed(&mut self) -> Option<Edits> {
        let content = Seen::Content(std::fs::read(edits_path(&self.repo_root)).ok());
        if self.seen == content {
            return None;
        }
        self.seen = content;
        Some(saved_edits(&self.repo_root))
    }
}
