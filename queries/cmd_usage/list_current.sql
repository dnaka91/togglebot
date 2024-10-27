SELECT kind, name, count FROM command_usage WHERE year = ? AND month = ?
ORDER BY count DESC;