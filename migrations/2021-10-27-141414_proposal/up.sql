CREATE TABLE proposals (
  id BIGINT PRIMARY KEY NOT NULL,
  title TEXT,
  summary TEXT,
  submit_output TEXT,
  executed_timestamp BIGINT NOT NULL DEFAULT 0,
  failed_timestamp BIGINT NOT NULL DEFAULT 0,
  failed TEXT
);
