CREATE TABLE companion_calls (
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    call_id TEXT NOT NULL,
    started_ms INTEGER,
    record TEXT NOT NULL,
    revision_ms INTEGER NOT NULL,
    PRIMARY KEY (device_id, call_id)
);
CREATE INDEX companion_calls_recent ON companion_calls (device_id, started_ms DESC, call_id DESC);
