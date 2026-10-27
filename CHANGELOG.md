# Changelog

Each app version needs a section in this file. The version is the one in
`package.json`, and the same version must be in `src-tauri/tauri.conf.json` and
`src-tauri/Cargo.toml`. CI rejects a bump that does not add the matching
section. The release draft uses that section as its body.

The entries say what changed and why it mattered, in the words of the person who
had to live with it.

## [0.2.1] - 2026-10-27

### Fixed

- The chat no longer gets stuck when a tool answers with something the app cannot read. The answer was encoded with a panic and the app is built to abort on a panic, so the whole process died in the middle of the turn: the message of the user stayed there without an answer and the file of the session could end up half written. The answer is now encoded without a panic, the session file is written whole or not at all, and a turn that waits for the user cannot wait forever.
- The children of the app die with it. On Linux the signal was already there, but on Windows the processes were left running and kept the processor busy after the app closed, one set for each time the app was opened and closed.
- The screenshot refuses a capture of one pixel. It answered "ok" with a picture of a single pixel when the selector matched a tiny box, and the model took that picture for the page.
- A tool that fails is shown as failed. The errors of the MCP servers were rendered as a success, so a broken server looked like a tool that did nothing.

### Added

- Every tool has an icon. Seven of them had none and left an empty gap in the list of the tools.
- The undo says what it does not undo. Only the edits of the files are rolled back, and the changes made with commands are not, which the dialog now says before the user confirms.
- The row of a tool shows a short summary instead of the whole list of the parameters while the tool runs. The raw parameters were shown until the tool answered, and the row jumped when it did.
- The changelog, the CI and the release work as one flow. Every version needs its section in `CHANGELOG.md`, the CI checks it and the checks of the app on every push and pull request, and the release takes the section of the version as the body of the draft.

### Changed

- The compatibility core. The shell, the paths, the permissions of the secret files, the language servers, the MCP servers and the archives now take the algorithm that fits the host instead of assuming Linux. The Windows build gets the job object that kills the tree of processes, the access control list that only the owner can read, the resolution of the commands that live in a `.cmd` file, and the archives that no longer need `gzip`, `unzip` or `tar` in the system.
