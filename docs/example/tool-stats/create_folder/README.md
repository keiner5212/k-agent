# create_folder

Create a directory (and any missing parents) at an absolute or workspace-relative path.

## Does

- Calls `fs::create_dir_all`, so missing parents are created in the same call.
- Is a no-op when the target directory already exists; reports `status: "exists"` with no error.
- Creates nested paths in one call (`./a/b/c/d` will create `a`, `a/b`, `a/b/c`, `a/b/c/d`).

## Does not

- Replace an existing file with a directory. Errors out when the path is a file rather than a missing or directory entry.
- Refuse to create a directory inside an existing directory tree. There is no protection against accidentally creating top-level dot-prefixed names like `.cache`.
- Touch any existing files. Use `write` for files and `delete` for cleanup.

## Options

| Name      | Type   | Required | Default | Notes                                          |
| --------- | ------ | -------- | ------- | ---------------------------------------------- |
| `dirPath` | string | yes      | -       | Absolute or workspace-relative directory path. |

## Response

| Field    | Type   | Notes                                                                                            |
| -------- | ------ | ------------------------------------------------------------------------------------------------ |
| `path`   | string | Workspace-relative directory that was created (or already existed).                              |
| `status` | string | `created` when this call created the directory; `exists` when the directory was already present. |

The TOON response body is `path` + one of `status: "created"` / `status: "exists"`.

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `create_folder tool requires a string \`dirPath\`.`: argument missing or wrong type.
- `create_folder tool \`dirPath\` is empty.`: argument is whitespace.
- `Unable to create directory \`<path>\` <io error>`: parent not creatable (permissions, missing parent component, etc.).
- `Unable to stat \`<path>\`: <io error>`: a stat call failed unexpectedly.
- `Path already exists and is not a directory: <path>`: target is an existing file.

## Source

`src-tauri/src/tools/create_folder.rs` - entry point: `CreateFolderTool::execute()`.
