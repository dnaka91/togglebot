INSERT INTO custom_commands (source, name, content) VALUES (?, ?, ?)
ON CONFLICT (source, name) DO UPDATE SET content = excluded.content;