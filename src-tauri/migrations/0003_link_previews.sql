-- What a pasted address turned out to be.
--
-- A cache and nothing more: every row can be deleted and the app rebuilds it
-- from the network the next time that address is pasted. Notes never point at
-- this table, so it is deliberately outside the foreign-key graph — losing it
-- costs an icon, never a word anybody wrote.
--
-- `fetched_at` is what makes a row expire, and `ok` distinguishes «this site
-- has no title» from «we never got an answer»: without it a host that is down
-- would be asked again on every keystroke.

CREATE TABLE link_previews (
    url        TEXT PRIMARY KEY,
    title      TEXT,
    icon       TEXT,
    ok         INTEGER NOT NULL DEFAULT 1,
    fetched_at INTEGER NOT NULL,
    CHECK (ok IN (0, 1))
);

CREATE INDEX idx_link_previews_fetched ON link_previews (fetched_at);
