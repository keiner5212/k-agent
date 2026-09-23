//! Always-on integration test that runs every tool in `k_agent_lib::tools`,
//! captures the response, measures performance statistics, and writes three
//! files per tool to `docs/example/tool-stats/<tool>/`:
//!
//! - `README.md`         hand-written specification (see each folder)
//! - `stats.md`          auto-generated performance metrics
//! - `response.toon`     auto-generated tool response in the same TOON the LLM sees
//!
//! Read-only tools (skill, read, list_directory) run against the real
//! `docs/` workspace so they exercise actual project content. Tools that
//! create state (write, edit, create_folder, delete) run against a
//! per-process scratch directory under `docs/.tmp/tool-examples-<id>/`
//! (gitignored) so the repo tree stays clean between runs.
//!
//! Re-run with `cargo test --test tools_examples` whenever a tool gains a
//! field or the wire format changes, then commit the regenerated files.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use k_agent_lib::tools::ask_user::{execute_async as ask_user_execute_async, AskUserAnswerEntry};
use k_agent_lib::tools::{
    execute, ToolContext, ASK_USER_TOOL_NAME, CREATE_FOLDER_TOOL_NAME, DELETE_TOOL_NAME,
    EDIT_TOOL_NAME, FETCH_URL_TOOL_NAME, GRAPHQL_TOOL_NAME, HTTP_REQUEST_TOOL_NAME,
    INTERNET_SEARCH_TOOL_NAME, LIST_DIRECTORY_TOOL_NAME, PAGE_SHOT_TOOL_NAME, READ_TOOL_NAME,
    SKILL_TOOL_NAME, TODO_TOOL_NAME, WRITE_TOOL_NAME,
};

const REALISTIC_FILE_BODY: &str = "# Draft: sample skill body\n\
    \n\
    The body below is realistic content (~3 KB) so the `read` tool actually\n\
    surfaces interesting line counts, the `write` tool reports a non-trivial\n\
    byte count, and the `edit` tool has a real block to relax-match against.\n\
    \n\
    ## When to load this skill\n\
    \n\
    Use this skill when the user asks for a code review pass before opening\n\
    a pull request. The skill should load after the model has identified the\n\
    files in scope but before it starts reading them line by line.\n\
    \n\
    ## Review checklist\n\
    \n\
    1. Read each touched file once for shape. Skim public surface, skip\n\
       internal helpers unless they carry risk.\n\
    2. Diff against the project's last green commit on the same branch.\n\
       Anything that lands outside the stated intent of the PR is suspect.\n\
    3. Confirm the change set leaves the test suite green. A review that\n\
       cannot reproduce the green state is a review that approves a guess.\n\
    4. Note any new shared module surface. New helpers in shared files are\n\
       rarely wrong on their own but they compound into debt fast.\n\
    5. Look for missing tests for behavior the PR introduces. Refactors\n\
       without test coverage trade confidence for diff size.\n\
    \n\
    ## Tone\n\
    \n\
    Reviews should be specific, calm, and actionable. Quote the line that\n\
    triggered each comment. Skip generic praise that does not tell the\n\
    author what to keep doing.\n";

struct ProcStats {
    wall_ns: u128,
    user_us: u64,
    sys_us: u64,
    rss_before_kb: u64,
    rss_after_kb: u64,
    host_peak_kb: u64,
}

#[cfg(unix)]
fn rusage_self() -> libc::rusage {
    let mut rusage: libc::rusage = unsafe { std::mem::zeroed() };
    let _ = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut rusage) };
    rusage
}

/// Process peak resident set size in KiB. `getrusage.ru_maxrss` is KB on Linux
/// and bytes on macOS (BSD semantics). Normalize to KiB.
#[cfg(unix)]
fn peak_rss_kb(rusage: &libc::rusage) -> u64 {
    let raw = rusage.ru_maxrss as u64;
    if cfg!(target_os = "macos") {
        raw / 1024
    } else {
        raw
    }
}

#[cfg(unix)]
fn rusage_diff(before: libc::rusage) -> (u64, u64, u64) {
    let after = rusage_self();
    let user_us = (after.ru_utime.tv_sec - before.ru_utime.tv_sec).max(0) as u64 * 1_000_000
        + (after.ru_utime.tv_usec - before.ru_utime.tv_usec).max(0) as u64;
    let sys_us = (after.ru_stime.tv_sec - before.ru_stime.tv_sec).max(0) as u64 * 1_000_000
        + (after.ru_stime.tv_usec - before.ru_stime.tv_usec).max(0) as u64;
    (user_us, sys_us, peak_rss_kb(&after))
}

#[cfg(not(unix))]
fn rusage_diff(_: ()) -> (u64, u64, u64) {
    (0, 0, 0)
}

/// Real-time resident set size in KiB. Reads `/proc/self/status` on Linux and
/// spawns `ps -o rss= -p <pid>` on macOS. Used to compute the RSS delta across
/// a single tool execution; the lifetime peak (which includes the entire test
/// binary) is not a fair attribution to the tool.
#[cfg(target_os = "linux")]
fn current_rss_kb() -> u64 {
    if let Ok(text) = fs::read_to_string("/proc/self/status") {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                if let Some(num) = rest.split_whitespace().next() {
                    return num.parse().unwrap_or(0);
                }
            }
        }
    }
    0
}

#[cfg(target_os = "macos")]
fn current_rss_kb() -> u64 {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output();
    if let Ok(out) = output {
        if out.status.success() {
            // ps on macOS reports RSS in kilobytes directly.
            return String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse()
                .unwrap_or(0);
        }
    }
    0
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn current_rss_kb() -> u64 {
    0
}

fn cpu_brand() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = fs::read_to_string("/proc/cpuinfo") {
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("model name") {
                    if let Some(value) = rest.split(':').nth(1) {
                        return value.trim().to_string();
                    }
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
        }
    }
    "unknown".to_string()
}

fn logical_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

#[cfg(unix)]
fn kernel_release() -> String {
    let mut utsname: libc::utsname = unsafe { std::mem::zeroed() };
    if unsafe { libc::uname(&mut utsname) } == 0 {
        let release = unsafe { std::ffi::CStr::from_ptr(utsname.release.as_ptr()) };
        return release.to_string_lossy().trim().to_string();
    }
    String::new()
}

#[cfg(not(unix))]
fn kernel_release() -> String {
    String::new()
}

/// Returns `(workspace_root, scratch_dir)`. The workspace is the project's
/// real `docs/` directory so read/list/skill tests touch real content.
/// Mutating tools operate on the per-process scratch dir under `docs/.tmp/`
/// (gitignored), which is cleaned up at the end of the test run.
fn make_workspace() -> (PathBuf, PathBuf) {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let workspace = repo_root.join("docs");
    let scratch_unique = format!(
        "tool-examples-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let scratch = workspace.join(".tmp").join(scratch_unique);
    fs::create_dir_all(&scratch).unwrap();
    (workspace, scratch)
}

fn cleanup_scratch(scratch: &Path) {
    let _ = fs::remove_dir_all(scratch);
}

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("example")
        .join("tool-stats")
}

fn tool_dir(tool: &str) -> PathBuf {
    output_dir().join(tool)
}

fn write_input(tool: &str, raw_args: &str) {
    let value: serde_json::Value = serde_json::from_str(raw_args)
        .unwrap_or_else(|_| serde_json::Value::String(raw_args.to_string()));
    let pretty = serde_json::to_string_pretty(&value).unwrap();
    let dir = tool_dir(tool);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("input.json"), pretty).unwrap();
}

async fn measure<F>(future: F) -> (ProcStats, k_agent_lib::tools::ToolOutcome)
where
    F: std::future::Future<Output = k_agent_lib::tools::ToolOutcome>,
{
    let before = rusage_self();
    let rss_before_kb = current_rss_kb();
    let started = Instant::now();
    let outcome = future.await;
    let wall_ns = started.elapsed().as_nanos();
    let rss_after_kb = current_rss_kb();
    let (user_us, sys_us, host_peak_kb) = rusage_diff(before);
    (
        ProcStats {
            wall_ns,
            user_us,
            sys_us,
            rss_before_kb,
            rss_after_kb,
            host_peak_kb,
        },
        outcome,
    )
}

fn write_stats(
    tool: &str,
    stats: &ProcStats,
    outcome: &k_agent_lib::tools::ToolOutcome,
    ctx: &ToolContext<'_>,
) {
    let dir = tool_dir(tool);
    fs::create_dir_all(&dir).unwrap();
    let response = &outcome.text;
    let bytes = response.len();
    let chars = response.chars().count();
    let lines = if response.is_empty() {
        0
    } else {
        response.lines().count()
    };
    let tokens = (chars + 3) / 4;
    let kind = if outcome.display.kind.is_empty() {
        "context"
    } else {
        outcome.display.kind.as_str()
    };
    let status = outcome.display.status.as_deref().unwrap_or("ok");
    let target_path = outcome.display.path.as_deref().unwrap_or("");
    let in_workspace = ctx.workspace.is_some() && !target_path.is_empty();
    let skill_name = outcome.display.skill_name.as_deref().unwrap_or("");
    let wall_ms = stats.wall_ns / 1_000_000;
    let wall_us = stats.wall_ns / 1_000;
    let user_ms = stats.user_us / 1_000;
    let sys_ms = stats.sys_us / 1_000;
    let rss_delta_kb = stats.rss_after_kb as i64 - stats.rss_before_kb as i64;
    let stats_md = format!(
        "# {tool}\n\
         \n\
         Auto-generated by `src-tauri/tests/tools_examples.rs`. Run the test to refresh\n\
         this file and the matching `<tool>.response.toon`.\n\
         \n\
         ## Outcome\n\
         \n\
         kind: `{kind}`  status: `{status}`  call_id: `{call_id}`\n\
         \n\
         ## Environment\n\
         \n\
         | Platform | Arch | Kernel | Logical CPUs | CPU model |\n\
         |---|---|---|---|---|\n\
         | {platform} | {arch} | {kernel} | {cpus} | {brand} |\n\
         \n\
         ## Performance\n\
         \n\
         How long the tool ran, how much CPU it burned, and how much resident\n\
         memory the host process held while it ran.\n\
         \n\
         | Wall time | CPU time (user + sys) | Host process RSS (during call) |\n\
         |---|---|---|\n\
         | **{wall_ms} ms** ({wall_us} us) | {user_ms} ms user + {sys_ms} ms sys | before: {rss_before_kb} KiB, after: {rss_after_kb} KiB, delta: {rss_delta_kb:+} KiB, lifetime peak: {host_peak_kb} KiB |\n\
         \n\
         RSS is sampled via `/proc/self/status` on Linux or `ps -o rss=` on\n\
         macOS, immediately before and after the call. The delta reflects\n\
         only this call; the lifetime peak comes from `getrusage.ru_maxrss`\n\
         and includes every shared library already loaded into the host\n\
         process, so it is not a per-tool attribution.\n\
         \n\
         ## Parallelism\n\
         \n\
         | Configured for this run | Host logical CPUs |\n\
         |---|---|\n\
         | {configured} | {cpus} |\n\
         \n\
         Each tool execution is single-threaded inside this test. Recursive\n\
         `list_directory` may spawn up to `min(configured, host_cpus, 16)` workers\n\
         at runtime; see `src-tauri/src/tools/list_directory.rs`.\n\
         \n\
         ## Response size\n\
         \n\
         | Bytes | Chars | Lines | Tokens (chars/4) |\n\
         |---|---|---|---|\n\
         | {bytes} | {chars} | {lines} | {tokens} |\n\
         \n\
         ## Target\n\
         \n\
         | Field | Value |\n\
         |---|---|\n\
         | Path | `{path}` |\n\
         | In workspace | {in_workspace} |\n\
         | Skill | `{skill}` |\n",
        tool = tool,
        platform = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        kernel = kernel_release(),
        cpus = logical_cpus(),
        brand = cpu_brand(),
        kind = kind,
        status = status,
        call_id = ctx.call_id,
        wall_ms = wall_ms,
        wall_us = wall_us,
        user_ms = user_ms,
        sys_ms = sys_ms,
        configured = ctx.parallelism,
        rss_before_kb = stats.rss_before_kb,
        rss_after_kb = stats.rss_after_kb,
        rss_delta_kb = rss_delta_kb,
        host_peak_kb = stats.host_peak_kb,
        bytes = bytes,
        chars = chars,
        lines = lines,
        tokens = tokens,
        path = target_path,
        in_workspace = in_workspace,
        skill = skill_name,
    );
    fs::write(dir.join("stats.md"), stats_md).unwrap();
    fs::write(dir.join("response.toon"), response).unwrap();
    eprintln!(
        "[tool-examples] {tool}: kind={kind} status={status} bytes={bytes} chars={chars} lines={lines}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn dumps_tool_examples() {
    let (workspace_docs, scratch) = make_workspace();
    let parallelism = logical_cpus().max(1);

    // Two contexts: read-only tools run against the real `docs/` workspace;
    // mutating tools run against the per-process scratch dir so their paths
    // resolve cleanly without the `.tmp/.../` prefix leaking into the
    // captured TOON.
    let ctx_docs = ToolContext::for_test(workspace_docs.clone(), parallelism);
    let ctx_scratch = ToolContext::for_test(scratch.clone(), parallelism);

    // Path layout inside the scratch dir:
    //   draft-skill.md      -> the file written, edited, then deleted
    //   nested/inner/deep/  -> a 3-level directory created by create_folder
    let realistic_path = "draft-skill.md";
    let nested_dir_path = "nested/inner/deep";

    // skill: app handle is None, so the tool errors. We still capture the
    // response so the docs show what a missing skill looks like. The skill
    // name here mirrors a real-world scenario so the error message is the
    // one a model would actually see.
    {
        let args = r#"{"name":"code-review"}"#;
        let (stats, outcome) =
            measure(async { execute(SKILL_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(SKILL_TOOL_NAME, args);
        write_stats(SKILL_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // todowrite: app handle is None, so persistence errors out. Captured so
    // the docs show the incremental argument validation path the model would
    // hit when it tries to write the session todo list without a live shell.
    {
        let args = r#"{"add":[{"id":"step-1","content":"Investigate ask_user persistence","status":"in_progress","priority":9},{"id":"step-2","content":"Wire todowrite into default agents","priority":5},{"id":"step-3","content":"Render todo list inline","status":"completed","priority":1}],"update":[{"id":"step-2","status":"in_progress"}]}"#;
        let (stats, outcome) =
            measure(async { execute(TODO_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(TODO_TOOL_NAME, args);
        write_stats(TODO_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // create_folder: builds a nested path under scratch so write/edit/delete
    // have somewhere to land. Mirrors a real "make a notes folder" call.
    {
        let args = format!(r#"{{"dirPath":"{nested_dir_path}"}}"#);
        let (stats, outcome) =
            measure(async { execute(CREATE_FOLDER_TOOL_NAME, &args, &ctx_scratch).await }).await;
        write_input(CREATE_FOLDER_TOOL_NAME, &args);
        write_stats(CREATE_FOLDER_TOOL_NAME, &stats, &outcome, &ctx_scratch);
    }

    // write: drops a ~3 KB markdown draft into the scratch dir so the
    // subsequent edit and delete have a real target.
    {
        let args = format!(
            r#"{{"filePath":"{realistic_path}","content":{}}}"#,
            serde_json::to_string(REALISTIC_FILE_BODY).unwrap()
        );
        let (stats, outcome) =
            measure(async { execute(WRITE_TOOL_NAME, &args, &ctx_scratch).await }).await;
        write_input(WRITE_TOOL_NAME, &args);
        write_stats(WRITE_TOOL_NAME, &stats, &outcome, &ctx_scratch);
    }

    // edit: rewrites a multi-line section in the draft above. Uses the
    // indentation-flexible replacer: oldString omits the leading indent that
    // the file has, so the simple replacer misses and the relaxed replacer
    // kicks in.
    {
        let old_string =
            "    1. Read each touched file once for shape. Skim public surface, skip\n\
            internal helpers unless they carry risk.\n\
            2. Diff against the project's last green commit on the same branch.\n\
            Anything that lands outside the stated intent of the PR is suspect.\n\
            3. Confirm the change set leaves the test suite green. A review that\n\
            cannot reproduce the green state is a review that approves a guess.";
        let new_string =
            "    1. Read each touched file once for shape. Skim public surface, skip\n\
            internal helpers unless they carry risk.\n\
            2. Diff against the project's last green commit on the same branch.\n\
            Anything that lands outside the stated intent of the PR is suspect.\n\
            2.5. If the diff is huge, ask the author to split it before reviewing.\n\
            3. Confirm the change set leaves the test suite green. A review that\n\
            cannot reproduce the green state is a review that approves a guess.";
        let args = format!(
            r#"{{"filePath":"{realistic_path}","oldString":{},"newString":{}}}"#,
            serde_json::to_string(old_string).unwrap(),
            serde_json::to_string(new_string).unwrap(),
        );
        let (stats, outcome) =
            measure(async { execute(EDIT_TOOL_NAME, &args, &ctx_scratch).await }).await;
        write_input(EDIT_TOOL_NAME, &args);
        write_stats(EDIT_TOOL_NAME, &stats, &outcome, &ctx_scratch);
    }

    // read: real file in docs/, with offset + limit so the output exercises
    // the "(Showing lines A-B of N)" footer and produces a non-trivial body.
    {
        let args = r#"{"filePath":"example/system-prompt.md","offset":1,"limit":40}"#;
        let (stats, outcome) =
            measure(async { execute(READ_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(READ_TOOL_NAME, args);
        write_stats(READ_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // list_directory: recursive walk of `example/` so the model sees the
    // nested `tool-stats/<tool>/` structure that lives there now.
    {
        let args = r#"{"dirPath":"example","recursive":true,"maxDepth":3}"#;
        let (stats, outcome) =
            measure(async { execute(LIST_DIRECTORY_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(LIST_DIRECTORY_TOOL_NAME, args);
        write_stats(LIST_DIRECTORY_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // delete: removes the ~3 KB draft file. `linesRemoved` should reflect the
    // line count so the chat UI can subtract it from the context walk.
    {
        let args = format!(r#"{{"path":"{realistic_path}"}}"#);
        let (stats, outcome) =
            measure(async { execute(DELETE_TOOL_NAME, &args, &ctx_scratch).await }).await;
        write_input(DELETE_TOOL_NAME, &args);
        write_stats(DELETE_TOOL_NAME, &stats, &outcome, &ctx_scratch);
    }

    // ask_user: synthesize a submitted answer via the registry. Use
    // `execute_async` directly (not the dispatcher's async path) so we can
    // pre-register the answer via the registry's `complete`, then race a
    // polling task against the wait. The workspace choice does not matter
    // for this tool; use the docs ctx to keep the test surface narrow.
    {
        let args = r#"{
            "questions": [{
                "id": "approach",
                "header": "Approach",
                "question": "Which approach should I take?",
                "options": [
                    {"label": "Refactor in place"},
                    {"label": "Extract helper"}
                ],
                "multiSelect": false,
                "allowFreeText": true
            }]
        }"#;
        let answer = vec![AskUserAnswerEntry {
            question_id: "approach".into(),
            selected: vec!["Extract helper".into()],
            free_text: String::new(),
            skipped: false,
        }];
        let call_id = ctx_docs.call_id.clone();
        let deliver = tokio::spawn({
            let answer = answer.clone();
            async move {
                for _ in 0..500 {
                    if k_agent_lib::tools::ask_user::ask_user_registry()
                        .complete(call_id.as_str(), answer.clone())
                    {
                        return true;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                }
                false
            }
        });
        let (stats, outcome) =
            measure(async { ask_user_execute_async(args, &ctx_docs).await }).await;
        let _ = deliver.await;
        write_input(ASK_USER_TOOL_NAME, args);
        write_stats(ASK_USER_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // fetch_url: real public HTTPS page. When offline the response.toon
    // captures the SSRF / network error path so the docs still describe
    // the wire shape.
    {
        let args = r#"{"url":"https://example.com/"}"#;
        let (stats, outcome) =
            measure(async { execute(FETCH_URL_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(FETCH_URL_TOOL_NAME, args);
        write_stats(FETCH_URL_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // internet_search: Bing first, DuckDuckGo fallback. Same offline
    // behaviour as fetch_url - the captured response reflects the real
    // error rather than fabricating a result.
    {
        let args = r#"{"query":"rust programming language","lang":"en-US"}"#;
        let (stats, outcome) =
            measure(async { execute(INTERNET_SEARCH_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(INTERNET_SEARCH_TOOL_NAME, args);
        write_stats(INTERNET_SEARCH_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // http_request: GET skips confirm. Closed port captures the network error.
    {
        let args = r#"{"url":"http://127.0.0.1:9/","method":"GET"}"#;
        let (stats, outcome) =
            measure(async { execute(HTTP_REQUEST_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(HTTP_REQUEST_TOOL_NAME, args);
        write_stats(HTTP_REQUEST_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // graphql: GET skips confirm. Same closed port.
    {
        let args =
            r#"{"url":"http://127.0.0.1:9/graphql","query":"{ __typename }","method":"GET"}"#;
        let (stats, outcome) =
            measure(async { execute(GRAPHQL_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(GRAPHQL_TOOL_NAME, args);
        write_stats(GRAPHQL_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // page_shot: no desktop shell in this harness, so the error path is captured.
    {
        let args = r#"{"url":"http://127.0.0.1:9/","width":800,"height":600}"#;
        let (stats, outcome) =
            measure(async { execute(PAGE_SHOT_TOOL_NAME, args, &ctx_docs).await }).await;
        write_input(PAGE_SHOT_TOOL_NAME, args);
        write_stats(PAGE_SHOT_TOOL_NAME, &stats, &outcome, &ctx_docs);
    }

    // Clean up the scratch directory so cargo test leaves docs/ tidy.
    cleanup_scratch(&scratch);

    eprintln!(
        "[tool-examples] dumped all 13 tools at {}",
        output_dir().display()
    );
}
