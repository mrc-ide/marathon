-- For now it is acceptable to modify this file. Once we start having
-- real-world uses we should freeze its contents and only update the schema by
-- writing new migrations.

CREATE TABLE task(
  `id` BLOB NOT NULL PRIMARY KEY CHECK(length(id) = 16),
  `request` BLOB NOT NULL,
  `status` TEXT NOT NULL
);

CREATE TABLE task_queue(
  `seq` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  `task` BLOB CHECK(length(task) = 16) NOT NULL,
  FOREIGN KEY(task) REFERENCES task(id)
);
