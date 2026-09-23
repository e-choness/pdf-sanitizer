# Using the app

## Adding files

- **Drag and drop** PDFs anywhere onto the window, or
- click **Add files…** to open the file picker.

Files are listed with their size and a status. Tick or untick files to choose
which ones the next run processes; **Sanitize** shows how many are selected.

## Running a batch

Click **Sanitize N files**. Up to *Concurrent files* (default 4) are processed
at once; the rest wait their turn. Each row shows its current stage:

| Stage | What is happening |
|---|---|
| Loading | Reading and parsing the PDF |
| Rewriting | Removing metadata, scripts, embedded files and links |
| Optimizing | Font subsetting and image recompression (if enabled) |
| Saving | Writing the cleaned PDF to a temporary file next to the original |
| Verifying | Re-reading the temporary file and checking it (see [Verification](./sanitization#verification)) |
| Replacing | Moving the original to the backup folder and putting the cleaned file in its place |

Settings are locked while a batch is running.

## What happens to your files

For each file that succeeds:

1. The original is **moved** to your backup folder. If a file with the same
   name already exists there, a number is added: `report (2).pdf`.
2. The cleaned PDF is moved to the **original path**, with the original name.

So anything that opens the file by path — shortcuts, links, other apps — gets
the sanitized version, and the original is always one folder away.

::: warning The original is only replaced after verification
If sanitizing, verifying or moving fails, the original stays exactly where it
was. If the final swap fails after the original was backed up, the app tries
to restore it and tells you where it is.
:::

## Stopping and retrying

- The **stop** button on a row cancels that file.
- **Stop all** cancels the whole batch; files that already finished stay done.
- **Retry N failed** runs failed files again — for example after fixing the
  backup folder or closing the PDF in another program.
- **Clear finished** removes completed files from the list (it does not touch
  the files on disk).

## After a run

The header shows a summary such as *"12 sanitized · 1 failed · saved 4.2 MB"*.
Each completed row shows the size before and after.
