SELECT kind AS "kind: CommandKind", name, count AS "count: u32"
FROM command_usage
WHERE year = ? AND month = ?
ORDER BY count DESC;