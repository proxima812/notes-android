-- Folders are gone: tags carry the whole burden of filing a note.
--
-- Two ways to keep a note findable was one too many — the picker asked twice
-- for the same decision. Tags stay; the folder tables and everything pointing
-- at them go. This drops user data: whatever folders existed, and which notes
-- were in them, are not recoverable from here.

DROP INDEX IF EXISTS idx_note_folders_folder;
DROP TABLE IF EXISTS note_folders;

-- `tasks.folder_id` was never read by the app, but it references `folders`, so
-- it has to go before that table can be dropped.
DROP INDEX IF EXISTS idx_tasks_folder;
ALTER TABLE tasks DROP COLUMN folder_id;

DROP INDEX IF EXISTS idx_folders_parent;
DROP TABLE IF EXISTS folders;
