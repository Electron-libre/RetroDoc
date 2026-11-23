# Scratch files

`$TMPDIR` is a directory of this session (`claude-scratch-<session id>`), removed when it ends. Write scratch files
(measures, logs, smoke runs, copies of a repository, browser profiles) under it, through `mktemp` or
`"$TMPDIR/<name>"`, never to a literal `/tmp/<name>` path, which outlives the session. For an artefact the user reads
after the session, create `$TMPDIR/cardinal-keep`, or write it where the user asks. When `$TMPDIR` is not such a
directory, no hook created it: nothing removes what goes there, so remove it yourself.
