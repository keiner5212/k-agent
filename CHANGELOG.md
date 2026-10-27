# Changelog

Each app version needs a section in this file. The version is the one in
`package.json`, and the same version must be in `src-tauri/tauri.conf.json` and
`src-tauri/Cargo.toml`. CI rejects a bump that does not add the matching
section. The release draft uses that section as its body.

The entries say what changed and why it mattered, in the words of the person who
had to live with it.

## [0.2.3] - 2026-10-27

### Fixed

- The release build stopped on a Tauri version check. The Rust crates had moved to a newer minor and the JavaScript packages had not. The packages now share the locked minor.
- A shell command queued in one chat was dropped while another chat was answering, and retry was blocked the same way. Each chat now starts its own command.
- The macOS build called a Linux-only process call, so that release could not compile. macOS now finds the app's children through its own process list and kills them on exit. Windows resumes a suspended child even when its thread shows up late, and a batch file whose path has spaces still starts.
- File writes that overlap no longer share one temporary name. A failed edit or patch rolls the file back the same way, whole or not at all.

## [0.2.2] - 2026-10-27

### Fixed

- The edit tool says what the file really looks like when the text is not found, and it never leaves a half written file. The message names the line endings of the file and the line that starts the same, with the indentation it has and the one the oldString asks for. The text goes to a file of its own and the target is replaced with a rename, so a failure leaves the file as it was. The write tool and the patch tool write the same way.
- The chat no longer gets stuck when a tool answers with something the app cannot read. The answer was encoded with a panic and the app is built to abort on a panic, so the whole process died in the middle of the turn: the message of the user stayed there without an answer and the file of the session could end up half written. The answer is now encoded without a panic, the session file is written whole or not at all, and a turn that waits for the user cannot wait forever.
- The children of the app die with it. On Linux the signal was already there, but on Windows the processes were left running and kept the processor busy after the app closed, one set for each time the app was opened and closed.
- The screenshot refuses a capture of one pixel. It answered "ok" with a picture of a single pixel when the selector matched a tiny box, and the model took that picture for the page.
- A tool that fails is shown as failed. The errors of the MCP servers were rendered as a success, so a broken server looked like a tool that did nothing.

- The button that improves the prompt works while the chat answers, and it says why it is off. It turned itself off when a turn was running and the tooltip blamed the model, which was already chosen, so it looked broken.
- A chat that one window deletes is not deleted by another window. The backend only removes the chats that the window that saved named, and a chat of another window survives.
- The names of the commands that need a confirmation follow the host. The names of Windows are not commands of Linux, and a program of Linux keeps its extension.
- Two writes of the same chat no longer share the name of the temporary file. The file of the session is written whole, and the temporary name carries the process and a counter of its own.

### Added

- Each chat runs on its own. A chat that is answering keeps going when you open another one, and the new chat can send its message and be answered while the first one finishes. The composer, the escape key and the queue of a chat only look at the turn of that chat.
- Every tool has an icon. Seven of them had none and left an empty gap in the list of the tools.
- The undo says what it does not undo. Only the edits of the files are rolled back, and the changes made with commands are not, which the dialog now says before the user confirms.
- The row of a tool shows a short summary instead of the whole list of the parameters while the tool runs. The raw parameters were shown until the tool answered, and the row jumped when it did.
- The changelog, the CI and the release work as one flow. Every version needs its section in `CHANGELOG.md`, the CI checks it and the checks of the app on every push and pull request, and the release takes the section of the version as the body of the draft.

### Changed

- The CI runs only when a pull request is opened and when the branch lands in `main`. The same commit was checked twice before, once for the push of the branch and once for the pull request.
- The dependencies of the build were updated and the lock file of the backend is kept now. The advisories of the frontend and of the backend are gone, the build of a release is the same everywhere, and the CI can watch for the new ones.

- The compatibility core. The shell, the paths, the permissions of the secret files, the language servers, the MCP servers and the archives now take the algorithm that fits the host instead of assuming Linux. The Windows build gets the job object that kills the tree of processes, the access control list that only the owner can read, the resolution of the commands that live in a `.cmd` file, and the archives that no longer need `gzip`, `unzip` or `tar` in the system.
