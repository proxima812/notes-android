-- Initial schema.
--
-- Conventions used throughout:
--   * identifiers are UUIDv7 strings in TEXT columns;
--   * every instant is INTEGER milliseconds since the Unix epoch, UTC;
--     wall-clock intent that must survive a timezone change is stored
--     separately as `timezone` + local components on reminders;
--   * soft delete is `deleted_at IS NOT NULL`; the trash view filters on it,
--     and only emptying the trash performs a hard delete;
--   * booleans are INTEGER 0/1 with CHECK constraints.

-- ---------------------------------------------------------------- folders --

CREATE TABLE folders (
    id                TEXT PRIMARY KEY,
    parent_folder_id  TEXT REFERENCES folders (id) ON DELETE CASCADE,
    name              TEXT    NOT NULL,
    color             TEXT,
    position          INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER,
    CHECK (length(name) > 0),
    CHECK (id <> parent_folder_id)
);

CREATE INDEX idx_folders_parent ON folders (parent_folder_id, position)
    WHERE deleted_at IS NULL;

-- ------------------------------------------------------------------ tags --

CREATE TABLE tags (
    id          TEXT PRIMARY KEY,
    name        TEXT    NOT NULL,
    color       TEXT,
    usage_count INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    CHECK (length(name) > 0)
);

-- Tag names are case-insensitive and unique; `#Работа` and `#работа` are one tag.
CREATE UNIQUE INDEX idx_tags_name ON tags (name COLLATE NOCASE);

-- ----------------------------------------------------------------- notes --

CREATE TABLE notes (
    id           TEXT PRIMARY KEY,
    note_type    TEXT    NOT NULL DEFAULT 'text',
    title        TEXT    NOT NULL DEFAULT '',
    -- Plain-text projection of the document, kept in sync by the notes service.
    -- It is what FTS indexes and what Markdown export starts from.
    content_text TEXT    NOT NULL DEFAULT '',
    -- Tiptap document as JSON. NULL for note types that only use blocks.
    content_json TEXT,
    color        TEXT,
    background   TEXT,
    is_pinned    INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)),
    is_favorite  INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),
    is_archived  INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1)),
    is_readonly  INTEGER NOT NULL DEFAULT 0 CHECK (is_readonly IN (0, 1)),
    position     INTEGER NOT NULL DEFAULT 0,
    word_count   INTEGER NOT NULL DEFAULT 0,
    char_count   INTEGER NOT NULL DEFAULT 0,
    -- Bumped on every persisted change; used for optimistic concurrency and
    -- to decide whether a new history snapshot is warranted.
    revision     INTEGER NOT NULL DEFAULT 1,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    deleted_at   INTEGER,
    CHECK (note_type IN (
        'text', 'rich_text', 'checklist', 'task_list', 'journal', 'daily_note',
        'meeting', 'idea', 'project', 'contact', 'shopping_list', 'habit',
        'password_hint', 'bookmark', 'code_snippet', 'voice_note', 'drawing'
    )),
    CHECK (revision > 0)
);

-- The default list: not deleted, not archived, pinned first, newest first.
CREATE INDEX idx_notes_active ON notes (is_pinned DESC, updated_at DESC)
    WHERE deleted_at IS NULL AND is_archived = 0;
CREATE INDEX idx_notes_archived ON notes (updated_at DESC)
    WHERE deleted_at IS NULL AND is_archived = 1;
CREATE INDEX idx_notes_trash ON notes (deleted_at DESC)
    WHERE deleted_at IS NOT NULL;
CREATE INDEX idx_notes_type ON notes (note_type, updated_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_notes_favorite ON notes (updated_at DESC)
    WHERE deleted_at IS NULL AND is_favorite = 1;
CREATE INDEX idx_notes_title_sort ON notes (title COLLATE NOCASE)
    WHERE deleted_at IS NULL;

-- ----------------------------------------------------------- note_blocks --

-- Structured blocks: checklist items (arbitrarily nested), drawings and other
-- units that need their own identity, ordering and completion state.
CREATE TABLE note_blocks (
    id              TEXT PRIMARY KEY,
    note_id         TEXT    NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    parent_block_id TEXT REFERENCES note_blocks (id) ON DELETE CASCADE,
    block_type      TEXT    NOT NULL,
    text            TEXT    NOT NULL DEFAULT '',
    is_checked      INTEGER NOT NULL DEFAULT 0 CHECK (is_checked IN (0, 1)),
    -- Free-form per-type payload (drawing strokes, code language, table data).
    data_json       TEXT,
    position        INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    CHECK (block_type IN (
        'checklist_item', 'paragraph', 'heading', 'quote', 'divider',
        'code', 'table', 'drawing', 'image', 'audio', 'file'
    )),
    CHECK (id <> parent_block_id)
);

CREATE INDEX idx_note_blocks_note ON note_blocks (note_id, position);
CREATE INDEX idx_note_blocks_parent ON note_blocks (parent_block_id, position);

-- ------------------------------------------------------------ join tables --

CREATE TABLE note_tags (
    note_id    TEXT    NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    tag_id     TEXT    NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (note_id, tag_id)
) WITHOUT ROWID;

CREATE INDEX idx_note_tags_tag ON note_tags (tag_id);

CREATE TABLE note_folders (
    note_id    TEXT    NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    folder_id  TEXT    NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (note_id, folder_id)
) WITHOUT ROWID;

CREATE INDEX idx_note_folders_folder ON note_folders (folder_id);

-- Wiki-style links and mentions between notes, used for backlinks.
CREATE TABLE note_links (
    source_note_id TEXT    NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    target_note_id TEXT    NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    link_type      TEXT    NOT NULL DEFAULT 'link',
    created_at     INTEGER NOT NULL,
    PRIMARY KEY (source_note_id, target_note_id, link_type),
    CHECK (link_type IN ('link', 'mention', 'embed')),
    CHECK (source_note_id <> target_note_id)
) WITHOUT ROWID;

CREATE INDEX idx_note_links_target ON note_links (target_note_id);

-- ----------------------------------------------------------------- tasks --

CREATE TABLE tasks (
    id                TEXT    PRIMARY KEY,
    -- A task may live inside a note or stand on its own.
    note_id           TEXT REFERENCES notes (id) ON DELETE CASCADE,
    parent_task_id    TEXT REFERENCES tasks (id) ON DELETE CASCADE,
    folder_id         TEXT REFERENCES folders (id) ON DELETE SET NULL,
    title             TEXT    NOT NULL,
    description       TEXT    NOT NULL DEFAULT '',
    status            TEXT    NOT NULL DEFAULT 'inbox',
    priority          TEXT    NOT NULL DEFAULT 'none',
    due_at            INTEGER,
    -- 0 => the due date is a whole day, 1 => `due_at` carries a meaningful time.
    due_has_time      INTEGER NOT NULL DEFAULT 0 CHECK (due_has_time IN (0, 1)),
    timezone          TEXT    NOT NULL DEFAULT 'UTC',
    recurrence_rule   TEXT,
    -- Keep re-firing until the user actually completes it.
    repeat_until_done INTEGER NOT NULL DEFAULT 0 CHECK (repeat_until_done IN (0, 1)),
    -- Spawn the next occurrence automatically once this one is completed.
    auto_create_next  INTEGER NOT NULL DEFAULT 0 CHECK (auto_create_next IN (0, 1)),
    estimate_minutes  INTEGER,
    actual_minutes    INTEGER,
    completed_at      INTEGER,
    position          INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER,
    CHECK (status IN ('inbox', 'planned', 'in_progress', 'waiting', 'completed', 'cancelled')),
    CHECK (priority IN ('none', 'low', 'medium', 'high', 'critical')),
    CHECK (length(title) > 0),
    CHECK (id <> parent_task_id),
    CHECK (estimate_minutes IS NULL OR estimate_minutes >= 0),
    CHECK (actual_minutes IS NULL OR actual_minutes >= 0),
    -- A completed task must record when, and an open task must not.
    CHECK ((status = 'completed') = (completed_at IS NOT NULL))
);

CREATE INDEX idx_tasks_open ON tasks (status, priority, due_at)
    WHERE deleted_at IS NULL AND status NOT IN ('completed', 'cancelled');
CREATE INDEX idx_tasks_due ON tasks (due_at)
    WHERE deleted_at IS NULL AND due_at IS NOT NULL;
CREATE INDEX idx_tasks_note ON tasks (note_id, position) WHERE note_id IS NOT NULL;
CREATE INDEX idx_tasks_parent ON tasks (parent_task_id, position) WHERE parent_task_id IS NOT NULL;
CREATE INDEX idx_tasks_folder ON tasks (folder_id) WHERE folder_id IS NOT NULL;
CREATE INDEX idx_tasks_completed ON tasks (completed_at DESC) WHERE completed_at IS NOT NULL;

CREATE TABLE task_tags (
    task_id    TEXT    NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    tag_id     TEXT    NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (task_id, tag_id)
) WITHOUT ROWID;

CREATE INDEX idx_task_tags_tag ON task_tags (tag_id);

CREATE TABLE task_dependencies (
    task_id            TEXT    NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    depends_on_task_id TEXT    NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    created_at         INTEGER NOT NULL,
    PRIMARY KEY (task_id, depends_on_task_id),
    CHECK (task_id <> depends_on_task_id)
) WITHOUT ROWID;

CREATE INDEX idx_task_dependencies_target ON task_dependencies (depends_on_task_id);

-- Every completion of a recurring task, so statistics survive the task itself.
CREATE TABLE task_completions (
    id           TEXT    PRIMARY KEY,
    task_id      TEXT    NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    completed_at INTEGER NOT NULL,
    due_at       INTEGER,
    -- Minutes between due date and completion; negative means done early.
    lateness_min INTEGER,
    created_at   INTEGER NOT NULL
);

CREATE INDEX idx_task_completions_task ON task_completions (task_id, completed_at DESC);

-- ------------------------------------------------------------- reminders --

CREATE TABLE reminders (
    id                    TEXT    PRIMARY KEY,
    note_id               TEXT REFERENCES notes (id) ON DELETE CASCADE,
    task_id               TEXT REFERENCES tasks (id) ON DELETE CASCADE,
    title                 TEXT    NOT NULL,
    body                  TEXT    NOT NULL DEFAULT '',
    -- First (or only) firing instant, in UTC milliseconds.
    scheduled_at          INTEGER NOT NULL,
    -- IANA zone the user meant. Recurrence is expanded in this zone so that
    -- "every day at 09:00" stays 09:00 across DST and after a zone change.
    timezone              TEXT    NOT NULL DEFAULT 'UTC',
    -- RFC 5545 RRULE; NULL means a one-shot reminder.
    recurrence_rule       TEXT,
    -- Optional heads-up before the main firing, in minutes.
    lead_time_minutes     INTEGER,
    -- Exact alarms are reserved for reminders the user explicitly asked for;
    -- 'inexact' is also the automatic fallback when the OS denies permission.
    exactness             TEXT    NOT NULL DEFAULT 'exact',
    sound                 TEXT    NOT NULL DEFAULT 'default',
    vibrate               INTEGER NOT NULL DEFAULT 1 CHECK (vibrate IN (0, 1)),
    silent                INTEGER NOT NULL DEFAULT 0 CHECK (silent IN (0, 1)),
    -- Ongoing, non-dismissable notification for critical work.
    persistent            INTEGER NOT NULL DEFAULT 0 CHECK (persistent IN (0, 1)),
    -- Raise importance on each missed firing.
    escalate              INTEGER NOT NULL DEFAULT 0 CHECK (escalate IN (0, 1)),
    snooze_minutes        INTEGER NOT NULL DEFAULT 10,
    -- 'ignore' fires during quiet hours anyway; 'defer' waits for the window
    -- to end; 'silence' fires without sound or vibration.
    quiet_hours_policy    TEXT    NOT NULL DEFAULT 'defer',
    skip_holidays         INTEGER NOT NULL DEFAULT 0 CHECK (skip_holidays IN (0, 1)),
    repeat_until_done     INTEGER NOT NULL DEFAULT 0 CHECK (repeat_until_done IN (0, 1)),
    -- Interval in minutes used while `repeat_until_done` is active.
    nag_interval_minutes  INTEGER,
    -- Move a firing the user never acknowledged to the next sensible slot.
    reschedule_overdue    INTEGER NOT NULL DEFAULT 0 CHECK (reschedule_overdue IN (0, 1)),
    -- Recurrence bounds: by count, by date, or unbounded when both are NULL.
    max_occurrences       INTEGER,
    ends_at               INTEGER,
    occurrences_fired     INTEGER NOT NULL DEFAULT 0,
    is_enabled            INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
    created_at            INTEGER NOT NULL,
    updated_at            INTEGER NOT NULL,
    deleted_at            INTEGER,
    CHECK (exactness IN ('exact', 'inexact')),
    CHECK (quiet_hours_policy IN ('ignore', 'defer', 'silence')),
    CHECK (length(title) > 0),
    CHECK (snooze_minutes > 0),
    CHECK (max_occurrences IS NULL OR max_occurrences > 0),
    CHECK (lead_time_minutes IS NULL OR lead_time_minutes > 0),
    CHECK (nag_interval_minutes IS NULL OR nag_interval_minutes > 0),
    -- A reminder is attached to a note, to a task, or to neither, never both.
    CHECK (note_id IS NULL OR task_id IS NULL)
);

CREATE INDEX idx_reminders_active ON reminders (scheduled_at)
    WHERE deleted_at IS NULL AND is_enabled = 1;
CREATE INDEX idx_reminders_note ON reminders (note_id) WHERE note_id IS NOT NULL;
CREATE INDEX idx_reminders_task ON reminders (task_id) WHERE task_id IS NOT NULL;

-- One row per concrete firing. This table is the source of truth that the
-- boot receiver replays after a reboot, an APK update or a timezone change.
CREATE TABLE reminder_occurrences (
    id                 TEXT    PRIMARY KEY,
    reminder_id        TEXT    NOT NULL REFERENCES reminders (id) ON DELETE CASCADE,
    occurrence_at      INTEGER NOT NULL,
    state              TEXT    NOT NULL DEFAULT 'scheduled',
    -- Stable 32-bit key for AlarmManager PendingIntent request codes, so that
    -- an alarm can be cancelled or replaced without keeping objects in memory.
    alarm_request_code INTEGER NOT NULL,
    -- Whether this particular firing was actually armed as an exact alarm.
    -- Differs from reminders.exactness when the permission was denied.
    is_exact           INTEGER NOT NULL DEFAULT 1 CHECK (is_exact IN (0, 1)),
    -- Set when the occurrence is a pre-notification rather than the main one.
    is_lead_time       INTEGER NOT NULL DEFAULT 0 CHECK (is_lead_time IN (0, 1)),
    fired_at           INTEGER,
    handled_at         INTEGER,
    snoozed_from       INTEGER,
    snooze_count       INTEGER NOT NULL DEFAULT 0,
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL,
    CHECK (state IN ('scheduled', 'fired', 'completed', 'skipped', 'snoozed', 'cancelled', 'missed')),
    CHECK (snooze_count >= 0)
);

CREATE UNIQUE INDEX idx_reminder_occurrences_code ON reminder_occurrences (alarm_request_code);
CREATE INDEX idx_reminder_occurrences_pending ON reminder_occurrences (occurrence_at)
    WHERE state IN ('scheduled', 'snoozed');
CREATE INDEX idx_reminder_occurrences_reminder ON reminder_occurrences (reminder_id, occurrence_at);

-- Audit trail written by the Kotlin plugin: what the OS actually did and which
-- notification action the user pressed, including while the UI was closed.
CREATE TABLE notification_events (
    id            TEXT    PRIMARY KEY,
    occurrence_id TEXT REFERENCES reminder_occurrences (id) ON DELETE SET NULL,
    reminder_id   TEXT REFERENCES reminders (id) ON DELETE SET NULL,
    event_type    TEXT    NOT NULL,
    -- Small structured payload (snooze minutes, denial reason, alarm mode).
    payload_json  TEXT,
    created_at    INTEGER NOT NULL,
    CHECK (event_type IN (
        'scheduled', 'schedule_failed', 'fired', 'delivered', 'dismissed',
        'action_done', 'action_snooze', 'action_skip', 'action_open',
        'boot_restored', 'timezone_recalculated', 'package_replaced',
        'permission_denied', 'downgraded_to_inexact'
    ))
);

CREATE INDEX idx_notification_events_occurrence ON notification_events (occurrence_id, created_at DESC);
CREATE INDEX idx_notification_events_time ON notification_events (created_at DESC);

-- ----------------------------------------------------------- attachments --

CREATE TABLE attachments (
    id             TEXT    PRIMARY KEY,
    note_id        TEXT REFERENCES notes (id) ON DELETE SET NULL,
    -- Path relative to the private attachments root. Absolute paths are never
    -- stored: the app sandbox path changes between installs.
    relative_path  TEXT    NOT NULL,
    name           TEXT    NOT NULL,
    mime           TEXT    NOT NULL,
    size_bytes     INTEGER NOT NULL,
    sha256         TEXT    NOT NULL,
    -- Text pulled out of the file (OCR, PDF text) so search can reach inside.
    extracted_text TEXT    NOT NULL DEFAULT '',
    -- Dimensions, duration, thumbnail path and similar per-type facts.
    metadata_json  TEXT,
    thumbnail_path TEXT,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    -- The blob outlives the row until the trash is emptied, so a restored note
    -- still finds its files. The GC only removes files with no live reference.
    deleted_at     INTEGER,
    CHECK (size_bytes >= 0),
    CHECK (length(relative_path) > 0)
);

CREATE UNIQUE INDEX idx_attachments_path ON attachments (relative_path);
CREATE INDEX idx_attachments_note ON attachments (note_id) WHERE note_id IS NOT NULL;
CREATE INDEX idx_attachments_hash ON attachments (sha256);
CREATE INDEX idx_attachments_orphans ON attachments (deleted_at) WHERE deleted_at IS NOT NULL;

-- ------------------------------------------------ searches and templates --

CREATE TABLE saved_searches (
    id             TEXT    PRIMARY KEY,
    name           TEXT    NOT NULL,
    -- Serialized SearchQuery; parsed and validated by the search service.
    query_json     TEXT    NOT NULL,
    -- A smart folder is a saved search pinned into the folder tree.
    is_smart_folder INTEGER NOT NULL DEFAULT 0 CHECK (is_smart_folder IN (0, 1)),
    icon           TEXT,
    color          TEXT,
    position       INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    CHECK (length(name) > 0)
);

CREATE TABLE search_history (
    id          TEXT    PRIMARY KEY,
    query       TEXT    NOT NULL,
    result_count INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    CHECK (length(query) > 0)
);

CREATE INDEX idx_search_history_time ON search_history (created_at DESC);

CREATE TABLE templates (
    id           TEXT    PRIMARY KEY,
    name         TEXT    NOT NULL,
    note_type    TEXT    NOT NULL DEFAULT 'text',
    title_pattern TEXT   NOT NULL DEFAULT '',
    content_json TEXT,
    content_text TEXT    NOT NULL DEFAULT '',
    -- Set when the template was proposed by the local pattern detector rather
    -- than written by hand, so the UI can explain where it came from.
    is_suggested INTEGER NOT NULL DEFAULT 0 CHECK (is_suggested IN (0, 1)),
    usage_count  INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    CHECK (length(name) > 0)
);

-- Repeating multi-step procedures (morning routine, weekly review).
CREATE TABLE routines (
    id              TEXT    PRIMARY KEY,
    name            TEXT    NOT NULL,
    definition_json TEXT    NOT NULL,
    recurrence_rule TEXT,
    timezone        TEXT    NOT NULL DEFAULT 'UTC',
    is_enabled      INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
    last_run_at     INTEGER,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    CHECK (length(name) > 0)
);

-- ------------------------------------------------- history and settings --

-- Change log and note version snapshots in one place.
CREATE TABLE activity_history (
    id            TEXT    PRIMARY KEY,
    entity_type   TEXT    NOT NULL,
    entity_id     TEXT    NOT NULL,
    action        TEXT    NOT NULL,
    -- Full previous state for 'snapshot' rows; a compact diff for the rest.
    snapshot_json TEXT,
    -- 1 when the user explicitly asked to keep this version, which exempts it
    -- from automatic pruning.
    is_pinned     INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)),
    created_at    INTEGER NOT NULL,
    CHECK (entity_type IN ('note', 'task', 'reminder', 'folder', 'tag', 'attachment')),
    CHECK (action IN ('created', 'updated', 'deleted', 'restored', 'archived',
                      'unarchived', 'completed', 'snapshot', 'purged'))
);

CREATE INDEX idx_activity_history_entity ON activity_history (entity_type, entity_id, created_at DESC);
CREATE INDEX idx_activity_history_time ON activity_history (created_at DESC);

CREATE TABLE app_settings (
    key        TEXT PRIMARY KEY,
    value      TEXT    NOT NULL,
    updated_at INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE backup_history (
    id          TEXT    PRIMARY KEY,
    -- Display location only; the archive itself lives wherever SAF put it.
    location    TEXT    NOT NULL,
    file_name   TEXT    NOT NULL,
    size_bytes  INTEGER NOT NULL,
    sha256      TEXT    NOT NULL,
    is_encrypted INTEGER NOT NULL DEFAULT 0 CHECK (is_encrypted IN (0, 1)),
    is_automatic INTEGER NOT NULL DEFAULT 0 CHECK (is_automatic IN (0, 1)),
    note_count  INTEGER NOT NULL DEFAULT 0,
    task_count  INTEGER NOT NULL DEFAULT 0,
    status      TEXT    NOT NULL DEFAULT 'completed',
    created_at  INTEGER NOT NULL,
    CHECK (status IN ('completed', 'failed', 'verified', 'corrupt')),
    CHECK (size_bytes >= 0)
);

CREATE INDEX idx_backup_history_time ON backup_history (created_at DESC);

-- --------------------------------------------------------- full-text search --

-- Plain (not external-content) FTS5 tables. They duplicate the indexed text,
-- which costs disk, but they support snippet()/highlight() and correct
-- UPDATE/DELETE on every SQLite build without contentless-delete caveats.
--
-- `prefix='2 3 4'` powers partial-word search; `remove_diacritics 2` makes
-- Russian `ё`/`е` and accented Latin behave the way users expect.

CREATE VIRTUAL TABLE notes_fts USING fts5 (
    note_id UNINDEXED,
    title,
    body,
    tags,
    tokenize = "unicode61 remove_diacritics 2",
    prefix = '2 3 4'
);

CREATE VIRTUAL TABLE tasks_fts USING fts5 (
    task_id UNINDEXED,
    title,
    description,
    tags,
    tokenize = "unicode61 remove_diacritics 2",
    prefix = '2 3 4'
);

CREATE VIRTUAL TABLE attachments_fts USING fts5 (
    attachment_id UNINDEXED,
    name,
    extracted_text,
    tokenize = "unicode61 remove_diacritics 2",
    prefix = '2 3 4'
);

-- Notes: the `tags` column is refreshed by the repository when tag links
-- change, since a trigger on note_tags cannot see the note's other tags
-- cheaply enough to be worth it on every write.

CREATE TRIGGER notes_fts_after_insert
AFTER INSERT ON notes
WHEN new.deleted_at IS NULL
BEGIN
    INSERT INTO notes_fts (note_id, title, body, tags)
    VALUES (new.id, new.title, new.content_text, '');
END;

CREATE TRIGGER notes_fts_after_update
AFTER UPDATE OF title, content_text, deleted_at ON notes
BEGIN
    DELETE FROM notes_fts WHERE note_id = old.id;
    INSERT INTO notes_fts (note_id, title, body, tags)
    SELECT new.id, new.title, new.content_text,
           COALESCE((SELECT group_concat(t.name, ' ')
                     FROM note_tags nt
                     JOIN tags t ON t.id = nt.tag_id
                     WHERE nt.note_id = new.id), '')
    WHERE new.deleted_at IS NULL;
END;

CREATE TRIGGER notes_fts_after_delete
AFTER DELETE ON notes
BEGIN
    DELETE FROM notes_fts WHERE note_id = old.id;
END;

CREATE TRIGGER tasks_fts_after_insert
AFTER INSERT ON tasks
WHEN new.deleted_at IS NULL
BEGIN
    INSERT INTO tasks_fts (task_id, title, description, tags)
    VALUES (new.id, new.title, new.description, '');
END;

CREATE TRIGGER tasks_fts_after_update
AFTER UPDATE OF title, description, deleted_at ON tasks
BEGIN
    DELETE FROM tasks_fts WHERE task_id = old.id;
    INSERT INTO tasks_fts (task_id, title, description, tags)
    SELECT new.id, new.title, new.description,
           COALESCE((SELECT group_concat(t.name, ' ')
                     FROM task_tags tt
                     JOIN tags t ON t.id = tt.tag_id
                     WHERE tt.task_id = new.id), '')
    WHERE new.deleted_at IS NULL;
END;

CREATE TRIGGER tasks_fts_after_delete
AFTER DELETE ON tasks
BEGIN
    DELETE FROM tasks_fts WHERE task_id = old.id;
END;

CREATE TRIGGER attachments_fts_after_insert
AFTER INSERT ON attachments
WHEN new.deleted_at IS NULL
BEGIN
    INSERT INTO attachments_fts (attachment_id, name, extracted_text)
    VALUES (new.id, new.name, new.extracted_text);
END;

CREATE TRIGGER attachments_fts_after_update
AFTER UPDATE OF name, extracted_text, deleted_at ON attachments
BEGIN
    DELETE FROM attachments_fts WHERE attachment_id = old.id;
    INSERT INTO attachments_fts (attachment_id, name, extracted_text)
    SELECT new.id, new.name, new.extracted_text
    WHERE new.deleted_at IS NULL;
END;

CREATE TRIGGER attachments_fts_after_delete
AFTER DELETE ON attachments
BEGIN
    DELETE FROM attachments_fts WHERE attachment_id = old.id;
END;

-- Keep the denormalised tag counter honest without a write from the service.
CREATE TRIGGER tags_usage_after_note_tag_insert
AFTER INSERT ON note_tags
BEGIN
    UPDATE tags SET usage_count = usage_count + 1 WHERE id = new.tag_id;
END;

CREATE TRIGGER tags_usage_after_note_tag_delete
AFTER DELETE ON note_tags
BEGIN
    UPDATE tags SET usage_count = MAX(0, usage_count - 1) WHERE id = old.tag_id;
END;

CREATE TRIGGER tags_usage_after_task_tag_insert
AFTER INSERT ON task_tags
BEGIN
    UPDATE tags SET usage_count = usage_count + 1 WHERE id = new.tag_id;
END;

CREATE TRIGGER tags_usage_after_task_tag_delete
AFTER DELETE ON task_tags
BEGIN
    UPDATE tags SET usage_count = MAX(0, usage_count - 1) WHERE id = old.tag_id;
END;
