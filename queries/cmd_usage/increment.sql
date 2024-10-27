INSERT INTO command_usage (year, month, kind, name, count) VALUES (?, ?, ?, ?, 1)
ON CONFLICT (year, month, kind, name) DO UPDATE SET count = count + 1;